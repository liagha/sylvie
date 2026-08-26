use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use opaque_ke::rand::rngs::OsRng;
use sqlx::SqlitePool;

use sylvie_core::error::Error;
use sylvie_core::opaque::{LogState, Setup};

const PENDING_TTL: u64 = 300;

#[derive(Clone, Copy)]
pub struct Limits {
    pub attempts: u32,
    pub window: Duration,
    pub session_ttl: Option<Duration>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            attempts: u32::MAX,
            window: Duration::from_secs(300),
            session_ttl: None,
        }
    }
}

pub struct Pending {
    pub login: LogState,
    pub user: String,
    pub born: Instant,
}

struct Strike {
    count: u32,
    born: Instant,
}

struct Inner {
    db: SqlitePool,
    setup: Setup,
    storage: PathBuf,
    max_file: u64,
    limits: Limits,
    web: pardeh::App,
    web_dir: PathBuf,
    pendings: Mutex<HashMap<String, Pending>>,
    floods: Mutex<HashMap<String, Strike>>,
}

#[derive(Clone)]
pub struct Ctx(Arc<Inner>);

impl Ctx {
    pub async fn build(
        db: SqlitePool,
        storage: PathBuf,
        max_file: u64,
        limits: Limits,
        web_dir: PathBuf,
    ) -> Self {
        let setup = match load_setup(&db).await {
            Ok(setup) => setup,
            Err(error) => panic!("server setup: {error}"),
        };
        let web = pardeh::App::new();
        crate::routes::web::seed(&web);
        Self(Arc::new(Inner {
            db,
            setup,
            storage,
            max_file,
            limits,
            web,
            web_dir,
            pendings: Mutex::new(HashMap::new()),
            floods: Mutex::new(HashMap::new()),
        }))
    }

    pub fn db(&self) -> &SqlitePool {
        &self.0.db
    }

    pub fn setup(&self) -> &Setup {
        &self.0.setup
    }

    pub fn max_file(&self) -> u64 {
        self.0.max_file
    }

    pub fn web(&self) -> &pardeh::App {
        &self.0.web
    }

    pub fn web_dir(&self) -> &PathBuf {
        &self.0.web_dir
    }

    pub fn limits(&self) -> Limits {
        self.0.limits
    }

    pub fn admit(&self, key: &str) -> bool {
        let mut floods = self.0.floods.lock().expect("flood lock");
        let now = Instant::now();
        let strike = floods.entry(key.to_string()).or_insert(Strike {
            count: 0,
            born: now,
        });
        if now.duration_since(strike.born) >= self.0.limits.window {
            strike.count = 0;
            strike.born = now;
        }
        strike.count += 1;
        strike.count <= self.0.limits.attempts
    }

    pub fn pending_insert(&self, id: String, pending: Pending) {
        let mut map = self.0.pendings.lock().expect("pending lock");
        map.retain(|_, p| p.born.elapsed().as_secs() < PENDING_TTL);
        map.insert(id, pending);
    }

    pub fn pending_take(&self, id: &str) -> Option<Pending> {
        let mut map = self.0.pendings.lock().expect("pending lock");
        let pending = map.remove(id)?;
        (pending.born.elapsed().as_secs() < PENDING_TTL).then_some(pending)
    }

    pub fn object_path(&self, id: &str) -> PathBuf {
        self.0.storage.join(id)
    }
}

async fn load_setup(db: &SqlitePool) -> Result<Setup, Error> {
    let row: Option<(Vec<u8>,)> = sqlx::query_as("select value from system where key = 'setup'")
        .fetch_optional(db)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    if let Some((bytes,)) = row {
        return Setup::deserialize(&bytes)
            .map_err(|_| Error::Internal("server setup unreadable".into()));
    }
    let setup = Setup::new(&mut OsRng);
    sqlx::query("insert into system(key, value) values ('setup', ?)")
        .bind(setup.serialize().to_vec())
        .execute(db)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    Ok(setup)
}

pub type Peer = SocketAddr;
