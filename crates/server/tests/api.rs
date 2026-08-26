use opaque_ke::rand::RngCore;
use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters,
};
use reqwest::{Client, Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use axum::http::header::SET_COOKIE;
use sylver::ctx::Limits;
use sylvie_core::codec;
use sylvie_core::message::{
    BlobReply, Device, FileItem, Grant, LoginFinish, LoginReply, LoginStart, Me, RegisterFinish,
    RegisterStart, RekeyFinish, RekeyStart, SecretItem, SecretPut, SecretValue, WrapValue,
};
use sylvie_core::opaque::{self, Suite};
use sylvie_core::vault;

const SMALL_LIMIT: u64 = 1024;
use std::net::SocketAddr;
use std::time::Duration;

struct Hub {
    base: String,
    dir: std::path::PathBuf,
}

impl Drop for Hub {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn spawn() -> Hub {
    spawn_full(256 * 1024 * 1024, Limits::default()).await
}

async fn spawn_with(max_file: u64) -> Hub {
    spawn_full(max_file, Limits::default()).await
}

async fn spawn_limits(limits: Limits) -> Hub {
    spawn_full(256 * 1024 * 1024, limits).await
}

async fn spawn_full(max_file: u64, limits: Limits) -> Hub {
    let dir = std::env::temp_dir().join(format!("sylvie-test-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(dir.join("files")).await.unwrap();
    let pool = sylver::db::open(&dir.join("test.db")).await.unwrap();
    let ctx = sylver::ctx::Ctx::build(
        pool,
        dir.join("files"),
        max_file,
        limits,
        dir.join("web"),
    )
    .await;
    let app = sylver::routes::build(ctx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    Hub {
        base: format!("http://{addr}"),
        dir,
    }
}

async fn json<B: Serialize, R: DeserializeOwned>(
    hub: &Hub,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: &B,
) -> (StatusCode, Option<R>) {
    let client = Client::new();
    let mut request = client.request(method, format!("{}{}", hub.base, path));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.json(body).send().await.unwrap();
    settle(response).await
}

async fn plain<R: DeserializeOwned>(
    hub: &Hub,
    method: Method,
    path: &str,
    token: Option<&str>,
) -> (StatusCode, Option<R>) {
    let client = Client::new();
    let mut request = client.request(method, format!("{}{}", hub.base, path));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.unwrap();
    settle(response).await
}

async fn settle<R: DeserializeOwned>(response: reqwest::Response) -> (StatusCode, Option<R>) {
    let status = response.status();
    if status != StatusCode::OK && status != StatusCode::CREATED {
        let _ = response.text().await;
        return (status, None);
    }
    (status, Some(response.json::<R>().await.unwrap()))
}

async fn fetch_bytes(hub: &Hub, path: &str, token: &str) -> (StatusCode, Vec<u8>) {
    let client = Client::new();
    let response = client
        .get(format!("{}{}", hub.base, path))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    if !status.is_success() {
        return (status, Vec::new());
    }
    (status, response.bytes().await.unwrap().to_vec())
}

struct Authed {
    token: String,
    device: String,
    export: Vec<u8>,
    vault: Vec<u8>,
}

async fn register(hub: &Hub, user: &str, password: &str, name: &str) -> Authed {
    let started = ClientRegistration::<Suite>::start(&mut OsRng, password.as_bytes()).unwrap();
    let (_, reply): (_, Option<BlobReply>) = json(
        hub,
        Method::POST,
        "/api/v1/auth/register/start",
        None,
        &RegisterStart {
            username: user.to_string(),
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await;
    let finished = started
        .state
        .finish(
            &mut OsRng,
            password.as_bytes(),
            opaque::reg_reply(&codec::decode(&reply.unwrap().message).unwrap()).unwrap(),
            ClientRegistrationFinishParameters::new(opaque::peer(user), None),
        )
        .unwrap();
    let mut vault_secret = [0u8; 32];
    OsRng.fill_bytes(&mut vault_secret);
    let kek = vault::root(finished.export_key.as_slice(), vault::VAULT).unwrap();
    let wrap = codec::encode(&vault::seal(&kek, &vault_secret).unwrap());
    let _ = json::<RegisterFinish, ()>(
        hub,
        Method::POST,
        "/api/v1/auth/register/finish",
        None,
        &RegisterFinish {
            username: user.to_string(),
            message: codec::encode(&finished.message.serialize()),
            wrap,
        },
    )
    .await;
    let mut authed = login(hub, user, password, None, Some(name)).await.unwrap();
    authed.vault = vault_secret.to_vec();
    authed
}

async fn login(
    hub: &Hub,
    user: &str,
    password: &str,
    device: Option<&str>,
    name: Option<&str>,
) -> Result<Authed, ()> {
    let started = ClientLogin::<Suite>::start(&mut OsRng, password.as_bytes()).map_err(|_| ())?;
    let (_, reply): (_, Option<LoginReply>) = json(
        hub,
        Method::POST,
        "/api/v1/auth/login/start",
        None,
        &LoginStart {
            username: user.to_string(),
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await;
    let reply = reply.ok_or(())?;
    let finished = started.state.finish(
        &mut OsRng,
        password.as_bytes(),
        opaque::log_reply(&codec::decode(&reply.message).map_err(|_| ())?).map_err(|_| ())?,
        ClientLoginFinishParameters::new(None, opaque::peer(user), None),
    );
    let finished = finished.map_err(|_| ())?;
    let (_, sealed): (_, Option<sylvie_core::message::Sealed>) = json(
        hub,
        Method::POST,
        "/api/v1/auth/login/finish",
        None,
        &LoginFinish {
            id: reply.id,
            message: codec::encode(&finished.message.serialize()),
            device: device.map(str::to_string),
            name: name.map(str::to_string),
        },
    )
    .await;
    let channel = vault::root(finished.session_key.as_slice(), vault::CHANNEL).map_err(|_| ())?;
    let grant: Vec<u8> = vault::open(
        &channel,
        &codec::decode(&sealed.ok_or(())?.data).map_err(|_| ())?,
    )
    .map_err(|_| ())?;
    let grant: Grant = serde_json::from_slice(&grant).map_err(|_| ())?;
    Ok(Authed {
        token: grant.token,
        device: grant.device,
        export: finished.export_key.as_slice().to_vec(),
        vault: Vec::new(),
    })
}

#[tokio::test]
async fn register_login_me_roundtrip() {
    let hub = spawn().await;
    let authed = register(&hub, "alee", "correct horse", "laptop").await;
    let (_, me): (_, Option<Me>) =
        plain(&hub, Method::GET, "/api/v1/me", Some(&authed.token)).await;
    let me = me.unwrap();
    assert_eq!(me.username, "alee");
    assert_eq!(me.device.name, "laptop");
    assert_eq!((me.secrets, me.files, me.devices), (0, 0, 1));
}

#[tokio::test]
async fn second_account_rejected() {
    let hub = spawn().await;
    register(&hub, "alee", "correct horse", "laptop").await;
    let started = ClientRegistration::<Suite>::start(&mut OsRng, b"other").unwrap();
    let (status, _): (_, Option<BlobReply>) = json(
        &hub,
        Method::POST,
        "/api/v1/auth/register/start",
        None,
        &RegisterStart {
            username: "mallory".into(),
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn wrong_password_and_unknown_user_fail() {
    let hub = spawn().await;
    register(&hub, "alee", "correct horse", "laptop").await;
    let wrong = login(&hub, "alee", "wrong horse", None, None).await;
    assert!(wrong.is_err());

    let ghost = login(&hub, "ghost", "whatever1", None, None).await;
    assert!(ghost.is_err());

    let client = Client::new();
    let response = client
        .post(format!("{}/api/v1/auth/login/finish", hub.base))
        .json(&LoginFinish {
            id: "bogus".into(),
            message: codec::encode(&[]),
            device: None,
            name: Some("x".into()),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unlock_leaves_session_alone() {
    let hub = spawn().await;
    let first = register(&hub, "alee", "correct horse", "laptop").await;
    let again = login(&hub, "alee", "correct horse", Some(&first.device), None)
        .await
        .unwrap();
    assert_eq!(again.device, first.device);
    assert_eq!(again.export, first.export);

    let (status, me): (_, Option<Me>) =
        plain(&hub, Method::GET, "/api/v1/me", Some(&first.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me.unwrap().devices, 1);

    let devices: Option<Vec<Device>> =
        plain(&hub, Method::GET, "/api/v1/devices", Some(&first.token))
            .await
            .1;
    assert_eq!(devices.unwrap().len(), 1);
}

#[tokio::test]
async fn revoked_device_loses_access() {
    let hub = spawn().await;
    let laptop = register(&hub, "alee", "correct horse", "laptop").await;
    let phone = login(&hub, "alee", "correct horse", None, Some("phone"))
        .await
        .unwrap();

    let (status, _): (_, Option<()>) = plain(
        &hub,
        Method::DELETE,
        &format!("/api/v1/devices/{}", phone.device),
        Some(&laptop.token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = plain::<Me>(&hub, Method::GET, "/api/v1/me", Some(&phone.token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, devices): (_, Option<Vec<Device>>) =
        plain(&hub, Method::GET, "/api/v1/devices", Some(&laptop.token)).await;
    let phone_row = devices
        .unwrap()
        .into_iter()
        .find(|d| d.id == phone.device)
        .unwrap();
    assert!(phone_row.revoked.is_some());
}

#[tokio::test]
async fn secrets_roundtrip_and_authorization() {
    let hub = spawn().await;
    let authed = register(&hub, "alee", "correct horse", "laptop").await;
    let key = vault::root(&authed.vault, vault::DATA).unwrap();

    let data = vault::seal(&key, b"github token value").unwrap();
    let (status, _): (_, Option<()>) = json(
        &hub,
        Method::PUT,
        "/api/v1/secrets/github",
        Some(&authed.token),
        &SecretPut {
            data: codec::encode(&data),
        },
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, value): (_, Option<SecretValue>) = plain(
        &hub,
        Method::GET,
        "/api/v1/secrets/github",
        Some(&authed.token),
    )
    .await;
    let opened = vault::open(&key, &codec::decode(&value.unwrap().data).unwrap()).unwrap();
    assert_eq!(opened, b"github token value");

    let (_, items): (_, Option<Vec<SecretItem>>) =
        plain(&hub, Method::GET, "/api/v1/secrets", Some(&authed.token)).await;
    assert_eq!(items.unwrap()[0].name, "github");

    let (status, _) = plain::<SecretValue>(&hub, Method::GET, "/api/v1/secrets/github", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _): (_, Option<()>) = plain(
        &hub,
        Method::DELETE,
        "/api/v1/secrets/github",
        Some(&authed.token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = plain::<SecretValue>(
        &hub,
        Method::GET,
        "/api/v1/secrets/github",
        Some(&authed.token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tampered_ciphertext_detected() {
    let hub = spawn().await;
    let attacker = register(&hub, "alee", "correct horse", "laptop").await;
    let key = vault::root(&attacker.vault, vault::DATA).unwrap();

    let mut data = vault::seal(&key, b"private").unwrap();
    let last = data.len() - 1;
    data[last] ^= 1;
    let _ = json::<SecretPut, ()>(
        &hub,
        Method::PUT,
        "/api/v1/secrets/note",
        Some(&attacker.token),
        &SecretPut {
            data: codec::encode(&data),
        },
    )
    .await;

    let (_, value): (_, Option<SecretValue>) = plain(
        &hub,
        Method::GET,
        "/api/v1/secrets/note",
        Some(&attacker.token),
    )
    .await;
    let stored = codec::decode(&value.unwrap().data).unwrap();
    assert!(vault::open(&key, &stored).is_err());
}

#[tokio::test]
async fn files_roundtrip_integrity_and_limit() {
    let hub = spawn_with(SMALL_LIMIT).await;
    let authed = register(&hub, "alee", "correct horse", "laptop").await;

    let payload = b"sylvie archive contents".to_vec();
    let (status, item): (_, Option<FileItem>) =
        upload(&hub, &authed.token, "notes.txt", &payload).await;
    assert_eq!(status, StatusCode::CREATED);
    let item = item.unwrap();
    assert_eq!(item.name, "notes.txt");
    assert_eq!(item.size as usize, payload.len());
    assert_eq!(item.hash, codec::digest(&payload));

    let (status, downloaded) = fetch_bytes(
        &hub,
        &format!("/api/v1/files/{}/content", item.id),
        &authed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(downloaded, payload);

    let (status, _): (_, Option<()>) = plain(
        &hub,
        Method::DELETE,
        &format!("/api/v1/files/{}", item.id),
        Some(&authed.token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = plain::<FileItem>(
        &hub,
        Method::GET,
        &format!("/api/v1/files/{}", item.id),
        Some(&authed.token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let big = vec![0u8; (SMALL_LIMIT + 1) as usize];
    let (status, _) = upload(&hub, &authed.token, "big.bin", &big).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

async fn upload(hub: &Hub, token: &str, name: &str, body: &[u8]) -> (StatusCode, Option<FileItem>) {
    let client = Client::new();
    let response = client
        .post(format!(
            "{}/api/v1/files?name={}",
            hub.base,
            net_query(name)
        ))
        .bearer_auth(token)
        .body(body.to_vec())
        .send()
        .await
        .unwrap();
    settle(response).await
}

fn net_query(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[tokio::test]
async fn login_flood_gated() {
    let hub = spawn_limits(Limits {
        attempts: 2,
        window: Duration::from_secs(60),
        session_ttl: None,
    })
    .await;
    for _ in 0..2 {
        let (status, _): (_, Option<LoginReply>) = json(
            &hub,
            Method::POST,
            "/api/v1/auth/login/start",
            None,
            &LoginStart {
                username: "ghost".into(),
                message: "broken".into(),
            },
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _): (_, Option<LoginReply>) = json(
        &hub,
        Method::POST,
        "/api/v1/auth/login/start",
        None,
        &LoginStart {
            username: "ghost".into(),
            message: "broken".into(),
        },
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    let (status, _): (_, Option<LoginReply>) = json(
        &hub,
        Method::POST,
        "/api/v1/auth/login/start",
        None,
        &LoginStart {
            username: "other".into(),
            message: "broken".into(),
        },
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn expired_session_rejected() {
    let hub = spawn_limits(Limits {
        attempts: 10,
        window: Duration::from_secs(60),
        session_ttl: Some(Duration::ZERO),
    })
    .await;
    let authed = register(&hub, "alee", "correct horse", "laptop").await;
    let (status, _) = plain::<Me>(&hub, Method::GET, "/api/v1/me", Some(&authed.token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rekey_requires_session() {
    let hub = spawn().await;
    register(&hub, "alee", "correct horse", "laptop").await;
    let started = ClientRegistration::<Suite>::start(&mut OsRng, b"whatever123").unwrap();
    let (status, _): (_, Option<BlobReply>) = json(
        &hub,
        Method::POST,
        "/api/v1/auth/rekey/start",
        None,
        &RekeyStart {
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rekey_rotates_kek_keeps_data_and_sessions() {
    let hub = spawn().await;
    let authed = register(&hub, "alee", "old password 1", "laptop").await;
    let data_key = vault::root(&authed.vault, vault::DATA).unwrap();
    let data = vault::seal(&data_key, b"precious").unwrap();
    let _ = json::<SecretPut, ()>(
        &hub,
        Method::PUT,
        "/api/v1/secrets/note",
        Some(&authed.token),
        &SecretPut {
            data: codec::encode(&data),
        },
    )
    .await;

    let unlock = login(&hub, "alee", "old password 1", Some(&authed.device), None)
        .await
        .unwrap();
    let (_, wrapped): (_, Option<WrapValue>) =
        plain(&hub, Method::GET, "/api/v1/vault", Some(&authed.token)).await;
    let kek_old = vault::root(&unlock.export, vault::VAULT).unwrap();
    let vault_secret =
        vault::open(&kek_old, &codec::decode(&wrapped.unwrap().data).unwrap()).unwrap();
    assert_eq!(vault_secret, authed.vault);

    let fresh = "new password 2";
    let started = ClientRegistration::<Suite>::start(&mut OsRng, fresh.as_bytes()).unwrap();
    let (_, reply): (_, Option<BlobReply>) = json(
        &hub,
        Method::POST,
        "/api/v1/auth/rekey/start",
        Some(&authed.token),
        &RekeyStart {
            message: codec::encode(&started.message.serialize()),
        },
    )
    .await;
    let finished = started
        .state
        .finish(
            &mut OsRng,
            fresh.as_bytes(),
            opaque::reg_reply(&codec::decode(&reply.unwrap().message).unwrap()).unwrap(),
            ClientRegistrationFinishParameters::new(opaque::peer("alee"), None),
        )
        .unwrap();
    let kek_new = vault::root(finished.export_key.as_slice(), vault::VAULT).unwrap();
    let wrap_new = vault::seal(&kek_new, &vault_secret).unwrap();
    let (status, _): (_, Option<()>) = json(
        &hub,
        Method::POST,
        "/api/v1/auth/rekey/finish",
        Some(&authed.token),
        &RekeyFinish {
            message: codec::encode(&finished.message.serialize()),
            wrap: codec::encode(&wrap_new),
        },
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert!(
        login(&hub, "alee", "old password 1", None, None)
            .await
            .is_err()
    );
    let renewed = login(&hub, "alee", fresh, Some(&authed.device), None)
        .await
        .unwrap();
    assert_eq!(renewed.export, finished.export_key.as_slice());

    let (_, wrapped): (_, Option<WrapValue>) =
        plain(&hub, Method::GET, "/api/v1/vault", Some(&authed.token)).await;
    let unwrapped = vault::open(&kek_new, &codec::decode(&wrapped.unwrap().data).unwrap()).unwrap();
    assert_eq!(unwrapped, vault_secret);

    let (_, value): (_, Option<SecretValue>) = plain(
        &hub,
        Method::GET,
        "/api/v1/secrets/note",
        Some(&authed.token),
    )
    .await;
    let opened = vault::open(&data_key, &codec::decode(&value.unwrap().data).unwrap()).unwrap();
    assert_eq!(opened, b"precious");

    let (status, _) = plain::<Me>(&hub, Method::GET, "/api/v1/me", Some(&authed.token)).await;
    assert_eq!(status, StatusCode::OK);
}

fn web_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

#[tokio::test]
async fn dashboard_gates_on_cookie() {
    let hub = spawn().await;
    let authed = register(&hub, "alee", "correct horse", "laptop").await;

    let client = web_client();
    let home = client.get(format!("{}/", hub.base)).send().await.unwrap();
    assert_eq!(home.status(), StatusCode::SEE_OTHER);
    assert_eq!(home.headers().get("location").unwrap(), "/login");

    let page = client
        .get(format!("{}/login", hub.base))
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    assert!(page.text().await.unwrap().contains("device token"));

    let bad = client
        .post(format!("{}/login", hub.base))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("token=wrong-token")
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::SEE_OTHER);
    assert_eq!(bad.headers().get("location").unwrap(), "/login");
    assert!(!bad.headers().contains_key(SET_COOKIE));

    let good = client
        .post(format!("{}/login", hub.base))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("token={}", authed.token))
        .send()
        .await
        .unwrap();
    assert_eq!(good.status(), StatusCode::SEE_OTHER);
    let cookie = good.headers().get(SET_COOKIE).unwrap().to_str().unwrap();
    assert!(cookie.starts_with("sylvie_token="));

    let page = client
        .get(format!("{}/", hub.base))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    let body = page.text().await.unwrap();
    assert!(body.contains("laptop"));
    assert!(body.contains("no secrets yet"));
}

#[tokio::test]
async fn api_accepts_cookie() {
    let hub = spawn().await;
    let authed = register(&hub, "alee", "correct horse", "laptop").await;

    let client = web_client();
    let reply = client
        .get(format!("{}/api/v1/me", hub.base))
        .header("cookie", format!("sylvie_token={}", authed.token))
        .send()
        .await
        .unwrap();
    assert_eq!(reply.status(), StatusCode::OK);
    let me: Me = reply.json().await.unwrap();
    assert_eq!(me.username, "alee");
    assert_eq!(me.device.name, "laptop");
}

#[tokio::test]
async fn dashboard_revoke_works() {
    let hub = spawn().await;
    let laptop = register(&hub, "alee", "correct horse", "laptop").await;
    let phone = login(&hub, "alee", "correct horse", None, Some("phone"))
        .await
        .unwrap();

    let client = web_client();
    let unlocked = client
        .post(format!("{}/login", hub.base))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("token={}", phone.token))
        .send()
        .await
        .unwrap();
    let cookie = unlocked
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();

    let gone = client
        .post(format!("{}/web/device/{}", hub.base, laptop.device))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), StatusCode::SEE_OTHER);

    let (status, _) = plain::<Me>(&hub, Method::GET, "/api/v1/me", Some(&laptop.token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, devices): (_, Option<Vec<Device>>) =
        plain(&hub, Method::GET, "/api/v1/devices", Some(&phone.token)).await;
    let row = devices
        .unwrap()
        .into_iter()
        .find(|d| d.id == laptop.device)
        .unwrap();
    assert!(row.revoked.is_some());
}

#[tokio::test]
async fn web_register_then_enroll_matches_cli() {
    let hub = spawn().await;
    let user = "alee";
    let password = "correct horse";
    let name = "web";
    let client = reqwest::Client::new();

    let start = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::start_registration(user, password).unwrap(),
    )
    .unwrap();
    let reply: reqwest::Response = client
        .post(format!("{}/api/v1/auth/register/start", hub.base))
        .json(&serde_json::json!({"username": user, "message": start["request"]}))
        .send()
        .await
        .unwrap();
    let reply: serde_json::Value = reply.json().await.unwrap();
    eprintln!("register/start reply: {reply}");
    let finished = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::finish_registration(
            start["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            reply["message"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let status = client
        .post(format!("{}/api/v1/auth/register/finish", hub.base))
        .json(&serde_json::json!({
            "username": user,
            "message": finished["message"],
            "wrap": finished["wrap"],
        }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let start = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::start_login(user, password).unwrap(),
    )
    .unwrap();
    let reply: serde_json::Value = client
        .post(format!("{}/api/v1/auth/login/start", hub.base))
        .json(&serde_json::json!({"username": user, "message": start["request"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let finished = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::finish_login(
            start["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            reply["message"].as_str().unwrap(),
            None::<String>,
            Some(name.to_string()),
        )
        .unwrap(),
    )
    .unwrap();
    let sealed: serde_json::Value = client
        .post(format!("{}/api/v1/auth/login/finish", hub.base))
        .json(&serde_json::json!({
            "id": reply["id"],
            "message": finished["message"],
            "name": name,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grant = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::open_login(
            finished["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            sealed["data"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!grant["token"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn web_secrets_and_rekey_flow() {
    let hub = spawn().await;
    let user = "alee";
    let password = "correct horse";
    let name = "web";
    let client = reqwest::Client::new();

    let start = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::start_registration(user, password).unwrap(),
    )
    .unwrap();
    let reply: serde_json::Value = client
        .post(format!("{}/api/v1/auth/register/start", hub.base))
        .json(&serde_json::json!({"username": user, "message": start["request"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let finished = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::finish_registration(
            start["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            reply["message"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    client
        .post(format!("{}/api/v1/auth/register/finish", hub.base))
        .json(&serde_json::json!({
            "username": user,
            "message": finished["message"],
            "wrap": finished["wrap"],
        }))
        .send()
        .await
        .unwrap();

    let start = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::start_login(user, password).unwrap(),
    )
    .unwrap();
    let reply: serde_json::Value = client
        .post(format!("{}/api/v1/auth/login/start", hub.base))
        .json(&serde_json::json!({"username": user, "message": start["request"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fin = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::finish_login(
            start["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            reply["message"].as_str().unwrap(),
            None::<String>,
            Some(name.to_string()),
        )
        .unwrap(),
    )
    .unwrap();
    let sealed: serde_json::Value = client
        .post(format!("{}/api/v1/auth/login/finish", hub.base))
        .json(&serde_json::json!({
            "id": reply["id"],
            "message": fin["message"],
            "name": name,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grant = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::open_login(
            fin["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            sealed["data"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let token = grant["token"].as_str().unwrap().to_string();
    assert!(!token.is_empty());
    let auth = format!("sylvie_token={token}");

    let wrap: serde_json::Value = client
        .get(format!("{}/api/v1/vault", hub.base))
        .header("cookie", &auth)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    sylvie_web::derive_vault(
        fin["handle"].as_str().unwrap().parse::<u64>().unwrap(),
        wrap["data"].as_str().unwrap(),
    )
    .unwrap();

    let handle = fin["handle"].as_str().unwrap().parse::<u64>().unwrap();
    let boxed = sylvie_web::seal_secret(handle, "top secret value").unwrap();
    let status = client
        .put(format!("{}/api/v1/secrets/vault_pw", hub.base))
        .header("cookie", &auth)
        .json(&serde_json::json!({"data": boxed}))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let got: serde_json::Value = client
        .get(format!("{}/api/v1/secrets/vault_pw", hub.base))
        .header("cookie", &auth)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let plain = sylvie_web::open_secret(handle, got["data"].as_str().unwrap()).unwrap();
    assert_eq!(plain, "top secret value");

    let new_password = "new correct horse";
    let rstart = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::rekey_start(handle, new_password).unwrap(),
    )
    .unwrap();
    let rreply: serde_json::Value = client
        .post(format!("{}/api/v1/auth/rekey/start", hub.base))
        .header("cookie", &auth)
        .json(&serde_json::json!({"message": rstart["request"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rfin = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::rekey_finish(handle, rreply["message"].as_str().unwrap(), new_password).unwrap(),
    )
    .unwrap();
    let status = client
        .post(format!("{}/api/v1/auth/rekey/finish", hub.base))
        .header("cookie", &auth)
        .json(&serde_json::json!({
            "message": rfin["message"],
            "wrap": rfin["wrap"],
        }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let start = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::start_login(user, new_password).unwrap(),
    )
    .unwrap();
    let reply: serde_json::Value = client
        .post(format!("{}/api/v1/auth/login/start", hub.base))
        .json(&serde_json::json!({"username": user, "message": start["request"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fin = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::finish_login(
            start["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            reply["message"].as_str().unwrap(),
            None::<String>,
            Some("again".to_string()),
        )
        .unwrap(),
    )
    .unwrap();
    let sealed: serde_json::Value = client
        .post(format!("{}/api/v1/auth/login/finish", hub.base))
        .json(&serde_json::json!({
            "id": reply["id"],
            "message": fin["message"],
            "name": "again",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grant = serde_json::from_str::<serde_json::Value>(
        &sylvie_web::open_login(
            fin["handle"].as_str().unwrap().parse::<u64>().unwrap(),
            sealed["data"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!grant["token"].as_str().unwrap().is_empty());
}
