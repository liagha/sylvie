# Security

Written to be read skeptically. v0.1 is a solid foundation, not a
production-hardened system.

## Threat model

Defended against:

- server-side database theft — secret values are ciphertext; the OPAQUE
  password file resists offline guessing (Argon2id inside the protocol)
- password interception or leakage through the server — the password never
  leaves the client; OPAQUE (RFC 9807) proves knowledge of it in the clear
- passive network observers on plain HTTP see only opaque handshakes and
  sealed blobs, but active attackers (tampering, replay of whole sessions)
  are **out of scope without TLS**
- stolen device — its token can be revoked from any other device instantly;
  revocation also blocks future logins *to that device id* even with the
  correct password

Not defended against:

- a compromised server serving malicious responses to a non-TLS client
- malware on your own machine reading passwords/tokens as you type them
- someone with your password enrolling a brand-new device (inherent to
  password authentication; mitigations would be enrollment approval from an
  existing device — future work)

## Key hierarchy

```text
password (human, never transmitted)
   └─ OPAQUE export_key            deterministic per password, client-only
       └─ HKDF "sylvie/vault"      KEK: wraps/unwraps the vault secret
vault_secret                       32 random bytes, generated client-side
   └─ HKDF "sylvie/data"           XChaCha20-Poly1305 key for secret values
login session_key (ephemeral)
   └─ HKDF "sylvie/channel"        seals the login reply (token delivery)
bearer token                       256-bit random, stored sha256(token)
```

Because secret values are sealed under the vault secret — not directly under
the password — `sylvie passwd` re-wraps one 32-byte blob on rotation and
every existing secret stays readable. The server stores only the wrapped
form.

None of these keys ever touch the server disk: they exist only inside CLI
processes for the duration of one command.

## What the server can see

- usernames, device names and counts, timestamps, file names, sizes, hashes
- secret **names** (plaintext, by design — this leaks information; consider
  it public if the DB leaks)
- file **contents** (stored as-is; integrity protected by SHA-256 recorded at
  upload, but confidentiality comes only from disk/filesystem protections)
- secret values: never, in any form but ciphertext

## Device compromise

Revoke it (`sylvie device revoke <id>`) from any surviving device: its token
dies immediately and the dead device id cannot re-enroll even with the
password. Files already downloaded and secrets already decrypted on the lost
machine are gone regardless.

## Recovery considerations

- **forget the password** → every secret value is permanently unreadable.
  There is no reset path by design. Back up important secrets elsewhere.
- **lose all devices** → enroll a new one with username + password; data
  survives because it lives on the server.
- **server disk dies** → files and secrets die together; back up
  `SYLVIE_DB_PATH` + `SYLVIE_STORAGE_PATH` together. The `system` table's
  setup blob is part of the DB and must not be regenerated independently.

## Honest limitations

1. Plain HTTP. Behind a reverse proxy with TLS the story is strong; exposed
   bare, an active attacker can strip/replay traffic.
2. Rate limiting exists (`auth/register/start`, `auth/login/start`; default
   10 per IP+username per 5 minutes) but is in-memory: it resets on restart
   and counts each proxy IP as itself unless real client IPs are forwarded
   and extracted.
3. Single user; no sharing model; no audit log beyond tracing output.
4. Secret names and file contents readable server-side (see above).
5. Pending-login states live in RAM; restarting the server drops half-finished
   logins (clients simply retry — harmless, but worth knowing).
6. Tokens never expire by default; set `SYLVIE_SESSION_TTL_DAYS` for expiry,
   or rely on revocation.
7. The crypto stack is modern and standard (opaque-ke 4, RFC 9807; RustCrypto
   AEAD), but Sylvie has seen no external review.

## Upgrades this design anticipates

per-device keypairs enabling device-to-device pairing and enrollment approval ·
encrypted file blobs (streaming AEAD behind `files.path`) · OS-keyring caching
of the derived keys.
