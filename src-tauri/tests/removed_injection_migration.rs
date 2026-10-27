use netcat_lib::storage::Store;
use rusqlite::{params, Connection};

#[test]
fn migration_six_removes_injection_rows_before_deserializing_and_preserves_proxy_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("netcat.sqlite3");
    drop(Store::open(&path, 100, 1024).unwrap());
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM schema_migrations WHERE version = 6", [])
        .unwrap();
    let proxy = r#"{"id":"proxy","name":"旧代理脚本","enabled":true,"priority":7,"methods":["GET"],"matchKind":"contains","urlPattern":"example.com","source":"function onRequest(request) { return request; }","kind":"proxy","platform":"android","builtin":false,"createdAt":"2026-01-01"}"#;
    for (id, payload) in [("proxy", proxy), ("injection", r#"{"kind":"injection"}"#)] {
        connection
            .execute(
                "INSERT INTO scripts VALUES (?1, ?2, 7, '2026-01-01')",
                params![id, payload],
            )
            .unwrap();
    }
    drop(connection);

    for _ in 0..2 {
        let store = Store::open(&path, 100, 1024).unwrap();
        let scripts = store.list_scripts().unwrap();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].id, "proxy");
        assert_eq!(scripts[0].name, "旧代理脚本");
        assert_eq!(scripts[0].priority, 7);
        assert_eq!(scripts[0].methods, ["GET"]);
        assert_eq!(scripts[0].url_pattern, "example.com");
        assert_eq!(
            scripts[0].source,
            "function onRequest(request) { return request; }"
        );
        assert_eq!(scripts[0].created_at, "2026-01-01");
    }
    let connection = Connection::open(&path).unwrap();
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM scripts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1);
    let versions: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 6",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(versions, 1);
    let retained: String = connection
        .query_row(
            "SELECT payload FROM scripts WHERE id = 'proxy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retained, proxy);
}
