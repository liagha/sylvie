use std::env;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_MAX_FILE: u64 = 256 * 1024 * 1024;
const DEFAULT_ATTEMPTS: u32 = 10;
const DEFAULT_WINDOW: u64 = 300;
const SECONDS_IN_DAY: u64 = 86_400;

pub struct Config {
    pub bind: String,
    pub database: PathBuf,
    pub storage: PathBuf,
    pub level: String,
    pub max_file: u64,
    pub attempts: u32,
    pub window: Duration,
    pub session_ttl: Option<Duration>,
}

impl Config {
    pub fn load() -> Self {
        let home = data_home().join("sylvie");
        let days: u64 = number("SYLVIE_SESSION_TTL_DAYS", 0);
        Self {
            bind: text("SYLVIE_BIND_ADDR", "127.0.0.1:7400"),
            database: path("SYLVIE_DB_PATH", home.join("sylvie.db")),
            storage: path("SYLVIE_STORAGE_PATH", home.join("files")),
            level: text("SYLVIE_LOG_LEVEL", "info"),
            max_file: number("SYLVIE_MAX_FILE_SIZE", DEFAULT_MAX_FILE),
            attempts: number("SYLVIE_AUTH_ATTEMPTS", DEFAULT_ATTEMPTS),
            window: Duration::from_secs(number("SYLVIE_AUTH_WINDOW_SECS", DEFAULT_WINDOW)),
            session_ttl: (days > 0).then(|| Duration::from_secs(days * SECONDS_IN_DAY)),
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

fn number<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
