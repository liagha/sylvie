// tests/common: shared harness for the server integration tests. Spins up a
// real sylver instance backed by a throwaway directory so each test talks to a
// live API over HTTP.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::PathBuf;

use sylver::ctx::Limits;
use uuid::Uuid;

pub struct Hub {
    pub base: String,
    dir: PathBuf,
}

impl Drop for Hub {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub async fn spawn() -> Hub {
    spawn_full(256 * 1024 * 1024, Limits::default()).await
}

pub async fn spawn_with(max_file: u64) -> Hub {
    spawn_full(max_file, Limits::default()).await
}

pub async fn spawn_limits(limits: Limits) -> Hub {
    spawn_full(256 * 1024 * 1024, limits).await
}

pub async fn spawn_full(max_file: u64, limits: Limits) -> Hub {
    let dir = std::env::temp_dir().join(format!("sylvie-test-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(dir.join("files")).await.unwrap();
    let pool = sylver::db::open(&dir.join("test.db")).await.unwrap();
    let ctx =
        sylver::ctx::Ctx::build(pool, dir.join("files"), max_file, limits, dir.join("web")).await;
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
