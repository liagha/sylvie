use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use sylvie_core::error::Error;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    pub user: String,
    pub device: String,
    pub token: String,
}

fn path() -> Result<PathBuf, Error> {
    dirs::config_dir()
        .map(|dir| dir.join("sylvie").join("config.toml"))
        .ok_or_else(|| Error::Internal("home directory unknown".into()))
}

pub fn load() -> Result<Option<Config>, Error> {
    let file = path()?;
    match fs::read_to_string(file) {
        Ok(text) => toml::from_str(&text)
            .map(Some)
            .map_err(|e| Error::Internal(format!("config unreadable: {e}"))),
        Err(_) => Ok(None),
    }
}

pub fn save(config: &Config) -> Result<(), Error> {
    let file = path()?;
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::Internal(e.to_string()))?;
    }
    fs::write(
        &file,
        toml::to_string(config).map_err(|e| Error::Internal(e.to_string()))?,
    )
    .map_err(|e| Error::Internal(e.to_string()))?;
    restrict(&file)?;
    Ok(())
}

pub fn clear() -> Result<(), Error> {
    match fs::remove_file(path()?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Internal(error.to_string())),
    }
}

#[cfg(unix)]
fn restrict(file: &PathBuf) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::Permissions::from_mode(0o600);
    fs::set_permissions(file, mode).map_err(|e| Error::Internal(e.to_string()))
}

#[cfg(not(unix))]
fn restrict(_file: &PathBuf) -> Result<(), Error> {
    Ok(())
}
