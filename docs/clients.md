# Clients

One account, unlimited devices, each revocable with `sylvie device revoke`.

## Desktop (Linux / macOS)

```bash
cargo install --git https://github.com/liagha/sylvie --bin sylvie
# or grab a prebuilt Linux binary from GitHub Releases
sylvie register --url https://hub.example.com --user alee   # first machine only
sylvie login    --url https://hub.example.com --user alee   # every other machine
```

## Android (Termux) — full client today

Termux runs the real CLI natively:

```bash
pkg update && pkg install rust git openssl
cargo install --git https://github.com/liagha/sylvie --bin sylvie
sylvie login --url https://hub.example.com --user alee
```

Build takes a few minutes on-device; afterwards you have full read/write:
secrets (with password prompt), files, device management. Keep Termux
unlocked only when in use; the stored token lives inside Termux's private
storage and dies with the app if revoked.

## iPhone — honest status

There is no iOS client yet, and the crypto has a consequence you should know:
secret **values** are sealed under a key derived from your password, so no
token alone can ever decrypt them — a real client is required.

What works from iOS *today*, without installing anything:

- files: `GET /api/v1/files` + `/content` with a bearer token (plaintext by
  design), via the Shortcuts app
- secret names/timestamps: readable metadata
- secret values: not decryptable until the planned web/mobile client exists

The API was built client-agnostic (`docs/protocol.md`) exactly for this; a
minimal web vault is the natural next Sylvie milestone.

## Revoking anything

```bash
sylvie device list          # find the id
sylvie device revoke <id>   # token dead instantly, re-login blocked
```

Do this the moment a device leaves your hands.
