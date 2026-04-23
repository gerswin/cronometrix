//! 02:00 local nightly reconcile task. Uses `tokio::time::sleep` computed from
//! `chrono-tz` next-2AM — no `cron` crate dependency.

use chrono::{NaiveTime, TimeZone};
use chrono_tz::Tz;
use tokio_util::sync::CancellationToken;

use crate::daily_records::service as dr_service;
use crate::state::AppState;

pub async fn nightly_reconcile_task(state: AppState, tz: Tz, shutdown: CancellationToken) {
    loop {
        let sleep_secs = seconds_until_next_2am(tz);
        let sleep_dur = std::time::Duration::from_secs(sleep_secs.max(1) as u64);

        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!("nightly reconcile shutdown");
                break;
            }
            _ = tokio::time::sleep(sleep_dur) => {
                tracing::info!("nightly reconcile starting");
                match dr_service::reconcile_prior_day(&state, tz).await {
                    Ok(n) => tracing::info!(employees = n, "nightly reconcile complete"),
                    Err(e) => tracing::error!(err = %e, "nightly reconcile failed"),
                }
            }
        }
    }
}

fn seconds_until_next_2am(tz: Tz) -> i64 {
    let now_local = chrono::Utc::now().with_timezone(&tz);
    let today_2am = tz
        .from_local_datetime(
            &now_local
                .date_naive()
                .and_time(NaiveTime::from_hms_opt(2, 0, 0).unwrap()),
        )
        .single();
    let target = match today_2am {
        Some(dt) if dt > now_local => dt,
        _ => {
            let tomorrow = now_local.date_naive() + chrono::Duration::days(1);
            tz.from_local_datetime(
                &tomorrow.and_time(NaiveTime::from_hms_opt(2, 0, 0).unwrap()),
            )
            .single()
            .expect("America/Caracas has no DST ambiguity")
        }
    };
    (target - now_local).num_seconds().max(1)
}
