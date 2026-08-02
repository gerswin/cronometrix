//! The port exists so application code can depend on a capability rather than a
//! manufacturer. This test never names Hikvision: it drives a fake reader,
//! which is exactly what a second vendor's adapter has to satisfy.
//!
//! What this file is NOT: coverage of the port's real-world behaviour. Every
//! test here would still pass with `impl BiometricReader for DeviceConnection`
//! deleted, because `FakeReader` is the only thing under test — the trait
//! bounds and the fake's own logic. Kept anyway as a compile-time contract
//! (a caller written against `BiometricReader` alone must build against this)
//! and as executable documentation of what a second vendor's adapter has to
//! satisfy. The actual coverage of `DeviceConnection`'s behaviour — request
//! shapes, ordering, firmware quirks — lives in the wiremock tests in
//! `isapi_client_test.rs`.

use async_trait::async_trait;
use cronometrix_api::devices::reader::{
    BiometricReader, DeviceCommand, ProvisionReport, ProvisioningIntent,
};

struct FakeReader {
    enrolled: std::sync::Mutex<Vec<String>>,
    /// Whether `provision` touched the reader's notification targets. Lets a
    /// test assert that a `None` webhook leaves them alone rather than only
    /// asserting on the report, which would miss a reader that clears its
    /// slots and then reports nothing.
    webhook_slots_touched: std::sync::Mutex<bool>,
}

#[async_trait]
impl BiometricReader for FakeReader {
    async fn provision(&self, intent: &ProvisioningIntent) -> anyhow::Result<ProvisionReport> {
        // A reader that cannot honour a request reports it rather than lying.
        let mut report = ProvisionReport::default();
        report.applied.push("clock");
        if intent.require_direction {
            report.unsupported.push("attendance_mode");
        }
        // A pull-mode device (`None`) must not have its notification targets
        // touched — clearing a slot nobody asked to push would be destructive
        // for no reason.
        if intent.event_webhook.is_some() {
            *self.webhook_slots_touched.lock().unwrap() = true;
            report.applied.push("event_webhook");
        }
        Ok(report)
    }

    async fn enroll(
        &self,
        person_id: &str,
        _display_name: &str,
        _face: &[u8],
    ) -> anyhow::Result<()> {
        self.enrolled.lock().unwrap().push(person_id.to_string());
        Ok(())
    }

    async fn revoke(&self, person_id: &str) -> anyhow::Result<()> {
        self.enrolled.lock().unwrap().retain(|id| id != person_id);
        Ok(())
    }

    async fn capture_face(&self) -> anyhow::Result<Vec<u8>> {
        Ok(vec![0xFF, 0xD8, 0xFF])
    }

    async fn send_command(&self, _command: DeviceCommand) -> anyhow::Result<String> {
        Ok("ok".to_string())
    }
}

#[tokio::test]
async fn a_reader_reports_what_it_could_not_apply_instead_of_failing_silently() {
    let reader = FakeReader {
        enrolled: std::sync::Mutex::new(Vec::new()),
        webhook_slots_touched: std::sync::Mutex::new(false),
    };
    let report = reader
        .provision(&ProvisioningIntent {
            now: chrono::DateTime::parse_from_rfc3339("2026-08-02T13:00:00-04:00").unwrap(),
            require_direction: true,
            day_split: "13:00:00".into(),
            event_webhook: None,
        })
        .await
        .expect("provision");

    assert_eq!(report.applied, vec!["clock"]);
    assert_eq!(
        report.unsupported,
        vec!["attendance_mode"],
        "an unsupported capability must be visible to the caller"
    );
}

#[tokio::test]
async fn enrol_and_revoke_round_trip_through_the_port() {
    let reader = FakeReader {
        enrolled: std::sync::Mutex::new(Vec::new()),
        webhook_slots_touched: std::sync::Mutex::new(false),
    };
    reader
        .enroll("person-1", "Person One", &[0xFF, 0xD8, 0xFF])
        .await
        .unwrap();
    assert_eq!(reader.enrolled.lock().unwrap().len(), 1);
    reader.revoke("person-1").await.unwrap();
    assert!(reader.enrolled.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_none_webhook_leaves_the_readers_notification_targets_untouched() {
    let reader = FakeReader {
        enrolled: std::sync::Mutex::new(Vec::new()),
        webhook_slots_touched: std::sync::Mutex::new(false),
    };
    let report = reader
        .provision(&ProvisioningIntent {
            now: chrono::DateTime::parse_from_rfc3339("2026-08-02T13:00:00-04:00").unwrap(),
            require_direction: false,
            day_split: "13:00:00".into(),
            event_webhook: None,
        })
        .await
        .expect("provision");

    assert!(
        !*reader.webhook_slots_touched.lock().unwrap(),
        "a pull-mode device (event_webhook: None) must not have its \
         notification slots cleared or written"
    );
    assert!(
        !report.applied.contains(&"event_webhook"),
        "nothing should be reported for a capability that was never attempted"
    );
}
