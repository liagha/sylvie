use std::net::SocketAddr;

use sylver::ctx::Limits;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let cfg = sylver::config::Config::load();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&cfg.level).unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let pool = sylver::db::open(&cfg.database)
        .await
        .expect("database open");
    tokio::fs::create_dir_all(&cfg.storage)
        .await
        .expect("storage dir");

    let limits = Limits {
        attempts: cfg.attempts,
        window: cfg.window,
        session_ttl: cfg.session_ttl,
    };
    let ctx = sylver::ctx::Ctx::build(
        pool,
        cfg.storage.clone(),
        cfg.max_file,
        limits,
        cfg.web_dir.clone(),
    )
    .await;
    let app = sylver::routes::build(ctx);
    let listener = tokio::net::TcpListener::bind(&cfg.bind)
        .await
        .expect("bind");
    tracing::info!(bind = %cfg.bind, database = %cfg.database.display(), "listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(stop())
    .await
    .expect("serve");
}

async fn stop() {
    let _ = tokio::signal::ctrl_c().await;
}
