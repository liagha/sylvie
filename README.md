# Sylvie

A self-hosted personal digital hub. One server holds your secrets, files, and
device registry; every machine you own connects as an independent, revocable
device. Secret values are end-to-end encrypted — the server stores ciphertext
only and never learns your password.

## Features

```text
authentication   OPAQUE (RFC 9807) — your password never leaves the machine
devices          one account, many machines, each independently revocable
secrets          end-to-end encrypted values (XChaCha20-Poly1305)
files            upload/download with SHA-256 integrity checks
password change  rotate without losing secrets (vault re-wrap)
api              versioned JSON at /api/v1, ready for non-CLI clients
persistence      SQLite (WAL) with embedded migrations
cli              Unix-style, composable, --json everywhere
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

| variable                  | default                           |
|---------------------------|-----------------------------------|
| `SYLVIE_BIND_ADDR`        | `127.0.0.1:7400`                  |
| `SYLVIE_DB_PATH`          | `$XDG_DATA_HOME/sylvie/sylvie.db` |
| `SYLVIE_STORAGE_PATH`     | `$XDG_DATA_HOME/sylvie/files`     |
| `SYLVIE_LOG_LEVEL`        | `info`                            |
| `SYLVIE_MAX_FILE_SIZE`    | `268435456` (256 MiB)             |
| `SYLVIE_AUTH_ATTEMPTS`    | `10` per IP+username per window   |
| `SYLVIE_AUTH_WINDOW_SECS` | `300`                             |
| `SYLVIE_SESSION_TTL_DAYS` | unset (sessions never expire)     |

The first account created becomes the only account; further registrations are
rejected. Additional machines join by logging in.

## First device

```bash
sylvie register --url http://host:7400 --user <you>
```

Prompts for a password and a device name. Later machines:

```bash
sylvie login --url http://host:7400 --user <you>
```

## Usage

```bash
sylvie status

sylvie secret list
sylvie secret set github           # prompts for the value
sylvie secret set github ghp_xxx   # or pass it as an argument
sylvie secret get github
sylvie secret delete github

sylvie file upload ./backup.tar.gz
sylvie file list
sylvie file download <id> [out]
sylvie file delete <id>

sylvie device list
sylvie device revoke <id>

sylvie passwd                      # rotate password; every secret survives
sylvie logout
```

Every command accepts `--json` for machine-readable output.

Secret operations ask for your password each time — that is what keeps them
end-to-end encrypted. Files, devices, and status run on the stored token.
`SYLVIE_PASSWORD` is honored instead of the prompt for scripting.

Client config lives in `~/.config/sylvie/config.toml`, mode `600`.

## Tests

```bash
cargo test
```

17 tests cover the vault cryptography and the full HTTP API: OPAQUE
registration/login, wrong passwords and unknown-user responses, flood
gating, session expiry, device revocation, secret round-trips and tamper
detection, password rotation preserving secrets, and file integrity.

## Documentation

- [`docs/architecture.md`](docs/architecture.md) — components and how they communicate
- [`docs/protocol.md`](docs/protocol.md) — wire format of `/api/v1`
- [`docs/security.md`](docs/security.md) — threat model, key hierarchy, honest limitations

## Known limitations

- secret names and file contents are readable on the server disk
  (secret values are not)
- plain HTTP — put a TLS reverse proxy in front for anything beyond localhost
- single user; rate-limit state is in-memory and resets on restart
- no external security review yet — read docs/security.md before trusting it

## License

[MIT](LICENSE)
