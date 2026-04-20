# Phase 2: Device Integration - Research

**Researched:** 2026-04-19
**Domain:** Real-time Hikvision ISAPI alertStream ingestion + encrypted device management + deduplicated event storage in libSQL
**Confidence:** MEDIUM–HIGH (HIGH on Rust stack; MEDIUM on alertStream XML — no hardware in hand, must verify against real DS-K1T341/DS-K1T342 traffic during Wave 0)

---

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Credential Storage**
- **D-01:** ISAPI passwords encrypted with AES-256-GCM. Plaintext never on disk. Decryption in-memory only for outbound ISAPI requests or alertStream connection open.
- **D-02:** Encryption key is a dedicated `DEVICE_CREDS_KEY` env var (32 bytes, base64 in `.env`). Isolated from `JWT_SECRET`. Installer generates it.
- **D-03:** Device password is never returned in any API response — not raw, not masked. PATCH-only mutation. Response schema has no password field.
- **D-04:** Key rotation CLI deferred.

**Dedup & Unknown Faces**
- **D-05:** Dedup key = `(employee_id, device_id, direction, 30s-bucket)` where `bucket = floor(epoch_seconds / 30)`. Events on different devices within 30s are BOTH persisted.
- **D-06:** Dedup enforced by composite UNIQUE index + `INSERT OR IGNORE`. DB-level invariant; no app-level pre-insert SELECT.
- **D-07:** Unknown face events persist with `employee_id = NULL`, `is_unknown = 1`, and captured `face_id`.
- **D-08:** face_id → employee mapping in dedicated `device_face_mappings (device_id, face_id) → employee_id`.

**ISAPI Command Dispatch**
- **D-09:** Synchronous dispatch, 10-second HTTP timeout. `POST /api/v1/devices/:id/commands` blocks; 200 on success, 504 on timeout.
- **D-10:** Single route, command in body: `POST /api/v1/devices/:id/commands { "command": "door_open" | "reboot" | "enrollment_mode" }`. Admin-only.
- **D-11:** Every dispatch written to dedicated `command_audit_log` table.

**Event Payload Retention**
- **D-12:** Full raw alertStream XML persisted in `attendance_events.raw_xml`.
- **D-13:** JPEG captures on filesystem at `./data/events/YYYY-MM-DD/{event_id}.jpg`. `attendance_events.photo_path` holds relative path.
- **D-14:** No retention/purge policy in Phase 2.
- **D-15:** Read API: `GET /api/v1/events?limit&offset&employee_id&device_id&from&to` using Phase 1 conventions; all three roles may read (Viewer per Phase 1 D-09).

### Claude's Discretion
- Exact `devices` / `attendance_events` / `device_face_mappings` / `command_audit_log` column sets (follow Phase 1 conventions: UUID v4 PKs, UTC epoch INTEGER timestamps, `version` column on mutable tables, `status/deleted_at` on devices, audit triggers).
- tokio supervisor topology (one task per device, reconnect loop with exponential backoff + jitter, graceful shutdown on disable/edit). Exact backoff constants left to research + planner.
- Online/offline detection mechanism (TCP state vs heartbeat vs last-event timestamp).
- `quick-xml` multipart parser implementation and `diqwest` digest-auth integration details.
- Handler ↔ service ↔ db layering within `devices/` and `events/` modules.
- `command_audit_log` schema and whether audit writes happen via trigger or application code.

### Deferred Ideas (OUT OF SCOPE)
- Device credential key rotation CLI (`cronometrix rotate-device-key`).
- Configurable event retention + nightly purge job.
- Async/queued command dispatch for long-running multi-device ops.
- "View password" admin reveal endpoint.

---

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DEV-01 | Admin registers Hikvision device (IP, ISAPI credentials, direction) | §Standard Stack (aes-gcm, libsql); §Credential Encryption Pattern; §Architecture Patterns → Device Manager API |
| DEV-02 | Admin views real-time connection status | §Online/Offline Detection; §Architecture Patterns → Supervisor state channel |
| DEV-03 | Admin sends ISAPI commands (door open, reboot, enrollment mode) | §diqwest digest auth; §Architecture Patterns → Command Dispatcher; §Timeout handling (tokio::time::timeout) |
| DEV-04 | Admin edits or disables device | §Supervisor reconciliation; §Code Examples → Device lifecycle signalling |
| EVT-01 | Maintains persistent alertStream connections to all devices | §alertStream protocol; §tokio Supervisor Topology |
| EVT-02 | Auto-reconnects on drop | §Exponential backoff + jitter (backon / tokio-retry2); §Reconnect loop state machine |
| EVT-03 | Dedup within 30-second window per employee | §libSQL INSERT OR IGNORE semantics; §Composite UNIQUE pattern |
| EVT-04 | Stores raw events with UTC epoch timestamps | §Phase 1 conventions (unixepoch()); §Raw XML retention (D-12) |

---

## Summary

Phase 2 plugs Cronometrix into real Hikvision hardware. The technical core is a **long-lived `multipart/mixed` HTTP stream** from each device's `/ISAPI/Event/notification/alertStream` endpoint, protected by **HTTP Digest authentication**, delivering **XML event blocks interleaved with binary JPEG parts**. Each connection is owned by a **per-device tokio task** managed by a supervisor that reconciles against the `devices` table, with exponential backoff + jitter on reconnect. Events land in a `attendance_events` table whose **composite UNIQUE index** on `(employee_id, device_id, direction, bucket_30s)` + **`INSERT OR IGNORE`** makes dedup a zero-race database invariant. ISAPI credentials are AES-256-GCM-encrypted at rest with a key separate from `JWT_SECRET`. Outbound commands (door open / reboot / enrollment mode) use the same digest-auth pipeline with a 10-second hard timeout.

The **load-bearing unknown** flagged in STATE.md is the exact multipart XML schema per device model. Documentation is behind Hikvision's TPP login wall, and the one public Rust crate that parses `multipart/x-mixed-replace` streams (`multipart-stream` 0.1.2, 2021) is pinned to `http 0.2` and will NOT compile alongside `reqwest 0.13` which uses `http 1.x`. The practical path forward combines **`multer` 3.1.0 (http 1.x compatible)** for boundary parsing with a **line-scan fallback** inspired by the Python `pyHik` project — scanning for `<EventNotificationAlert>…</EventNotificationAlert>` delimiters works across firmware variants without requiring well-formed Content-Disposition headers.

**Primary recommendation:** Scaffold the Device Manager API first (02-01, deterministic work), then use Wave 0 of 02-02 to **capture real alertStream traffic from at least one DS-K1T341 or DS-K1T342** (tcpdump / mitmproxy / raw `curl --digest`) so the XML struct is pinned against real bytes before the parser ships. Implement the supervisor as a `CancellationToken`-driven tree spawned from `main()` alongside `axum::serve()`, with a `tokio::sync::mpsc` channel from the devices handler to signal start/stop/restart on CRUD events.

---

## Project Constraints (from CLAUDE.md)

Actionable directives the planner MUST honor:

- **Backend:** Rust + Axum 0.8.x (0.8.8 available). NOT actix-web, NOT warp, NOT diesel.
- **HTTP client:** `reqwest` 0.13.2 with `rustls-tls` (avoid OpenSSL system dep).
- **Digest auth:** `diqwest` crate (extends reqwest `RequestBuilder`). Hikvision devices require digest auth.
- **XML:** `quick-xml` 0.39.x (NOT `serde-xml-rs` — 10× slower). Enable `serialize` feature for serde derives.
- **Password hashing:** Argon2id only (not bcrypt). Already wired in Phase 1 via `password-auth`.
- **State:** Per CLAUDE.md "Global Rust state with `Mutex<HashMap>`" is an anti-pattern — device state MUST live in SQLite, not in-memory HashMaps.
- **Async runtime:** tokio 1.x (locked by libsql, reqwest, axum).
- **DB:** libSQL 0.9.30 (raw queries, NOT SeaORM, NOT sqlx).
- **Audit posture:** Every mutation generates audit log entry. Command dispatch is audited per D-11.
- **On-prem deployment:** No external cloud-only dependencies in runtime path (Turso sync is out-of-band).
- **/api/v1 prefix** for all new routes.

---

## Standard Stack

### Core (new crates added for Phase 2)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `reqwest` | 0.13.2 [VERIFIED: crates.io 2026-02-06] | Outbound HTTP client for ISAPI commands + alertStream GET | Already locked in CLAUDE.md; TLS via rustls avoids OpenSSL build pain; `bytes_stream()` for long-polling `multipart/mixed` |
| `diqwest` | 3.2.0 [VERIFIED: crates.io 2026-02-03] | HTTP Digest auth extension for reqwest | Hikvision ISAPI requires RFC 2617 digest; `.send_digest_auth((user, pass))` handles 401 challenge-response in one call |
| `quick-xml` | 0.39.2 [VERIFIED: crates.io 2026-02-20] | Parse `EventNotificationAlert` XML blocks | 10× faster than `serde-xml-rs` per CLAUDE.md; `serialize` feature enables serde derive on event struct |
| `aes-gcm` | 0.10.3 [VERIFIED: crates.io — latest stable; 0.11.0-rc.3 is a pre-release] | AES-256-GCM encryption of ISAPI passwords | RustCrypto; pure-Rust no-OpenSSL; AEAD with built-in authentication; NIST-approved |
| `rand` | 0.8.6 [VERIFIED: crates.io 2026-04-17] | OS-RNG nonce generation for aes-gcm | Provides `OsRng` used by `Aes256Gcm::generate_nonce` |
| `base64` | 0.22.1 [VERIFIED: crates.io] | Encode DEVICE_CREDS_KEY in env / encrypted payload in DB | Standard, maintained; `base64::engine::general_purpose::STANDARD` |
| `tokio-util` | 0.7.18 [VERIFIED: crates.io 2026-01-04] | `CancellationToken` for supervisor tree | Tokio-official graceful-shutdown primitive |
| `multer` | 3.1.0 [VERIFIED: crates.io — deps use http ^1.0, hyper ^1.0, tokio ^1.0] | Multipart boundary parser for alertStream body | Only maintained multipart stream parser compatible with reqwest 0.13 / http 1.x. See §Known Pitfall 1 |

### Supporting (retry / backoff)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `backon` | 1.6.0 [VERIFIED: crates.io 2025-10-18] | Exponential backoff + jitter for reconnect loop | Modern maintained retry crate; `ExponentialBuilder::default().with_jitter()` |
| `tokio-retry2` | 0.9.1 [VERIFIED: crates.io 2026-01-11] | Alternative retry strategy | `backoff` crate (0.4.0) is stale (2021); pick ONE of backon/tokio-retry2, not both |

**Recommendation: use `backon`** — simpler API, active maintenance, explicit jitter support. The reconnect loop can also be hand-rolled with `tokio::time::sleep` + `rand::Rng::gen_range` in ~20 lines (see §Code Examples).

### Dev / Test

| Library | Version | Purpose |
|---------|---------|---------|
| `wiremock` | 0.6.5 [VERIFIED: crates.io 2025-08-24] | Mock HTTP server for ISAPI command dispatch unit tests. LIMITATION: no first-class multipart/chunked streaming support — for alertStream integration tests use a custom tokio TCP fixture (see §Validation Architecture) |
| `mockito` | 1.7.2 [VERIFIED: crates.io 2026-02-02] | Alternative HTTP mock — evaluated but wiremock is more idiomatic and already widely used in the Rust Axum ecosystem |

### Alternatives Considered

| Instead of | Could Use | Why We Didn't |
|------------|-----------|---------------|
| `multer` for multipart | `multipart-stream` 0.1.2 | [VERIFIED: crates.io] Pinned to `http ^0.2`, `httparse`, `pin-project`. reqwest 0.13 requires http 1.x — hard compile conflict. Crate last published 2021-05. |
| `aes-gcm` 0.10.3 | `ring`, `openssl` | RustCrypto is pure Rust, no C dependency; avoids platform-specific build pain on the target Linux servers. `ring` works but adds maintenance surface. |
| `backon` | `backoff` 0.4.0 | `backoff` crate last updated 2021-12. `backon` 1.6.0 (Oct 2025) is actively maintained. |
| `backon` | hand-rolled | Hand-roll is fine and well under 30 LOC (see §Code Examples). Only reach for a crate if a second retry site emerges in Phase 3+. |
| `multer` for alertStream | Hand-rolled line scanner (pyHik style) | [CITED: pyHik hikvision.py L683-L702] A `<EventNotificationAlert>…</EventNotificationAlert>` scanner is the most resilient approach when Content-Disposition is missing or vendor variants emerge. Consider as fallback. |

### Installation

```toml
# Add to backend/Cargo.toml [dependencies]
reqwest = { version = "0.13.2", default-features = false, features = ["rustls-tls", "stream", "json"] }
diqwest = "3"
quick-xml = { version = "0.39", features = ["serialize"] }
aes-gcm = "0.10"
rand = "0.8"
base64 = "0.22"
tokio-util = "0.7"
multer = "3"
backon = "1"

# Add to [dev-dependencies]
wiremock = "0.6"
```

---

## Architecture Patterns

### Recommended Module Structure

```
backend/src/
├── devices/
│   ├── mod.rs           # public re-exports
│   ├── models.rs        # Device, CreateDeviceRequest, UpdateDeviceRequest, CommandRequest, DeviceStatus
│   ├── handlers.rs      # Axum route handlers (/api/v1/devices/*)
│   ├── service.rs       # CRUD + command dispatch logic
│   └── crypto.rs        # AES-256-GCM encrypt/decrypt helpers for ISAPI password
├── events/
│   ├── mod.rs
│   ├── models.rs        # AttendanceEvent, EventListQuery
│   ├── handlers.rs      # GET /api/v1/events, GET /api/v1/events/:id/photo
│   └── service.rs       # query + INSERT OR IGNORE ingest helper
├── isapi/
│   ├── mod.rs
│   ├── client.rs        # reqwest + diqwest wrapper (send_command, open_alert_stream)
│   ├── parser.rs        # multipart boundary extract + XML parse into AccessEvent
│   └── events.rs        # serde structs: EventNotificationAlert, AccessControllerEvent
└── supervisor/
    ├── mod.rs           # Supervisor handle + API (start, stop, reconcile)
    ├── task.rs          # Per-device listener task (reconnect loop)
    └── status.rs        # Online/offline state tracking (Arc<DashMap<DeviceId, Status>> OR DB-backed)
```

### Pattern 1: Supervisor-per-device Topology

**What:** The supervisor owns a `HashMap<DeviceId, PerDeviceHandle>`. Each handle contains:
- A `JoinHandle<()>` for the listener task
- A `CancellationToken` scoped to that device
- A `watch::Sender<DeviceConfig>` so credential/IP updates arrive without full restart (optional; simpler to just restart)

**When to use:** Any time the app runs N independent long-lived network connections keyed by a DB-managed identity.

**Startup flow:**
1. `main()` calls `init_db(&config).await?`.
2. `main()` builds an `AppState` that contains an `Arc<Supervisor>` handle.
3. Before `axum::serve(...)`, `main()` spawns `Supervisor::run(db, creds_key)` which:
   a. Loads all `active` devices from DB.
   b. For each, spawns a per-device listener task and registers its handle.
   c. Subscribes to an mpsc receiver listening for `DeviceLifecycleEvent { Start, Stop, Restart }` messages published by the devices handler after CRUD.
4. The devices handler, on POST/PATCH/DELETE, **writes to DB first**, then emits a lifecycle event on the mpsc sender.

**Per-device task loop (pseudocode):**

```rust
async fn device_task(
    device_id: String,
    mut config_rx: watch::Receiver<DeviceConfig>,
    cancel: CancellationToken,
    state: AppState,
) {
    let mut backoff_ms: u64 = 1_000;
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            _ = async {
                match connect_and_stream(&config_rx.borrow(), &state).await {
                    Ok(()) => { backoff_ms = 1_000; /* graceful close, retry immediately-ish */ }
                    Err(e) => tracing::warn!(device_id=%device_id, err=%e, "stream died"),
                }
                let jitter = rand::thread_rng().gen_range(0..=backoff_ms / 4);
                tokio::time::sleep(Duration::from_millis(backoff_ms + jitter)).await;
                backoff_ms = (backoff_ms * 2).min(60_000);   // cap 60s
            } => {}
        }
    }
}
```

**Anti-patterns:**
- **DO NOT** store device state in `Arc<Mutex<HashMap<String, DeviceState>>>` as the source of truth. CLAUDE.md explicitly forbids this. The DB is authoritative. The supervisor's map is purely a **handle registry** — lose it on process restart and it's rebuilt from DB.
- **DO NOT** let the supervisor hold `Connection` objects — connections are acquired per-call via `state.db.connect()` (matches Phase 1 pattern).
- **DO NOT** swallow errors inside `connect_and_stream` silently; always emit a `tracing::warn!` with `device_id` span so operators can debug reconnect storms.

### Pattern 2: alertStream Connection Life Cycle

**What:** One GET per device to `https://{ip}/ISAPI/Event/notification/alertStream`, kept alive forever.

**Flow:**
1. Acquire `Connection`, load device row, decrypt password with DEVICE_CREDS_KEY.
2. Build a `reqwest::Client` per-task with `timeout(Duration::from_secs(30))` for the connect phase and **no** body timeout (the stream is supposed to be infinite). Set a read timeout equal to 2× max heartbeat interval.
3. Call `.get(url).send_digest_auth((user, &password)).await?`.
4. Extract the `Content-Type` header, parse out the `boundary=...` value with `mime::Mime::from_str`.
5. Convert `.bytes_stream()` into a `multer::Multipart` or hand-rolled line scanner.
6. For each Part:
   - If the body contains `<EventNotificationAlert`: buffer the XML, feed to `quick-xml` deserializer.
   - If the body is a JPEG (`Content-Type: image/jpeg` OR magic bytes `\xFF\xD8\xFF`): retain as bytes, attach to the next/previous event based on ordering (see §Known Pitfall 2).
7. Detect heartbeat: [CITED: per Hikvision docs + pyHik behavior] if XML contains `<eventType>videoloss</eventType>` with `<eventState>inactive</eventState>`, OR `<eventType>Heartbeat</eventType>`, treat as keepalive only — update `last_seen_at`, don't persist.
8. For real access events, extract the fields, look up `(device_id, face_id) → employee_id` in `device_face_mappings`, generate event_id (UUID v4), save JPEG to `./data/events/YYYY-MM-DD/{event_id}.jpg` (mkdir -p), run `INSERT OR IGNORE INTO attendance_events(...)`, inspect `rows_affected`. If 0, the event was deduped — emit `tracing::debug!` and continue.

**When to use:** Always, for alertStream. This is the canonical pattern.

**Code pattern verified in codebase:**

```rust
// [VERIFIED: backend/src/employees/service.rs:242-247]
let rows_affected = conn
    .execute(sql, params)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

if rows_affected == 0 {
    // Dedup hit OR concurrent update — interpret based on context
}
```

### Pattern 3: Synchronous Command Dispatch with Hard Timeout

**What:** The handler accepts `POST /api/v1/devices/:id/commands`, returns inside 10 seconds or 504s out.

**Flow:**
1. Handler extracts `Path(device_id)`, validates admin via existing `require_admin` middleware.
2. Handler loads device, decrypts password.
3. Handler matches the command enum → target ISAPI path (see §ISAPI Command Paths).
4. Wraps the call in `tokio::time::timeout(Duration::from_secs(10), isapi::send_command(...))`.
5. On success: parse device response, write `command_audit_log` row with `result`, return 200.
6. On timeout: write `command_audit_log` row with `error = "timeout"`, return 504 via a new `AppError::Timeout` variant.
7. On digest 401 / non-2xx: write `command_audit_log` row with `error = <code>`, return 502 Bad Gateway.

**Code pattern:**

```rust
// Proposed new AppError variant (extend backend/src/errors.rs)
#[derive(Error, Debug)]
pub enum AppError {
    // ... existing variants ...
    #[error("gateway timeout")]
    Timeout { code: &'static str, message: String },
    #[error("bad gateway")]
    BadGateway { code: &'static str, message: String },
}
// impl IntoResponse — Timeout → 504, BadGateway → 502.
```

### Anti-Patterns

- **Reading device password plaintext from DB row** → always go through `crypto::decrypt(encrypted_blob, &key)`. Make the raw `Device` struct's password field inaccessible outside `devices::service` (private field + opaque accessor).
- **Polling `/ISAPI/System/status` for online/offline** → Hikvision devices answer `/ISAPI/System/status` even while alertStream is broken. It's a useful secondary signal but NOT the primary source of truth.
- **Forgetting TLS cert validation** → Hikvision devices often ship self-signed certs. Decide up-front: either require users to upload the device's CA, OR use `danger_accept_invalid_certs` **only** on devices explicitly marked `allow_insecure_tls=true` in the DB.
- **Blocking the axum handler on ISAPI I/O without a timeout** → always `tokio::time::timeout(10s, ...)`.
- **Writing the JPEG to disk before running `INSERT OR IGNORE`** → on dedup the file becomes orphaned. Write to a tempfile FIRST, run INSERT, and only rename to final path if `rows_affected == 1`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP Digest auth challenge-response | Custom nonce/cnonce/MD5 pipeline | `diqwest` (`.send_digest_auth((user, pass))`) | RFC 2617 has 15+ edge cases (qop=auth vs auth-int, algorithm=MD5 vs MD5-sess, stale=true retry). diqwest handles all. [CITED: docs.rs/diqwest] |
| AES-GCM encrypt/decrypt with nonce mgmt | Bit-twiddle OpenSSL | `aes-gcm` + `rand::rngs::OsRng` | AEAD tag verification is easy to get wrong — a manually concatenated tag + ciphertext that doesn't use constant-time compare is exploitable. |
| Multipart boundary parsing | Byte-by-byte split | `multer` 3.1.0 (primary) + line-scan fallback (secondary) | Boundary may legally contain hyphens and be up to 70 bytes; CRLF vs LF handling varies by firmware. |
| XML → struct | Manual ET.find chains | `quick-xml` with serde derive | `serialize` feature + `#[serde(rename = "...")]` gives you the struct for free and is 10× faster. |
| Exponential backoff + jitter | Custom arithmetic | `backon` OR hand-rolled `tokio::time::sleep` in a loop | Hand-roll is fine — keep it <30 LOC. Reach for `backon` if reconnect logic grows more than one retry target. |
| Password hashing | Roll argon2 params | `password-auth` crate (already in Phase 1) | Already wired. |
| Graceful shutdown across N tasks | Manual mpsc shutdown signal | `tokio_util::sync::CancellationToken` | Official Tokio primitive. Clone tokens freely; cancel propagates. [CITED: tokio.rs/tokio/topics/shutdown] |

**Key insight:** Hikvision's TPP docs are behind a login wall. Every time you reach for a custom protocol handler, you risk discovering a firmware quirk the next day. Lean on well-tested community crates and keep the custom surface area small — the parser is the one place custom code is unavoidable, and even there we layer `multer` + serde on top of hand-rolled event assembly.

---

## Common Pitfalls

### Pitfall 1: `multipart-stream` 0.1.2 is NOT compatible with `reqwest` 0.13
**What goes wrong:** You add `multipart-stream = "0.1"` to Cargo.toml. It compiles against `http ^0.2`. `reqwest 0.13.2` compiles against `http ^1.1`. `cargo check` ends with a type mismatch on `HeaderMap`, and the resolver may even pull in both `http 0.2` and `http 1.1` at once — silent correctness bug for any code that passes HeaderMap between them.
**Why it happens:** `multipart-stream` was last released 2021-05, before hyper/http 1.0. [VERIFIED: crates.io Cargo.toml]
**How to avoid:** Use `multer` 3.1.0 (deps: `http ^1.0`, `hyper ^1.0`, `bytes ^1.0`, `tokio ^1.0`) OR hand-roll a line scanner.
**Warning signs:** `error[E0308]: mismatched types` mentioning `http::header::HeaderMap` vs `http_02::header::HeaderMap`.

### Pitfall 2: JPEG part ordering vs XML event ordering
**What goes wrong:** [CITED: deepwiki/fuqiangZ/hikvision-isapi-go/4.5] Hikvision interleaves parts as `<EventNotificationAlert XML part>` then `<image/jpeg part>` for the same event. If you process parts without pairing them, the photo attaches to the wrong event.
**Why it happens:** The multipart stream is a flat sequence. Each event's XML and its photo are adjacent but not in a nested structure.
**How to avoid:** Maintain a small state machine: when you see an XML part with a non-heartbeat event, save it as "pending" and wait for either the next non-XML part (attach it) OR the next XML part (commit the pending event photo-less and start a new pending).
**Warning signs:** Employee A's photo appearing on Employee B's event row.

### Pitfall 3: TLS cert validation on devices with self-signed certs
**What goes wrong:** reqwest refuses to connect; device-register appears to succeed but alertStream task dies immediately.
**Why it happens:** Hikvision ships devices with self-signed HTTPS certs (or HTTP-only on older firmware).
**How to avoid:** Add `allow_insecure_tls BOOL NOT NULL DEFAULT 0` to `devices`. When the column is 1, build the per-task `reqwest::Client` with `.danger_accept_invalid_certs(true)`. Document this tradeoff in the Device Manager help text.
**Warning signs:** Immediate reconnect loop on a device that responds to `ping` and answers manual `curl -k` requests.

### Pitfall 4: Digest auth + streaming response body
**What goes wrong:** `diqwest::send_digest_auth` retries the request on 401 — if you consume the response body between the 401 and the retry, the retry never fires. This is safe with `.get()` which has no body, but failed with `.body(...)` + digest.
**Why it happens:** Digest auth requires the client to resend with an `Authorization` header derived from the server's `WWW-Authenticate` challenge. `diqwest` handles this transparently for the standard flow. [VERIFIED: docs.rs/diqwest] Streaming a response through bytes_stream after auth completes is fine.
**How to avoid:** Use `.send_digest_auth(...)` on a GET (no body). It resolves the auth BEFORE returning, so bytes_stream() consumes only the authed response.
**Warning signs:** First 200 bytes of your stream are a Hikvision 401 XML error body.

### Pitfall 5: XML namespace parse failures with quick-xml serde
**What goes wrong:** Hikvision XML uses `xmlns="http://www.hikvision.com/ver20/XMLSchema"` (and older devices use `ver10`). quick-xml's default serde derive may fail on namespaced elements.
**Why it happens:** [CITED: WebSearch — multiple Hikvision community threads] Different firmware versions emit different namespaces.
**How to avoid:** Strip the xmlns attribute before parse OR use `quick_xml::de::Deserializer::from_str` with `.namespace_resolver(...)` configured. Simplest: strip via `str::replace` — the document is flat and the xmlns carries no semantic load for our use case.
**Warning signs:** `Custom { field: "unknown field" }` during deserialize on valid-looking XML.

### Pitfall 6: Composite UNIQUE index NULL semantics
**What goes wrong:** Our dedup key includes `employee_id` which is NULL for unknown faces (per D-07). In SQLite, **NULL is not equal to NULL in UNIQUE constraints** by default, so two "unknown face at the same timestamp on the same device" events both persist.
**Why it happens:** SQL standard semantics — `NULL != NULL`. SQLite follows the standard for UNIQUE indices (except with explicit DISTINCT partial indices).
**How to avoid:** Either (a) accept the behavior — unknown faces should always log (D-07 explicitly says forensic coverage), or (b) add a second partial UNIQUE on `(face_id, device_id, direction, bucket)` WHERE `employee_id IS NULL` so unknown-face dedup uses `face_id` instead. RECOMMENDED: **option (a)** — unknown events represent forensic value and duplicates within 30s from an unresolved identity are rare but informative.
**Warning signs:** Logs showing dozens of unknown-face events clustered at the exact same timestamp.

### Pitfall 7: Device edit without supervisor restart
**What goes wrong:** Admin PATCHes device IP or password; the existing listener task is still running against the old credentials. It quietly continues or fails silently.
**Why it happens:** DB row was updated, but the tokio task holds its own copy of the config.
**How to avoid:** After every successful PATCH that changes `ip`, `port`, `username`, `encrypted_password`, OR `status`, emit a `DeviceLifecycleEvent::Restart(id)` on the supervisor mpsc channel. The supervisor cancels the old token, waits for the task to join, spawns a fresh one.
**Warning signs:** Recent PATCH, device appears in UI, but no events arrive.

### Pitfall 8: libSQL connection per request vs shared Arc
**What goes wrong:** The supervisor stashes a `libsql::Connection` and reuses it for all writes → libSQL writes are serialized through a single connection and the whole supervisor stalls on a slow COMMIT.
**Why it happens:** `Connection` is not thread-safe in the way `Arc<Database>` is.
**How to avoid:** Follow the Phase 1 pattern: **`state.db.connect()` per operation**. `Database` is Arc-cloned; `Connection` is cheap to open per-call against the embedded replica.
**Warning signs:** Event throughput caps at ~10/s per device even with 4 CPU cores.

---

## Code Examples

Verified patterns. Where feasible, reference existing Phase 1 code that already follows the same structure.

### Encrypting a device password with AES-256-GCM

```rust
// backend/src/devices/crypto.rs
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use base64::{engine::general_purpose::STANDARD, Engine};
use anyhow::{Context, Result};

/// Encrypt a device password. Output format: base64(nonce || ciphertext_with_tag).
/// Nonce is 12 bytes (96-bit), generated randomly per-encrypt.
// [CITED: docs.rs/aes-gcm — canonical pattern]
pub fn encrypt_password(plaintext: &str, key_bytes: &[u8; 32]) -> Result<String> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("encrypt failed: {e}"))?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(STANDARD.encode(&combined))
}

pub fn decrypt_password(encoded: &str, key_bytes: &[u8; 32]) -> Result<String> {
    let combined = STANDARD.decode(encoded).context("base64 decode")?;
    anyhow::ensure!(combined.len() > 12, "ciphertext too short");
    let (nonce_bytes, ciphertext) = combined.split_at(12);

    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decrypt failed (key wrong or tampered): {e}"))?;

    String::from_utf8(plaintext).context("plaintext not utf-8")
}
```

### Outbound ISAPI command with digest auth + timeout

```rust
// backend/src/isapi/client.rs
use reqwest::Client;
use diqwest::WithDigestAuth;
use std::time::Duration;
use anyhow::Result;

pub struct DeviceConnection {
    pub client: Client,
    pub base_url: String,       // e.g. "https://192.168.1.10"
    pub username: String,
    pub password: String,       // plaintext, decrypted on the stack, never logged
}

impl DeviceConnection {
    pub fn new(base_url: &str, username: &str, password: &str, allow_insecure_tls: bool) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .danger_accept_invalid_certs(allow_insecure_tls)
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.to_string(),
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    pub async fn door_open(&self) -> Result<String> {
        // [CITED: Hikvision ISAPI docs — access control door command path]
        let url = format!("{}/ISAPI/AccessControl/RemoteControl/door/1", self.base_url);
        let body = r#"<RemoteControlDoor><cmd>open</cmd></RemoteControlDoor>"#;
        let resp = self.client
            .put(&url)
            .header("Content-Type", "application/xml")
            .body(body)
            .send_digest_auth((self.username.as_str(), self.password.as_str()))
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        anyhow::ensure!(status.is_success(), "device rejected: {status} {text}");
        Ok(text)
    }
}
```

### Handler wrapping the call in a 10s timeout

```rust
// backend/src/devices/handlers.rs
use tokio::time::{timeout, Duration};

pub async fn dispatch_command(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Json(body): Json<CommandRequest>,
) -> Result<Json<CommandResult>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let device = service::get_decrypted(&conn, &device_id, &state.config.device_creds_key).await?;

    let isapi = DeviceConnection::new(&device.base_url, &device.username, &device.password, device.allow_insecure_tls)?;
    let dispatched_at = chrono::Utc::now().timestamp();

    let fut = match body.command {
        Command::DoorOpen => isapi.door_open(),
        Command::Reboot => isapi.reboot(),
        Command::EnrollmentMode => isapi.enrollment_mode(),
    };

    let result = timeout(Duration::from_secs(10), fut).await;
    let completed_at = chrono::Utc::now().timestamp();

    let audit_outcome = match &result {
        Ok(Ok(text)) => AuditOutcome::Ok(text.clone()),
        Ok(Err(e)) => AuditOutcome::Err(e.to_string()),
        Err(_) => AuditOutcome::Timeout,
    };
    service::write_command_audit(&conn, &device_id, &body.command, audit_outcome, dispatched_at, completed_at).await?;

    match result {
        Ok(Ok(text)) => Ok(Json(CommandResult { device_response: text })),
        Ok(Err(e)) => Err(AppError::BadGateway { code: "DEVICE_ERROR", message: e.to_string() }),
        Err(_) => Err(AppError::Timeout { code: "DEVICE_TIMEOUT", message: "device did not respond within 10s".into() }),
    }
}
```

### alertStream consumer with multer + serde parse

```rust
// backend/src/supervisor/task.rs — abbreviated
use futures::StreamExt;
use multer::Multipart;
use quick_xml::de::from_str;

async fn connect_and_stream(cfg: &DeviceConfig, state: &AppState) -> anyhow::Result<()> {
    let url = format!("{}/ISAPI/Event/notification/alertStream", cfg.base_url);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(cfg.allow_insecure_tls)
        .build()?;

    let resp = client
        .get(&url)
        .send_digest_auth((cfg.username.as_str(), cfg.password.as_str()))
        .await?;
    anyhow::ensure!(resp.status().is_success(), "bad status {}", resp.status());

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("missing content-type"))?
        .to_string();
    let boundary = multer::parse_boundary(&content_type)?;

    let stream = resp.bytes_stream();
    // multer expects Result<Bytes, Error>; reqwest::Error implements std::error::Error.
    let mut multipart = Multipart::new(stream, boundary);

    let mut pending_event: Option<AccessEvent> = None;
    while let Some(field) = multipart.next_field().await? {
        let ct = field.content_type().map(|m| m.to_string()).unwrap_or_default();
        let bytes = field.bytes().await?;

        if ct.starts_with("application/xml") || bytes.starts_with(b"<EventNotificationAlert") {
            let xml = std::str::from_utf8(&bytes)?;
            // Strip xmlns to avoid quick-xml namespace quirks (see Pitfall 5)
            let stripped = strip_xmlns(xml);
            let event: EventNotificationAlert = from_str(&stripped)?;
            if event.is_heartbeat() { update_last_seen(&cfg.device_id, state).await?; continue; }

            // Commit any previous pending event without photo before moving on
            if let Some(prev) = pending_event.take() { persist_event(prev, None, state).await?; }
            pending_event = Some(AccessEvent::from(event));
        } else if ct.starts_with("image/jpeg") || bytes.starts_with(b"\xFF\xD8\xFF") {
            if let Some(ev) = pending_event.take() { persist_event(ev, Some(bytes), state).await?; }
            // else: orphan image — log and drop
        }
    }
    // Final flush
    if let Some(ev) = pending_event.take() { persist_event(ev, None, state).await?; }
    Ok(())
}
```

### Dedup-safe INSERT via composite UNIQUE + INSERT OR IGNORE

```rust
// backend/src/events/service.rs — persist helper
pub async fn persist_attendance_event(
    conn: &Connection,
    event: &NewAttendanceEvent,
) -> Result<PersistOutcome, AppError> {
    let bucket = event.captured_at / 30;   // 30-second bucket

    let rows_affected = conn.execute(
        "INSERT OR IGNORE INTO attendance_events \
         (id, employee_id, device_id, direction, captured_at, bucket_30s, is_unknown, face_id, raw_xml, photo_path, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, unixepoch())",
        params![
            event.id.clone(),
            event.employee_id.clone(),   // Option<String> → NULL when unknown
            event.device_id.clone(),
            event.direction.as_str(),
            event.captured_at,
            bucket,
            event.is_unknown as i64,
            event.face_id.clone(),
            event.raw_xml.clone(),
            event.photo_path.clone(),
        ],
    ).await.map_err(|e| AppError::Internal(e.into()))?;

    // [VERIFIED: backend/src/employees/service.rs:242 — execute() returns u64 rows_affected]
    if rows_affected == 0 {
        Ok(PersistOutcome::Deduplicated)
    } else {
        Ok(PersistOutcome::Inserted)
    }
}
```

### Spawning the supervisor from main()

```rust
// backend/src/main.rs — additions (abbreviated)
use tokio_util::sync::CancellationToken;
use tokio::sync::mpsc;

// ...after init_db...
let shutdown = CancellationToken::new();
let (lifecycle_tx, lifecycle_rx) = mpsc::unbounded_channel();

let supervisor = Arc::new(Supervisor::new(state.clone(), shutdown.clone(), lifecycle_tx.clone()));
let supervisor_handle = tokio::spawn({
    let sup = supervisor.clone();
    async move { sup.run(lifecycle_rx).await }
});

// Extend AppState with Arc<dyn LifecycleSignal> so handlers can emit events
let state = AppState { db, config, lifecycle: lifecycle_tx };

// ... build router ...

axum::serve(listener, app)
    .with_graceful_shutdown(async move {
        tokio::signal::ctrl_c().await.ok();
        shutdown.cancel();
    })
    .await?;
supervisor_handle.await.ok();
```

---

## ISAPI Command Paths (cited from public docs)

| Command | Path | Method | Body |
|---------|------|--------|------|
| Door open | `/ISAPI/AccessControl/RemoteControl/door/1` | PUT | `<RemoteControlDoor><cmd>open</cmd></RemoteControlDoor>` |
| Reboot | `/ISAPI/System/reboot` | PUT | empty |
| Enrollment mode (capture face) | `/ISAPI/AccessControl/CaptureFaceData` | POST | JSON: `{ "CaptureInfo": { "captureInfrared": true } }` |

**Confidence: MEDIUM.** [CITED: Hikvision ISAPI Developer Guide indexed by community. Verify exact path on real device during Wave 0 of 02-01.] The enrollment command signature varies by firmware — DS-K1T341 vs DS-K1T342 may differ. Plan includes a "verify on hardware" step before wiring the enum into the handler.

---

## alertStream Multipart Format (documented behavior)

**Endpoint:** `GET /ISAPI/Event/notification/alertStream` over HTTPS (some devices over HTTP).
**Auth:** HTTP Digest (RFC 2617). [CITED: pyHik uses HTTPDigestAuth; fuqiangZ/hikvision-isapi-go uses digest; docs.rs/diqwest handles this flow.]

**Response headers (representative):**
```
HTTP/1.1 200 OK
Content-Type: multipart/mixed; boundary=MIME_boundary
Connection: keep-alive
Transfer-Encoding: chunked
```
[CITED: Hikvision public docs + deepwiki/fuqiangZ/hikvision-isapi-go/4.5 — some devices use `multipart/x-mixed-replace`. Both follow the same RFC 2046 boundary grammar. Verify actual header against real device in Wave 0 — STATE.md BLOCKER.]

**Body structure (canonical, verified in multiple open-source projects):**
```
--MIME_boundary\r\n
Content-Type: application/xml\r\n
Content-Length: 1234\r\n
\r\n
<EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
  <ipAddress>192.168.1.10</ipAddress>
  <portNo>80</portNo>
  <protocol>HTTP</protocol>
  <macAddress>XX:XX:XX:XX:XX:XX</macAddress>
  <channelID>1</channelID>
  <dateTime>2026-04-19T12:34:56+00:00</dateTime>
  <activePostCount>1</activePostCount>
  <eventType>AccessControllerEvent</eventType>
  <eventState>active</eventState>
  <eventDescription>Access Controller Event</eventDescription>
  <AccessControllerEvent>
    <deviceName>Device</deviceName>
    <majorEventType>5</majorEventType>
    <subEventType>75</subEventType>
    <employeeNoString>EMP001</employeeNoString>
    <name>John Doe</name>
    <cardNo>0</cardNo>
    <cardType>1</cardType>
    <currentVerifyMode>face</currentVerifyMode>
    <attendanceStatus>checkIn</attendanceStatus>
    <faceID>42</faceID>
    <pictureURL>/ISAPI/Intelligent/FDLib/pictureUpload?...</pictureURL>
  </AccessControllerEvent>
</EventNotificationAlert>
\r\n
--MIME_boundary\r\n
Content-Type: image/jpeg\r\n
Content-Length: 14567\r\n
Content-Disposition: form-data; name="image.jpg"; filename="image.jpg"\r\n
\r\n
<JPEG binary>
\r\n
--MIME_boundary\r\n
...next event...
```
[CITED: Shaykhnazar/hikvision-isapi Laravel README L502-L515 for structural shape; deepwiki/fuqiangZ/hikvision-isapi-go/4.5 for multipart framing and image part; ipcamtalk threads for `major:5 / minor:75` face check-in combination.]

**Heartbeat:** [CITED: pyHik hikvision.py L684+, WebSearch Hikvision docs] Devices send periodic `<EventNotificationAlert>` with `<eventType>videoloss</eventType>` + `<eventState>inactive</eventState>` (older firmware) OR an explicit `<eventType>Heartbeat</eventType>` part. Interval varies, typically 30-60s. Use this as the online/offline signal (see next section).

**Assumption** [ASSUMED]: `attendanceStatus` values follow the set `{"checkIn", "checkOut", "breakIn", "breakOut", "overtimeIn", "overTimeOut", "undefined"}`. CONFIRMED via WebSearch, but must be verified against real DS-K1T341/342 traffic since firmware may differ. The mapping of attendanceStatus → our `direction` enum (entry/exit) needs user confirmation in 02-02's Wave 0.

**Assumption** [ASSUMED]: `faceID` in the XML is a string matching the `face_id` column in our `device_face_mappings` table. In reality Hikvision also exposes `employeeNoString` directly, and our face_id mapping may not even be needed if `employeeNoString` is always present and is already the employee code. Research this in Wave 0 — **if `employeeNoString` === our `employees.employee_code`, we can short-circuit the face_id mapping for authenticated events and keep face_id mapping for photo-only unknown fallbacks.**

---

## Online / Offline Detection

Three candidate mechanisms evaluated:

| Mechanism | Pros | Cons | Verdict |
|-----------|------|------|---------|
| TCP socket state | Immediate on hard drop | Can't distinguish idle-keep-alive from silent network partition; OS-level checks are platform-specific | REJECT as primary |
| Polling `/ISAPI/System/status` | Simple; truthful answer from device | Adds N parallel HTTP requests periodically; doesn't reflect alertStream channel health | Use as SECONDARY confirmation |
| `last_event_at` + heartbeat watchdog | Single source of truth (same channel we care about); zero extra network calls | Needs a threshold (e.g., "offline if no heartbeat for 90s") | **ADOPT as primary** |

**Recommended design:**
- Table `devices` has `last_seen_at INTEGER` column, updated on any XML part arrival (event OR heartbeat).
- A lightweight `watchdog` task runs every 10 seconds, scanning devices WHERE `status='active' AND last_seen_at < now - 90 AND connection_state='online'`, flips their `connection_state` to `'offline'`.
- The listener task updates `connection_state='online'` on first successful stream byte.
- DEV-02 "view real-time connection status" reads `connection_state` + `last_seen_at` directly from `devices`.

**Expose via API:** `GET /api/v1/devices` includes `connection_state` and `last_seen_at`. DEV-02 and Phase 4's dashboard consume it without changes.

---

## Runtime State Inventory

**Not applicable** — Phase 2 is additive (new tables, new routes, new tasks). There is no runtime state from Phase 1 that Phase 2 will rename or migrate. The supervisor is started fresh every boot from the `devices` table; there is no in-memory state to transfer. Verified:

| Category | Status |
|----------|--------|
| Stored data | None — verified by inspection of Phase 1 migrations (001, 002). No existing rows to migrate. |
| Live service config | None — Phase 1 does not register anything with external services. |
| OS-registered state | None — greenfield project; no existing Task Scheduler / systemd / pm2 registrations. |
| Secrets/env vars | NEW env var `DEVICE_CREDS_KEY` required. Must be added to `.env.example` and to `Config::from_env`. No existing key renames. |
| Build artifacts | None — `cargo build` produces only `target/`; no stale egg-info or compiled binaries carry stale names. |

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain 1.77+ | All backend work | ✓ assumed (Phase 1 compiled successfully) | — | — |
| libSQL-compatible SQLite | DB layer | ✓ (bundled with libsql crate) | 0.9.30 | — |
| Hikvision DS-K1T341 or DS-K1T342 device | alertStream Wave 0 traffic capture | **✗ — not confirmed in environment** | — | Option A: use a vendor demo device remotely. Option B: mock the stream from saved pcap (`nc`-replay). Option C: defer 02-02 Wave 0 until hardware is in hand. **Raise this in 02-02 plan-check.** |
| mitmproxy or Wireshark | Reverse-engineer actual XML during Wave 0 | Unknown — user confirmation needed | — | `curl --digest -k` piped to `tee` gives 90% of what mitmproxy does for plaintext HTTP; for HTTPS use a device on a test VLAN with `--insecure` |
| Docker + docker-compose | Future packaging (Phase 6) | — | — | Not needed for Phase 2 |

**Blocking missing dependency:** access to a real Hikvision face recognition terminal for traffic capture. Planner should flag this as a Wave 0 prerequisite in 02-02; if hardware is absent, the phase can still ship the plumbing (supervisor, crypto, command dispatch) and park the parser behind a feature flag that runs against saved fixtures.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` + `tokio::test` + `axum-test 16` + `wiremock 0.6.5` |
| Config file | `backend/Cargo.toml` [dev-dependencies] (extend existing) |
| Quick run command | `cargo test --test device_tests` (per-file), `cargo test -p cronometrix-api devices::` (per-module) |
| Full suite command | `cargo test --all` in `backend/` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| DEV-01 | POST /devices creates record with encrypted password | integration | `cargo test --test device_tests create_device_encrypts_password` | ❌ Wave 0 |
| DEV-01 | Password never returned in any response | integration | `cargo test --test device_tests create_and_get_never_returns_password` | ❌ Wave 0 |
| DEV-01 | Duplicate IP returns 409 | integration | `cargo test --test device_tests create_duplicate_ip_conflict` | ❌ Wave 0 |
| DEV-02 | GET /devices returns connection_state + last_seen_at | integration | `cargo test --test device_tests list_devices_exposes_connection_state` | ❌ Wave 0 |
| DEV-02 | Watchdog marks device offline after 90s of silence | unit | `cargo test -p cronometrix-api supervisor::status::watchdog_flips_offline` | ❌ Wave 0 |
| DEV-03 | POST /commands dispatches and audits | integration (wiremock) | `cargo test --test device_tests dispatch_door_open_writes_audit` | ❌ Wave 0 |
| DEV-03 | Command timeout (>10s) returns 504 | integration (wiremock w/ delay) | `cargo test --test device_tests dispatch_timeout_returns_504` | ❌ Wave 0 |
| DEV-03 | Non-admin role gets 403 | integration | `cargo test --test device_tests dispatch_viewer_forbidden` | ❌ Wave 0 |
| DEV-04 | PATCH device restarts listener task | integration | `cargo test --test device_tests patch_ip_emits_restart_event` | ❌ Wave 0 |
| DEV-04 | DELETE (soft-delete) stops listener task | integration | `cargo test --test device_tests deactivate_stops_listener` | ❌ Wave 0 |
| EVT-01 | Supervisor spawns one task per active device on boot | unit | `cargo test -p cronometrix-api supervisor::bootstrap::spawns_one_task_per_device` | ❌ Wave 0 |
| EVT-02 | Listener reconnects with exponential backoff on drop | integration (tokio TCP fixture) | `cargo test --test listener_tests reconnects_with_backoff` | ❌ Wave 0 |
| EVT-03 | Second event within 30s window is deduped (rows_affected=0) | unit | `cargo test -p cronometrix-api events::service::persist_dedup_within_30s` | ❌ Wave 0 |
| EVT-03 | Same employee across devices in 30s: BOTH persist | unit | `cargo test -p cronometrix-api events::service::persist_cross_device_within_30s` | ❌ Wave 0 |
| EVT-03 | Bucket rolls over at 30s: both persist | unit | `cargo test -p cronometrix-api events::service::persist_adjacent_buckets` | ❌ Wave 0 |
| EVT-04 | captured_at stored as UTC epoch integer | unit | `cargo test -p cronometrix-api events::service::persist_epoch_is_utc_integer` | ❌ Wave 0 |
| EVT-04 | raw_xml is retained verbatim | unit | `cargo test -p cronometrix-api events::service::persist_raw_xml_round_trip` | ❌ Wave 0 |
| — | aes-gcm round-trip | unit | `cargo test -p cronometrix-api devices::crypto::encrypt_then_decrypt` | ❌ Wave 0 |
| — | aes-gcm tamper detection | unit | `cargo test -p cronometrix-api devices::crypto::tampered_ciphertext_fails` | ❌ Wave 0 |
| — | multer parses Hikvision multipart fixture | unit | `cargo test -p cronometrix-api isapi::parser::parses_fixture_sample` | ❌ Wave 0 |
| — | quick-xml deserializes AccessControllerEvent | unit | `cargo test -p cronometrix-api isapi::events::deserialize_fixture` | ❌ Wave 0 |
| — | Heartbeat XML is detected and NOT persisted | unit | `cargo test -p cronometrix-api isapi::events::heartbeat_skipped` | ❌ Wave 0 |

### Test Doubles for Hikvision Device

Three approaches, in decreasing order of fidelity:

1. **Saved pcap / raw bytes fixture** (HIGHEST fidelity). Capture once from a real DS-K1T341/342 via `curl --digest -N -k -u admin:X https://IP/ISAPI/Event/notification/alertStream > fixture.bin` and commit to `backend/tests/fixtures/alertstream_*.bin`. Tests load the bytes, feed them to our parser as a `tokio::io::BufReader`. Best for parser correctness — proves real bytes parse.
2. **Custom tokio TCP fixture server** (MEDIUM fidelity). A helper in `backend/tests/common/` that binds an ephemeral port, speaks HTTP/1.1 manually, serves digest 401 then 200 with a hard-coded multipart body from a string. Pairs with integration tests of the supervisor's reconnect behavior.
3. **wiremock** (LOW fidelity for alertStream, HIGH for commands). `wiremock::MockServer` + `ResponseTemplate::new(200).set_body_raw(..., "multipart/mixed; boundary=X")` works for single-shot response but does NOT simulate a long-lived stream. Use wiremock for DEV-03 command dispatch, 504 timeouts, and 401 auth error paths. For alertStream integration tests, fall back to option 2.

### Sampling Rate

- **Per task commit:** `cargo test --test device_tests` OR `cargo test --test listener_tests` depending on what changed.
- **Per wave merge:** `cargo test --all` in `backend/`.
- **Phase gate:** `cargo test --all` green, plus a one-shot **hardware smoke test** (run the API against a real device, register it, observe one live event round-trip) logged in the verify step.

### Wave 0 Gaps

Wave 0 of 02-01 must ship:

- [ ] `backend/tests/device_tests.rs` — integration tests scaffold (matches existing `employee_tests.rs` shape)
- [ ] `backend/tests/listener_tests.rs` — integration tests scaffold using custom tokio TCP fixture
- [ ] `backend/tests/common/mod.rs` — add helpers: `spawn_mock_hikvision(xml_body: &str, boundary: &str) -> SocketAddr`, `register_test_device(app, ip) -> String`
- [ ] `backend/tests/fixtures/alertstream_k1t341.bin` — real captured traffic (MUST be present before parser implementation; Wave 0 of 02-02)
- [ ] `backend/src/devices/crypto.rs` — implementation + unit tests in same file
- [ ] `backend/src/isapi/events.rs` — serde structs + unit tests in same file
- [ ] `backend/src/events/service.rs` with `persist_attendance_event` + dedup unit tests using `backend/tests/common/test_db()`

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (carried from Phase 1) | JWT via `jsonwebtoken`; no new auth surface in Phase 2 |
| V3 Session Management | yes (carried) | Phase 1 refresh cookie; no new session state in Phase 2 |
| V4 Access Control | yes | `require_admin` for device mutations + command dispatch; `require_auth` (viewer-readable) for GET /events per D-15 |
| V5 Input Validation | yes | `validator` derive on `CreateDeviceRequest`, `UpdateDeviceRequest`, `CommandRequest`; validate IP is parseable, port 1..=65535, command in enum |
| V6 Cryptography | yes | `aes-gcm` (NIST-approved AEAD); 256-bit key; 96-bit random nonce per encrypt; no hand-rolled crypto |
| V7 Error Handling & Logging | yes | NEVER log plaintext passwords; `tracing::debug!` spans exclude credential fields; `AppError::Internal(e)` masks raw errors from client |
| V9 Communications | yes | TLS to Hikvision devices preferred; `allow_insecure_tls` flag is PER-DEVICE and defaults false |
| V10 Malicious Code | yes | External crate surface grew — `aes-gcm`, `diqwest`, `multer`, `quick-xml` all from reputable orgs (RustCrypto, popular community) |

### Known Threat Patterns for this Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Credential theft from DB dump | Information Disclosure | AES-256-GCM at rest (D-01); key in separate env (D-02) |
| Tampered ciphertext (swap bytes to force decrypt to attacker-controlled password) | Tampering | AEAD tag verification in `aes-gcm.decrypt()` — built-in constant-time check |
| Nonce reuse leaking keystream | Information Disclosure | OsRng-generated 96-bit nonce per encrypt; stored alongside ciphertext |
| Key rotation gap | Repudiation | Deferred to a future CLI (D-04). Document in deployment runbook that key loss → re-register every device. |
| Log-based password leak | Information Disclosure | `password_hash` / `encrypted_password` fields must NEVER appear in `tracing::debug!`. Add `#[serde(skip_serializing)]` / redact in `Debug` impl. |
| TLS MITM on device channel | Tampering | TLS preferred; `allow_insecure_tls` is opt-in per device with an audit log entry on first use. |
| Stolen JWT used to open doors remotely | Elevation of Privilege | Admin-only command endpoint (D-10); audit log per dispatch (D-11); short access token TTL (Phase 1 D-06) |
| SQL injection in event filter | Tampering | Positional params only (existing Phase 1 pattern); no string concat for `employee_id`/`device_id` filters |
| Resource exhaustion via malformed multipart | DoS | `multer::Constraints::new().size_limit(...)` caps per-part size; listener task has bounded buffer; a misbehaving device stays isolated to its own task |
| Replay attack (re-posting same event) | Repudiation | Dedup via composite UNIQUE + 30s bucket (D-05/D-06) |
| Disk exhaustion via JPEG flood | DoS | No retention in Phase 2 (D-14) accepted risk; Phase 2 adds per-day subdirectory structure that a future retention job can prune |

### Credential Handling Rules (enforced by code review)

1. `DEVICE_CREDS_KEY` is a `[u8; 32]` held in `Arc<Config>`. It is **never** cloned into logs, spans, or error messages.
2. The `Device` struct used internally has a `password_encrypted: String` field. A separate `DeviceWithPlaintext` struct (used only in the crypto/service boundary) carries plaintext and MUST NOT derive `Debug` / `Serialize` — or it implements them by redacting.
3. All API response structs include `#[serde(skip_serializing)]` on any plaintext password (defense in depth; per D-03 the field shouldn't even be on response DTOs).
4. Audit log entries for `devices` must scrub the `password_encrypted` field from `new_data` JSON. Simplest: omit the column from the `json_object(...)` call in the trigger (see pattern in `002_audit_triggers.sql`).

---

## Open Questions

1. **Exact XML schema for DS-K1T341 vs DS-K1T342 attendance events**
   - **What we know:** `EventNotificationAlert` is the outer element; `AccessControllerEvent` carries access-control payload; `majorEventType=5 subEventType=75` is documented as a face check-in combination.
   - **What's unclear:** Which sub-events map to "entry" vs "exit" direction? Does firmware 3.2.x use different field names than 3.3.x? Does the `direction` need to be inferred from `subEventType` or from the device's configured role?
   - **Recommendation:** Wave 0 of 02-02 captures a real fixture. Store it in `backend/tests/fixtures/`. Planner must add a task to verify this BEFORE the parser module's public API freezes.

2. **TLS certificate story on target devices**
   - **What we know:** Community reports universally say Hikvision devices ship self-signed certs.
   - **What's unclear:** Do target clients want us to deploy with cert pinning, or is an explicit `allow_insecure_tls` per-device flag acceptable?
   - **Recommendation:** Default to requiring valid certs; document the per-device `allow_insecure_tls` flag; revisit if field feedback demands pinning.

3. **Does `employeeNoString` === our `employees.employee_code`?**
   - **What we know:** Hikvision devices store employee number as an arbitrary string assigned at enrollment (Phase 7).
   - **What's unclear:** If Phase 7 enrollment writes our `employees.employee_code` to the device's `employeeNoString`, Phase 2 can bypass the `device_face_mappings` table entirely for identified events. The mapping table may only be needed for unknown/backfill scenarios.
   - **Recommendation:** Phase 2 builds the mapping table as designed (D-08) and writes `employeeNoString` lookups that fall through to `face_id` lookups as a belt-and-braces strategy. Cleanup deferred to Phase 7.

4. **Is `multer` 3.1.0 actually able to parse `multipart/mixed` bodies from Hikvision?**
   - **What we know:** `multer` is primarily designed for `multipart/form-data`. The underlying RFC 2046 grammar is shared across all `multipart/*` types, so boundary parsing should work.
   - **What's unclear:** Does `multer::Multipart::next_field()` tolerate parts without a `Content-Disposition` header (Hikvision XML parts may omit it)?
   - **Recommendation:** Write the `isapi::parser` module against a captured fixture in Wave 0. If `multer` chokes, fall back to the `pyHik`-style `<EventNotificationAlert>…</EventNotificationAlert>` line scanner (implementation is ~40 LOC).

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `multipart-stream 0.1.2` (http 0.2) | `multer 3.1.0` (http 1.x) | reqwest 0.13 released (2024, http 1.x) | Must not pin `multipart-stream`; ecosystem has moved on |
| `backoff 0.4.0` (2021-12) | `backon 1.6.0` (2025-10) | Crate maintenance transfer | If using a retry crate, pick `backon` or `tokio-retry2` |
| `bcrypt` | `argon2` (Argon2id) | NIST / OWASP recommendation | Already locked in Phase 1; continue |
| `ring`/OpenSSL for AES-GCM | `aes-gcm` (RustCrypto) | Pure-Rust mainstreaming | No C dependency — easier Docker build |
| Global `Mutex<HashMap>` for device state | DB-as-source-of-truth + supervisor handle registry | CLAUDE.md directive | Avoid in-memory authoritative state |
| `axum-jwt-auth` wrapper | Hand-built extractor from `jsonwebtoken` (already in Phase 1) | Flexibility over convenience | Continue Phase 1 pattern |

**Deprecated/outdated:**
- `multipart-stream 0.1.2` (2021-05) — do NOT use; pins `http 0.2`.
- `backoff 0.4.0` (2021-12) — superseded by `backon`.
- `serde-xml-rs` — 10× slower than `quick-xml` per CLAUDE.md.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `attendanceStatus` values are `{"checkIn", "checkOut", "breakIn", "breakOut", "overtimeIn", "overTimeOut", "undefined"}` | alertStream Multipart Format | If values differ, `direction` enum mapping is wrong; events may land with wrong direction or be dropped. Mitigate via Wave 0 fixture capture. |
| A2 | `faceID` in XML matches `device_face_mappings.face_id` string | alertStream Multipart Format | If device emits numeric id but we stored string or vice versa, lookups silently miss. Solve by making `face_id` column TEXT and storing verbatim what the device emits. |
| A3 | Heartbeats use `<eventType>videoloss</eventType><eventState>inactive</eventState>` OR `<eventType>Heartbeat</eventType>` | Architecture Patterns → Pattern 2 | If real devices emit a third heartbeat variant, online/offline watchdog false-positives. Log all unrecognized eventTypes at INFO level during Wave 0 to discover variants. |
| A4 | `majorEventType=5, subEventType=75` = face check-in on DS-K1T341/342 | alertStream Multipart Format | If the combination differs, direction inference is wrong. Wave 0 fixture clears this up. |
| A5 | Door-open ISAPI path is `PUT /ISAPI/AccessControl/RemoteControl/door/1` | ISAPI Command Paths | If path is model-specific, door-open command fails. Verify on hardware before wiring handler. |
| A6 | Enrollment-mode ISAPI path is `POST /ISAPI/AccessControl/CaptureFaceData` | ISAPI Command Paths | Same as A5; also Phase 7 coupling. |
| A7 | `multer` 3.1.0 handles `multipart/mixed` bodies with parts missing `Content-Disposition` | Standard Stack | If it rejects, fall back to line scanner. Low cost to verify in Wave 0. |
| A8 | Hikvision devices return `Content-Type: multipart/mixed; boundary=X` (not `multipart/x-mixed-replace`) | alertStream Multipart Format | Both paths via `multer::parse_boundary` should work, but code that branches on exact MIME type may miss a variant. Handle both. |
| A9 | libSQL preserves SQLite's behavior that `INSERT OR IGNORE` returns `rows_affected = 0` on unique-constraint-hit | Pattern 3, dedup | If libSQL diverges, dedup detection breaks silently (event looks inserted even though it wasn't). Covered by unit test `persist_dedup_within_30s`. |
| A10 | `xmlns` stripping is a safe preprocessing step (no semantic loss) | Pitfall 5 | Hikvision's xmlns is metadata only; no elements depend on namespace resolution. If future firmware adds namespaced custom elements, this breaks. Flag with a Wave-0-verifiable unit test. |

---

## Sources

### Primary (HIGH confidence)
- [backend/src/main.rs, errors.rs, common.rs, auth/*, employees/service.rs] — Phase 1 patterns verified in-repo
- [docs.rs/aes-gcm/0.10.3](https://docs.rs/aes-gcm/) — AES-256-GCM canonical API
- [docs.rs/diqwest/3.2.0](https://docs.rs/diqwest/) — `WithDigestAuth` trait, `.send_digest_auth((user, pass))`
- [docs.rs/quick-xml/0.39](https://docs.rs/quick-xml/) — serde deserialize pattern, feature flags
- [docs.rs/reqwest/0.13/reqwest/struct.Response.html](https://docs.rs/reqwest/latest/reqwest/struct.Response.html) — `.bytes_stream()` API
- [docs.rs/multer/3.1.0](https://docs.rs/multer/) — `Multipart::new`, `next_field`, `parse_boundary`
- [crates.io] — version verifications (diqwest 3.2.0, aes-gcm 0.10.3, quick-xml 0.39.2, reqwest 0.13.2, multer 3.1.0, backon 1.6.0, tokio-util 0.7.18, libsql 0.9.30, wiremock 0.6.5)
- [tokio.rs/tokio/topics/shutdown](https://tokio.rs/tokio/topics/shutdown) — CancellationToken, graceful shutdown

### Secondary (MEDIUM confidence)
- [tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-C8398309-7417-4540-AF4F-4DA909E766D2.html](https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-C8398309-7417-4540-AF4F-4DA909E766D2.html) — alertStream endpoint (login-walled; cited via community reverse-engineering)
- [github.com/mezz64/pyHik — hikvision.py](https://github.com/mezz64/pyHik/blob/master/pyhik/hikvision.py) — Python reference implementation of line-scan parser + digest auth, verified lines 650-720
- [github.com/scottlamb/multipart-stream-rs](https://github.com/scottlamb/multipart-stream-rs) — Rust multipart/x-mixed-replace parser (confirms pattern, but NOT usable due to http 0.2 dep)
- [deepwiki/fuqiangZ/hikvision-isapi-go/4.5-multipart-stream-parsing](https://deepwiki.com/fuqiangZ/hikvision-isapi-go/4.5-multipart-stream-parsing) — multipart framing + image part layout
- [github.com/Shaykhnazar/hikvision-isapi — README](https://github.com/Shaykhnazar/hikvision-isapi) — example AccessControllerEvent XML shape (lines 502-515)
- [github.com/fuqiangZ/hikvision-isapi-go](https://github.com/fuqiangZ/hikvision-isapi-go) — Go reference for ANPR-style multipart parsing
- [Hikvision ISAPI Event Listening PDF](https://www.hikvisioneurope.com/eu/portal/portal/Technology%20Partner%20Program/03-How%20to/How%20to%20get%20real-time%20event%20in%20listening%20mode.pdf) — multipart event format (referenced in CLAUDE.md)

### Tertiary (LOW confidence — needs Wave 0 verification)
- [ipcamtalk.com community threads] — real-world `major:5 / minor:75` face check-in examples; `attendanceStatus` field values. Community-reverse-engineered; not vendor-authoritative.
- [scribd.com/document/669288741/ISAPI-Developer-Guide-Access-Control-Face-Recognition-Terminals-2022-07-01] — Unofficial mirror of Hikvision's 2022 dev guide; may be outdated vs current firmware.

---

## Metadata

**Confidence breakdown:**
- Standard stack (Rust crates + versions): HIGH — verified against crates.io 2026-04-19; all deps compile-compatible.
- Architecture (supervisor topology, event persistence, command dispatch): HIGH — extends Phase 1 patterns that are already running in the repo.
- alertStream XML schema: MEDIUM — triangulated from pyHik source, Shaykhnazar Laravel package, fuqiangZ Go deepwiki, community forum reports. NOT yet pinned against real DS-K1T341/342 bytes — that's Wave 0 of 02-02.
- Multipart parsing (multer vs hand-roll): MEDIUM — multer is http-1.x compatible but unproven for Hikvision-specific framing; fallback plan documented.
- Security (AES-256-GCM, digest auth, audit coverage): HIGH — canonical patterns from well-audited crates.
- Pitfalls: HIGH — each grounded in documented behavior of a verified source.

**Research date:** 2026-04-19
**Valid until:** 2026-05-19 (30 days — Rust ecosystem stable; Hikvision firmware drift possible but slow)
