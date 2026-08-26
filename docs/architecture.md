# Architecture

Sylvie is three crates in one Rust workspace talking over a versioned HTTP
JSON API backed by SQLite.

```text
┌───────────┐   HTTPS/HTTP   ┌───────────────────────────────┐
│ sylvie    │ ─────────────► │ sylvie-server                 │
│ (client)  │ ◄───────────── │ axum router                   │
└───────────┘                │  ├─ routes/auth    OPAQUE     │
                             │  ├─ routes/account bearer     │
                             │  ├─ routes/device             │
                             │  ├─ routes/secret             │
                             │  └─ routes/file               │
                             │ sqlx pool ─► SQLite (WAL)     │
                             │ storage dir ─► file blobs     │
                             └───────────────────────────────┘
```

## core (`sylvie-core`)

Everything both sides must agree on, nothing either side owns:

- `error::Error` — domain error taxonomy plus wire code mapping
- `message` — every request/response struct on the wire
- `opaque` — the single ciphersuite definition (`Suite`) and message
  de/serialization helpers
- `vault` — HKDF-SHA512 key derivation and XChaCha20-Poly1305 seal/open
- `codec` — base64 variants and SHA-256 digests

## server (`sylvie-server`)

Single process, single database.

- `ctx::Ctx` — cheaply cloneable state: pool, `ServerSetup`, storage path,
  body limit, `Limits` (flood gate + session TTL), the pending-login map
  (in-memory, 5 minute TTL), and the flood counter per IP+username
- `db` — connect options (WAL, foreign keys) and embedded migrations
- `reply::Failure` — converts `core::Error` into status codes plus
  `{"error": code}` bodies; internal causes are logged, never returned
- `routes::account::Account` — bearer-token extractor resolving to
  `(owner, device)`; revoked devices fail here
- `routes/auth` — OPAQUE registration, login, password rekey, vault wrap
  delivery, device enrollment, session issuance
- `routes/{secret,file}` — resource CRUD scoped by owner

Request lifecycle: extractor authenticates → handler validates → sqlx
executes → DTO out. No middleware stacks beyond a body-size limit.

## client (`sylvie`)

- `config` — TOML at `~/.config/sylvie/config.toml`, mode 600
- `net` — reqwest wrapper; maps error bodies back into `core::Error`
- `session` — client side of OPAQUE registration, login, and rekey
- `commands/*` — one module per noun (`auth`, `secret`, `file`, `device`)
- `ask` — terminal prompting (rpassword for hidden input)

Two credential tiers drive every command:

| tier        | credential              | commands                     |
|-------------|-------------------------|------------------------------|
| session     | stored bearer token     | status, files, devices       |
| vault       | password → export key   | secret get/set               |

Secret operations run a fresh OPAQUE handshake bound to the already-enrolled
device; the handshake yields `export_key`, which unwraps the stored vault
secret, which yields the data key. No new session is created — see
docs/security.md for why that layering exists.

## Data model

```text
system    key, value(server setup blob)
users     id, username, record(opaque password file), wrap(sealed vault secret), created
devices   id, owner, name, created, revoked?
sessions  hash(sha256 of token), device, created
secrets   owner, name, data(nonce‖ciphertext), created, updated
files     id, owner, name, size, hash, path, created, updated
```

File bytes live under the storage directory named by file id; metadata lives
in `files`. Deleting a row deletes its blob.

## Deliberate non-goings (v0.1)

no microservices, no background workers, no caching layers, no plugin system.
Extension points that matter later: per-device keypairs for pairwise sync,
chunked uploads behind `files.path`, and a notes table beside `secrets`.
