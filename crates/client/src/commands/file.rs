use std::path::PathBuf;

use clap::Subcommand;
use reqwest::Client;

use sylvie_core::codec;
use sylvie_core::error::Error;
use sylvie_core::message::FileItem;

use crate::config;
use crate::net;

const BASE: &str = "/api/v1/files";

#[derive(Subcommand)]
pub enum Command {
    Upload { path: PathBuf },
    Download { id: String, out: Option<PathBuf> },
    List,
    Delete { id: String },
}

fn stored() -> Result<(String, String), Error> {
    let saved =
        config::load()?.ok_or_else(|| Error::Internal("not logged in; run sylvie login".into()))?;
    Ok((saved.url, saved.token))
}

pub async fn run(http: &Client, cmd: Command, json: bool) -> Result<(), Error> {
    match cmd {
        Command::Upload { path } => upload(http, &path, json).await,
        Command::Download { id, out } => download(http, &id, out, json).await,
        Command::List => list(http, json).await,
        Command::Delete { id } => delete(http, &id, json).await,
    }
}

async fn upload(http: &Client, path: &std::path::Path, json: bool) -> Result<(), Error> {
    let (url, token) = stored()?;
    let body = std::fs::read(path).map_err(|e| Error::Internal(e.to_string()))?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or(Error::Request)?;
    let item: FileItem = net::post_raw(
        http,
        &url,
        &format!("{BASE}?name={}", net::query_value(&name)),
        Some(&token),
        body,
    )
    .await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&item).map_err(|e| Error::Internal(e.to_string()))?
        );
        return Ok(());
    }
    println!(
        "stored · {} · {} bytes · {}",
        item.name, item.size, item.hash
    );
    Ok(())
}

async fn download(http: &Client, id: &str, out: Option<PathBuf>, json: bool) -> Result<(), Error> {
    let (url, token) = stored()?;
    let item: FileItem = net::get(http, &url, &format!("{BASE}/{id}"), Some(&token)).await?;
    let data = net::bytes(http, &url, &format!("{BASE}/{id}/content"), Some(&token)).await?;
    let digest = codec::digest(&data);
    if digest != item.hash {
        return Err(Error::Crypto);
    }
    let target: PathBuf = out.unwrap_or_else(|| PathBuf::from(&item.name));
    std::fs::write(&target, &data).map_err(|e| Error::Internal(e.to_string()))?;
    if json {
        println!(
            r#"{{"path": {:?}, "size": {}, "hash": "{}"}}"#,
            target.display().to_string(),
            data.len(),
            digest
        );
        return Ok(());
    }
    println!("written · {} · {} bytes", target.display(), data.len());
    Ok(())
}

async fn list(http: &Client, json: bool) -> Result<(), Error> {
    let (url, token) = stored()?;
    let items: Vec<FileItem> = net::get(http, &url, BASE, Some(&token)).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&items).map_err(|e| Error::Internal(e.to_string()))?
        );
        return Ok(());
    }
    for item in items {
        println!(
            "{}\t{}\t{}\t{}",
            item.id, item.updated, item.size, item.name
        );
    }
    Ok(())
}

async fn delete(http: &Client, id: &str, json: bool) -> Result<(), Error> {
    let (url, token) = stored()?;
    net::remove(http, &url, &format!("{BASE}/{id}"), Some(&token)).await?;
    say(&format!("gone · {id}"), json)?;
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
