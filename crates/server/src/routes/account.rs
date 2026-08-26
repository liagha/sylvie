use axum::extract::FromRequestParts;
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::http::request::Parts;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use sylvie_core::codec;
use sylvie_core::error::Error;

use crate::ctx::Ctx;
use crate::reply::{Failure, internal};

const BEARER: &str = "Bearer ";
const TOKEN: &str = "sylvie_token";

pub struct Account {
    pub owner: String,
    pub device: String,
}

impl FromRequestParts<Ctx> for Account {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, ctx: &Ctx) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or(Error::Auth)?;
        let row: Option<(String, String, String)> = sqlx::query_as(
            "select d.owner, d.id, s.created \
             from sessions s join devices d on d.id = s.device \
             where s.hash = ? and d.revoked is null",
        )
        .bind(codec::digest(token.as_bytes()))
        .fetch_optional(ctx.db())
        .await
        .map_err(internal)?;
        let (owner, device, created) = row.ok_or(Error::Auth)?;
        fresh(ctx, &created)?;
        Ok(Self { owner, device })
    }
}

fn bearer_token(parts: &Parts) -> Option<String> {
    if let Some(token) = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|text| text.strip_prefix(BEARER))
    {
        return Some(token.to_string());
    }
    let cookies = parts.headers.get(COOKIE)?.to_str().ok()?;
    let prefix = format!("{TOKEN}=");
    cookies
        .split(';')
        .find_map(|pair| pair.trim().strip_prefix(&prefix).map(str::to_string))
}

fn fresh(ctx: &Ctx, created: &str) -> Result<(), Failure> {
    let ttl = match ctx.limits().session_ttl {
        Some(ttl) => ttl,
        None => return Ok(()),
    };
    let born = OffsetDateTime::parse(created, &Rfc3339).map_err(|_| Failure(Error::Auth))?;
    if (OffsetDateTime::now_utc() - born)
        > time::SignedDuration::try_from(ttl).map_err(|_| Failure(Error::Auth))?
    {
        return Err(Error::Auth.into());
    }
    Ok(())
}
