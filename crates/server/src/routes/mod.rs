pub mod account;
pub mod auth;
pub mod device;
pub mod file;
pub mod secret;
pub mod web;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};

use sylvie_core::error::Error;

use crate::ctx::Ctx;

pub(crate) fn sane(text: &str, limit: usize) -> bool {
    !text.is_empty()
        && text.len() <= limit
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'))
}

pub(crate) fn ident(raw: &str) -> Result<String, Error> {
    uuid::Uuid::parse_str(raw)
        .map(|id| id.to_string())
        .map_err(|_| Error::Request)
}

pub fn build(ctx: Ctx) -> Router {
    let limit = ctx.max_file();
    let pages = web::router(ctx.clone());
    Router::new()
        .route("/api/v1/auth/register/start", post(auth::register_start))
        .route("/api/v1/auth/register/finish", post(auth::register_finish))
        .route("/api/v1/auth/login/start", post(auth::login_start))
        .route("/api/v1/auth/login/finish", post(auth::login_finish))
        .route("/api/v1/me", get(auth::me))
        .route("/api/v1/vault", get(auth::vault))
        .route("/api/v1/auth/rekey/start", post(auth::rekey_start))
        .route("/api/v1/auth/rekey/finish", post(auth::rekey_finish))
        .route("/api/v1/devices", get(device::list))
        .route("/api/v1/devices/{id}", delete(device::revoke))
        .route("/api/v1/secrets", get(secret::list))
        .route(
            "/api/v1/secrets/{name}",
            put(secret::put).get(secret::get).delete(secret::del),
        )
        .route("/api/v1/files", post(file::upload).get(file::list))
        .route("/api/v1/files/{id}", get(file::meta).delete(file::remove))
        .route("/api/v1/files/{id}/content", get(file::content))
        .layer(DefaultBodyLimit::max(limit as usize))
        .with_state(ctx)
        .merge(pages)
}
