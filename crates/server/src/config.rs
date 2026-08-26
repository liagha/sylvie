use std::env;
use std::path::PathBuf;

const DEFAULT_MAX_FILE: u64 = 256 * 1024 * 1024;

pub struct Config {
    pub bind: String,
    pub database: PathBuf,
    pub storage: PathBuf,
    pub level: String,
    pub max_file: u64,
}

impl Config {
    pub fn load() -> Self {
        let home = data_home().join("sylvie");
        Self {
            bind: text("SYLVIE_BIND_ADDR", "127.0.0.1:7400"),
            database: path("SYLVIE_DB_PATH", home.join("sylvie.db")),
            storage: path("SYLVIE_STORAGE_PATH", home.join("files")),
            level: text("SYLVIE_LOG_LEVEL", "info"),
            max_file: env::var("SYLVIE_MAX_FILE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_FILE),
        }
    }
}

fn data_home() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn text(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn path(key: &str, default: PathBuf) -> PathBuf {
    env::var(key).map(PathBuf::from).unwrap_or(default)
}
