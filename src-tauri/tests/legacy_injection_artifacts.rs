use std::fs;

use netcat_lib::migration::cleanup_legacy_injection_artifacts;

#[test]
fn missing_session_artifact_is_successful_and_repeatable() {
    let root = tempfile::tempdir().unwrap();
    cleanup_legacy_injection_artifacts(root.path()).unwrap();
    cleanup_legacy_injection_artifacts(root.path()).unwrap();
    fs::create_dir(root.path().join("frida")).unwrap();
    fs::write(root.path().join("frida/keep.txt"), "keep").unwrap();
    cleanup_legacy_injection_artifacts(root.path()).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("frida/keep.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn removes_managed_session_and_its_empty_directory() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("frida")).unwrap();
    fs::write(root.path().join("frida/frida-session.js"), "legacy script").unwrap();
    cleanup_legacy_injection_artifacts(root.path()).unwrap();
    assert!(!root.path().join("frida").exists());
    cleanup_legacy_injection_artifacts(root.path()).unwrap();
}

#[test]
fn removes_only_the_managed_session_and_preserves_unrelated_contents() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("frida");
    fs::create_dir_all(directory.join("custom")).unwrap();
    fs::write(directory.join("frida-session.js"), "legacy script").unwrap();
    fs::write(directory.join("custom/script.js"), "custom script").unwrap();
    fs::write(directory.join("notes.txt"), "notes").unwrap();
    cleanup_legacy_injection_artifacts(root.path()).unwrap();
    assert!(!directory.join("frida-session.js").exists());
    assert_eq!(
        fs::read_to_string(directory.join("custom/script.js")).unwrap(),
        "custom script"
    );
    assert_eq!(
        fs::read_to_string(directory.join("notes.txt")).unwrap(),
        "notes"
    );
}

#[test]
fn reports_non_missing_io_errors_without_removing_other_data() {
    let root = tempfile::tempdir().unwrap();
    let session = root.path().join("frida/frida-session.js");
    fs::create_dir_all(&session).unwrap();
    fs::write(session.join("keep.txt"), "keep").unwrap();
    assert!(cleanup_legacy_injection_artifacts(root.path()).is_err());
    assert_eq!(
        fs::read_to_string(session.join("keep.txt")).unwrap(),
        "keep"
    );
}
