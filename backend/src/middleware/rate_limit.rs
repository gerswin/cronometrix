//! Rate limiting for the public, unauthenticated surface (H-12, L-01).
//!
//! Before this module there was NO rate limiting anywhere in this backend —
//! see the (Argon2 DoS follow-up) comment in `setup/handlers.rs`. That left
//! three routes reachable at unlimited request rate:
//!   - `POST /auth/login`   — credential stuffing at zero cost.
//!   - `GET  /setup/status` — installation enumeration (is this box already
//!     set up?) at zero cost.
//!   - `POST /setup/init`   — the cheap `SELECT COUNT(*)` gate in
//!     `setup::handlers::setup_init` stops a flood from reaching Argon2 on an
//!     *already-initialized* box, but on a *fresh* box it does nothing: every
//!     request before the first admin exists reaches the hash. Rate limiting
//!     is the actual cost gate for that window.
//!
//! # Why not key on the TCP peer address
//!
//! Production traffic reaches this backend as: browser -> Cloudflare edge
//! -> `cloudflared` (tunnel container) -> nginx (`deploy/nginx.conf`, the
//! `gateway` service) -> this backend, over the docker-compose network. The
//! TCP peer axum sees for every request is nginx's container address —
//! identical for every real end user. A key extractor built on
//! `ConnectInfo`/peer IP (tower_governor's own default, `PeerIpKeyExtractor`)
//! would therefore throttle the *entire installation* as a single client:
//! worse than no limit, because it locks out legitimate users while leaving
//! an attacker on the same path indistinguishable from them.
//!
//! # Why not take the first (or last) `X-Forwarded-For` entry either
//!
//! `deploy/nginx.conf` sets `X-Forwarded-For: $proxy_add_x_forwarded_for` on
//! every `/api/` location, which *appends* nginx's own view of the previous
//! hop rather than overwriting the header. Two trusted hops append to this
//! chain before it reaches us:
//!   1. Cloudflare's edge appends the real client IP it terminated TLS for.
//!      Per Cloudflare's own documented behavior it appends to whatever
//!      `X-Forwarded-For` the client already sent — it does NOT replace a
//!      client-supplied header. An attacker can therefore prepend arbitrary
//!      fake entries; naively taking the *first* entry (what
//!      tower_governor's `SmartIpKeyExtractor` does) reads the attacker's own
//!      spoofed value, not Cloudflare's trustworthy one.
//!   2. nginx then appends its own `$remote_addr`, which — because
//!      `cloudflared` connects to nginx over the private docker network — is
//!      `cloudflared`'s container address, constant for every external
//!      client. Naively taking the *last* entry reintroduces the exact
//!      "everyone is one client" failure this module exists to avoid, one
//!      layer further out.
//!
//! The one entry that IS trustworthy is the second-to-last: the value
//! Cloudflare's edge appended, immediately before nginx's own useless
//! append. It cannot be forged by the request's origin because there is no
//! network path into this deployment that bypasses Cloudflare — the tunnel
//! has no public origin IP to attack directly (see CLAUDE.md "Network
//! access"). `TrustedProxyKeyExtractor` below implements exactly this: it
//! trusts the last two hops of `X-Forwarded-For` (nginx, Cloudflare) and
//! nothing to their left.
//!
//! In any environment without that nginx hop in front — local dev, `make
//! e2e` (`frontend/playwright.config.ts` hits the backend directly on
//! `127.0.0.1:4001`), `docker-compose.local.yml` (no `cloudflared`) — no
//! `X-Forwarded-For` header arrives at all. We fall back to the TCP peer
//! address (via `ConnectInfo`, wired up in `main.rs` through
//! `into_make_service_with_connect_info`) in that case, which is exactly
//! right for those single-hop topologies.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request},
};
use governor::middleware::NoOpMiddleware;
use tokio_util::sync::CancellationToken;
use tower_governor::{
    governor::{GovernorConfig, GovernorConfigBuilder, SharedRateLimiter},
    key_extractor::KeyExtractor,
    GovernorError, GovernorLayer,
};

const X_FORWARDED_FOR: &str = "x-forwarded-for";

/// # Two tiers, not one
///
/// `/setup/status` is NOT called only at login time. `frontend/src/proxy.ts`
/// (Next.js middleware, `matcher: ['/dashboard/:path*', '/timesheet/:path*',
/// '/employees/:path*', '/devices/:path*', '/enrollment/:path*', ...]`)
/// calls it on every single navigation to a protected route, for every user,
/// all the time — that's expected, legitimate, high-frequency traffic per
/// real client, nothing like a login attempt. `/auth/login` and
/// `/setup/init`, by contrast, are genuinely rare per real client (a handful
/// of attempts per session at most) and are exactly what H-12 is worried
/// about (credential stuffing, Argon2 grinding). Giving all three routes one
/// shared quota — the first cut of this module — meant either the status
/// endpoint's legitimate per-navigation volume forced the login quota
/// absurdly high, or a login-appropriate quota broke every page navigation.
/// Splitting them lets `/auth/login` + `/setup/init` keep a tight,
/// meaningful cost gate while `/setup/status` gets the headroom its actual
/// traffic pattern requires.
///
/// # Numbers, derived from `make e2e`, not guessed
///
/// `make e2e` hits the backend directly with no nginx in front (see module
/// docs above), so every request from every one of Playwright's 81 tests
/// (`npx playwright test --list`) — logins AND every `proxy.ts` navigation
/// check — lands on the same `ConnectInfo` fallback key. That makes the full
/// suite a genuine, if unusually concentrated, stress test of each tier's
/// budget. A first attempt at a single shared `BURST_SIZE = 40` / 1 req/s
/// broke logins around test ~40 (28 failures); `250`/4 req/s still broke 12;
/// `500`/10 req/s still broke 4 — the `/setup/status` per-navigation volume
/// was consuming the shared bucket faster than login demand alone would
/// have. Splitting the tiers and giving `/setup/status` its own generous
/// budget below passed the full suite; see task-4-report.md for that run.
pub const LOGIN_INIT_BURST_SIZE: u32 = 60;
pub const LOGIN_INIT_REPLENISH_PERIOD: Duration = Duration::from_millis(250); // 4 req/s sustained

pub const STATUS_BURST_SIZE: u32 = 600;
pub const STATUS_REPLENISH_PERIOD: Duration = Duration::from_millis(50); // 20 req/s sustained

/// How often the keyed rate-limit store is swept of stale entries so it
/// doesn't grow without bound over process uptime (one entry per distinct
/// key ever seen). Mirrors the cleanup pattern from the tower_governor
/// crate-level docs.
const CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

/// Extracts the client IP to key rate limiting on, per the trust model
/// documented at the top of this module: `X-Forwarded-For`'s second-to-last
/// entry (the value nginx received, i.e. what Cloudflare's edge appended),
/// falling back to the TCP peer address when no proxy hop is present.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrustedProxyKeyExtractor;

impl KeyExtractor for TrustedProxyKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(ip) = client_ip_from_headers(req.headers()) {
            return Ok(ip);
        }

        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip())
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Pulls the trustworthy client IP out of an `X-Forwarded-For` header value,
/// per the two-trusted-hop model: nginx appends `$remote_addr` last
/// (`cloudflared`'s address — useless, constant), Cloudflare's edge appends
/// the real client IP immediately before that. Everything further left may
/// be attacker-supplied (Cloudflare appends to, rather than replacing, a
/// client-sent `X-Forwarded-For`) and is never trusted.
///
/// Free function (rather than inlined in the extractor) so it has a direct
/// unit-test surface without needing to construct an `http::Request`.
pub fn client_ip_from_headers(headers: &HeaderMap) -> Option<IpAddr> {
    let raw = headers.get(X_FORWARDED_FOR)?.to_str().ok()?;

    let entries: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let candidate = match entries.len() {
        0 => return None,
        // Only one hop recorded: no proxy chain to strip from, use it as-is
        // (this is what a single nginx-only hop, without Cloudflare in
        // front, would produce).
        1 => entries[0],
        // >= 2 hops: skip nginx's own trailing append and use the one
        // Cloudflare's edge appended before it.
        n => entries[n - 2],
    };

    candidate.parse::<IpAddr>().ok()
}

/// Rate-limiting middleware type, shared by both tiers below. Apply with
/// `.route_layer(...)` to a sub-router containing only the routes that tier
/// covers — NOT the whole `public_routes` router, which also carries the SSE
/// stream and the device push webhook (neither should be throttled
/// per-client-IP the same way).
pub type PublicAuthRateLimit = GovernorLayer<TrustedProxyKeyExtractor, NoOpMiddleware, Body>;

/// Builds a layer plus a handle to its underlying limiter, so the caller can
/// hand the limiter to [`run_cleanup`] and the layer to a router.
fn build(
    period: Duration,
    burst: u32,
) -> (PublicAuthRateLimit, SharedRateLimiter<IpAddr, NoOpMiddleware>) {
    let mut builder = GovernorConfigBuilder::default();
    let mut builder = builder.key_extractor(TrustedProxyKeyExtractor);
    let config: GovernorConfig<TrustedProxyKeyExtractor, NoOpMiddleware> = builder
        .period(period)
        .burst_size(burst)
        .finish()
        .expect("rate limit config: period and burst must both be non-zero");

    let limiter = config.limiter().clone();
    (GovernorLayer::new(config), limiter)
}

/// The strict tier: `/auth/login` and `/setup/init` (H-12). Both are rare
/// per legitimate client and are exactly the credential-stuffing /
/// Argon2-grinding cost gate this module exists for.
pub fn login_and_init_rate_limit() -> (PublicAuthRateLimit, SharedRateLimiter<IpAddr, NoOpMiddleware>)
{
    build(LOGIN_INIT_REPLENISH_PERIOD, LOGIN_INIT_BURST_SIZE)
}

/// The generous tier: `/setup/status` alone (L-01). Called on every
/// protected-route navigation by `frontend/src/proxy.ts`, not just at login
/// — see the module-level doc comment above `LOGIN_INIT_BURST_SIZE` for why
/// this needs its own, much larger budget than login.
pub fn status_rate_limit() -> (PublicAuthRateLimit, SharedRateLimiter<IpAddr, NoOpMiddleware>) {
    build(STATUS_REPLENISH_PERIOD, STATUS_BURST_SIZE)
}

/// Periodically evicts stale entries from the keyed rate-limit store.
/// Without this, the store (one entry per distinct key ever observed, e.g.
/// per attacker-rotated source address) grows for the lifetime of the
/// process. Exits on `cancel`, matching every other background loop in this
/// codebase (see `supervisor::watchdog::watchdog_task`).
pub async fn run_cleanup(
    limiter: SharedRateLimiter<IpAddr, NoOpMiddleware>,
    cancel: CancellationToken,
) {
    let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
    interval.tick().await; // first tick fires immediately; skip it
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                tracing::info!("rate limit cleanup cancelled");
                return;
            }
            _ = interval.tick() => {
                limiter.retain_recent();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with_xff(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            axum::http::HeaderValue::from_str(value).unwrap(),
        );
        headers
    }

    // Exercises both arms of TrustedProxyKeyExtractor::extract's
    // `if let Some(ip) = client_ip_from_headers(...)` directly, at the
    // KeyExtractor level rather than through the full router, so both the
    // header path and the ConnectInfo fallback path are covered regardless
    // of which one a given HTTP-level test happens to exercise.
    #[test]
    fn extractor_uses_the_forwarded_header_when_present() {
        let mut req = Request::builder().uri("/").body(()).unwrap();
        req.headers_mut().insert(
            "x-forwarded-for",
            axum::http::HeaderValue::from_static("203.0.113.7, 172.20.0.5"),
        );

        let ip = TrustedProxyKeyExtractor.extract(&req).unwrap();
        assert_eq!(ip, "203.0.113.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extractor_falls_back_to_connect_info_without_the_header() {
        let mut req = Request::builder().uri("/").body(()).unwrap();
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        req.extensions_mut().insert(ConnectInfo(addr));

        let ip = TrustedProxyKeyExtractor.extract(&req).unwrap();
        assert_eq!(ip, addr.ip());
    }

    #[test]
    fn extractor_errors_without_header_or_connect_info() {
        let req = Request::builder().uri("/").body(()).unwrap();
        let result = TrustedProxyKeyExtractor.extract(&req);
        assert!(matches!(result, Err(GovernorError::UnableToExtractKey)));
    }

    #[tokio::test]
    async fn cleanup_exits_promptly_when_cancelled() {
        let (_layer, limiter) = login_and_init_rate_limit();
        let cancel = CancellationToken::new();
        cancel.cancel();

        // The first `interval.tick()` inside run_cleanup fires immediately
        // (tokio interval semantics), and the already-cancelled token then
        // wins the select! race right away — this must not block for
        // CLEANUP_INTERVAL (300s).
        tokio::time::timeout(Duration::from_secs(5), run_cleanup(limiter, cancel))
            .await
            .expect("run_cleanup did not exit promptly after cancellation");
    }

    #[test]
    fn single_hop_uses_that_entry() {
        let headers = headers_with_xff("203.0.113.7");
        assert_eq!(
            client_ip_from_headers(&headers),
            Some("203.0.113.7".parse().unwrap())
        );
    }

    #[test]
    fn two_hops_uses_second_to_last_not_last() {
        // "<cloudflare-appended-client-ip>, <nginx-appended-remote_addr>"
        let headers = headers_with_xff("203.0.113.7, 172.20.0.5");
        assert_eq!(
            client_ip_from_headers(&headers),
            Some("203.0.113.7".parse().unwrap())
        );
    }

    #[test]
    fn spoofed_prefix_entries_are_ignored() {
        // An attacker-supplied X-Forwarded-For gets Cloudflare's real client
        // IP appended, then nginx's own remote_addr appended after that.
        // Neither the spoofed prefix nor nginx's trailing entry should win.
        let headers = headers_with_xff("6.6.6.6, 9.9.9.9, 203.0.113.7, 172.20.0.5");
        assert_eq!(
            client_ip_from_headers(&headers),
            Some("203.0.113.7".parse().unwrap())
        );
    }

    #[test]
    fn missing_header_returns_none() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip_from_headers(&headers), None);
    }

    #[test]
    fn unparseable_entry_returns_none() {
        let headers = headers_with_xff("not-an-ip");
        assert_eq!(client_ip_from_headers(&headers), None);
    }

    #[test]
    fn empty_entries_are_filtered_before_indexing() {
        // Trailing comma / stray whitespace shouldn't shift which entry we
        // pick.
        let headers = headers_with_xff("203.0.113.7, 172.20.0.5, ");
        assert_eq!(
            client_ip_from_headers(&headers),
            Some("203.0.113.7".parse().unwrap())
        );
    }
}
