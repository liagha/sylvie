# Protocol

All endpoints live under `/api/v1`. Requests and responses are JSON unless
stated otherwise. Errors are `{"error": code}` with an appropriate status:
`bad_request`, `unauthorized`, `forbidden`, `not_found`, `conflict`,
`too_large`, `rate_limited` (429), `crypto`, `protocol`, `internal`.

Registration and login starts are throttled per source IP and username
(default 10 attempts per 5 minutes; see README for the env knobs).

Binary OPAQUE messages travel base64-encoded inside the `message` field of
JSON objects. All blobs below are that encoding.

## Registration (first account only; rejected with `conflict` afterwards)

```text
POST /api/v1/auth/register/start
  { username, message }                    message = RegistrationRequest
→ { message }                              message = RegistrationResponse

POST /api/v1/auth/register/finish
  { username, message }                    message = RegistrationUpload
→ 204
```

The client keeps its `export_key` from the finish step.

## Login

```text
POST /api/v1/auth/login/start
  { username, message }                    message = CredentialRequest
→ { id, message }                          message = CredentialResponse
```

Unknown usernames receive a structurally identical dummy response. The server
holds `ServerLogin` state in memory under `id` for at most 5 minutes.

```text
POST /api/v1/auth/login/finish
  { id, message, device?, name? }          message = CredentialFinalization
→ { data }                                 data = sealed Grant JSON
```

Two modes:

- **enroll** (`device` absent): requires `name`; creates a device row and a
  session; sealed payload is `{"token", "device"}`.
- **unlock** (`device` present): verifies the device is alive and owned by the
  authenticated user; creates no session; sealed token field is empty.

In both modes only a client holding the password can decrypt `data`, because
the sealing key is `HKDF-SHA512(session_key, "sylvie/channel")` where
`session_key` is the OPAQUE handshake output.

## Authenticated calls

```text
Authorization: Bearer <token>
```

Tokens are 32 random bytes, url-base64 on the wire, stored only as SHA-256.
No expiry; revocation is the mechanism.

```text
GET    /api/v1/me            → { username, device{name,id,...}, secrets, files, devices }
GET    /api/v1/devices       → [ { id, name, created, revoked? } ]
DELETE /api/v1/devices/{id}  → 204   (kills sessions, marks revoked)

GET    /api/v1/secrets       → [ { name, updated } ]
PUT    /api/v1/secrets/{name}  { data }   data = sealed value, 64 KiB cap → 204
GET    /api/v1/secrets/{name}  → { data }
DELETE /api/v1/secrets/{name}  → 204

POST   /api/v1/files?name=…   raw body → 201 { id, name, size, hash, ... }
GET    /api/v1/files          → [ FileItem ]
GET    /api/v1/files/{id}     → FileItem
GET    /api/v1/files/{id}/content → application/octet-stream
DELETE /api/v1/files/{id}     → 204
```

Names (`username`, device `name`, secret `name`) allow
`[A-Za-z0-9._@-]` only; secret names ≤128 bytes, file names ≤200,
usernames/devices ≤64. Secret values are opaque to the server: the client
seals them as `24-byte XChaCha nonce ‖ ciphertext` before upload.

## Ciphersuite

Fixed for v0.1 (see `core::opaque::Suite`):

- OPRF group: Ristretto255
- key exchange: TripleDH over Ristretto255 with SHA-512
- KSF: Argon2id (library defaults)
- identifiers: client side bound to the username in every handshake

Changing any parameter breaks compatibility with existing databases;
there is no negotiation.
