use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use sylvie_core::error::Error;

use crate::ctx::Ctx;
use crate::reply::{Failure, internal};

const BEARER: &str = "Bearer ";

pub struct Account {
    pub owner: String,
    pub device: String,
}

impl FromRequestParts<Ctx> for Account {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, ctx: &Ctx) -> Result<Self, Self::Rejection> {
        let header = parts.headers.get(AUTHORIZATION).ok_or(Error::Auth)?;
        let header = header.to_str().map_err(|_| Error::Auth)?;
        let token = header.strip_prefix(BEARER).ok_or(Error::Auth)?;
        let row: Option<(String, String)> = sqlx::query_as(
            "select d.owner, d.id \
             from sessions s join devices d on d.id = s.device \
             where s.hash = ? and d.revoked is null",
        )
        .bind(sylvie_core::codec::digest(token.as_bytes()))
        .fetch_optional(ctx.db())
        .await
        .map_err(internal)?;
        let (owner, device) = row.ok_or(Error::Auth)?;
        Ok(Self { owner, device })
    }
}
