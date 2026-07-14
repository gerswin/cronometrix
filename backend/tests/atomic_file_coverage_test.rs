use cronometrix_api::storage::atomic_file::{
    inspect_owned_file, read_owned_file, remove_owned_file, AtomicFileGuard,
};

#[test]
fn owned_file_round_trip_reads_inspects_and_removes_exact_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("captures");
    let path = root.join("capture.jpg");
    let guard = AtomicFileGuard::write(&root, "capture.jpg", b"jpeg-bytes").unwrap();
    let identity = guard.identity();

    assert_eq!(
        read_owned_file(&root, &path, identity).unwrap(),
        b"jpeg-bytes"
    );
    let inspection = inspect_owned_file(&root, &path).unwrap();
    assert_eq!(inspection.identity(), identity);
    assert!(inspection.modified() <= std::time::SystemTime::now());

    guard.keep();
    remove_owned_file(&root, &path, identity).unwrap();
    assert!(!path.exists());
}

#[test]
fn owned_file_operations_reject_invalid_paths_and_non_regular_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("captures");
    std::fs::create_dir_all(&root).unwrap();
    let valid = root.join("valid.jpg");
    std::fs::write(&valid, b"valid").unwrap();
    let identity = inspect_owned_file(&root, &valid).unwrap().identity();

    for invalid in [
        root.join("nested/capture.jpg"),
        root.join("capture.png"),
        root.join("capture"),
        tmp.path().join("outside.jpg"),
    ] {
        assert!(inspect_owned_file(&root, &invalid).is_err());
        assert!(read_owned_file(&root, &invalid, identity).is_err());
        assert!(remove_owned_file(&root, &invalid, identity).is_err());
    }

    let directory = root.join("directory.jpg");
    std::fs::create_dir(&directory).unwrap();
    assert!(inspect_owned_file(&root, &directory).is_err());
    assert!(read_owned_file(&root, &directory, identity).is_err());
}

#[test]
fn identity_mismatch_preserves_replacement_and_missing_path_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("captures");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("capture.jpg");
    std::fs::write(&path, b"original").unwrap();
    let original_identity = inspect_owned_file(&root, &path).unwrap().identity();

    let replacement = root.join("replacement.jpg");
    std::fs::write(&replacement, b"replacement").unwrap();
    std::fs::rename(&replacement, &path).unwrap();

    assert!(read_owned_file(&root, &path, original_identity).is_err());
    assert!(remove_owned_file(&root, &path, original_identity).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"replacement");

    let missing = root.join("missing.jpg");
    remove_owned_file(&root, &missing, original_identity).unwrap();
}

#[test]
fn atomic_write_rejects_empty_path_and_reports_parent_creation_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let absent_root = tmp.path().join("absent");
    assert!(AtomicFileGuard::write(&absent_root, "", b"nope").is_err());
    assert!(!absent_root.exists());

    let root_is_file = tmp.path().join("root-file");
    std::fs::write(&root_is_file, b"not-a-directory").unwrap();
    let error = AtomicFileGuard::write(&root_is_file, "capture.jpg", b"nope").unwrap_err();
    assert!(error.to_string().contains("create atomic file parent"));
}
