use std::path::{Path, PathBuf};

/// Filesystem roots injected via AppState (Phase 8, D-18/D-19).
/// Read once at startup from env-or-default; overridden in tests via `for_test`.
/// Eliminates the cwd-dependent + process-global env-var-race anti-pattern.
#[derive(Clone, Debug)]
pub struct Paths {
    pub leaves_root: PathBuf,
    pub events_root: PathBuf,
    pub enrollments_root: PathBuf,
    pub captures_tmp_root: PathBuf,
    pub overrides_root: PathBuf,
}

impl Paths {
    /// Production constructor — read each path from env, fall back to default.
    /// Preserves the env var names and defaults used by the deleted
    /// service::*_root() helpers (D-21 backwards compatibility).
    pub fn from_env() -> Self {
        Self {
            leaves_root: env_or_default("CRONOMETRIX_LEAVES_ROOT", "./data/leaves"),
            events_root: env_or_default("CRONOMETRIX_EVENTS_ROOT", "./data/events"),
            enrollments_root: env_or_default("ENROLLMENTS_DIR", "./data/enrollments"),
            captures_tmp_root: env_or_default(
                "CRONOMETRIX_CAPTURES_TMP",
                "/tmp/enrollments-captures",
            ),
            overrides_root: env_or_default("DATA_DIR", "./data").join("overrides"),
        }
    }

    /// Test constructor — every field is a subdirectory of the supplied tempdir.
    /// Caller is responsible for keeping the underlying TempDir alive for the
    /// test's duration (see common::test_state_with_tmpdir).
    pub fn for_test(tmp: &Path) -> Self {
        Self {
            leaves_root: tmp.join("leaves"),
            events_root: tmp.join("events"),
            enrollments_root: tmp.join("enrollments"),
            captures_tmp_root: tmp.join("captures-tmp"),
            overrides_root: tmp.join("overrides"),
        }
    }
}

fn env_or_default(key: &str, default: &str) -> PathBuf {
    std::env::var(key)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}
