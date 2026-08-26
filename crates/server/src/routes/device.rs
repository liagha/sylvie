use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use sylvie_core::error::Error;
use sylvie_core::message::Device;

use crate::clock::stamp;
use crate::ctx::Ctx;
use crate::reply::{Failure, internal};
use crate::routes::account::Account;
use crate::routes::ident;

pub async fn list(State(ctx): State<Ctx>, account: Account) -> Result<Json<Vec<Device>>, Failure> {
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "select id, name, created, revoked from devices where owner = ? order by created",
    )
    .bind(&account.owner)
    .fetch_all(ctx.db())
    .await
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, name, created, revoked)| Device {
                id,
                name,
                created,
                revoked,
            })
            .collect(),
    ))
}

pub async fn revoke(
    State(ctx): State<Ctx>,
    account: Account,
    Path(id): Path<String>,
) -> Result<StatusCode, Failure> {
    let id = ident(&id)?;
    let row: Option<(Option<String>,)> =
        sqlx::query_as("select revoked from devices where id = ? and owner = ?")
            .bind(&id)
            .bind(&account.owner)
            .fetch_optional(ctx.db())
            .await
            .map_err(internal)?;
    match row {
        None => Err(Error::Missing.into()),
        Some((Some(_),)) => Ok(StatusCode::NO_CONTENT),
        Some((None,)) => {
            sqlx::query("update devices set revoked = ? where id = ?")
                .bind(stamp())
                .bind(&id)
                .execute(ctx.db())
                .await
                .map_err(internal)?;
            sqlx::query("delete from sessions where device = ?")
                .bind(&id)
                .execute(ctx.db())
                .await
                .map_err(internal)?;
            tracing::info!(owner = %account.owner, device = %id, "device revoked");
            Ok(StatusCode::NO_CONTENT)
        }
    }
}
