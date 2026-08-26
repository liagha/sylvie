use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters,
};
use reqwest::Client;

use sylvie_core::codec;
use sylvie_core::error::Error;
use sylvie_core::message::{
    BlobReply, LoginFinish, LoginReply, LoginStart, RegisterFinish, RegisterStart, Sealed,
};
use sylvie_core::opaque::{self, Suite};
use sylvie_core::vault;

use crate::net;

const REGISTER_START: &str = "/api/v1/auth/register/start";
const REGISTER_FINISH: &str = "/api/v1/auth/register/finish";
const LOGIN_START: &str = "/api/v1/auth/login/start";
const LOGIN_FINISH: &str = "/api/v1/auth/login/finish";

pub struct Session {
    pub token: String,
    pub device: String,
    pub export: Vec<u8>,
}

pub async fn register(
    http: &Client,
    base: &str,
    user: &str,
    password: &str,
    name: &str,
) -> Result<Session, Error> {
    let started = ClientRegistration::<Suite>::start(&mut OsRng, password.as_bytes())
        .map_err(|_| Error::Protocol)?;
    let reply: BlobReply = net::post(
        http,
        base,
        REGISTER_START,
        None,
        &RegisterStart {
            username: user.to_string(),
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await?;
    let finished = started
        .state
        .finish(
            &mut OsRng,
            password.as_bytes(),
            opaque::reg_reply(&codec::decode(&reply.message)?)?,
            ClientRegistrationFinishParameters::new(opaque::peer(user), None),
        )
        .map_err(|_| Error::Protocol)?;
    let _: () = net::post(
        http,
        base,
        REGISTER_FINISH,
        None,
        &RegisterFinish {
            username: user.to_string(),
            message: codec::encode(&finished.message.serialize()),
        },
    )
    .await?;
    login(http, base, user, password, None, Some(name)).await
}

pub async fn login(
    http: &Client,
    base: &str,
    user: &str,
    password: &str,
    device: Option<&str>,
    name: Option<&str>,
) -> Result<Session, Error> {
    let started = ClientLogin::<Suite>::start(&mut OsRng, password.as_bytes())
        .map_err(|_| Error::Protocol)?;
    let reply: LoginReply = net::post(
        http,
        base,
        LOGIN_START,
        None,
        &LoginStart {
            username: user.to_string(),
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await?;
    let finished = started
        .state
        .finish(
            &mut OsRng,
            password.as_bytes(),
            opaque::log_reply(&codec::decode(&reply.message)?)?,
            ClientLoginFinishParameters::new(None, opaque::peer(user), None),
        )
        .map_err(|_| Error::Auth)?;
    let sealed: Sealed = net::post(
        http,
        base,
        LOGIN_FINISH,
        None,
        &LoginFinish {
            id: reply.id,
            message: codec::encode(&finished.message.serialize()),
            device: device.map(str::to_string),
            name: name.map(str::to_string),
        },
    )
    .await?;
    let channel = vault::root(finished.session_key.as_slice(), vault::CHANNEL)?;
    let grant: Vec<u8> = vault::open(&channel, &codec::decode(&sealed.data)?)?;
    let grant: sylvie_core::message::Grant =
        serde_json::from_slice(&grant).map_err(|_| Error::Protocol)?;
    Ok(Session {
        token: grant.token,
        device: grant.device,
        export: finished.export_key.as_slice().to_vec(),
    })
}
