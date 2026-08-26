use reqwest::Client;

use sylvie_core::error::Error;
use sylvie_core::message::Me;

use crate::ask;
use crate::config::{self, Config};
use crate::net;
use crate::session;

fn host() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|_| "cli".to_string())
}

fn password(given: Option<String>, fresh: bool) -> String {
    match given {
        Some(value) => value,
        None => match std::env::var("SYLVIE_PASSWORD") {
            Ok(value) => value,
            Err(_) => {
                let label = if fresh { "password (new)" } else { "password" };
                if fresh {
                    ask::hidden_twice(label)
                } else {
                    ask::hidden(label)
                }
            }
        },
    }
}

fn stored() -> Result<Config, Error> {
    config::load()?.ok_or_else(|| Error::Internal("not logged in; run sylvie login".into()))
}

fn device_name(given: Option<String>) -> String {
    given.unwrap_or_else(host)
}

pub async fn register(
    http: &Client,
    url: &str,
    user: &str,
    pass: Option<String>,
    name: Option<String>,
    json: bool,
) -> Result<(), Error> {
    let secret = password(pass, true);
    if secret.len() < 8 {
        return Err(Error::Internal("password too short (min 8)".into()));
    }
    let session = session::register(http, url, user, &secret, &device_name(name)).await?;
    config::save(&Config {
        url: url.to_string(),
        user: user.to_string(),
        device: session.device.clone(),
        token: session.token,
    })?;
    report(&session.device, json)
}

pub async fn login(
    http: &Client,
    url: &str,
    user: &str,
    pass: Option<String>,
    name: Option<String>,
    json: bool,
) -> Result<(), Error> {
    let secret = password(pass, false);
    let session = session::login(http, url, user, &secret, None, Some(&device_name(name))).await?;
    config::save(&Config {
        url: url.to_string(),
        user: user.to_string(),
        device: session.device.clone(),
        token: session.token,
    })?;
    report(&session.device, json)
}

pub async fn passwd(http: &Client, fresh: Option<String>, json: bool) -> Result<(), Error> {
    let config = stored()?;
    let old = match std::env::var("SYLVIE_PASSWORD") {
        Ok(value) => value,
        Err(_) => ask::hidden("current password"),
    };
    let secret = match fresh {
        Some(value) => value,
        None => ask::hidden_twice("new password"),
    };
    if secret.len() < 8 {
        return Err(Error::Internal("password too short (min 8)".into()));
    }
    session::rekey(
        http,
        &config.url,
        &config.user,
        &old,
        &secret,
        &config.device,
        &config.token,
    )
    .await?;
    say("password changed", json)?;
    Ok(())
}

pub async fn logout(http: &Client, json: bool) -> Result<(), Error> {
    let config = stored()?;
    net::remove(
        http,
        &config.url,
        &format!("/api/v1/devices/{}", config.device),
        Some(&config.token),
    )
    .await?;
    config::clear()?;
    say("logged out", json)?;
    Ok(())
}

pub async fn status(http: &Client, json: bool) -> Result<(), Error> {
    let config = stored()?;
    let me: Me = net::get(http, &config.url, "/api/v1/me", Some(&config.token)).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&me).map_err(|e| Error::Internal(e.to_string()))?
        );
        return Ok(());
    }
    println!(
        "{} @ {} · device {} ({}) · {} secrets, {} files",
        me.username, config.url, me.device.name, me.device.id, me.secrets, me.files
    );
    Ok(())
}

fn report(device: &str, json: bool) -> Result<(), Error> {
    if json {
        println!(r#"{{"device": "{device}"}}"#);
    } else {
        println!("ok · device {device}");
    }
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
