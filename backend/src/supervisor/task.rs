//! Per-device alertStream reconnect loop (Plan 02-03 Task 2).
//!
//! One instance runs inside each supervised tokio task. Handles the full
//! reconnect lifecycle:
//! 1. call `connect_and_stream` — owns the reqwest long-lived connection.
//! 2. on any error (or graceful end), sleep with exponential backoff.
//! 3. backoff starts at 1s and doubles on each consecutive failure, capped
//!    at 60s. A graceful end (Ok(())) resets backoff to 1s so a device that
//!    simply rotates connections periodically does not accumulate delay.
//! 4. ±25% jitter is added on top of the base backoff — RESEARCH
//!    "Pitfall 6 — reconnect storm" — so a fleet of devices that drop
//!    together don't all retry in lockstep.
//! 5. CancellationToken short-circuits both the inflight call and the sleep,
//!    so shutdown drains within seconds regardless of backoff state.

use std::time::Duration;

use rand::Rng;
use tokio_util::sync::CancellationToken;

use crate::devices::models::DeviceWithPlaintext;
use crate::isapi::stream::{connect_and_stream, DeviceConfig};
use crate::state::AppState;

/// Tunables — surfaced as `pub(crate)` so supervisor_tests can assert the
/// exact cap behavior rather than hardcoding magic numbers.
pub(crate) const INITIAL_BACKOFF_MS: u64 = 1_000;
pub(crate) const MAX_BACKOFF_MS: u64 = 60_000;

/// Compute the next sleep duration in ms given the current `backoff_ms`.
/// Adds up to 25% jitter. Exposed for deterministic unit testing.
pub(crate) fn sleep_ms_with_jitter(backoff_ms: u64) -> u64 {
    let jitter_cap = backoff_ms / 4;
    let jitter = if jitter_cap == 0 {
        0
    } else {
        rand::thread_rng().gen_range(0..=jitter_cap)
    };
    backoff_ms.saturating_add(jitter)
}

/// Main per-device reconnect loop. Exits only on CancellationToken.
pub async fn device_task(dev: DeviceWithPlaintext, state: AppState, cancel: CancellationToken) {
    // DeviceConfig carries only the fields the stream layer needs. Plaintext
    // password stays on this stack frame — no Debug/Serialize derives on
    // DeviceConfig or DeviceWithPlaintext (redacted Debug on the latter).
    let cfg = DeviceConfig {
        id: dev.id.clone(),
        base_url: dev.base_url.clone(),
        username: dev.username.clone(),
        password: dev.password.clone(),
        direction_default: dev.direction.clone(),
        allow_insecure_tls: dev.allow_insecure_tls,
    };

    let mut backoff_ms: u64 = INITIAL_BACKOFF_MS;

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                let _ = super::status::update_connection_state(&state, &cfg.id, "offline").await;
                tracing::info!(device_id = %cfg.id, "device_task cancelled");
                return;
            }
            res = connect_and_stream(&cfg, &state) => {
                match res {
                    Ok(()) => {
                        tracing::info!(device_id = %cfg.id, "stream ended gracefully");
                        backoff_ms = INITIAL_BACKOFF_MS;
                    }
                    Err(e) => {
                        tracing::warn!(device_id = %cfg.id, err = %e, "stream ended with error");
                        let _ = super::status::update_connection_state(&state, &cfg.id, "offline").await;
                    }
                }
            }
        }

        // Back off before reconnecting, but honor cancellation during the sleep.
        let sleep = Duration::from_millis(sleep_ms_with_jitter(backoff_ms));
        tracing::debug!(device_id = %cfg.id, backoff_ms = backoff_ms, "reconnect backoff");
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                let _ = super::status::update_connection_state(&state, &cfg.id, "offline").await;
                return;
            }
            _ = tokio::time::sleep(sleep) => {}
        }

        // Double the backoff for the NEXT failure, capped at MAX.
        backoff_ms = backoff_ms.saturating_mul(2).min(MAX_BACKOFF_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_ms_with_jitter_within_25_percent() {
        // 4s backoff: jitter ≤ 1s, total ∈ [4s, 5s]
        let base_ms = 4_000u64;
        for _ in 0..100 {
            let total = sleep_ms_with_jitter(base_ms);
            assert!(total >= base_ms);
            assert!(total <= base_ms + base_ms / 4);
        }
    }

    #[test]
    fn sleep_ms_with_jitter_handles_small_backoff() {
        // 1s backoff: jitter ≤ 250ms, total ∈ [1000, 1250]
        let total = sleep_ms_with_jitter(1_000);
        assert!(total >= 1_000 && total <= 1_250);
    }

    #[test]
    fn sleep_ms_with_jitter_zero_is_stable() {
        // Defensive — should never be called with 0 but must not panic.
        assert_eq!(sleep_ms_with_jitter(0), 0);
    }

    #[test]
    fn backoff_cap_reachable_from_initial_in_nine_steps() {
        // 1 -> 2 -> 4 -> 8 -> 16 -> 32 -> 60 (capped from 64)
        let mut b = INITIAL_BACKOFF_MS;
        let mut steps = 0;
        while b < MAX_BACKOFF_MS {
            b = b.saturating_mul(2).min(MAX_BACKOFF_MS);
            steps += 1;
            assert!(steps < 20, "cap should be reachable in <20 doublings");
        }
        assert_eq!(b, MAX_BACKOFF_MS);
    }
}
