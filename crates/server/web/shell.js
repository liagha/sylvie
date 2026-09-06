// dashboard scripting: drives the sylvie-web wasm to register, unlock, and
// manage vault items from the browser, turning server and crypto errors into
// readable messages. unlocks once per page via a native dialog and keeps the
// vault open until the lock chip is clicked.

import init, {
    start_registration, finish_registration,
    start_login, finish_login, open_login,
    derive_vault, seal_secret, open_secret,
    rekey_start, rekey_finish, drop_session,
} from "/assets/sylvie_web.js";

const TOKEN = "sylvie_token";
let vkey = null;

function el(id) { return document.getElementById(id); }

function fail(node, error) {
    if (!node) return;
    node.textContent = error && error.message ? error.message : String(error);
}

function describe(code) {
    const known = {
        bad_request: "malformed request",
        unauthorized: "authentication required",
        forbidden: "insufficient rights",
        not_found: "missing resource",
        conflict: "resource already exists",
        too_large: "payload exceeds limit",
        rate_limited: "too many attempts",
        crypto: "cryptographic failure",
        protocol: "protocol violation",
        internal: "server error",
    };
    return known[code] || code;
}

function explain(raw, status) {
    try {
        const data = JSON.parse(raw);
        if (data && typeof data.error === "string") return describe(data.error);
    } catch {}
    if (!raw) return `request failed (${status})`;
    return raw;
}

function setCookie(token) {
    document.cookie = `${TOKEN}=${token}; Path=/; SameSite=Lax; Max-Age=31536000`;
}

async function api(method, path, body, raw) {
    const opt = { method, credentials: "same-origin", headers: {} };
    if (body !== undefined) {
        if (raw) {
            opt.body = body;
        } else {
            opt.headers["Content-Type"] = "application/json";
            opt.body = JSON.stringify(body);
        }
    }
    const res = await fetch(path, opt);
    if (!res.ok) throw explain(await res.text(), res.status);
    const type = res.headers.get("content-type") || "";
    if (type.includes("application/json")) return await res.json();
    return res;
}

function getJson(path) { return api("GET", path); }
function postJson(path, body) { return api("POST", path, body, false); }
function putJson(path, body) { return api("PUT", path, body, false); }

async function unlock(password) {
    const me = await getJson("/api/v1/me");
    const start = JSON.parse(start_login(me.username, password));
    const reply = await postJson("/api/v1/auth/login/start", {
        username: me.username,
        message: start.request,
    });
    const fin = JSON.parse(finish_login(BigInt(start.handle), reply.message, me.device.id, null));
    await postJson("/api/v1/auth/login/finish", {
        id: reply.id,
        message: fin.message,
        device: me.device.id,
    });
    const wrap = (await getJson("/api/v1/vault")).data;
    derive_vault(BigInt(fin.handle), wrap);
    return BigInt(fin.handle);
}

async function register(user, password, name) {
    if (password.length < 8) throw new Error("password too short (min 8)");
    const start = JSON.parse(start_registration(user, password));
    const reply = await postJson("/api/v1/auth/register/start", {
        username: user,
        message: start.request,
    });
    const fin = JSON.parse(finish_registration(BigInt(start.handle), reply.message));
    await postJson("/api/v1/auth/register/finish", {
        username: user,
        message: fin.message,
        wrap: fin.wrap,
    });
    await enroll(user, password, name);
}

async function enroll(user, password, name) {
    const start = JSON.parse(start_login(user, password));
    const reply = await postJson("/api/v1/auth/login/start", {
        username: user,
        message: start.request,
    });
    const fin = JSON.parse(finish_login(BigInt(start.handle), reply.message, null, name));
    const sealed = await postJson("/api/v1/auth/login/finish", {
        id: reply.id,
        message: fin.message,
        name,
    });
    const grant = JSON.parse(open_login(BigInt(fin.handle), sealed.data));
    drop_session(BigInt(fin.handle));
    setCookie(grant.token);
    location.href = "/";
}

function run(btn, work) {
    if (!btn) return Promise.resolve(work());
    const label = btn.textContent;
    btn.disabled = true;
    btn.textContent = "…";
    return Promise.resolve(work()).finally(() => {
        btn.disabled = false;
        btn.textContent = label;
    });
}

function fallback(text) {
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    let ok = false;
    try { ok = document.execCommand("copy"); } catch {}
    area.remove();
    return ok;
}

async function grab(btn, text) {
    const ok = navigator.clipboard && window.isSecureContext
        ? await navigator.clipboard.writeText(text).then(() => true).catch(() => fallback(text))
        : fallback(text);
    const label = btn.textContent;
    btn.textContent = ok ? "copied" : "failed";
    setTimeout(() => { btn.textContent = label; }, ok ? 1200 : 2000);
}

function lock() {
    if (vkey !== null) {
        drop_session(vkey);
        vkey = null;
    }
    showLock();
}

function showLock() {
    const chip = el("lock-state");
    if (!chip) return;
    chip.textContent = vkey === null ? "locked" : "unlocked";
    chip.className = vkey === null ? "chip" : "chip on";
}

async function lockAsk() {
    const dialog = el("unlock-dialog");
    const form = el("unlock-form");
    const pass = el("unlock-password");
    const go = el("unlock-go");
    const msg = el("unlock-msg");
    msg.textContent = "";
    dialog.showModal();
    pass.focus();
    const ok = await new Promise((resolve) => {
        let done = false;
        const finish = (value) => { if (!done) { done = true; resolve(value); } };
        form.onsubmit = async (event) => {
            event.preventDefault();
            go.disabled = true;
            go.textContent = "…";
            msg.textContent = "";
            try {
                vkey = await unlock(pass.value);
                finish(true);
            } catch (error) {
                fail(msg, error);
                go.disabled = false;
                go.textContent = "unlock";
                pass.select();
            }
        };
        el("unlock-cancel").onclick = () => finish(false);
        dialog.oncancel = () => finish(false);
    });
    dialog.close();
    pass.value = "";
    showLock();
    return ok;
}

async function want(work) {
    if (vkey === null && !(await lockAsk())) return false;
    await work(vkey);
    return true;
}

async function secretGet(name) {
    const msg = el("secret-msg");
    const view = el("secret-view");
    const code = el("secret-code");
    try {
        const boxed = (await getJson(`/api/v1/secrets/${encodeURIComponent(name)}`)).data;
        const plain = open_secret(vkey, boxed);
        code.textContent = plain;
        view.classList.add("on");
        msg.textContent = "";
    } catch (error) {
        fail(msg, error);
    }
}

async function secretSet(name, value) {
    const msg = el("secret-msg");
    try {
        const boxed = seal_secret(vkey, value);
        await putJson(`/api/v1/secrets/${encodeURIComponent(name)}`, { data: boxed });
        location.reload();
    } catch (error) {
        fail(msg, error);
    }
}

async function fileUpload(file) {
    const msg = el("file-msg");
    try {
        const bytes = await file.arrayBuffer();
        await api(
            "POST",
            `/api/v1/files?name=${encodeURIComponent(file.name)}`,
            new Uint8Array(bytes),
            true,
        );
        location.reload();
    } catch (error) {
        fail(msg, error);
    }
}

async function passwd(next) {
    const msg = el("passwd-msg");
    try {
        if (next.length < 8) throw new Error("password too short (min 8)");
        const start = JSON.parse(rekey_start(vkey, next));
        const reply = await postJson("/api/v1/auth/rekey/start", { message: start.request });
        const fin = JSON.parse(rekey_finish(vkey, reply.message, next));
        await postJson("/api/v1/auth/rekey/finish", {
            message: fin.message,
            wrap: fin.wrap,
        });
        location.reload();
    } catch (error) {
        fail(msg, error);
    }
}

async function showStatus() {
    const node = el("status");
    if (!node) return;
    try {
        const me = await getJson("/api/v1/me");
        node.textContent =
            `${me.username} · ${me.device.name} · ${me.secrets} secrets, ${me.files} files`;
    } catch {
        node.textContent = "";
    }
}

function wire() {
    document.addEventListener("submit", (event) => {
        const form = event.target;
        const action = form.getAttribute("action") || "";
        if (!action.includes("/web/")) return;
        const row = form.closest("tr");
        const cell = row && row.querySelector("td");
        const label = cell ? cell.textContent.trim() : "";
        const verb = action.includes("/web/file") || action.includes("/web/secret")
            ? "delete"
            : "revoke";
        const noun = action.includes("/web/file")
            ? "file"
            : action.includes("/web/secret")
                ? "secret"
                : "device";
        if (!confirm(label ? `${verb} ${noun} ${label}?` : `${verb} this ${noun}?`)) {
            event.preventDefault();
        }
    }, true);

    const reg = el("form-register");
    if (reg) {
        reg.addEventListener("submit", (event) => {
            event.preventDefault();
            const data = new FormData(reg);
            const msg = el("register-msg");
            msg.textContent = "";
            run(reg.querySelector("button"), async () => {
                try {
                    await register(data.get("user"), data.get("password"), data.get("name") || "web");
                } catch (error) {
                    fail(msg, error);
                }
            });
        });
    }

    const login = el("form-login");
    if (login) {
        login.addEventListener("submit", (event) => {
            event.preventDefault();
            const data = new FormData(login);
            const msg = el("login-msg");
            msg.textContent = "";
            run(login.querySelector("button"), async () => {
                try {
                    await enroll(data.get("user"), data.get("password"), data.get("name") || "web");
                } catch (error) {
                    fail(msg, error);
                }
            });
        });
    }

    const sget = el("secret-get");
    if (sget) {
        sget.addEventListener("submit", (event) => {
            event.preventDefault();
            const name = new FormData(sget).get("name");
            if (!name) return;
            run(sget.querySelector("button"), () => want(() => secretGet(name)));
        });
    }

    const sset = el("secret-set");
    if (sset) {
        sset.addEventListener("submit", (event) => {
            event.preventDefault();
            const data = new FormData(sset);
            const name = data.get("name");
            const value = data.get("value");
            if (!name || value === "") return;
            run(sset.querySelector("button"), () => want(() => secretSet(name, value)));
        });
    }

    const upload = el("file-upload");
    if (upload) {
        upload.addEventListener("submit", (event) => {
            event.preventDefault();
            const file = upload.querySelector("input[type=file]").files[0];
            if (!file) return;
            run(upload.querySelector("button"), () => fileUpload(file));
        });
    }

    const change = el("passwd");
    if (change) {
        change.addEventListener("submit", (event) => {
            event.preventDefault();
            const next = new FormData(change).get("new");
            run(change.querySelector("button"), () => want(() => passwd(next)));
        });
    }

    const copy = el("secret-copy");
    if (copy) {
        copy.addEventListener("click", () => grab(copy, el("secret-code").textContent));
    }

    const hide = el("secret-hide");
    if (hide) {
        hide.addEventListener("click", () => {
            const view = el("secret-view");
            view.classList.remove("on");
            el("secret-code").textContent = "";
        });
    }

    const chip = el("lock-state");
    if (chip) {
        chip.addEventListener("click", () => { if (vkey !== null) lock(); else lockAsk(); });
    }

    showStatus();
    showLock();
}

await init();
wire();