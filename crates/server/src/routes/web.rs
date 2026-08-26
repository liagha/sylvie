// web dashboard: a single-page hub view rendered by pardeh, with a browser-side
// module (shell.js) that drives the OPAQUE + vault crypto through the sylvie-web
// wasm so the experience matches the CLI exactly — secrets stay end-to-end
// encrypted and the server never sees a password or plaintext value.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use pardeh::{
    Item, Node, Signals, a, button, card, div, field, form, h1, input, list, script, span, table,
    tbody, td, textarea, th, tr,
};

use sqlx::SqlitePool;

use sylvie_core::codec;

use crate::ctx::Ctx;
use crate::routes::{ident, sane};

const COOKIE: &str = "sylvie_token";

pub fn seed(app: &pardeh::App) {
    app.signals().define("devices", Vec::<Item>::new());
    app.signals().define("secrets", Vec::<Item>::new());
    app.signals().define("files", Vec::<Item>::new());
}

fn file_region(key: &'static str, empty: &'static str) -> impl Fn(&Signals) -> Node {
    move |signals: &Signals| {
        let rows = signals.get::<Vec<Item>>(key);
        let body = if rows.is_empty() {
            tr().kid(
                td().attr("colspan", "4")
                    .kid(span().class("muted").text(empty)),
            )
        } else {
            tbody().kids(rows.iter().map(|item| {
                tr().kid(td().text(item.label.clone()))
                    .kid(td().kid(span().class("muted").text(item.meta.clone())))
                    .kid(td().class("mono").text(item.key.clone()))
                    .kid(
                        td().kid(
                            form()
                                .class("inline")
                                .attr("method", "post")
                                .attr("action", format!("/web/file/{}", item.key))
                                .kid(button().attr("type", "submit").text("delete"))
                                .kid(
                                    a().attr("href", format!("/api/v1/files/{}/content", item.key))
                                        .attr("download", "")
                                        .text("download"),
                                ),
                        ),
                    )
            }))
        };
        table()
            .kid(
                tr().kid(th())
                    .kid(th())
                    .kid(th())
                    .kid(th())
                    .kid(th().text("")),
            )
            .kid(tbody().kid(body))
    }
}

fn styles() -> Node {
    div().raw(format!("<style>{CSS}</style>"))
}

const CSS: &str = r#"
:root { color-scheme: dark }
* { box-sizing: border-box }
body { margin: 0; background: #0e1116; color: #d7dde6; font: 15px/1.55 system-ui, sans-serif; padding: 2.5rem 1rem }
.wrap { max-width: 58rem; margin: auto; display: grid; gap: 1.6rem }
h1 { font-size: 1.3rem; letter-spacing: .04em; margin: 0 }
h2 { font-size: .82rem; text-transform: uppercase; letter-spacing: .12em; color: #8b96a5; margin: 0 0 .6rem }
.top { display: flex; justify-content: space-between; align-items: center }
.card { background: #151a21; border: 1px solid #232a33; border-radius: 12px; overflow: hidden }
.card .head h1 { font-size: .78rem; text-transform: uppercase; letter-spacing: .14em; color: #8b96a5; padding: .8rem 1rem .5rem }
.card .tools { padding: .6rem 1rem; border-top: 1px solid #1d242c; display: grid; gap: .5rem }
table { width: 100%; border-collapse: collapse }
th { display: none }
td { padding: .5rem .9rem; border-top: 1px solid #1d242c; font-size: .92rem }
tr:hover td { background: #181f27 }
.muted { color: #7d8894; font-size: .85rem }
.mono { font-family: ui-monospace, monospace; font-size: .78rem; color: #9fb2c8 }
form.inline { display: inline; margin: 0 }
button, .link { background: #1d2833; border: 1px solid #33414f; color: #cfe0f5; border-radius: 7px; padding: .22rem .75rem; cursor: pointer; font-size: .82rem; text-decoration: none; display: inline-block }
button:hover, .link:hover { background: #26333f }
input[type=password], input[type=text], textarea { width: 100%; padding: .55rem .7rem; background: #10151b; color: #d7dde6; border: 1px solid #33414f; border-radius: 8px; font-size: .95rem; font-family: inherit }
textarea { resize: vertical; min-height: 3.2rem }
.row { display: grid; gap: .5rem; grid-template-columns: 1fr auto }
.login { max-width: 26rem; margin: 12vh auto 0; display: grid; gap: 1.2rem }
.login .card .head h1 { font-size: 1rem; text-transform: none; letter-spacing: .04em; color: #d7dde6; padding: 1rem 1rem .2rem }
.field { display: grid; gap: .35rem; padding: 0 1rem 1rem }
.field label { font-size: .8rem; color: #8b96a5 }
.hint { display: block; color: #7d8894; font-size: .8rem; padding: 0 1rem 1rem }
.note { color: #7d8894; font-size: .82rem; padding: 0 1rem .4rem }
.status { color: #8b96a5; font-size: .82rem }
.err { color: #e8798c; font-size: .82rem; min-height: 1rem }
"#;

fn app_script() -> Node {
    script()
        .attr("type", "module")
        .attr("src", "/assets/shell.js")
}

fn dashboard(signals: &Signals) -> Node {
    div().kid(styles()).kid(app_script()).kid(
        div()
            .class("wrap")
            .kid(
                div()
                    .class("top")
                    .kid(h1().text("sylvie"))
                    .kid(div().class("status").attr("id", "status").text(""))
                    .kid(logout_form()),
            )
            .kid(card(
                "devices",
                signals.region(
                    "devices",
                    list("devices", "no devices yet", "/web/device", "revoke"),
                ),
                None,
            ))
            .kid(card(
                "secrets",
                signals.region(
                    "secrets",
                    list("secrets", "no secrets yet", "/web/secret", "delete"),
                ),
                Some(secret_tools()),
            ))
            .kid(card(
                "files",
                signals.region("files", file_region("files", "no files yet")),
                Some(file_tools()),
            ))
            .kid(passwd_card()),
    )
}

fn secret_tools() -> Node {
    div()
        .class("tools")
        .kid(
            form()
                .attr("id", "secret-get")
                .attr("class", "row")
                .kid(
                    input()
                        .attr("type", "text")
                        .attr("name", "name")
                        .attr("placeholder", "name to read"),
                )
                .kid(button().attr("type", "submit").text("get")),
        )
        .kid(
            form()
                .attr("id", "secret-set")
                .attr("class", "tools")
                .kid(
                    input()
                        .attr("type", "text")
                        .attr("name", "name")
                        .attr("placeholder", "name to store"),
                )
                .kid(
                    textarea()
                        .attr("name", "value")
                        .attr("placeholder", "value (prompted for password)"),
                )
                .kid(div().class("err").attr("id", "secret-msg").text("")),
        )
}

fn file_tools() -> Node {
    div()
        .class("tools")
        .kid(
            form()
                .attr("id", "file-upload")
                .attr("class", "row")
                .kid(input().attr("type", "file").attr("name", "file"))
                .kid(button().attr("type", "submit").text("upload")),
        )
        .kid(div().class("err").attr("id", "file-msg").text(""))
}

fn passwd_card() -> Node {
    div()
        .class("card")
        .kid(
            div()
                .class("head")
                .kid(h1().text("account"))
                .kid(div().class("tools"))
                .kid(logout_form()),
        )
        .kid(
            form()
                .attr("id", "passwd")
                .attr("class", "tools")
                .kid(field("current password", "old", "password", ""))
                .kid(field("new password (min 8)", "new", "password", ""))
                .kid(button().attr("type", "submit").text("change password"))
                .kid(div().class("err").attr("id", "passwd-msg").text("")),
        )
}

fn logout_form() -> Node {
    form()
        .class("inline")
        .attr("method", "post")
        .attr("action", "/logout")
        .kid(button().text("log out"))
}

fn login_body() -> Node {
    div().kid(styles()).kid(app_script()).kid(
        div()
            .class("login")
            .kid(
                div()
                    .class("card")
                    .kid(div().class("head").kid(h1().text("create account")))
                    .kid(
                        form()
                            .attr("id", "form-register")
                            .kid(field("username", "user", "text", "you"))
                            .kid(field(
                                "password",
                                "password",
                                "password",
                                "min 8 characters",
                            ))
                            .kid(field("device name", "name", "text", "this browser"))
                            .kid(
                                div()
                                    .class("field")
                                    .kid(button().attr("type", "submit").text("create account")),
                            ),
                    )
                    .kid(div().class("err").attr("id", "register-msg").text("")),
            )
            .kid(
                div()
                    .class("card")
                    .kid(div().class("head").kid(h1().text("unlock with password")))
                    .kid(
                        form()
                            .attr("id", "form-login")
                            .kid(field("username", "user", "text", "you"))
                            .kid(field("password", "password", "password", ""))
                            .kid(field("device name", "name", "text", "this browser"))
                            .kid(
                                div()
                                    .class("field")
                                    .kid(button().attr("type", "submit").text("unlock")),
                            ),
                    )
                    .kid(div().class("err").attr("id", "login-msg").text("")),
            )
            .kid(
                div()
                    .class("card")
                    .kid(
                        div()
                            .class("head")
                            .kid(h1().text("or paste a device token")),
                    )
                    .kid(
                        form()
                            .attr("method", "post")
                            .attr("action", "/login")
                            .kid(input().attr("type", "password").attr("name", "token"))
                            .kid(
                                span()
                                    .class("hint")
                                    .text("from `sylvie token` on a device already enrolled"),
                            )
                            .kid(
                                div()
                                    .class("field")
                                    .kid(button().attr("type", "submit").text("unlock")),
                            ),
                    ),
            ),
    )
}

pub fn router(ctx: Ctx) -> Router {
    let script_ctx = ctx.clone();
    let events_ctx = ctx.clone();
    Router::new()
        .route("/", get(index))
        .route("/login", get(login_get).post(login_post))
        .route("/logout", post(logout))
        .route("/assets/{*path}", get(asset))
        .route(
            pardeh::SCRIPT_PATH,
            get(move || async move { script_ctx.web().script_response() }),
        )
        .route(
            "/__pardeh/events",
            get(move |headers: HeaderMap| async move { events(&events_ctx, &headers).await }),
        )
        .route("/web/device/{id}", post(web_device))
        .route("/web/secret/{name}", post(web_secret))
        .route("/web/file/{id}", post(web_file))
        .with_state(ctx)
}

async fn index(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    let Some(account) = account(ctx.db(), &headers).await else {
        return see("/login");
    };
    refresh(ctx.web().signals(), ctx.db(), &account.0).await;
    let body = dashboard(ctx.web().signals());
    ctx.web().page("sylvie", body)
}

async fn login_get(State(ctx): State<Ctx>) -> Response {
    ctx.web().page("unlock", login_body())
}

async fn login_post(
    State(ctx): State<Ctx>,
    Form(fields): Form<HashMap<String, String>>,
) -> Response {
    let Some(token) = fields.get("token").cloned() else {
        return see("/login");
    };
    let Some((_, device)) = known_token(ctx.db(), &token).await else {
        return see("/login");
    };
    tracing::info!(device = %device, "dashboard unlocked");
    see_with("/".into(), cookie(&token))
}

async fn logout(State(_ctx): State<Ctx>, headers: HeaderMap) -> Response {
    let _ = account(_ctx.db(), &headers).await;
    see_with("/login".into(), clear_cookie())
}

async fn events(ctx: &Ctx, headers: &HeaderMap) -> Response {
    if account(ctx.db(), headers).await.is_none() {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
    ctx.web().events()
}

async fn web_device(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = ident(&id) else {
        return see("/");
    };
    let Some((owner, _)) = account(ctx.db(), &headers).await else {
        return see("/login");
    };
    let owned: Option<(Option<String>,)> =
        sqlx::query_as("select revoked from devices where id = ? and owner = ?")
            .bind(&id)
            .bind(&owner)
            .fetch_optional(ctx.db())
            .await
            .unwrap_or(None);
    match owned {
        None => return see("/"),
        Some((Some(_),)) => {}
        Some((None,)) => {
            let _ = sqlx::query("update devices set revoked = ? where id = ?")
                .bind(crate::clock::stamp())
                .bind(&id)
                .execute(ctx.db())
                .await;
            let _ = sqlx::query("delete from sessions where device = ?")
                .bind(&id)
                .execute(ctx.db())
                .await;
        }
    }
    refresh(ctx.web().signals(), ctx.db(), &owner).await;
    see("/")
}

async fn web_secret(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if !sane(&name, 128) {
        return see("/");
    }
    let Some((owner, _)) = account(ctx.db(), &headers).await else {
        return see("/login");
    };
    let _ = sqlx::query("delete from secrets where owner = ? and name = ?")
        .bind(&owner)
        .bind(&name)
        .execute(ctx.db())
        .await;
    refresh(ctx.web().signals(), ctx.db(), &owner).await;
    see("/")
}

async fn web_file(State(ctx): State<Ctx>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let Ok(id) = ident(&id) else {
        return see("/");
    };
    let Some((owner, _)) = account(ctx.db(), &headers).await else {
        return see("/login");
    };
    let path: Option<(String,)> =
        sqlx::query_as("select path from files where id = ? and owner = ?")
            .bind(&id)
            .bind(&owner)
            .fetch_optional(ctx.db())
            .await
            .unwrap_or(None);
    if let Some((path,)) = path {
        let _ = sqlx::query("delete from files where id = ?")
            .bind(&id)
            .execute(ctx.db())
            .await;
        let _ = tokio::fs::remove_file(ctx.object_path(&path)).await;
    }
    refresh(ctx.web().signals(), ctx.db(), &owner).await;
    see("/")
}

async fn refresh(signals: &Signals, db: &SqlitePool, owner: &str) {
    let devices: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "select id, name, created, revoked from devices where owner = ? order by created desc",
    )
    .bind(owner)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let secrets: Vec<(String, String)> =
        sqlx::query_as("select name, updated from secrets where owner = ? order by updated desc")
            .bind(owner)
            .fetch_all(db)
            .await
            .unwrap_or_default();

    let files: Vec<(String, String, i64, String)> = sqlx::query_as(
        "select id, name, size, updated from files where owner = ? order by created desc",
    )
    .bind(owner)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let devices = devices
        .into_iter()
        .map(|(key, label, created, revoked)| Item {
            key,
            label,
            meta: match revoked {
                Some(revoked) => format!("{created} · revoked {revoked}"),
                None => created,
            },
        })
        .collect::<Vec<_>>();

    let secrets = secrets
        .into_iter()
        .map(|(name, updated)| Item {
            key: name.clone(),
            label: name,
            meta: updated,
        })
        .collect::<Vec<_>>();

    let files = files
        .into_iter()
        .map(|(key, name, size, updated)| Item {
            key,
            label: name,
            meta: format!("{} · {}", human(size as u64), updated),
        })
        .collect::<Vec<_>>();

    signals.set("devices", devices);
    signals.set("secrets", secrets);
    signals.set("files", files);
}

async fn account(db: &sqlx::SqlitePool, headers: &HeaderMap) -> Option<(String, String)> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{COOKIE}=");
    let token = cookies
        .split(';')
        .find_map(|pair| pair.trim().strip_prefix(&prefix))?;
    known_token(db, token).await
}

async fn known_token(db: &sqlx::SqlitePool, token: &str) -> Option<(String, String)> {
    sqlx::query_as(
        "select d.owner, d.id \
         from sessions s join devices d on d.id = s.device \
         where s.hash = ? and d.revoked is null",
    )
    .bind(codec::digest(token.as_bytes()))
    .fetch_optional(db)
    .await
    .ok()?
}

fn cookie(token: &str) -> HeaderValue {
    header_value(format!("{COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/"))
}

fn clear_cookie() -> HeaderValue {
    header_value(format!("{COOKIE}=; Max-Age=0; Path=/"))
}

fn header_value(text: String) -> HeaderValue {
    HeaderValue::from_str(&text).expect("cookie header")
}

fn see(to: &str) -> Response {
    Redirect::to(to).into_response()
}

fn see_with(to: String, cookie: HeaderValue) -> Response {
    let mut response = see(&to);
    response.headers_mut().insert(SET_COOKIE, cookie);
    response
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < 3 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

async fn asset(State(ctx): State<Ctx>, Path(path): Path<String>) -> Response {
    let root = match ctx.web_dir().canonicalize() {
        Ok(root) => root,
        Err(_) => return (StatusCode::NOT_FOUND).into_response(),
    };
    let target = match root.join(&path).canonicalize() {
        Ok(target) => target,
        Err(_) => return (StatusCode::NOT_FOUND).into_response(),
    };
    if target.strip_prefix(&root).is_err() {
        return (StatusCode::NOT_FOUND).into_response();
    }
    let data = match tokio::fs::read(&target).await {
        Ok(data) => data,
        Err(_) => return (StatusCode::NOT_FOUND).into_response(),
    };
    let mime = mime_for(&target);
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_str(mime)
                .unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream")),
        )],
        data,
    )
        .into_response()
}

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("html") => "text/html; charset=utf-8",
        Some("map") => "application/json",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}
