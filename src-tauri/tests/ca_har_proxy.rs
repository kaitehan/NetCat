use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use http_body_util::BodyExt;
use netcapture_lib::{
    ca::{CaManager, SecretStore},
    export::build_har,
    models::{MatchKind, MockRule, TrafficDetail, TrafficSummary},
    proxy::build_mock_response,
};

#[derive(Clone, Default)]
struct MemorySecrets(Arc<Mutex<Option<String>>>);

impl SecretStore for MemorySecrets {
    fn save_private_key(&self, value: &str) -> Result<(), netcapture_lib::error::AppError> {
        *self.0.lock().unwrap() = Some(value.to_string());
        Ok(())
    }

    fn load_private_key(&self) -> Result<Option<String>, netcapture_lib::error::AppError> {
        Ok(self.0.lock().unwrap().clone())
    }
}

fn detail() -> TrafficDetail {
    TrafficDetail {
        summary: TrafficSummary {
            id: "traffic-1".into(),
            started_at: "2026-10-23T10:00:00Z".into(),
            method: "GET".into(),
            url: "https://example.test/users".into(),
            host: "example.test".into(),
            status: Some(200),
            duration_ms: Some(25),
            response_bytes: 2,
            content_type: Some("application/json".into()),
            mocked: true,
            truncated: false,
            error: None,
        },
        request_headers: BTreeMap::new(),
        response_headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
        request_body: Vec::new(),
        response_body: b"{}".to_vec(),
        request_body_text: Some(String::new()),
        response_body_text: Some("{}".into()),
        request_original_bytes: 0,
        response_original_bytes: 2,
    }
}

#[test]
fn ca_writes_only_public_certificate_to_disk() {
    let directory = tempfile::tempdir().unwrap();
    let secrets = MemorySecrets::default();
    let manager = CaManager::new(directory.path().to_path_buf(), secrets.clone());

    let status = manager.generate().unwrap();
    let certificate = std::fs::read_to_string(status.certificate_path.unwrap()).unwrap();

    assert!(certificate.contains("BEGIN CERTIFICATE"));
    assert!(!certificate.contains("PRIVATE KEY"));
    assert!(secrets
        .load_private_key()
        .unwrap()
        .unwrap()
        .contains("PRIVATE KEY"));
    assert!(manager.load_issuer().is_ok());
}

#[test]
fn har_uses_version_1_2_and_preserves_mock_marker() {
    let har = build_har(&[detail()]).unwrap();
    assert_eq!(har["log"]["version"], "1.2");
    assert_eq!(
        har["log"]["entries"][0]["request"]["url"],
        "https://example.test/users"
    );
    assert_eq!(har["log"]["entries"][0]["_netcapture_mocked"], true);
}

#[tokio::test]
async fn mock_response_normalizes_transport_headers() {
    let rule = MockRule {
        id: "mock-1".into(),
        name: "users".into(),
        enabled: true,
        priority: 1,
        methods: vec!["GET".into()],
        match_kind: MatchKind::Contains,
        url_pattern: "/users".into(),
        status: 201,
        response_headers: BTreeMap::from([
            ("Content-Type".into(), "application/json".into()),
            ("Content-Length".into(), "999".into()),
            ("Transfer-Encoding".into(), "chunked".into()),
        ]),
        response_body: "{\"ok\":true}".into(),
        delay_ms: 0,
        created_at: "2026-10-23T10:00:00Z".into(),
    };

    let response = build_mock_response(&rule).unwrap();
    assert_eq!(response.status().as_u16(), 201);
    assert_eq!(response.headers()["content-length"], "11");
    assert!(!response.headers().contains_key("transfer-encoding"));
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body.as_ref(), b"{\"ok\":true}");
}
