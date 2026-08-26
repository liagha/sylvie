use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use pardeh::{Node, Signals, button, div, form, h1, input, span, table, tbody, td, th, tr};
use sqlx::SqlitePool;

use sylvie_core::codec;

use crate::ctx::Ctx;
use crate::routes::{ident, sane};

const COOKIE: &str = "sylvie_token";

#[derive(Clone)]
struct Item {
    key: String,
    label: String,
    meta: String,
}

pub fn seed(app: &pardeh::App) {
    app.signals().define("devices", Vec::<Item>::new());
    app.signals().define("secrets", Vec::<Item>::new());
    app.signals().define("files", Vec::<Item>::new());
}

fn list_region(
    key: &'static str,
    empty: &'static str,
    action: &'static str,
    action_label: &'static str,
) -> impl Fn(&Signals) -> Node {
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
                                .attr("action", format!("{action}/{}", item.key))
                                .kid(button().attr("type", "submit").text(action_label)),
                        ),
                    )
            }))
        };
        table()
            .kid(tr().kid(th()).kid(th()).kid(th()).kid(th()))
            .kid(tbody().kid(body))
    }
}

fn card(title: &'static str, region: Node) -> Node {
    div()
        .class("card")
        .kid(div().class("head").kid(h1().text(title)))
        .kid(region)
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
.top { display: flex; justify-content: space-between; align-items: center }
.card { background: #151a21; border: 1px solid #232a33; border-radius: 12px; overflow: hidden }
.card .head h1 { font-size: .78rem; text-transform: uppercase; letter-spacing: .14em; color: #8b96a5; padding: .8rem 1rem .5rem }
table { width: 100%; border-collapse: collapse }
th { display: none }
td { padding: .5rem .9rem; border-top: 1px solid #1d242c; font-size: .92rem }
tr:hover td { background: #181f27 }
.muted { color: #7d8894; font-size: .85rem }
.mono { font-family: ui-monospace, monospace; font-size: .78rem; color: #9fb2c8 }
form.inline { display: inline; margin: 0 }
button { background: #1d2833; border: 1px solid #33414f; color: #cfe0f5; border-radius: 7px; padding: .22rem .75rem; cursor: pointer; font-size: .82rem }
button:hover { background: #26333f }
input[type=password] { width: 100%; padding: .65rem .8rem; background: #10151b; color: #d7dde6; border: 1px solid #33414f; border-radius: 8px; font-size: .95rem }
.login { max-width: 24rem; margin: 18vh auto 0 }
.hint { display: block; color: #7d8894; font-size: .82rem; padding: 0 1rem 1rem }
"#;

fn dashboard(signals: &Signals) -> Node {
    div().kid(styles()).kid(
        div()
            .class("wrap")
            .kid(
                div()
                    .class("top")
                    .kid(h1().text("sylvie"))
                    .kid(logout_form()),
            )
            .kid(card(
                "devices",
                signals.region(
                    "devices",
                    list_region("devices", "no devices yet", "/web/device", "revoke"),
                ),
            ))
            .kid(card(
                "secrets",
                signals.region(
                    "secrets",
                    list_region("secrets", "no secrets yet", "/web/secret", "delete"),
                ),
            ))
            .kid(card(
                "files",
                signals.region(
                    "files",
                    list_region("files", "no files yet", "/web/file", "delete"),
                ),
            )),
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
    div().kid(styles()).kid(
        div().class("login").kid(
            div().class("card").kid(
                div().class("head").kid(h1().text("sylvie hub")).kid(
                    form()
                        .attr("method", "post")
                        .attr("action", "/login")
                        .kid(input().attr("type", "password").attr("name", "token"))
                        .kid(
                            span()
                                .class("hint")
                                .text("paste a device token — sylvie token prints it"),
                        ),
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
