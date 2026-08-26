use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use sylvie_core::error::Error;

#[derive(Deserialize)]
struct Fault {
    error: String,
}

pub fn http() -> Result<Client, Error> {
    Client::builder()
        .build()
        .map_err(|e| Error::Internal(e.to_string()))
}

fn transport(error: reqwest::Error) -> Error {
    Error::Internal(format!("connection failed: {error}"))
}

async fn deliver<R: DeserializeOwned>(response: Response) -> Result<R, Error> {
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(remote(status, &text));
    }
    if status == StatusCode::NO_CONTENT {
        return serde_json::from_str("null").map_err(|e| Error::Internal(e.to_string()));
    }
    response
        .json::<R>()
        .await
        .map_err(|e| Error::Internal(format!("malformed reply: {e}")))
}

fn remote(status: StatusCode, text: &str) -> Error {
    match serde_json::from_str::<Fault>(text) {
        Ok(fault) => Error::from_code(&fault.error),
        Err(_) => Error::Internal(format!("http {}", status.as_u16())),
    }
}

fn bearer(request: reqwest::RequestBuilder, token: Option<&str>) -> reqwest::RequestBuilder {
    match token {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

pub async fn post<B: Serialize, R: DeserializeOwned>(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
    body: &B,
) -> Result<R, Error> {
    let request = bearer(client.post(format!("{base}{path}")), token);
    deliver(request.json(body).send().await.map_err(transport)?).await
}

pub async fn post_raw<R: DeserializeOwned>(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
    body: Vec<u8>,
) -> Result<R, Error> {
    let request = bearer(client.post(format!("{base}{path}")), token);
    deliver(request.body(body).send().await.map_err(transport)?).await
}

pub async fn put<B: Serialize, R: DeserializeOwned>(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
    body: &B,
) -> Result<R, Error> {
    let request = bearer(client.put(format!("{base}{path}")), token);
    deliver(request.json(body).send().await.map_err(transport)?).await
}

pub async fn get<R: DeserializeOwned>(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
) -> Result<R, Error> {
    let request = bearer(client.get(format!("{base}{path}")), token);
    deliver(request.send().await.map_err(transport)?).await
}

pub async fn remove(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
) -> Result<(), Error> {
    let request = bearer(client.delete(format!("{base}{path}")), token);
    deliver(request.send().await.map_err(transport)?).await
}

pub async fn bytes(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let request = bearer(client.get(format!("{base}{path}")), token);
    let response = request.send().await.map_err(transport)?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(remote(status, &text));
    }
    Ok(response.bytes().await.map_err(transport)?.to_vec())
}

pub fn query_value(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
