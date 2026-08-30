// dashboard scripting: drives the sylvie-web wasm to register, unlock, and
// manage vault items from the browser, turning server and crypto errors into
// readable messages.

import init, {
    start_registration, finish_registration,
    start_login, finish_login, open_login,
    derive_vault, seal_secret, open_secret,
    rekey_start, rekey_finish, drop_session,
} from "/assets/sylvie_web.js";

const TOKEN = "sylvie_token";

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

async function secretGet(name) {
    const msg = el("secret-msg");
    try {
        const password = prompt("password to unlock");
        if (!password) return;
        const handle = await unlock(password);
        const boxed = (await getJson(`/api/v1/secrets/${encodeURIComponent(name)}`)).data;
        const plain = open_secret(handle, boxed);
        drop_session(handle);
        msg.style.color = "#9fb2c8";
        msg.textContent = plain;
    } catch (error) {
        msg.style.color = "#e8798c";
        fail(msg, error);
    }
}

async function secretSet(name, value) {
    const msg = el("secret-msg");
    try {
        const password = prompt("password to unlock");
        if (!password) return;
        const handle = await unlock(password);
        const boxed = seal_secret(handle, value);
        await putJson(`/api/v1/secrets/${encodeURIComponent(name)}`, { data: boxed });
        drop_session(handle);
        location.reload();
    } catch (error) {
        msg.style.color = "#e8798c";
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

async function passwd(oldPassword, newPassword) {
    const msg = el("passwd-msg");
    try {
        if (newPassword.length < 8) throw new Error("password too short (min 8)");
        const handle = await unlock(oldPassword);
        const start = JSON.parse(rekey_start(handle, newPassword));
        const reply = await postJson("/api/v1/auth/rekey/start", { message: start.request });
        const fin = JSON.parse(rekey_finish(handle, reply.message, newPassword));
        await postJson("/api/v1/auth/rekey/finish", {
            message: fin.message,
            wrap: fin.wrap,
        });
        drop_session(handle);
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
    const reg = el("form-register");
    if (reg) {
        reg.addEventListener("submit", async (event) => {
            event.preventDefault();
            const msg = el("register-msg");
            msg.textContent = "";
            try {
                const data = new FormData(reg);
                await register(
                    data.get("user"),
                    data.get("password"),
                    data.get("name") || "web",
                );
            } catch (error) {
                fail(msg, error);
            }
        });
    }

    const login = el("form-login");
    if (login) {
        login.addEventListener("submit", async (event) => {
            event.preventDefault();
            const msg = el("login-msg");
            msg.textContent = "";
            try {
                const data = new FormData(login);
                await enroll(
                    data.get("user"),
                    data.get("password"),
                    data.get("name") || "web",
                );
            } catch (error) {
                fail(msg, error);
            }
        });
    }

    const sget = el("secret-get");
    if (sget) {
        sget.addEventListener("submit", async (event) => {
            event.preventDefault();
            const name = new FormData(sget).get("name");
            if (name) await secretGet(name);
        });
    }

    const sset = el("secret-set");
    if (sset) {
        sset.addEventListener("submit", async (event) => {
            event.preventDefault();
            const data = new FormData(sset);
            const name = data.get("name");
            const value = data.get("value");
            if (name && value !== "") await secretSet(name, value);
        });
    }

    const upload = el("file-upload");
    if (upload) {
        upload.addEventListener("submit", async (event) => {
            event.preventDefault();
            const file = upload.querySelector("input[type=file]").files[0];
            if (file) await fileUpload(file);
        });
    }

    const change = el("passwd");
    if (change) {
        change.addEventListener("submit", async (event) => {
            event.preventDefault();
            const data = new FormData(change);
            await passwd(data.get("old"), data.get("new"));
        });
    }

    showStatus();
}

await init();
wire();
