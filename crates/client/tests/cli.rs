// tests/cli: drives the `sylvie` client in-process against a live sylver instance so the
// client path (URL handling, config, crypto, error display) is covered end to end.

use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

async fn step(
    bin: &str,
    cfg: &std::path::Path,
    label: &str,
    args: &[&str],
    password: &str,
) -> std::process::Output {
    eprintln!("==> {label}");
    tokio::time::timeout(
        Duration::from_secs(20),
        Command::new(bin)
            .env("XDG_CONFIG_HOME", cfg)
            .env("SYLVIE_PASSWORD", password)
            .args(args)
            .output(),
    )
    .await
    .unwrap_or_else(|_| panic!("{label} timed out"))
    .unwrap_or_else(|e| panic!("{label} failed: {e}"))
}

async fn spawn() -> (String, PathBuf) {
    let dir = std::env::temp_dir().join(format!("sylvie-cli-test-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(dir.join("files")).await.unwrap();
    let pool = sylver::db::open(&dir.join("test.db")).await.unwrap();
    let ctx = sylver::ctx::Ctx::build(
        pool,
        dir.join("files"),
        256 * 1024 * 1024,
        sylver::ctx::Limits::default(),
        dir.join("web"),
    )
    .await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        sylver::routes::serve(listener, ctx).await.unwrap();
    });
    (format!("http://{addr}"), dir)
}

#[tokio::test]
async fn cli_register_secrets_rekey() {
    let (url, server_dir) = spawn().await;
    let cfg = std::env::temp_dir().join(format!("sylvie-cli-cfg-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&cfg).unwrap();
    let bin = env!("CARGO_BIN_EXE_sylvie");

    let out = step(
        bin,
        &cfg,
        "register",
        &[
            "register",
            "--url",
            &url,
            "--user",
            "alee",
            "--password",
            "password",
            "--name",
            "cli",
            "--json",
        ],
        "password",
    )
    .await;
    assert!(
        out.status.success(),
        "register failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = step(
        bin,
        &cfg,
        "secret set",
        &["secret", "set", "note", "topsecret", "--json"],
        "password",
    )
    .await;
    assert!(
        out.status.success(),
        "secret set failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = step(
        bin,
        &cfg,
        "secret get",
        &["secret", "get", "note", "--json"],
        "password",
    )
    .await;
    assert!(
        out.status.success(),
        "secret get failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["name"], "note");
    assert_eq!(value["value"], "topsecret");

    let out = step(
        bin,
        &cfg,
        "passwd",
        &["passwd", "--new", "newpassword", "--json"],
        "password",
    )
    .await;
    assert!(
        out.status.success(),
        "passwd failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = step(
        bin,
        &cfg,
        "login old",
        &[
            "login",
            "--url",
            &url,
            "--user",
            "alee",
            "--password",
            "password",
            "--name",
            "cli-old",
            "--json",
        ],
        "password",
    )
    .await;
    assert!(
        !out.status.success(),
        "login with the old password should have failed"
    );

    let out = step(
        bin,
        &cfg,
        "login new",
        &[
            "login",
            "--url",
            &url,
            "--user",
            "alee",
            "--password",
            "newpassword",
            "--name",
            "cli2",
            "--json",
        ],
        "newpassword",
    )
    .await;
    assert!(
        out.status.success(),
        "login with new password failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = step(
        bin,
        &cfg,
        "secret get after rekey",
        &["secret", "get", "note", "--json"],
        "newpassword",
    )
    .await;
    assert!(
        out.status.success(),
        "secret get after rekey failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        value["value"], "topsecret",
        "secret must survive a password change"
    );

    let _ = std::fs::remove_dir_all(&cfg);
    let _ = server_dir;
}
