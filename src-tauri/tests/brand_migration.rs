use std::fs;

use netcat_lib::{migration::migrate_legacy_database, storage::Store};

const MAX_ENTRIES: usize = 5_000;
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

fn open_store(path: &std::path::Path) -> Store {
    Store::open(path, MAX_ENTRIES, MAX_BODY_BYTES).unwrap()
}

#[test]
fn migrates_legacy_database_without_removing_the_source() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join("com.netcapture.desktop");
    let current = root.path().join("com.kaitehan.netcat");
    let legacy_database = legacy.join("netcapture.sqlite3");
    let current_database = current.join("netcat.sqlite3");
    let legacy_store = open_store(&legacy_database);
    legacy_store
        .set_setting("capture_enabled", "false")
        .unwrap();
    drop(legacy_store);

    assert!(migrate_legacy_database(&legacy, &current).unwrap());

    let current_store = open_store(&current_database);
    assert_eq!(
        current_store
            .get_setting("capture_enabled")
            .unwrap()
            .as_deref(),
        Some("false")
    );
    assert!(legacy_database.exists());
}

#[test]
fn keeps_an_existing_netcat_database_and_is_idempotent() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join("com.netcapture.desktop");
    let current = root.path().join("com.kaitehan.netcat");
    let legacy_store = open_store(&legacy.join("netcapture.sqlite3"));
    legacy_store.set_setting("source", "legacy").unwrap();
    let current_store = open_store(&current.join("netcat.sqlite3"));
    current_store.set_setting("source", "current").unwrap();
    drop(legacy_store);
    drop(current_store);

    assert!(!migrate_legacy_database(&legacy, &current).unwrap());
    assert!(!migrate_legacy_database(&legacy, &current).unwrap());

    let current_store = open_store(&current.join("netcat.sqlite3"));
    assert_eq!(
        current_store.get_setting("source").unwrap().as_deref(),
        Some("current")
    );
}

#[test]
fn reports_a_corrupt_legacy_database_without_creating_an_empty_target() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join("com.netcapture.desktop");
    let current = root.path().join("com.kaitehan.netcat");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("netcapture.sqlite3"), b"not a sqlite database").unwrap();

    assert!(migrate_legacy_database(&legacy, &current).is_err());
    assert!(!current.join("netcat.sqlite3").exists());
}
