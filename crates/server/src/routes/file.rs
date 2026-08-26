use std::collections::HashMap;

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::Response;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use sylvie_core::error::Error;
use sylvie_core::message::FileItem;

use crate::clock::stamp;
use crate::reply::{Failure, internal};

fn sql<E: std::fmt::Display>(error: E) -> Error {
    Error::Internal(error.to_string())
}
use crate::ctx::Ctx;
use crate::routes::account::Account;
use crate::routes::{ident, sane};

const NAME_LIMIT: usize = 200;

pub async fn upload(
    State(ctx): State<Ctx>,
    account: Account,
    Query(query): Query<HashMap<String, String>>,
    body: Bytes,
) -> Result<(StatusCode, Json<FileItem>), Failure> {
    let name = query.get("name").ok_or(Error::Request)?;
    sane(name, NAME_LIMIT).ok_or(Error::Request)?;
    if body.len() as u64 > ctx.max_file() {
        return Err(Error::Large.into());
    }
    let id = Uuid::new_v4().to_string();
    tokio::fs::write(ctx.object_path(&id), &body)
        .await
        .map_err(internal)?;
    let now = stamp();
    let digest = sylvie_core::codec::digest(&body);
    sqlx::query(
        "insert into files(id, owner, name, size, hash, path, created, updated) \
         values (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&account.owner)
    .bind(name)
    .bind(body.len() as i64)
    .bind(&digest)
    .bind(&id)
    .bind(&now)
    .bind(&now)
    .execute(ctx.db())
    .await
    .map_err(internal)?;
    tracing::info!(owner = %account.owner, file = %id, size = body.len(), "file stored");
    Ok((
        StatusCode::CREATED,
        Json(FileItem {
            id,
            name: name.clone(),
            size: body.len() as u64,
            hash: digest,
            created: now.clone(),
            updated: now,
        }),
    ))
}

pub async fn list(
    State(ctx): State<Ctx>,
    account: Account,
) -> Result<Json<Vec<FileItem>>, Failure> {
    let rows: Vec<(String, String, i64, String, String, String)> = sqlx::query_as(
        "select id, name, size, hash, created, updated from files where owner = ? order by created desc",
    )
    .bind(&account.owner)
    .fetch_all(ctx.db())
    .await
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, name, size, hash_v, created, updated)| FileItem {
                id,
                name,
                size: size as u64,
                hash: hash_v,
                created,
                updated,
            })
            .collect(),
    ))
}

pub async fn meta(
    State(ctx): State<Ctx>,
    account: Account,
    Path(id): Path<String>,
) -> Result<Json<FileItem>, Failure> {
    fetch_item(ctx.db(), &ident(&id)?, &account.owner)
        .await
        .map(Json)
        .map_err(Failure)
}

pub async fn content(
    State(ctx): State<Ctx>,
    account: Account,
    Path(id): Path<String>,
) -> Result<Response, Failure> {
    let item = fetch_item(ctx.db(), &ident(&id)?, &account.owner).await?;
    let file = tokio::fs::File::open(ctx.object_path(&item.id))
        .await
        .map_err(internal)?;
    let disposition = format!("attachment; filename=\"{}\"", item.name.replace('"', "'"));
    Response::builder()
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(CONTENT_DISPOSITION, disposition)
        .body(Body::from_stream(ReaderStream::with_capacity(file, 65536)))
        .map_err(internal)
}

pub async fn remove(
    State(ctx): State<Ctx>,
    account: Account,
    Path(id): Path<String>,
) -> Result<StatusCode, Failure> {
    let id = ident(&id)?;
    let row: Option<(String,)> =
        sqlx::query_as("select path from files where id = ? and owner = ?")
            .bind(&id)
            .bind(&account.owner)
            .fetch_optional(ctx.db())
            .await
            .map_err(internal)?;
    let path = row.ok_or(Error::Missing)?.0;
    sqlx::query("delete from files where id = ?")
        .bind(&id)
        .execute(ctx.db())
        .await
        .map_err(internal)?;
    if let Err(error) = tokio::fs::remove_file(ctx.object_path(&path)).await {
        tracing::warn!(file = %id, error = %error, "orphaned object");
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_item(db: &sqlx::SqlitePool, id: &str, owner: &str) -> Result<FileItem, Error> {
    let row: Option<(String, String, i64, String, String, String)> = sqlx::query_as(
        "select id, name, size, hash, created, updated from files where id = ? and owner = ?",
    )
    .bind(id)
    .bind(owner)
    .fetch_optional(db)
    .await
    .map_err(sql)?;
    row.map(|(id, name, size, hash_v, created, updated)| FileItem {
        id,
        name,
        size: size as u64,
        hash: hash_v,
        created,
        updated,
    })
    .ok_or(Error::Missing)
}
