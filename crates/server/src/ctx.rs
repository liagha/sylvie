use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use opaque_ke::rand::rngs::OsRng;
use sqlx::SqlitePool;

use sylvie_core::error::Error;
use sylvie_core::opaque::{LogState, Setup};

const PENDING_TTL: u64 = 300;

pub struct Pending {
    pub login: LogState,
    pub user: String,
    pub born: Instant,
}

pub(crate) struct Inner {
    db: SqlitePool,
    setup: Setup,
    storage: PathBuf,
    max_file: u64,
    pendings: Mutex<HashMap<String, Pending>>,
}

#[derive(Clone)]
pub struct Ctx(Arc<Inner>);

impl Ctx {
    pub async fn build(db: SqlitePool, storage: PathBuf, max_file: u64) -> Self {
        let setup = match load_setup(&db).await {
            Ok(setup) => setup,
            Err(error) => panic!("server setup: {error}"),
        };
        Self(Arc::new(Inner {
            db,
            setup,
            storage,
            max_file,
            pendings: Mutex::new(HashMap::new()),
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
