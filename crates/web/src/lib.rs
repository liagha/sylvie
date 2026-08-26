use std::cell::RefCell;
use std::collections::HashMap;

use serde_json::json;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use sylvie_core::codec;
use sylvie_core::opaque::{self, Suite};
use sylvie_core::vault;

use opaque_ke::rand::RngCore;
use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters,
};

type Fail<T> = Result<T, String>;

#[cfg(not(target_arch = "wasm32"))]
pub use {
    derive_vault_impl as derive_vault, drop_session_impl as drop_session,
    finish_login_impl as finish_login, finish_registration_impl as finish_registration,
    open_login_impl as open_login, open_secret_impl as open_secret,
    seal_secret_impl as seal_secret, start_login_impl as start_login,
    start_registration_impl as start_registration,
};

struct Session {
    username: String,
    password: Vec<u8>,
    registration: Option<ClientRegistration<Suite>>,
    login: Option<ClientLogin<Suite>>,
    export: Vec<u8>,
    datakey: Vec<u8>,
}

thread_local! {
    static SESSIONS: RefCell<HashMap<u64, Session>> = RefCell::new(HashMap::new());
}

fn put(session: Session) -> u64 {
    SESSIONS.with(|cell| {
        let mut sessions = cell.borrow_mut();
        let id = sessions.keys().copied().max().unwrap_or(0) + 1;
        sessions.insert(id, session);
        id
    })
}

fn take(id: u64) -> Fail<Session> {
    SESSIONS
        .with(|cell| cell.borrow_mut().remove(&id))
        .ok_or_else(|| bad("unknown handle"))
}

fn edit<R>(id: u64, apply: impl FnOnce(&mut Session) -> R) -> Fail<R> {
    SESSIONS
        .with(|cell| {
            let mut sessions = cell.borrow_mut();
            sessions.get_mut(&id).map(apply)
        })
        .ok_or_else(|| bad("unknown handle"))
}

fn json(value: serde_json::Value) -> Fail<String> {
    serde_json::to_string(&value).map_err(|_| bad("encode failure"))
}

fn bad(text: &str) -> String {
    text.to_string()
}

pub fn start_registration_impl(username: &str, password: &str) -> Fail<String> {
    let started = ClientRegistration::<Suite>::start(&mut OsRng, password.as_bytes())
        .map_err(|_| bad("registration start failed"))?;
    let handle = put(Session {
        username: username.to_string(),
        password: password.as_bytes().to_vec(),
        registration: Some(started.state),
        login: None,
        export: Vec::new(),
        datakey: Vec::new(),
    });
    json(json!({
        "handle": handle.to_string(),
        "username": username,
        "request": codec::encode(&started.message.serialize()),
    }))
}

pub fn finish_registration_impl(handle: u64, response: &str) -> Fail<String> {
    let mut session = take(handle)?;
    let started = session
        .registration
        .take()
        .ok_or_else(|| bad("registration already finished"))?;
    let finished = started
        .finish(
            &mut OsRng,
            &session.password,
            opaque::reg_reply(&codec::decode(response).map_err(|_| bad("bad response"))?)
                .map_err(|_| bad("bad response"))?,
            ClientRegistrationFinishParameters::new(opaque::peer(&session.username), None),
        )
        .map_err(|_| bad("registration rejected"))?;

    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let kek = vault::root(finished.export_key.as_slice(), vault::VAULT)
        .map_err(|_| bad("key derivation failed"))?;
    let wrap = vault::seal(&kek, &secret).map_err(|_| bad("seal failed"))?;

    session.export = finished.export_key.as_slice().to_vec();
    let handle = put(session);
    json(json!({
        "handle": handle.to_string(),
        "message": codec::encode(&finished.message.serialize()),
        "wrap": codec::encode(&wrap),
    }))
}

pub fn start_login_impl(username: &str, password: &str) -> Fail<String> {
    let started = ClientLogin::<Suite>::start(&mut OsRng, password.as_bytes())
        .map_err(|_| bad("login start failed"))?;
    let handle = put(Session {
        username: username.to_string(),
        password: password.as_bytes().to_vec(),
        registration: None,
        login: Some(started.state),
        export: Vec::new(),
        datakey: Vec::new(),
    });
    json(json!({
        "handle": handle.to_string(),
        "username": username,
        "request": codec::encode(&started.message.serialize()),
    }))
}

#[allow(clippy::too_many_arguments)]
pub fn finish_login_impl(
    handle: u64,
    reply: &str,
    device: Option<String>,
    name: Option<String>,
) -> Fail<String> {
    let mut session = take(handle)?;
    let started = session
        .login
        .take()
        .ok_or_else(|| bad("login already finished"))?;
    let finished = started
        .finish(
            &mut OsRng,
            &session.password,
            opaque::log_reply(&codec::decode(reply).map_err(|_| bad("bad reply"))?)
                .map_err(|_| bad("bad reply"))?,
            ClientLoginFinishParameters::new(None, opaque::peer(&session.username), None),
        )
        .map_err(|_| bad("wrong password or unknown user"))?;

    session.login = None;
    session.export = finished.export_key.as_slice().to_vec();
    let handle = put(session);
    json(json!({
        "handle": handle.to_string(),
        "message": codec::encode(&finished.message.serialize()),
        "device": device,
        "name": name,
    }))
}

pub fn open_login_impl(handle: u64, sealed: &str) -> Fail<String> {
    let channel = peek_channel(handle)?;
    let grant = vault::open(
        &channel,
        &codec::decode(sealed).map_err(|_| bad("bad seal"))?,
    )
    .map_err(|_| bad("wrong password"))?;
    let grant: serde_json::Value = serde_json::from_slice(&grant).map_err(|_| bad("bad grant"))?;
    json(json!({
        "token": grant["token"],
        "device": grant["device"],
    }))
}

pub fn derive_vault_impl(handle: u64, wrapped: &str) -> Fail<()> {
    let kek = edit(handle, |session| vault::root(&session.export, vault::VAULT))
        .and_then(|inner| inner.map_err(|_| bad("key derivation failed")))?;
    let secret = vault::open(&kek, &codec::decode(wrapped).map_err(|_| bad("bad wrap"))?)
        .map_err(|_| bad("wrong password"))?;
    let datakey = vault::root(&secret, vault::DATA).map_err(|_| bad("key derivation failed"))?;
    edit(handle, |session| session.datakey = datakey)?;
    Ok(())
}

pub fn seal_secret_impl(handle: u64, plain: &str) -> Fail<String> {
    let key = datakey_of(handle)?;
    vault::seal(&key, plain.as_bytes())
        .map(|sealed| codec::encode(&sealed))
        .map_err(|_| bad("seal failed"))
}

pub fn open_secret_impl(handle: u64, boxed: &str) -> Fail<String> {
    let key = datakey_of(handle)?;
    let plain = vault::open(&key, &codec::decode(boxed).map_err(|_| bad("bad data"))?)
        .map_err(|_| bad("wrong key"))?;
    String::from_utf8(plain).map_err(|_| bad("binary secret"))
}

fn peek_channel(handle: u64) -> Fail<Vec<u8>> {
    edit(handle, |session| {
        vault::root(&session.export, vault::CHANNEL)
    })
    .and_then(|inner| inner.map_err(|_| bad("key derivation failed")))
}

pub fn drop_session_impl(handle: u64) -> Fail<()> {
    take(handle)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn drop_session(handle: u64) -> Result<(), JsValue> {
    drop_session_impl(handle).map_err(|error| JsValue::from_str(&error))
}

fn datakey_of(handle: u64) -> Fail<Vec<u8>> {
    let key = edit(handle, |session| session.datakey.clone())?;
    if key.is_empty() {
        return Err(bad("vault not derived"));
    }
    Ok(key)
}
#[cfg(target_arch = "wasm32")]
mod exports {
    use super::*;
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub fn start_registration(username: &str, password: &str) -> Result<String, JsValue> {
        start_registration_impl(username, password).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn finish_registration(handle: u64, response: &str) -> Result<String, JsValue> {
        finish_registration_impl(handle, response).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn start_login(username: &str, password: &str) -> Result<String, JsValue> {
        start_login_impl(username, password).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn finish_login(
        handle: u64,
        reply: &str,
        device: Option<String>,
        name: Option<String>,
    ) -> Result<String, JsValue> {
        finish_login_impl(handle, reply, device, name).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn open_login(handle: u64, sealed: &str) -> Result<String, JsValue> {
        open_login_impl(handle, sealed).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn derive_vault(handle: u64, wrapped: &str) -> Result<(), JsValue> {
        derive_vault_impl(handle, wrapped).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn seal_secret(handle: u64, plain: &str) -> Result<String, JsValue> {
        seal_secret_impl(handle, plain).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn open_secret(handle: u64, boxed: &str) -> Result<String, JsValue> {
        open_secret_impl(handle, boxed).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen]
    pub fn drop_session(handle: u64) {
        let _ = take(handle);
    }
}
