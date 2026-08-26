use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("malformed request")]
    Request,
    #[error("authentication required")]
    Auth,
    #[error("insufficient rights")]
    Forbidden,
    #[error("missing resource")]
    Missing,
    #[error("resource already exists")]
    Conflict,
    #[error("payload exceeds limit")]
    Large,
    #[error("too many attempts")]
    Flood,
    #[error("cryptographic failure")]
    Crypto,
    #[error("protocol violation")]
    Protocol,
    #[error("{0}")]
    Internal(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Request => "bad_request",
            Self::Auth => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Missing => "not_found",
            Self::Conflict => "conflict",
            Self::Large => "too_large",
            Self::Flood => "rate_limited",
            Self::Crypto => "crypto",
            Self::Protocol => "protocol",
            Self::Internal(_) => "internal",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "bad_request" => Self::Request,
            "unauthorized" => Self::Auth,
            "forbidden" => Self::Forbidden,
            "not_found" => Self::Missing,
            "conflict" => Self::Conflict,
            "too_large" => Self::Large,
            "rate_limited" => Self::Flood,
            "crypto" => Self::Crypto,
            "protocol" => Self::Protocol,
            _ => Self::Internal("remote failure".into()),
        }
    }
}
