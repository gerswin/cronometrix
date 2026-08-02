//! The port exists so application code can depend on a capability rather than a
//! manufacturer. This test never names Hikvision: it drives a fake reader,
//! which is exactly what a second vendor's adapter has to satisfy.

use async_trait::async_trait;
use cronometrix_api::devices::reader::{
    BiometricReader, DeviceCommand, ProvisionReport, ProvisioningIntent,
};

struct FakeReader {
    enrolled: std::sync::Mutex<Vec<String>>,
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
        Ok(report)
    }

    async fn enroll(&self, person_id: &str, _display_name: &str, _face: &[u8]) -> anyhow::Result<()> {
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

    async fn execute(&self, _command: DeviceCommand) -> anyhow::Result<String> {
        Ok("ok".to_string())
    }
}

#[tokio::test]
async fn a_reader_reports_what_it_could_not_apply_instead_of_failing_silently() {
    let reader = FakeReader {
        enrolled: std::sync::Mutex::new(Vec::new()),
    };
    let report = reader
        .provision(&ProvisioningIntent {
            local_time: "2026-08-02T13:00:00".into(),
            time_zone: "CST+4:00:00".into(),
            require_direction: true,
            day_split: "13:00:00".into(),
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
    };
    reader.enroll("person-1", "Person One", &[0xFF, 0xD8, 0xFF]).await.unwrap();
    assert_eq!(reader.enrolled.lock().unwrap().len(), 1);
    reader.revoke("person-1").await.unwrap();
    assert!(reader.enrolled.lock().unwrap().is_empty());
}
