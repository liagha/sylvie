use opaque_ke::rand::RngCore;
use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters,
};
use reqwest::Client;

use sylvie_core::codec;
use sylvie_core::error::Error;
use sylvie_core::message::{
    BlobReply, LoginFinish, LoginReply, LoginStart, RegisterFinish, RegisterStart, RekeyFinish,
    RekeyStart, Sealed, WrapValue,
};
use sylvie_core::opaque::{self, Suite};
use sylvie_core::vault;

use crate::net;

const REGISTER_START: &str = "/api/v1/auth/register/start";
const REGISTER_FINISH: &str = "/api/v1/auth/register/finish";
const LOGIN_START: &str = "/api/v1/auth/login/start";
const LOGIN_FINISH: &str = "/api/v1/auth/login/finish";
const REKEY_START: &str = "/api/v1/auth/rekey/start";
const REKEY_FINISH: &str = "/api/v1/auth/rekey/finish";
const VAULT: &str = "/api/v1/vault";

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
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let kek = vault::root(finished.export_key.as_slice(), vault::VAULT)?;
    let wrap = vault::seal(&kek, &secret)?;
    let _: () = net::post(
        http,
        base,
        REGISTER_FINISH,
        None,
        &RegisterFinish {
            username: user.to_string(),
            message: codec::encode(&finished.message.serialize()),
            wrap: codec::encode(&wrap),
        },
    )
    .await?;
    login(http, base, user, password, None, Some(name)).await
}

pub async fn rekey(
    http: &Client,
    base: &str,
    user: &str,
    old: &str,
    fresh: &str,
    device: &str,
    token: &str,
) -> Result<(), Error> {
    let session = login(http, base, user, old, Some(device), None).await?;
    let kek_old = vault::root(&session.export, vault::VAULT)?;
    let wrapped: WrapValue = net::get(http, base, VAULT, Some(token)).await?;
    let secret = vault::open(&kek_old, &codec::decode(&wrapped.data)?)?;

    let started = ClientRegistration::<Suite>::start(&mut OsRng, fresh.as_bytes())
        .map_err(|_| Error::Protocol)?;
    let reply: BlobReply = net::post(
        http,
        base,
        REKEY_START,
        Some(token),
        &RekeyStart {
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await?;
    let finished = started
        .state
        .finish(
            &mut OsRng,
            fresh.as_bytes(),
            opaque::reg_reply(&codec::decode(&reply.message)?)?,
            ClientRegistrationFinishParameters::new(opaque::peer(user), None),
        )
        .map_err(|_| Error::Protocol)?;
    let kek_new = vault::root(finished.export_key.as_slice(), vault::VAULT)?;
    let wrap = vault::seal(&kek_new, &secret)?;
    let _: () = net::post(
        http,
        base,
        REKEY_FINISH,
        Some(token),
        &RekeyFinish {
            message: codec::encode(&finished.message.serialize()),
            wrap: codec::encode(&wrap),
        },
    )
    .await?;
    Ok(())
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
