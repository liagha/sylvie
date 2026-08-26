use clap::Subcommand;
use reqwest::Client;

use sylvie_core::error::Error;
use sylvie_core::message::Device;

use crate::config;
use crate::net;

const BASE: &str = "/api/v1/devices";

#[derive(Subcommand)]
pub enum Command {
    List,
    Revoke { id: String },
}

fn stored() -> Result<(String, String), Error> {
    let saved =
        config::load()?.ok_or_else(|| Error::Internal("not logged in; run sylvie login".into()))?;
    Ok((saved.url, saved.token))
}

pub async fn run(http: &Client, cmd: Command, json: bool) -> Result<(), Error> {
    match cmd {
        Command::List => list(http, json).await,
        Command::Revoke { id } => revoke(http, &id, json).await,
    }
}

async fn list(http: &Client, json: bool) -> Result<(), Error> {
    let (url, token) = stored()?;
    let devices: Vec<Device> = net::get(http, &url, BASE, Some(&token)).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&devices).map_err(|e| Error::Internal(e.to_string()))?
        );
        return Ok(());
    }
    for device in devices {
        match device.revoked {
            Some(_) => println!(
                "{}\t{}\t{} [revoked]",
                device.id, device.created, device.name
            ),
            None => println!("{}\t{}\t{}", device.id, device.created, device.name),
        }
    }
    Ok(())
}

async fn revoke(http: &Client, id: &str, json: bool) -> Result<(), Error> {
    let (url, token) = stored()?;
    net::remove(http, &url, &format!("{BASE}/{id}"), Some(&token)).await?;
    say(&format!("revoked · {id}"), json)?;
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
