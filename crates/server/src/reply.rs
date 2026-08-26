use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use sylvie_core::error::Error;

pub struct Failure(pub Error);

impl From<Error> for Failure {
    fn from(value: Error) -> Self {
        Self(value)
    }
}

#[derive(Serialize)]
struct Fault<'a> {
    error: &'a str,
}

pub(crate) fn internal<E: std::fmt::Display>(error: E) -> Failure {
    Failure(Error::Internal(error.to_string()))
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        if let Error::Internal(text) = &self.0 {
            tracing::error!(error = %text, "internal failure");
        }
        let status = match &self.0 {
            Error::Request => StatusCode::BAD_REQUEST,
            Error::Auth => StatusCode::UNAUTHORIZED,
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::Missing => StatusCode::NOT_FOUND,
            Error::Conflict => StatusCode::CONFLICT,
            Error::Large => StatusCode::PAYLOAD_TOO_LARGE,
            Error::Crypto | Error::Protocol | Error::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (
            status,
            Json(Fault {
                error: self.0.code(),
            }),
        )
            .into_response()
    }
}
