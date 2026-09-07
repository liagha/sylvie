// web dashboard: a single-page hub view rendered by pardeh, with a browser-side
// module (shell.js) that drives the OPAQUE + vault crypto through the sylvie-web
// wasm so the experience matches the CLI exactly — secrets stay end-to-end
// encrypted and the server never sees a password or plaintext value.

use std::collections::HashMap;

use axum::Form;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use pardeh::{
    Item, Node, Signals, button, card, code, delete, dialog, div, field, form, h1, href, input,
    label, list, p, script, slice, span, textarea,
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

fn styles() -> Node {
    div().raw(format!("<style>{CSS}</style>"))
}

const CSS: &str = r#"
.login { max-width: 22rem; margin: 9vh auto 0; padding: 0 1rem }
.login button[type=submit] { width: 100% }
.brand { padding: 1.7rem 1.5rem .4rem; text-align: center }
.brand h1 { font-size: 1.5rem; letter-spacing: .14em }
.brand p { margin: .3rem 0 0; color: var(--muted); font-size: .82rem }
.tabs { display: grid }
.tabs > input { position: absolute; opacity: 0; pointer-events: none }
.tab-head { display: grid; grid-template-columns: repeat(3, 1fr); margin-top: 1.1rem }
.tab-head label { padding: .65rem 0; text-align: center; color: var(--muted); font-size: .78rem; text-transform: uppercase; letter-spacing: .1em; cursor: pointer; transition: color .12s }
.tab-head label:hover { color: #cfe0f5 }
.panel { display: none; padding: 1rem 1.2rem 1.3rem; gap: .65rem }
#t-create:checked ~ .tab-head label[for=t-create],
#t-unlock:checked ~ .tab-head label[for=t-unlock],
#t-restore:checked ~ .tab-head label[for=t-restore] { color: #cfe0f5; box-shadow: inset 0 -2px 0 var(--accent) }
#t-create:checked ~ .p-create { display: grid }
#t-unlock:checked ~ .p-unlock { display: grid }
#t-restore:checked ~ .p-restore { display: grid }
.reveal { display: none }
.reveal.on { display: block; margin-top: .4rem }
.reveal code { display: block; overflow-x: auto; background: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: .6rem .75rem; color: var(--good); font-family: ui-monospace, monospace; font-size: .82rem; white-space: pre-wrap; word-break: break-all }
.btns { display: flex; gap: .5rem; justify-content: flex-end; margin-top: .6rem }
.top { display: flex; justify-content: space-between; align-items: center; gap: 1rem; flex-wrap: wrap; padding-bottom: 1.15rem; border-bottom: 1px solid var(--line2) }
.actions { display: flex; align-items: center; gap: .75rem; flex-wrap: wrap }
.status { color: var(--muted); font-size: .82rem; min-height: 1.3rem }
.chip { border: 1px solid var(--line); background: var(--panel); color: var(--muted); border-radius: 999px; padding: .3rem .9rem; font-size: .78rem; cursor: pointer; transition: color .12s, border-color .12s }
.chip::before { content: ""; display: inline-block; width: 6px; height: 6px; border-radius: 50%; margin-right: .5rem; background: #55606c; vertical-align: 1px }
.chip.on { color: var(--good); border-color: #2f5a44 }
.chip.on::before { background: var(--good) }
.hint { display: block; color: var(--faint); font-size: .8rem }
@media (max-width: 620px) {
  .login { margin-top: 5vh }
  .top { flex-direction: column; align-items: flex-start; gap: .6rem }
  .actions { width: 100%; justify-content: space-between }
}
"#;

fn app_script() -> Node {
    script()
        .attr("type", "module")
        .attr("src", "/assets/shell.js")
}

fn dashboard(signals: &Signals) -> Node {
    div()
        .kid(styles())
        .kid(app_script())
        .kid(
            div()
                .class("wrap")
                .kid(
                    div()
                        .class("top")
                        .kid(h1().text("sylvie"))
                        .kid(
                            div()
                                .class("actions")
                                .kid(div().class("status").attr("id", "status").text(""))
                                .kid(
                                    button()
                                        .attr("type", "button")
                                        .attr("id", "lock-state")
                                        .text("locked"),
                                )
                                .kid(logout_form()),
                        ),
                )
                .kid(card(
                    "devices",
                    signals.region(
                        "devices",
                        list(
                            "devices",
                            "no devices yet",
                            slice().remove(delete("/web/device", "revoke", true)),
                        ),
                    ),
                    None,
                ))
                .kid(card(
                    "secrets",
                    signals.region(
                        "secrets",
                        list(
                            "secrets",
                            "no secrets yet",
                            slice().remove(delete("/web/secret", "delete", true)),
                        ),
                    ),
                    Some(secret_tools()),
                ))
                .kid(card(
                    "files",
                    signals.region(
                        "files",
                        list(
                            "files",
                            "no files yet",
                            slice()
                                .remove(delete("/web/file", "delete", true))
                                .link(href("/api/v1/files/{}/content", "download")),
                        ),
                    ),
                    Some(file_tools()),
                ))
                .kid(card("account", passwd_form(), None))
                .kid(unlock_dialog()),
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
                        .attr("placeholder", "value (unlock to read or store)"),
                )
                .kid(button().attr("type", "submit").text("set"))
                .kid(
                    div()
                        .class("err")
                        .attr("role", "alert")
                        .attr("id", "secret-msg")
                        .text(""),
                ),
        )
        .kid(
            div()
                .attr("id", "secret-view")
                .class("reveal")
                .kid(code().attr("id", "secret-code"))
                .kid(
                    div()
                        .class("btns")
                        .kid(button().attr("type", "button").attr("id", "secret-copy").text("copy"))
                        .kid(button().attr("type", "button").attr("id", "secret-hide").text("hide")),
                ),
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
        .kid(
            div()
                .class("err")
                .attr("role", "alert")
                .attr("id", "file-msg")
                .text(""),
        )
}

fn passwd_form() -> Node {
    form()
        .attr("id", "passwd")
        .class("tools")
        .kid(
            field("new password (min 8)")
                .name("new")
                .kind("password")
                .auto("new-password")
                .need()
                .min(8)
                .node(),
        )
        .kid(div().class("field").kid(button().attr("type", "submit").text("change password")))
        .kid(
            div()
                .class("err")
                .attr("role", "alert")
                .attr("id", "passwd-msg")
                .text(""),
        )
}

fn unlock_dialog() -> Node {
    dialog()
        .attr("id", "unlock-dialog")
        .kid(
            form()
                .attr("id", "unlock-form")
                .attr("class", "tools")
                .kid(div().class("title").text("unlock vault"))
                .kid(
                    field("password")
                        .name("password")
                        .kind("password")
                        .auto("current-password")
                        .placeholder("unlock this vault")
                        .id("unlock-password")
                        .need()
                        .node(),
                )
                .kid(
                    div()
                        .class("row")
                        .kid(button().attr("type", "submit").attr("id", "unlock-go").text("unlock"))
                        .kid(
                            button()
                                .attr("type", "button")
                                .attr("id", "unlock-cancel")
                                .text("cancel"),
                        ),
                )
                .kid(
                    div()
                        .class("err")
                        .attr("role", "alert")
                        .attr("id", "unlock-msg")
                        .text(""),
                ),
        )
}

fn logout_form() -> Node {
    form()
        .class("inline")
        .attr("method", "post")
        .attr("action", "/logout")
        .kid(button().text("log out"))
}

fn radio(id: &str, on: bool) -> Node {
    let mut node = input()
        .attr("type", "radio")
        .attr("name", "tab")
        .attr("id", id);
    if on {
        node = node.attr("checked", "");
    }
    node
}

fn login_body(create: bool) -> Node {
    div()
        .kid(styles())
        .kid(app_script())
        .kid(
            div()
                .class("login")
                .kid(
                    div()
                        .class("card")
                        .kid(
                            div()
                                .class("brand")
                                .kid(h1().text("sylvie"))
                                .kid(p().text("end-to-end encrypted personal hub")),
                        )
                        .kid(
                            div()
                                .class("tabs")
                                .kid(radio("t-create", create))
                                .kid(radio("t-unlock", !create))
                                .kid(radio("t-restore", false))
                                .kid(
                                    div()
                                        .class("tab-head")
                                        .kid(label().attr("for", "t-create").text("create"))
                                        .kid(label().attr("for", "t-unlock").text("unlock"))
                                        .kid(label().attr("for", "t-restore").text("restore")),
                                )
                                .kid(div().class("panel p-create").kid(register_form()))
                                .kid(div().class("panel p-unlock").kid(login_form()))
                                .kid(div().class("panel p-restore").kid(token_form())),
                        ),
                ),
        )
}

fn register_form() -> Node {
    form()
        .attr("id", "form-register")
        .kid(
            field("username")
                .name("user")
                .kind("text")
                .auto("username")
                .placeholder("you")
                .need()
                .node(),
        )
        .kid(
            field("password")
                .name("password")
                .kind("password")
                .auto("new-password")
                .placeholder("min 8 characters")
                .need()
                .min(8)
                .node(),
        )
        .kid(field("device name").name("name").placeholder("this browser").node())
        .kid(div().class("field").kid(button().attr("type", "submit").text("create account")))
        .kid(
            div()
                .class("err")
                .attr("role", "alert")
                .attr("id", "register-msg")
                .text(""),
        )
}

fn login_form() -> Node {
    form()
        .attr("id", "form-login")
        .kid(
            field("username")
                .name("user")
                .kind("text")
                .auto("username")
                .placeholder("you")
                .need()
                .node(),
        )
        .kid(
            field("password")
                .name("password")
                .kind("password")
                .auto("current-password")
                .placeholder("your password")
                .need()
                .node(),
        )
        .kid(field("device name").name("name").placeholder("this browser").node())
        .kid(div().class("field").kid(button().attr("type", "submit").text("unlock")))
        .kid(
            div()
                .class("err")
                .attr("role", "alert")
                .attr("id", "login-msg")
                .text(""),
        )
}

fn token_form() -> Node {
    form()
        .attr("method", "post")
        .attr("action", "/login")
        .kid(
            field("device token")
                .name("token")
                .kind("password")
                .placeholder("sylvie token")
                .need()
                .node(),
        )
        .kid(span().class("hint").text("from a device already enrolled"))
        .kid(div().class("field").kid(button().attr("type", "submit").text("unlock")))
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
    ctx.web().page("sylvie", dashboard(ctx.web().signals()))
}

async fn login_get(State(ctx): State<Ctx>) -> Response {
    let has: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(ctx.db())
        .await
        .unwrap_or_default();
    ctx.web().page("unlock", login_body(has == 0))
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
    header_value(format!(
        "{COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Secure"
    ))
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