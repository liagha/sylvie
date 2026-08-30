use std::collections::HashMap;

use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters, ServerLogin, ServerLoginParameters, ServerRegistration,
};
use serde_json::Value;

use sylvie_core::codec;
use sylvie_core::opaque::{self, Suite};

fn field(text: &str, key: &str) -> String {
    let value: Value = serde_json::from_str(text).unwrap();
    value[key].as_str().unwrap().to_string()
}

struct Hub {
    setup: opaque::Setup,
    users: HashMap<String, Vec<u8>>,
    wraps: HashMap<String, Vec<u8>>,
    pendings: HashMap<String, (opaque::LogState, String)>,
}

impl Hub {
    fn new() -> Self {
        Self {
            setup: opaque::Setup::new(&mut OsRng),
            users: HashMap::new(),
            wraps: HashMap::new(),
            pendings: HashMap::new(),
        }
    }

    fn register_start(&self, user: &str, request_b64: &str) -> String {
        let ask = opaque::reg_ask(&codec::decode(request_b64).unwrap()).unwrap();
        let started =
            ServerRegistration::<Suite>::start(&self.setup, ask, user.as_bytes()).unwrap();
        codec::encode(&started.message.serialize())
    }

    fn register_finish(&mut self, user: &str, message_b64: &str, wrap_b64: &str) {
        let give = opaque::reg_give(&codec::decode(message_b64).unwrap()).unwrap();
        let record = ServerRegistration::<Suite>::finish(give);
        self.users.insert(user.into(), record.serialize().to_vec());
        self.wraps
            .insert(user.into(), codec::decode(wrap_b64).unwrap());
    }

    fn login_start(&mut self, user: &str, request_b64: &str, id: &str) -> String {
        let record = self
            .users
            .get(user)
            .map(|bytes| opaque::record_load(bytes).unwrap());
        let ask = opaque::log_ask(&codec::decode(request_b64).unwrap()).unwrap();
        let params = ServerLoginParameters {
            identifiers: opaque::peer(user),
            ..Default::default()
        };
        let started = ServerLogin::<Suite>::start(
            &mut OsRng,
            &self.setup,
            record,
            ask,
            user.as_bytes(),
            params,
        )
        .unwrap();
        self.pendings
            .insert(id.into(), (started.state, user.into()));
        codec::encode(&started.message.serialize())
    }

    fn login_finish(&mut self, user: &str, id: &str, message_b64: &str) -> String {
        let (state, _) = self.pendings.remove(id).unwrap();
        let give = opaque::log_give(&codec::decode(message_b64).unwrap()).unwrap();
        let params = ServerLoginParameters {
            identifiers: opaque::peer(user),
            ..Default::default()
        };
        state.finish(give, params).unwrap();
        "sealed-payload".to_string()
    }

    fn wrap_of(&self, user: &str) -> String {
        codec::encode(&self.wraps[user])
    }

    fn rekey_start(&self, user: &str, request_b64: &str) -> String {
        let ask = opaque::reg_ask(&codec::decode(request_b64).unwrap()).unwrap();
        let started =
            ServerRegistration::<Suite>::start(&self.setup, ask, user.as_bytes()).unwrap();
        codec::encode(&started.message.serialize())
    }

    fn rekey_finish(&mut self, user: &str, message_b64: &str, wrap_b64: &str) {
        let give = opaque::reg_give(&codec::decode(message_b64).unwrap()).unwrap();
        let record = ServerRegistration::<Suite>::finish(give);
        self.users.insert(user.into(), record.serialize().to_vec());
        self.wraps
            .insert(user.into(), codec::decode(wrap_b64).unwrap());
    }
}

#[test]
fn rekey_preserves_secrets() {
    let mut hub = Hub::new();

    let start = sylvie_web::start_registration("alee", "first password ok").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let response = hub.register_start("alee", &field(&start, "request"));
    let finished = sylvie_web::finish_registration(handle, &response).unwrap();
    let handle: u64 = field(&finished, "handle").parse().unwrap();
    hub.register_finish(
        "alee",
        &field(&finished, "message"),
        &field(&finished, "wrap"),
    );
    let _ = sylvie_web::drop_session(handle);

    let wrap = hub.wrap_of("alee");

    let start = sylvie_web::start_login("alee", "first password ok").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let reply = hub.login_start("alee", &field(&start, "request"), "pending-r1");
    let finished = sylvie_web::finish_login(handle, &reply, None, Some("web".into())).unwrap();
    let handle: u64 = field(&finished, "handle").parse().unwrap();
    sylvie_web::derive_vault(handle, &wrap).unwrap();
    let boxed = sylvie_web::seal_secret(handle, "vault secret value").unwrap();
    assert_eq!(
        sylvie_web::open_secret(handle, &boxed).unwrap(),
        "vault secret value"
    );
    let _ = sylvie_web::drop_session(handle);

    let start = sylvie_web::start_login("alee", "first password ok").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let reply = hub.login_start("alee", &field(&start, "request"), "pending-r2");
    let finished = sylvie_web::finish_login(handle, &reply, Some("web".into()), None).unwrap();
    let _ = hub.login_finish("alee", "pending-r2", &field(&finished, "message"));
    sylvie_web::derive_vault(handle, &wrap).unwrap();
    let started = sylvie_web::rekey_start(handle, "second password ok").unwrap();
    let response = hub.rekey_start("alee", &field(&started, "request"));
    let finished = sylvie_web::rekey_finish(handle, &response, "second password ok").unwrap();
    hub.rekey_finish(
        "alee",
        &field(&finished, "message"),
        &field(&finished, "wrap"),
    );
    let _ = sylvie_web::drop_session(handle);

    let wrap = hub.wrap_of("alee");

    let start = sylvie_web::start_login("alee", "second password ok").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let reply = hub.login_start("alee", &field(&start, "request"), "pending-r3");
    let finished = sylvie_web::finish_login(handle, &reply, None, Some("web".into())).unwrap();
    let handle: u64 = field(&finished, "handle").parse().unwrap();
    sylvie_web::derive_vault(handle, &wrap).unwrap();
    assert_eq!(
        sylvie_web::open_secret(handle, &boxed).unwrap(),
        "vault secret value"
    );
    let _ = sylvie_web::drop_session(handle);
}

#[test]
fn full_browser_flow_matches_cli_crypto() {
    let mut hub = Hub::new();

    let start = sylvie_web::start_registration("alee", "long enough password").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let response = hub.register_start(&field(&start, "username"), &field(&start, "request"));
    let finished = sylvie_web::finish_registration(handle, &response).unwrap();
    let handle: u64 = field(&finished, "handle").parse().unwrap();
    hub.register_finish(
        "alee",
        &field(&finished, "message"),
        &field(&finished, "wrap"),
    );
    let _ = sylvie_web::drop_session(handle);

    let wrap = hub.wrap_of("alee");

    {
        use opaque_ke::{ClientLogin, ClientLoginFinishParameters};
        let started = ClientLogin::<Suite>::start(&mut OsRng, b"long enough password").unwrap();
        let reply = hub.login_start(
            "alee",
            &codec::encode(&started.message.serialize()),
            "direct-1",
        );
        let finished = started
            .state
            .finish(
                &mut OsRng,
                b"long enough password",
                opaque::log_reply(&codec::decode(&reply).unwrap()).unwrap(),
                ClientLoginFinishParameters::new(None, opaque::peer("alee"), None),
            )
            .expect("direct cli-style login must succeed");
        let _ = hub.login_finish(
            "alee",
            "direct-1",
            &codec::encode(&finished.message.serialize()),
        );
    }

    let start = sylvie_web::start_login("alee", "long enough password").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let reply = hub.login_start("alee", &field(&start, "request"), "pending-1");
    let finished = match sylvie_web::finish_login(handle, &reply, None, Some("web".into())) {
        Ok(value) => value,
        Err(error) => panic!("login failed: {error:?}"),
    };
    let handle: u64 = field(&finished, "handle").parse().unwrap();
    let _ = hub.login_finish("alee", "pending-1", &field(&finished, "message"));

    let opened = sylvie_web::open_login(handle, "sealed-payload");
    assert!(opened.is_err());

    sylvie_web::derive_vault(handle, &wrap)
        .unwrap_or_else(|error| panic!("derive failed: {error:?}"));
    let boxed = sylvie_web::seal_secret(handle, "the github token").unwrap();
    assert_ne!(boxed, "the github token");
    assert_eq!(
        sylvie_web::open_secret(handle, &boxed).unwrap(),
        "the github token"
    );
    let _ = sylvie_web::drop_session(handle);
}

#[test]
fn wrong_password_fails_at_client() {
    let mut hub = Hub::new();

    let start = sylvie_web::start_registration("alee", "long enough password").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let response = hub.register_start(&field(&start, "username"), &field(&start, "request"));
    let finished = sylvie_web::finish_registration(handle, &response).unwrap();
    hub.register_finish(
        "alee",
        &field(&finished, "message"),
        &field(&finished, "wrap"),
    );
    let _ = sylvie_web::drop_session(field(&finished, "handle").parse::<u64>().unwrap());

    let start = sylvie_web::start_login("alee", "wrong password indeed").unwrap();
    let handle: u64 = field(&start, "handle").parse().unwrap();
    let reply = hub.login_start("alee", &field(&start, "request"), "pending-2");

    assert!(sylvie_web::finish_login(handle, &reply, None, None).is_err());
}

#[test]
fn harness_direct_roundtrip() {
    let mut hub = Hub::new();

    let started = ClientRegistration::<Suite>::start(&mut OsRng, b"long enough password").unwrap();
    let response = hub.register_start("alee", &codec::encode(&started.message.serialize()));
    let finished = started
        .state
        .finish(
            &mut OsRng,
            b"long enough password",
            opaque::reg_reply(&codec::decode(&response).unwrap()).unwrap(),
            ClientRegistrationFinishParameters::new(opaque::peer("alee"), None),
        )
        .unwrap();
    hub.register_finish(
        "alee",
        &codec::encode(&finished.message.serialize()),
        &codec::encode(&[0u8; 32]),
    );

    let started = ClientLogin::<Suite>::start(&mut OsRng, b"long enough password").unwrap();
    let reply = hub.login_start("alee", &codec::encode(&started.message.serialize()), "p1");
    let finished = started
        .state
        .finish(
            &mut OsRng,
            b"long enough password",
            opaque::log_reply(&codec::decode(&reply).unwrap()).unwrap(),
            ClientLoginFinishParameters::new(None, opaque::peer("alee"), None),
        )
        .expect("harness direct login must succeed");
    hub.login_finish("alee", "p1", &codec::encode(&finished.message.serialize()));
}
