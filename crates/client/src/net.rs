// http client: talks to the hub api, fixing the server url so a bare host
// or a stray www. still reaches the right endpoint over https.

use reqwest::{Client, Method, Response, StatusCode};
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

fn normalize(raw: String) -> String {
    let mut url = raw.trim().to_string();
    for prefix in ["https://www.", "http://www.", "www."] {
        if let Some(stripped) = url.strip_prefix(prefix) {
            url = stripped.to_string();
            break;
        }
    }
    if !url.contains("://") {
        url = format!("https://{url}");
    }
    url.trim_end_matches('/').to_string()
}

fn target(
    client: &Client,
    method: Method,
    base: &str,
    path: &str,
    token: Option<&str>,
) -> reqwest::RequestBuilder {
    let url = format!("{}{}", normalize(base.to_string()), path);
    bearer(client.request(method, url), token)
}

fn transport(error: reqwest::Error) -> Error {
    let detail = if error.is_builder() {
        "malformed request".to_string()
    } else if error.is_connect() {
        "cannot reach the server (connection refused)".to_string()
    } else if error.is_timeout() {
        "the server took too long to respond".to_string()
    } else if error.is_redirect() {
        "unexpected redirect from the server".to_string()
    } else if error.is_body() || error.is_decode() {
        "could not read the server reply".to_string()
    } else {
        format!("connection failed: {error}")
    };
    Error::Internal(detail)
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
    let request = target(client, Method::POST, base, path, token);
    deliver(request.json(body).send().await.map_err(transport)?).await
}

pub async fn post_raw<R: DeserializeOwned>(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
    body: Vec<u8>,
) -> Result<R, Error> {
    let request = target(client, Method::POST, base, path, token);
    deliver(request.body(body).send().await.map_err(transport)?).await
}

pub async fn put<B: Serialize, R: DeserializeOwned>(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
    body: &B,
) -> Result<R, Error> {
    let request = target(client, Method::PUT, base, path, token);
    deliver(request.json(body).send().await.map_err(transport)?).await
}

pub async fn get<R: DeserializeOwned>(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
) -> Result<R, Error> {
    let request = target(client, Method::GET, base, path, token);
    deliver(request.send().await.map_err(transport)?).await
}

pub async fn remove(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
) -> Result<(), Error> {
    let request = target(client, Method::DELETE, base, path, token);
    deliver(request.send().await.map_err(transport)?).await
}

pub async fn bytes(
    client: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let request = target(client, Method::GET, base, path, token);
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
