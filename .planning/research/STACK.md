# Stack Research

**Domain:** Biometric time & attendance system — on-premise + cloud sync hybrid
**Researched:** 2026-04-11
**Confidence:** HIGH (core Rust/React stack verified against current docs; ISAPI integration patterns MEDIUM due to closed proprietary protocol)

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 1.77+ (stable) | Backend runtime | Memory-safe, zero-cost async, ideal for high-throughput webhook processing and time calculation logic |
| Axum | 0.8.x (0.8.8 current) | HTTP server framework | Tokio-native, Tower middleware ecosystem, ergonomic extractors; announced 0.8.0 in Jan 2025 with breaking changes from 0.7 |
| Tokio | 1.51.x | Async runtime | The de-facto standard, used by Axum/libSQL/reqwest; no realistic alternative for this stack |
| Next.js | 15.x | Frontend framework | App Router stable, SSR/SSG for admin dashboard, TypeScript-first; React 19 compatible |
| React | 19.x | UI library | Peer of Next.js 15; concurrent features (Suspense, Server Components) beneficial for real-time dashboard |
| TypeScript | 5.x | Frontend type safety | Required for TanStack Table, shadcn/ui, and Zod schema sharing |

### Backend — Rust Crate Ecosystem

| Crate | Version | Purpose | Why This One |
|-------|---------|---------|--------------|
| `axum` | 0.8.8 | HTTP server, routing, extractors | Tokio-native, best-in-class ergonomics; Tower integration gives CORS, tracing, compression for free |
| `tokio` | 1.51.1 | Async runtime | Required by axum; `features = ["full"]` for dev, trim for production |
| `tower-http` | 0.6.x | Middleware (CORS, tracing, compression, timeout) | Standard Tower middleware collection; use `features = ["cors", "trace", "compression-gzip", "timeout"]` |
| `libsql` | latest | SQLite local + Turso cloud sync | Official SDK from Turso; `Builder::new_remote_replica()` for embedded replica mode |
| `reqwest` | 0.13.2 | ISAPI HTTP client (outbound) | Most popular Rust HTTP client; async, TLS, connection pooling |
| `diqwest` | latest | Digest auth for ISAPI | Extends reqwest with RFC 2617 digest auth flow — Hikvision devices require digest auth |
| `quick-xml` | 0.39.x | XML parsing for ISAPI events | 10x faster than serde-xml-rs; serde integration via `features = ["serialize"]` |
| `serde` | 1.0.x | Serialization/deserialization | Universal; `features = ["derive"]` |
| `serde_json` | 1.0.x | JSON API request/response bodies | Required by axum's Json extractor |
| `jsonwebtoken` | 10.3.0 | JWT creation and validation | Standard Rust JWT library; supports HS256/RS256; use for auth tokens |
| `argon2` (RustCrypto) | 0.5.x | Password hashing | OWASP-recommended over bcrypt; `argon2id` variant; from `RustCrypto/password-hashes` |
| `chrono` | 0.4.42 | Date/time arithmetic | Attendance time calculations (tolerance windows, shift detection, lunch deduction) |
| `uuid` | 1.x | ID generation | UUID v4 for records; audit log IDs |
| `tracing` | 0.1.x | Structured logging | Tokio ecosystem standard; pairs with `tracing-subscriber` |
| `tracing-subscriber` | 0.3.x | Log output formatting | JSON output for production, pretty for dev |
| `anyhow` | 1.x | Error handling in application code | Ergonomic error propagation; use `thiserror` for library boundaries |
| `thiserror` | 2.x | Typed errors for axum handlers | Derive `IntoResponse` on custom error types |
| `validator` | 0.19.x | Request payload validation | Derive-based validation macros; combine with serde |
| `dotenv` / `dotenvy` | latest | Environment config | `dotenvy` is the maintained fork; for TURSO_DATABASE_URL, TURSO_AUTH_TOKEN |

### Frontend — React/Next.js Ecosystem

| Library | Version | Purpose | Why This One |
|---------|---------|---------|--------------|
| `@tanstack/react-table` | v8.x | Data tables (employee list, timesheet, audit log) | Headless — no UI lock-in; virtualizes large datasets; server-side sort/filter/paginate built-in |
| `@tanstack/react-query` | v5.x | Server state management, cache, background refetch | Industry standard; SSR hydration support for Next.js App Router; replaces ad-hoc fetch + useState |
| `shadcn/ui` | latest (copy-paste) | Component library (forms, dialogs, dropdowns) | You own the code — no upgrade breaking; built on Radix UI + Tailwind; pairs perfectly with TanStack Table |
| `tailwindcss` | 4.x | Utility CSS | Required by shadcn/ui; v4 released early 2025 with Vite-native engine |
| `react-hook-form` | 7.x | Form state management | Zero re-renders on input; uncontrolled components; integrates with Zod via `@hookform/resolvers` |
| `zod` | 3.x | Schema validation + TypeScript types | Single schema definition for form validation AND type inference; share with backend via contract |
| `react-big-calendar` | 1.x | Holiday/shift calendar UI | Free, MIT, gcal-style views; month/week/day/agenda; drag-and-drop for holiday config |
| `recharts` | 3.x | Dashboard charts (KPIs, attendance trends) | v3 released mid-2025 with improved accessibility and TypeScript; built on D3; MIT |
| `@tanstack/react-virtual` | v3.x | Virtualized lists (large employee/event tables) | Pairs with TanStack Table; renders only visible rows for 10k+ record tables |
| `date-fns` | 3.x | Date formatting/arithmetic | Tree-shakeable; use as react-big-calendar localizer; consistent with backend chrono semantics |
| `axios` | latest | HTTP client to Rust backend | OR use native fetch — for CRUD calls, TanStack Query manages the cache layer |
| `next-auth` / `jose` | latest | Session management | `jose` for JWT verification in Next.js middleware; or `next-auth` v5 if SSO is needed later |
| `lucide-react` | latest | Icon set | Default for shadcn/ui; consistent, tree-shakeable |
| `xlsx` | latest | Excel export (pre-payroll reports) | Client-side Excel generation; use `SheetJS/xlsx` community edition |
| `jspdf` + `jspdf-autotable` | latest | PDF export (reports, audit trails) | Client-side PDF; autotable for tabular data |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo-watch` | Auto-recompile Rust on save | `cargo watch -x run` |
| `sqlx-cli` | Migration management | Use even with libSQL — migration files stay compatible |
| `cargo-nextest` | Faster test runner for Rust | Drop-in replacement for `cargo test` |
| `Biome` | Linter + formatter for TypeScript | Replaces ESLint + Prettier in one tool; much faster |
| `Vitest` | Frontend unit/integration tests | Native ESM, Vite-powered; pairs with React Testing Library |
| `Bruno` / `Postman` | ISAPI endpoint testing | Bruno preferred (local, git-friendly) |

---

## Installation

### Rust Backend

```toml
# Cargo.toml
[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip", "timeout"] }
libsql = "0.6"                          # verify latest on crates.io
reqwest = { version = "0.13", features = ["json", "rustls-tls"] }
diqwest = "1"                            # digest auth for ISAPI
quick-xml = { version = "0.39", features = ["serialize"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jsonwebtoken = "10"
argon2 = "0.5"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
anyhow = "1"
thiserror = "2"
dotenvy = "0.15"
```

### Frontend

```bash
# Scaffold
npx create-next-app@latest cronometrix-ui --typescript --tailwind --app --src-dir

# Core
npm install @tanstack/react-table @tanstack/react-query @tanstack/react-virtual
npm install react-hook-form zod @hookform/resolvers
npm install react-big-calendar date-fns recharts
npm install xlsx jspdf jspdf-autotable
npm install lucide-react

# shadcn/ui (interactive installer)
npx shadcn@latest init

# Dev dependencies
npm install -D vitest @testing-library/react @testing-library/user-event
npm install -D @biomejs/biome
```

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Axum version | 0.8.x | 0.7.x | 0.7.x is EOL; 0.8.0 released Jan 2025 with breaking changes — migrate forward, not backward |
| ORM | raw libSQL queries | SeaORM, sqlx | libSQL crate IS the right abstraction layer for Turso sync; adding SeaORM creates an unnecessary wrapper that fights the embedded replica API |
| XML parsing | `quick-xml` | `serde-xml-rs` | `quick-xml` is 10x faster AND supports serde derives; `serde-xml-rs` is slower with no advantage |
| Password hashing | `argon2` (RustCrypto) | `bcrypt` | OWASP recommends argon2id over bcrypt; bcrypt has 72-byte password limit; argon2 is PHC winner |
| Form library | `react-hook-form` | Formik | RHF is uncontrolled (zero re-renders); Formik is controlled and slow on large forms like timesheet editor |
| Calendar | `react-big-calendar` | FullCalendar premium | react-big-calendar is fully MIT; FullCalendar premium features require paid license (overkill for holiday config use case) |
| Charts | Recharts | Tremor | Tremor is built on Recharts anyway; direct Recharts gives more control for custom attendance trend visualizations |
| State management | TanStack Query | Redux Toolkit | RTK is for client-side state; attendance data is server state — TanStack Query is the correct tool |
| Auth JWT | `jsonwebtoken` crate | `axum-jwt-auth` | `axum-jwt-auth` wraps `jsonwebtoken`; building your own extractor from `jsonwebtoken` gives RBAC flexibility without opinionated wrapper constraints |
| HTTP client | `reqwest` | `hyper` directly | Reqwest wraps hyper with ergonomic API; direct hyper only needed if you need extreme raw control |
| Digest auth | `diqwest` | manual implementation | `diqwest` correctly handles the challenge-response flow for digest auth (401 → parse WWW-Authenticate → retry with computed MD5) |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `diesel` ORM | Synchronous only; fights Tokio's async model; migration from diesel to async-compatible layer is painful | Raw `libsql` queries with typed structs |
| `actix-web` | Not wrong, but Axum is the tokio-team's official framework and has better Tower integration for this stack | `axum` 0.8 |
| `warp` | Unmaintained/stagnant; the trait-based approach is harder to extend | `axum` 0.8 |
| `moment.js` (frontend) | 300KB bloated, deprecated by maintainers | `date-fns` 3.x (tree-shakeable) |
| `react-query` v3/v4 | Older API; v5 has improved TypeScript inference and streaming/suspense for App Router | `@tanstack/react-query` v5 |
| `axios` for background fetching | TanStack Query manages caching/refetch — don't bypass it with raw axios calls outside query functions | TanStack Query `queryFn` wrapping fetch/axios |
| `react-table` v7 | v7 is deprecated; completely different API from TanStack Table v8 | `@tanstack/react-table` v8 |
| `emotion` / `styled-components` | Runtime CSS-in-JS conflicts with Next.js App Router RSC (React Server Components); Tailwind has no runtime | `tailwindcss` 4.x |
| `next-auth` v4 | v4 was designed for Pages Router; v5 is the App Router-compatible version | `next-auth` v5 (Auth.js) OR custom JWT middleware |
| Global Rust state with `Mutex<HashMap>` | Race conditions under concurrent webhook bursts; device state should live in DB | SQLite as single source of truth for device state |

---

## ISAPI Integration Patterns

Hikvision ISAPI is a RESTful HTTP protocol with two integration directions:

### Inbound: Device pushes events to Cronometrix (attendance webhooks)

The device sends HTTP POST to a configured listener endpoint. Event payloads are `multipart/form-data` containing:
- `EventNotificationAlert` XML block (attendance event, employee ID, face capture time)
- Optional binary JPEG (face photo at the moment of capture)

**Implementation:** Axum route receives the multipart body. Parse XML with `quick-xml`. Extract employee card number, event time, device IP, and entry/exit direction. Persist to SQLite immediately. Queue Turso sync.

Authentication from device to server: Basic or Digest (configure `httpAuthType = "basic"` on the device — simpler for server-to-receive direction since Axum handles it via Authorization header check).

### Outbound: Cronometrix sends commands to devices (door open, enrollment, sync profiles)

Cronometrix acts as HTTP client calling the device's ISAPI endpoints. Devices require **Digest Authentication** (RFC 2617). Flow:
1. Send initial request — device returns 401 + `WWW-Authenticate: Digest realm=...`
2. Compute MD5 digest with credentials + nonce
3. Retry with `Authorization: Digest ...` header

Use `diqwest` crate which wraps `reqwest` and handles this challenge-response automatically.

Key outbound ISAPI endpoints:
- `PUT /ISAPI/AccessControl/UserInfo/SetUp` — enroll employee face profile
- `PUT /ISAPI/RemoteControl/door/0` — remote door open
- `GET /ISAPI/System/status` — device health check
- `POST /ISAPI/Event/notification/httpHosts` — configure webhook listener URL

All request/response bodies are XML. Use `quick-xml` with serde derives for typed serialization.

---

## Authentication & RBAC Architecture

### Backend (Rust/Axum)

- Issue JWT on login (`jsonwebtoken` crate, HS256, secret from env)
- Claims include: `sub` (user_id), `role` (Admin | Supervisor | Viewer), `exp`, `iat`
- Axum extractor validates JWT from `Authorization: Bearer <token>` header
- Role enforcement via Tower middleware layer applied per router group:
  - `/admin/*` — Admin only
  - `/supervisor/*` — Admin + Supervisor
  - `/viewer/*` — All authenticated roles
- Password stored as `argon2id` hash; verify on login

### Frontend (Next.js)

- Store JWT in `httpOnly` cookie (XSS-safe) or memory (for SPA)
- Next.js middleware reads cookie, redirects unauthenticated requests
- TanStack Query attaches Bearer token via `defaultOptions.queries.queryFn` wrapper
- Role-based UI gating via React context (derived from decoded JWT claims)
- No server-side RBAC enforcement in Next.js — backend is authoritative

---

## Stack Patterns by Variant

**If Tauri desktop wrapper is added later (future milestone):**
- Keep Axum running as a sidecar process, or migrate to Tauri's Rust backend commands
- `tauri-plugin-libsql` exists for direct libSQL in Tauri (see DEV.to article, MEDIUM confidence)
- Avoid embedding business logic in Next.js Server Actions — keep it in Rust so Tauri migration is smooth

**If you need real-time push to dashboard (device status, live photo feed):**
- Add Server-Sent Events (SSE) endpoint in Axum — simpler than WebSockets for one-directional push
- TanStack Query `refetchInterval` is adequate for polling (every 5s) as a starting point
- Upgrade to SSE when polling feels laggy in production

**If multiple concurrent Hikvision devices send simultaneous webhooks:**
- Tokio handles concurrent async tasks natively — no additional work needed
- Ensure `libsql` connection is shared via `Arc<Database>` in Axum state
- SQLite WAL mode enabled by default in libSQL embedded replicas

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `axum` 0.8.x | `tower-http` 0.6.x | tower-http 0.5.x is for axum 0.7; must use 0.6 with 0.8 |
| `axum` 0.8.x | `tokio` 1.x | Any tokio 1.x works |
| `libsql` | `tokio` 1.x | async-first, requires tokio runtime |
| `reqwest` 0.13.x | `tokio` 1.x | Use `rustls-tls` feature to avoid OpenSSL system dependency |
| Next.js 15 | React 19 | Next.js 15 requires React 19; `@tanstack/react-query` v5 is compatible |
| `@tanstack/react-table` v8 | `@tanstack/react-virtual` v3 | Must use matching major versions |
| `tailwindcss` 4.x | Next.js 15 | Tailwind 4 uses a different config format; shadcn/ui supports it |
| `react-big-calendar` | `date-fns` 3.x | Use date-fns as localizer; moment.js localizer is deprecated |
| `jsonwebtoken` 10.x | N/A | Breaking change from 8.x: encoding/decoding API changed |

---

## Sources

- [Axum 0.8.8 docs.rs](https://docs.rs/axum/latest/axum/) — version confirmed HIGH confidence
- [Tokio 1.51.1 docs.rs](https://docs.rs/tokio/latest/tokio/) — version confirmed HIGH confidence
- [reqwest 0.13.2 docs.rs](https://docs.rs/reqwest/latest/reqwest/) — version confirmed HIGH confidence
- [jsonwebtoken 10.3.0 docs.rs](https://docs.rs/jsonwebtoken/latest/jsonwebtoken/) — version confirmed HIGH confidence
- [Turso Rust Quickstart](https://docs.turso.tech/sdk/rust/quickstart) — embedded replica API verified HIGH confidence
- [Turso Offline Sync Beta](https://turso.tech/blog/turso-offline-sync-public-beta) — Rust support confirmed MEDIUM confidence
- [Axum 0.8.0 Announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) — release date verified HIGH confidence
- [Hikvision ISAPI Event Listening](https://www.hikvisioneurope.com/eu/portal/portal/Technology%20Partner%20Program/03-How%20to/How%20to%20get%20real-time%20event%20in%20listening%20mode.pdf) — multipart event format MEDIUM confidence
- [Hikvision TPP Integration Center](https://tpp.hikvision.com/) — digest auth requirement MEDIUM confidence
- [diqwest crate](https://docs.rs/diqwest) — digest auth reqwest extension MEDIUM confidence
- [quick-xml performance comparison](https://capnfabs.net/posts/parsing-huge-xml-quickxml-rust-serde/) — 10x perf advantage MEDIUM confidence
- [TanStack Table v8](https://tanstack.com/table/latest) — current version HIGH confidence
- [TanStack Query v5](https://tanstack.com/query/latest) — SSR/App Router support HIGH confidence
- [shadcn/ui Data Table docs](https://ui.shadcn.com/docs/components/radix/data-table) — TanStack Table integration HIGH confidence
- [Recharts v3 changelog](https://blog.logrocket.com/best-react-chart-libraries-2025/) — v3 release mid-2025 MEDIUM confidence
- [RustCrypto password-hashes](https://github.com/RustCrypto/password-hashes) — argon2 crate HIGH confidence
- [chrono 0.4.42](https://crates.io/crates/chrono) — version MEDIUM confidence (from WebSearch)
- [react-hook-form + Zod shadcn guide](https://ui.shadcn.com/docs/forms/react-hook-form) — canonical pattern HIGH confidence

---

*Stack research for: Cronometrix — biometric time & attendance system*
*Researched: 2026-04-11*
