use opaque_ke::{
    CipherSuite, CredentialFinalization, CredentialRequest, CredentialResponse, Identifiers,
    RegistrationRequest, RegistrationResponse, RegistrationUpload, Ristretto255, ServerLogin,
    ServerRegistration, ServerSetup, TripleDh,
};
use sha2::Sha512;

use crate::error::Error;

pub struct Suite;

impl CipherSuite for Suite {
    type OprfCs = Ristretto255;
    type KeyExchange = TripleDh<Ristretto255, Sha512>;
    type Ksf = opaque_ke::argon2::Argon2<'static>;
}

pub type Setup = ServerSetup<Suite>;
pub type Record = ServerRegistration<Suite>;
pub type LogState = ServerLogin<Suite>;

pub type RegAsk = RegistrationRequest<Suite>;
pub type RegReply = RegistrationResponse<Suite>;
pub type RegGive = RegistrationUpload<Suite>;
pub type LogAsk = CredentialRequest<Suite>;
pub type LogReply = CredentialResponse<Suite>;
pub type LogGive = CredentialFinalization<Suite>;

fn fail() -> Error {
    Error::Protocol
}

pub fn peer(name: &str) -> Identifiers<'_> {
    Identifiers {
        client: Some(name.as_bytes()),
        server: None,
    }
}

pub fn reg_ask(bytes: &[u8]) -> Result<RegAsk, Error> {
    RegAsk::deserialize(bytes).map_err(|_| fail())
}

pub fn reg_reply(bytes: &[u8]) -> Result<RegReply, Error> {
    RegReply::deserialize(bytes).map_err(|_| fail())
}

pub fn reg_give(bytes: &[u8]) -> Result<RegGive, Error> {
    RegGive::deserialize(bytes).map_err(|_| fail())
}

pub fn log_ask(bytes: &[u8]) -> Result<LogAsk, Error> {
    LogAsk::deserialize(bytes).map_err(|_| fail())
}

pub fn log_reply(bytes: &[u8]) -> Result<LogReply, Error> {
    LogReply::deserialize(bytes).map_err(|_| fail())
}

pub fn log_give(bytes: &[u8]) -> Result<LogGive, Error> {
    LogGive::deserialize(bytes).map_err(|_| fail())
}

pub fn record_load(bytes: &[u8]) -> Result<Record, Error> {
    Record::deserialize(bytes).map_err(|_| fail())
}
