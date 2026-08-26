use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use sylvie_core::codec;
use sylvie_core::error::Error;
use sylvie_core::message::{SecretItem, SecretPut, SecretValue};

use crate::clock::stamp;
use crate::ctx::Ctx;
use crate::reply::{Failure, internal};
use crate::routes::account::Account;
use crate::routes::sane;

const SECRET_LIMIT: usize = 64 * 1024;

pub async fn put(
    State(ctx): State<Ctx>,
    account: Account,
    Path(name): Path<String>,
    Json(req): Json<SecretPut>,
) -> Result<StatusCode, Failure> {
    sane(&name, 128).ok_or(Error::Request)?;
    let data = codec::decode(&req.data)?;
    if data.len() > SECRET_LIMIT {
        return Err(Error::Large.into());
    }
    let now = stamp();
    sqlx::query(
        "insert into secrets(owner, name, data, created, updated) values (?, ?, ?, ?, ?) \
         on conflict(owner, name) do update set data = excluded.data, updated = excluded.updated",
    )
    .bind(&account.owner)
    .bind(&name)
    .bind(data)
    .bind(&now)
    .bind(&now)
    .execute(ctx.db())
    .await
    .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get(
    State(ctx): State<Ctx>,
    account: Account,
    Path(name): Path<String>,
) -> Result<Json<SecretValue>, Failure> {
    sane(&name, 128).ok_or(Error::Request)?;
    let row: Option<(Vec<u8>,)> =
        sqlx::query_as("select data from secrets where owner = ? and name = ?")
            .bind(&account.owner)
            .bind(&name)
            .fetch_optional(ctx.db())
            .await
            .map_err(internal)?;
    Ok(Json(SecretValue {
        data: codec::encode(&row.ok_or(Error::Missing)?.0),
    }))
}

pub async fn list(
    State(ctx): State<Ctx>,
    account: Account,
) -> Result<Json<Vec<SecretItem>>, Failure> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("select name, updated from secrets where owner = ? order by updated desc")
            .bind(&account.owner)
            .fetch_all(ctx.db())
            .await
            .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(name, updated)| SecretItem { name, updated })
            .collect(),
    ))
}

pub async fn del(
    State(ctx): State<Ctx>,
    account: Account,
    Path(name): Path<String>,
) -> Result<StatusCode, Failure> {
    sane(&name, 128).ok_or(Error::Request)?;
    let gone = sqlx::query("delete from secrets where owner = ? and name = ?")
        .bind(&account.owner)
        .bind(&name)
        .execute(ctx.db())
        .await
        .map_err(internal)?
        .rows_affected();
    if gone == 0 {
        return Err(Error::Missing.into());
    }
    Ok(StatusCode::NO_CONTENT)
}
