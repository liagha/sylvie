use std::path::PathBuf;

use clap::Subcommand;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::io::AsyncReadExt;

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

fn bar(total: u64, label: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(&format!(
            " {label} [{{bar:30}}] {{bytes}}/{{total_bytes}} ({{elapsed}})"
        ))
        .unwrap()
        .progress_chars("=>-"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
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
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or(Error::Request)?;
    let total = std::fs::metadata(path)
        .map_err(|e| Error::Internal(e.to_string()))?
        .len();
    let pb = bar(total, "uploading");
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let mut body = Vec::with_capacity(total as usize);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        pb.inc(n as u64);
    }
    pb.finish_with_message("done");
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
        "stored · {} · {} · {}",
        item.name,
        HumanBytes(item.size),
        item.hash
    );
    Ok(())
}

async fn download(http: &Client, id: &str, out: Option<PathBuf>, json: bool) -> Result<(), Error> {
    let (url, token) = stored()?;
    let item: FileItem = net::get(http, &url, &format!("{BASE}/{id}"), Some(&token)).await?;
    let target: PathBuf = out.unwrap_or_else(|| PathBuf::from(&item.name));
    let request = net::build_get(http, &url, &format!("{BASE}/{id}/content"), Some(&token))?;
    let mut response = request.send().await.map_err(net::transport_err)?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(net::remote_err(status, &text));
    }
    let total = response.content_length().unwrap_or(item.size);
    let pb = bar(total, "downloading");
    let mut data = Vec::with_capacity(total as usize);
    while let Some(chunk) = response.chunk().await.map_err(net::transport_err)? {
        data.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }
    pb.finish_with_message("done");
    let digest = codec::digest(&data);
    if digest != item.hash {
        return Err(Error::Crypto);
    }
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
    println!(
        "written · {} · {} · {}",
        target.display(),
        HumanBytes(data.len() as u64),
        digest
    );
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
            item.id,
            item.updated,
            HumanBytes(item.size),
            item.name
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
