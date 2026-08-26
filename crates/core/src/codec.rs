use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

use crate::error::Error;

pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub fn decode(text: &str) -> Result<Vec<u8>, Error> {
    STANDARD.decode(text).map_err(|_| Error::Protocol)
}

pub fn encode_token(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn decode_token(text: &str) -> Result<Vec<u8>, Error> {
    URL_SAFE_NO_PAD.decode(text).map_err(|_| Error::Protocol)
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
