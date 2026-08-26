# Sylvie

A self-hosted personal digital hub. One server holds your secrets, files, and
device registry; every machine you own connects as an independent, revocable
device. Secret values are end-to-end encrypted — the server stores ciphertext
only and never learns your password.

## Status: v0.1

A complete vertical slice:

```text
authentication (OPAQUE, RFC 9807)
devices (per-device identity and revocation)
secrets (client-side XChaCha20-Poly1305)
files (SHA-256 integrity, streamed download)
CLI + versioned JSON API (/api/v1)
SQLite persistence with migrations
```

## Layout

```text
crates/core      shared models, protocol types, crypto helpers
crates/client    sylvie CLI
crates/server    sylvie-server
migrations/      SQL migrations
docs/            architecture.md · protocol.md · security.md
```

## Build

```bash
cargo build --release
```

Produces `target/release/sylvie` and `target/release/sylvie-server`.

## Run the server

```bash
./target/release/sylvie-server
```

Configuration via environment variables (all optional):

| variable               | default                          |
|------------------------|----------------------------------|
| `SYLVIE_BIND_ADDR`     | `127.0.0.1:7400`                 |
| `SYLVIE_DB_PATH`       | `$XDG_DATA_HOME/sylvie/sylvie.db`|
| `SYLVIE_STORAGE_PATH`  | `$XDG_DATA_HOME/sylvie/files`    |
| `SYLVIE_LOG_LEVEL`     | `info`                           |
| `SYLVIE_MAX_FILE_SIZE` | `268435456` (256 MiB)            |

The first account created becomes the only account; further registrations are
rejected. Additional machines join by logging in.

## First device

```bash
sylvie register --url http://host:7400 --user alee
```

Prompts for a password and device name. Later machines:

```bash
sylvie login --url http://host:7400 --user alee
```

## Usage

```bash
sylvie status

sylvie secret list
sylvie secret set github          # prompts for value
sylvie secret set github ghp_xxx  # or pass it / reads stdin-free scripts use SYLVIE_PASSWORD
sylvie secret get github
sylvie secret delete github

sylvie file upload ./backup.tar.gz
sylvie file list
sylvie file download <id> [out]
sylvie file delete <id>

sylvie device list
sylvie device revoke <id>

sylvie logout
```

Every command accepts `--json` for machine-readable output. Secrets require
your password each time (that is what keeps them end-to-end); files and device
management use the stored token. `SYLVIE_PASSWORD` env var is honored for
scripting. Client config lives in `~/.config/sylvie/config.toml` (mode 600).

## Tests

```bash
cargo test
```

13 tests cover vault cryptography and the full HTTP API including OPAQUE
registration/login, wrong passwords, revocation, secret round-trips, tampering
detection, and file integrity.

## Documentation

- `docs/architecture.md` — components and how they communicate
- `docs/protocol.md` — wire format of `/api/v1`
- `docs/security.md` — threat model, key hierarchy, honest limitations

## Known limitations (v0.1)

- secret names and file contents are readable on the server disk (values are not)
- plain HTTP — put a TLS reverse proxy in front for anything beyond localhost
- no rate limiting, no password change flow, single user
