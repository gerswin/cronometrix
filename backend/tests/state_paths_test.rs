//! Unit tests for `state::paths::Paths`. Targets the 33.33% baseline gap
//! from Plan 03 (08-04A bucket row 16). `for_test` is the only path
//! covered by existing tests transitively; `from_env` (production
//! constructor + env_or_default helper) and the env-var precedence rules
//! were entirely uncovered.
//!
//! Important: these tests serialise via a Mutex because they mutate the
//! process-global env (CRONOMETRIX_LEAVES_ROOT, etc.). This is the only
//! place in the repo allowed to do so — production code reads paths from
//! `state.paths.*` so other tests do not need env mutation. The Mutex
//! prevents parallel tests from clobbering each other's env state.

use cronometrix_api::state::Paths;
use std::path::PathBuf;
use std::sync::Mutex;

// Process-wide gate — every test below acquires this before mutating env.
static ENV_LOCK: Mutex<()> = Mutex::new(());

const ENV_KEYS: &[&str] = &[
    "CRONOMETRIX_LEAVES_ROOT",
    "CRONOMETRIX_EVENTS_ROOT",
    "ENROLLMENTS_DIR",
    "CRONOMETRIX_CAPTURES_TMP",
    "DATA_DIR",
];

fn unset_all() {
    for k in ENV_KEYS {
        std::env::remove_var(k);
    }
}

#[test]
fn for_test_subdirs_under_tempdir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = Paths::for_test(tmp.path());
    assert_eq!(p.leaves_root, tmp.path().join("leaves"));
    assert_eq!(p.events_root, tmp.path().join("events"));
    assert_eq!(p.enrollments_root, tmp.path().join("enrollments"));
    assert_eq!(p.captures_tmp_root, tmp.path().join("captures-tmp"));
    assert_eq!(p.overrides_root, tmp.path().join("overrides"));
    // Each path is contained in the tempdir root.
    for field in [
        &p.leaves_root,
        &p.events_root,
        &p.enrollments_root,
        &p.captures_tmp_root,
        &p.overrides_root,
    ] {
        assert!(field.starts_with(tmp.path()));
    }
}

#[test]
fn for_test_clone_produces_equal_paths() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = Paths::for_test(tmp.path());
    let p2 = p.clone();
    assert_eq!(p.leaves_root, p2.leaves_root);
    assert_eq!(p.events_root, p2.events_root);
}

#[test]
fn from_env_defaults_when_no_vars_set() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();

    let p = Paths::from_env();
    assert_eq!(p.leaves_root, PathBuf::from("./data/leaves"));
    assert_eq!(p.events_root, PathBuf::from("./data/events"));
    assert_eq!(p.enrollments_root, PathBuf::from("./data/enrollments"));
    assert_eq!(
        p.captures_tmp_root,
        PathBuf::from("/tmp/enrollments-captures")
    );
    // overrides_root = DATA_DIR/overrides; default DATA_DIR = "./data"
    assert_eq!(p.overrides_root, PathBuf::from("./data").join("overrides"));
}

#[test]
fn from_env_honours_each_env_var() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();

    std::env::set_var("CRONOMETRIX_LEAVES_ROOT", "/var/x/leaves");
    std::env::set_var("CRONOMETRIX_EVENTS_ROOT", "/var/x/events");
    std::env::set_var("ENROLLMENTS_DIR", "/var/x/enroll");
    std::env::set_var("CRONOMETRIX_CAPTURES_TMP", "/var/x/cap-tmp");
    std::env::set_var("DATA_DIR", "/var/x/data");

    let p = Paths::from_env();
    assert_eq!(p.leaves_root, PathBuf::from("/var/x/leaves"));
    assert_eq!(p.events_root, PathBuf::from("/var/x/events"));
    assert_eq!(p.enrollments_root, PathBuf::from("/var/x/enroll"));
    assert_eq!(p.captures_tmp_root, PathBuf::from("/var/x/cap-tmp"));
    assert_eq!(
        p.overrides_root,
        PathBuf::from("/var/x/data").join("overrides")
    );

    unset_all();
}

#[test]
fn from_env_partial_override_falls_back_per_field() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();

    // Only set leaves and events; the rest must default.
    std::env::set_var("CRONOMETRIX_LEAVES_ROOT", "/only-leaves");
    std::env::set_var("CRONOMETRIX_EVENTS_ROOT", "/only-events");

    let p = Paths::from_env();
    assert_eq!(p.leaves_root, PathBuf::from("/only-leaves"));
    assert_eq!(p.events_root, PathBuf::from("/only-events"));
    assert_eq!(p.enrollments_root, PathBuf::from("./data/enrollments"));
    assert_eq!(
        p.captures_tmp_root,
        PathBuf::from("/tmp/enrollments-captures")
    );
    // DATA_DIR not set — overrides falls back to ./data/overrides
    assert_eq!(p.overrides_root, PathBuf::from("./data").join("overrides"));

    unset_all();
}

#[test]
fn from_env_data_dir_only_changes_overrides_root() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();

    std::env::set_var("DATA_DIR", "/custom/data");
    let p = Paths::from_env();

    assert_eq!(
        p.overrides_root,
        PathBuf::from("/custom/data").join("overrides")
    );
    // The other fields are unaffected by DATA_DIR.
    assert_eq!(p.leaves_root, PathBuf::from("./data/leaves"));
    assert_eq!(p.events_root, PathBuf::from("./data/events"));

    unset_all();
}

#[test]
fn debug_impl_does_not_panic() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = Paths::for_test(tmp.path());
    let s = format!("{:?}", p);
    assert!(s.contains("Paths"));
    assert!(s.contains("leaves_root"));
}
