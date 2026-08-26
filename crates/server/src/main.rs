use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let cfg = sylvie_server::config::Config::load();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&cfg.level).unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let pool = sylvie_server::db::open(&cfg.database)
        .await
        .expect("database open");
    tokio::fs::create_dir_all(&cfg.storage)
        .await
        .expect("storage dir");

    let app = sylvie_server::routes::build(
        sylvie_server::ctx::Ctx::build(pool, cfg.storage.clone(), cfg.max_file).await,
    );
    let listener = tokio::net::TcpListener::bind(&cfg.bind)
        .await
        .expect("bind");
    tracing::info!(bind = %cfg.bind, database = %cfg.database.display(), "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(stop())
        .await
        .expect("serve");
}

async fn stop() {
    let _ = tokio::signal::ctrl_c().await;
}
