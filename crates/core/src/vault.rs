use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use hkdf::Hkdf;
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::Sha512;

use crate::error::Error;

pub const VAULT: &[u8] = b"sylvie/vault";
pub const DATA: &[u8] = b"sylvie/data";
pub const CHANNEL: &[u8] = b"sylvie/channel";

const KEY: usize = 32;
const NONCE: usize = 24;

pub fn root(secret: &[u8], label: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; KEY];
    Hkdf::<Sha512>::new(None, secret)
        .expand(label, &mut out)
        .map_err(|_| Error::Crypto)?;
    Ok(out)
}

pub fn seal(key: &[u8], plain: &[u8]) -> Result<Vec<u8>, Error> {
    let key = Key::try_from(key).map_err(|_| Error::Crypto)?;
    let cipher = XChaCha20Poly1305::new(&key);
    let mut raw = [0u8; NONCE];
    OsRng.fill_bytes(&mut raw);
    let nonce = XNonce::try_from(&raw[..]).map_err(|_| Error::Crypto)?;
    let boxed = cipher.encrypt(&nonce, plain).map_err(|_| Error::Crypto)?;
    let mut out = raw.to_vec();
    out.extend_from_slice(&boxed);
    Ok(out)
}

pub fn open(key: &[u8], sealed: &[u8]) -> Result<Vec<u8>, Error> {
    if sealed.len() <= NONCE {
        return Err(Error::Crypto);
    }
    let key = Key::try_from(key).map_err(|_| Error::Crypto)?;
    let cipher = XChaCha20Poly1305::new(&key);
    let (nonce, boxed) = sealed.split_at(NONCE);
    let nonce = XNonce::try_from(nonce).map_err(|_| Error::Crypto)?;
    cipher.decrypt(&nonce, boxed).map_err(|_| Error::Crypto)
}
