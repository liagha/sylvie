use std::io::Write as _;

use clap::Subcommand;
use reqwest::Client;

use sylvie_core::codec;
use sylvie_core::error::Error;
use sylvie_core::message::{SecretItem, SecretPut, SecretValue};
use sylvie_core::vault;

use crate::ask;
use crate::config::{self, Config};
use crate::net;
use crate::session;

const BASE: &str = "/api/v1/secrets";

#[derive(Subcommand)]
pub enum Command {
    List,
    Get { name: String },
    Set { name: String, value: Option<String> },
    Delete { name: String },
}

fn stored() -> Result<Config, Error> {
    config::load()?.ok_or_else(|| Error::Internal("not logged in; run sylvie login".into()))
}

fn password() -> String {
    match std::env::var("SYLVIE_PASSWORD") {
        Ok(value) => value,
        Err(_) => ask::hidden("password"),
    }
}

async fn unlock(http: &Client) -> Result<(Config, Vec<u8>), Error> {
    let stored = stored()?;
    let session = session::login(
        http,
        &stored.url,
        &stored.user,
        &password(),
        Some(&stored.device),
        None,
    )
    .await?;
    let key = vault::root(&session.export, vault::VAULT)?;
    Ok((stored, key))
}

pub async fn run(http: &Client, cmd: Command, json: bool) -> Result<(), Error> {
    match cmd {
        Command::List => list(http, json).await,
        Command::Get { name } => get(http, &name, json).await,
        Command::Set { name, value } => set(http, &name, value, json).await,
        Command::Delete { name } => delete(http, &name, json).await,
    }
}

async fn list(http: &Client, json: bool) -> Result<(), Error> {
    let config = stored()?;
    let items: Vec<SecretItem> = net::get(http, &config.url, BASE, Some(&config.token)).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&items).map_err(|e| Error::Internal(e.to_string()))?
        );
        return Ok(());
    }
    for item in items {
        println!("{}\t{}", item.updated, item.name);
    }
    Ok(())
}

async fn get(http: &Client, name: &str, json: bool) -> Result<(), Error> {
    let (config, key) = unlock(http).await?;
    let value: SecretValue = net::get(
        http,
        &config.url,
        &format!("{BASE}/{name}"),
        Some(&config.token),
    )
    .await?;
    let plain = vault::open(&key, &codec::decode(&value.data)?)?;
    if json {
        println!(
            r#"{{"name": "{name}", "value": {:?}}}"#,
            String::from_utf8_lossy(&plain)
        );
    } else {
        std::io::stdout()
            .write_all(&plain)
            .map_err(|e| Error::Internal(e.to_string()))?;
        println!();
    }
    Ok(())
}

async fn set(http: &Client, name: &str, value: Option<String>, json: bool) -> Result<(), Error> {
    let (config, key) = unlock(http).await?;
    let plain = match value {
        Some(value) => value,
        None => ask::hidden("value"),
    };
    let data = vault::seal(&key, plain.as_bytes())?;
    let _: () = net::put(
        http,
        &config.url,
        &format!("{BASE}/{name}"),
        Some(&config.token),
        &SecretPut {
            data: codec::encode(&data),
        },
    )
    .await?;
    say(&format!("ok · {name}"), json)?;
    Ok(())
}

async fn delete(http: &Client, name: &str, json: bool) -> Result<(), Error> {
    let config = stored()?;
    let _: () = net::remove(
        http,
        &config.url,
        &format!("{BASE}/{name}"),
        Some(&config.token),
    )
    .await?;
    say(&format!("gone · {name}"), json)?;
    Ok(())
}

fn say(message: &str, json: bool) -> Result<(), Error> {
    if json {
        println!(r#"{{"status": "{message}"}}"#);
    } else {
        println!("{message}");
    }
    Ok(())
}
