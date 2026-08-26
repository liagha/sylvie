use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use opaque_ke::rand::RngCore;
use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{ServerLogin, ServerLoginParameters, ServerRegistration};
use sqlx::SqlitePool;
use uuid::Uuid;

use sylvie_core::codec;
use sylvie_core::error::Error;
use sylvie_core::message::{
    BlobReply, Device, Grant, LoginFinish, LoginReply, LoginStart, Me, RegisterFinish,
    RegisterStart, RekeyFinish, RekeyStart, Sealed, WrapValue,
};
use sylvie_core::opaque::{self, Suite};
use sylvie_core::vault;

use crate::clock::stamp;
use crate::ctx::{Ctx, Pending};
use crate::reply::{Failure, internal};
use crate::routes::account::Account;
use crate::routes::sane;
use std::net::SocketAddr;

fn sql<E: std::fmt::Display>(error: E) -> Error {
    Error::Internal(error.to_string())
}

fn gate(ctx: &Ctx, peer: SocketAddr, user: &str) -> Result<(), Failure> {
    let key = format!("{}:{user}", peer.ip());
    ctx.admit(&key)
        .then_some(())
        .ok_or(Error::Flood)
        .map_err(Failure::from)
}

fn blob(text: &str) -> Result<Vec<u8>, Failure> {
    codec::decode(text).map_err(|_| Error::Request.into())
}

pub async fn register_start(
    State(ctx): State<Ctx>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(req): Json<RegisterStart>,
) -> Result<Json<BlobReply>, Failure> {
    sane(&req.username, 64).ok_or(Error::Request)?;
    gate(&ctx, peer, &req.username)?;
    unclaimed(ctx.db()).await?;
    let ask = opaque::reg_ask(&blob(&req.message)?)?;
    let started = ServerRegistration::<Suite>::start(ctx.setup(), ask, req.username.as_bytes())
        .map_err(|_| Error::Request)?;
    Ok(Json(BlobReply {
        message: codec::encode(&started.message.serialize()),
    }))
}

pub async fn register_finish(
    State(ctx): State<Ctx>,
    Json(req): Json<RegisterFinish>,
) -> Result<StatusCode, Failure> {
    sane(&req.username, 64).ok_or(Error::Request)?;
    let give = opaque::reg_give(&blob(&req.message)?)?;
    let record = ServerRegistration::<Suite>::finish(give);
    let mut tx = ctx.db().begin().await.map_err(internal)?;
    unclaimed(&mut *tx).await?;
    sqlx::query("insert into users(id, username, record, wrap, created) values (?, ?, ?, ?, ?)")
        .bind(Uuid::new_v4().to_string())
        .bind(&req.username)
        .bind(record.serialize().to_vec())
        .bind(codec::decode(&req.wrap)?)
        .bind(stamp())
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    tracing::info!(user = %req.username, "account created");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn login_start(
    State(ctx): State<Ctx>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginStart>,
) -> Result<Json<LoginReply>, Failure> {
    sane(&req.username, 64).ok_or(Error::Request)?;
    gate(&ctx, peer, &req.username)?;
    let row: Option<(Vec<u8>,)> = sqlx::query_as("select record from users where username = ?")
        .bind(&req.username)
        .fetch_optional(ctx.db())
        .await
        .map_err(internal)?;
    let record = match row {
        Some((bytes,)) => Some(opaque::record_load(&bytes)?),
        None => None,
    };
    let ask = opaque::log_ask(&blob(&req.message)?)?;
    let params = ServerLoginParameters {
        identifiers: opaque::peer(&req.username),
        ..Default::default()
    };
    let started = ServerLogin::<Suite>::start(
        &mut OsRng,
        ctx.setup(),
        record,
        ask,
        req.username.as_bytes(),
        params,
    )
    .map_err(|_| Error::Request)?;
    let id = Uuid::new_v4().to_string();
    ctx.pending_insert(
        id.clone(),
        Pending {
            login: started.state,
            user: req.username,
            born: std::time::Instant::now(),
        },
    );
    Ok(Json(LoginReply {
        id,
        message: codec::encode(&started.message.serialize()),
    }))
}

pub async fn login_finish(
    State(ctx): State<Ctx>,
    Json(req): Json<LoginFinish>,
) -> Result<Json<Sealed>, Failure> {
    let pending = ctx.pending_take(&req.id).ok_or(Error::Auth)?;
    let give = opaque::log_give(&blob(&req.message)?)?;
    let params = ServerLoginParameters {
        identifiers: opaque::peer(&pending.user),
        ..Default::default()
    };
    let done = pending
        .login
        .finish(give, params)
        .map_err(|_| Error::Auth)?;
    let (token, device) = match &req.device {
        Some(id) => (String::new(), verify(ctx.db(), &pending.user, id).await?),
        None => match req.name.as_deref() {
            Some(name) => enroll(ctx.db(), &pending.user, name).await?,
            None => return Err(Error::Request.into()),
        },
    };
    let grant = Grant { token, device };
    let channel = vault::root(done.session_key.as_slice(), vault::CHANNEL)?;
    let sealed = vault::seal(&channel, &serde_json::to_vec(&grant).map_err(internal)?)?;
    tracing::info!(
        user = %pending.user,
        device = %grant.device,
        enrolled = req.device.is_none(),
        "login complete"
    );
    Ok(Json(Sealed {
        data: codec::encode(&sealed),
    }))
}

pub async fn rekey_start(
    State(ctx): State<Ctx>,
    account: Account,
    Json(req): Json<RekeyStart>,
) -> Result<Json<BlobReply>, Failure> {
    let (username,): (String,) = sqlx::query_as("select username from users where id = ?")
        .bind(&account.owner)
        .fetch_one(ctx.db())
        .await
        .map_err(internal)?;
    let ask = opaque::reg_ask(&blob(&req.message)?)?;
    let started = ServerRegistration::<Suite>::start(ctx.setup(), ask, username.as_bytes())
        .map_err(|_| Error::Request)?;
    Ok(Json(BlobReply {
        message: codec::encode(&started.message.serialize()),
    }))
}

pub async fn rekey_finish(
    State(ctx): State<Ctx>,
    account: Account,
    Json(req): Json<RekeyFinish>,
) -> Result<StatusCode, Failure> {
    let give = opaque::reg_give(&blob(&req.message)?)?;
    let record = ServerRegistration::<Suite>::finish(give);
    sqlx::query("update users set record = ?, wrap = ? where id = ?")
        .bind(record.serialize().to_vec())
        .bind(codec::decode(&req.wrap)?)
        .bind(&account.owner)
        .execute(ctx.db())
        .await
        .map_err(internal)?;
    tracing::info!(owner = %account.owner, "password changed");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn vault(State(ctx): State<Ctx>, account: Account) -> Result<Json<WrapValue>, Failure> {
    let row: Option<(Option<Vec<u8>>,)> = sqlx::query_as("select wrap from users where id = ?")
        .bind(&account.owner)
        .fetch_optional(ctx.db())
        .await
        .map_err(internal)?;
    let data = row.ok_or(Error::Missing)?.0.ok_or(Error::Protocol)?;
    Ok(Json(WrapValue {
        data: codec::encode(&data),
    }))
}

pub async fn me(State(ctx): State<Ctx>, account: Account) -> Result<Json<Me>, Failure> {
    let (username,): (String,) = sqlx::query_as("select username from users where id = ?")
        .bind(&account.owner)
        .fetch_one(ctx.db())
        .await
        .map_err(internal)?;
    let row: (String, String, String, Option<String>) =
        sqlx::query_as("select id, name, created, revoked from devices where id = ?")
            .bind(&account.device)
            .fetch_one(ctx.db())
            .await
            .map_err(internal)?;
    Ok(Json(Me {
        username,
        device: Device {
            id: row.0,
            name: row.1,
            created: row.2,
            revoked: row.3,
        },
        secrets: counted(ctx.db(), "secrets", &account.owner).await?,
        files: counted(ctx.db(), "files", &account.owner).await?,
        devices: counted(ctx.db(), "devices", &account.owner).await?,
    }))
}

async fn counted(db: &SqlitePool, table: &str, owner: &str) -> Result<u32, Error> {
    let statement = match table {
        "secrets" => "select count(*) from secrets where owner = ?",
        "files" => "select count(*) from files where owner = ?",
        _ => "select count(*) from devices where owner = ?",
    };
    let (n,): (i64,) = sqlx::query_as(statement)
        .bind(owner)
        .fetch_one(db)
        .await
        .map_err(sql)?;
    Ok(n as u32)
}

async fn unclaimed(db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>) -> Result<(), Error> {
    let (users,): (i64,) = sqlx::query_as("select count(*) from users")
        .fetch_one(db)
        .await
        .map_err(sql)?;
    if users != 0 {
        return Err(Error::Conflict);
    }
    Ok(())
}

async fn owner_id(db: &SqlitePool, user: &str) -> Result<String, Error> {
    let row: Option<(String,)> = sqlx::query_as("select id from users where username = ?")
        .bind(user)
        .fetch_optional(db)
        .await
        .map_err(sql)?;
    row.map(|r| r.0).ok_or(Error::Auth)
}

async fn enroll(db: &SqlitePool, user: &str, name: &str) -> Result<(String, String), Error> {
    sane(name, 64).ok_or(Error::Request)?;
    let owner = owner_id(db, user).await?;
    let device = Uuid::new_v4().to_string();
    sqlx::query("insert into devices(id, owner, name, created) values (?, ?, ?, ?)")
        .bind(&device)
        .bind(&owner)
        .bind(name)
        .bind(stamp())
        .execute(db)
        .await
        .map_err(sql)?;
    issue(db, &device).await
}

async fn verify(db: &SqlitePool, user: &str, device: &str) -> Result<String, Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "select d.id from devices d join users u on u.id = d.owner \
         where d.id = ? and u.username = ? and d.revoked is null",
    )
    .bind(device)
    .bind(user)
    .fetch_optional(db)
    .await
    .map_err(sql)?;
    row.map(|r| r.0).ok_or(Error::Auth)
}

async fn issue(db: &SqlitePool, device: &str) -> Result<(String, String), Error> {
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let token = codec::encode_token(&raw);
    sqlx::query("insert into sessions(hash, device, created) values (?, ?, ?)")
        .bind(sylvie_core::codec::digest(token.as_bytes()))
        .bind(device)
        .bind(stamp())
        .execute(db)
        .await
        .map_err(sql)?;
    Ok((token, device.to_string()))
}
