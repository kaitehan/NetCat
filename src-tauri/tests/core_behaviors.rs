use std::collections::BTreeMap;

use netcapture_lib::{
    export::{build_curl, build_exchange_copy, build_har},
    models::{MatchKind, MockRule, ProxyConfig, ProxyStatus, TrafficCopyKind, TrafficDraft},
    rules::{select_mock, validate_rule},
    storage::Store,
};

#[test]
fn proxy_defaults_use_the_product_port_and_capture_traffic() {
    let config = ProxyConfig::default();
    let status = ProxyStatus::default();

    assert_eq!(config.port, 28086);
    assert!(config.capture_enabled);
    assert_eq!(status.port, 28086);
}

fn draft(index: usize, response_body: Vec<u8>) -> TrafficDraft {
    TrafficDraft {
        id: format!("traffic-{index}"),
        started_at: format!("2026-10-23T10:00:{index:02}Z"),
        method: "GET".into(),
        url: format!("https://example.test/{index}"),
        host: "example.test".into(),
        status: Some(200),
        duration_ms: Some(10),
        request_headers: BTreeMap::new(),
        response_headers: BTreeMap::new(),
        request_body: Vec::new(),
        response_body,
        content_type: Some("application/octet-stream".into()),
        mocked: false,
        error: None,
    }
}

#[test]
fn exchange_copy_filters_transport_headers_and_formats_each_target() {
    let store = Store::open_in_memory(10, 1024).unwrap();
    let mut exchange = draft(1, b"{\"ok\":true}".to_vec());
    exchange.method = "POST".into();
    exchange.request_body = b"{\"name\":\"O'Reilly\"}".to_vec();
    exchange.request_headers = BTreeMap::from([
        ("authorization".into(), "Bearer secret".into()),
        ("content-length".into(), "19".into()),
        ("host".into(), "example.test".into()),
        ("x-trace".into(), "abc".into()),
    ]);
    exchange.response_headers =
        BTreeMap::from([("x-b".into(), "2".into()), ("x-a".into(), "1".into())]);
    store.insert_exchange(exchange).unwrap();
    let detail = store.get_traffic_detail("traffic-1").unwrap();

    let curl = build_exchange_copy(&detail, TrafficCopyKind::Curl).unwrap();
    assert!(curl.contains("authorization: Bearer secret"));
    assert!(curl.contains("x-trace: abc"));
    assert!(!curl.contains("content-length"));
    assert!(!curl.contains("host: example.test"));

    let response_headers = build_exchange_copy(&detail, TrafficCopyKind::ResponseHeaders).unwrap();
    assert_eq!(response_headers, "x-a: 1\nx-b: 2");

    let browser = build_exchange_copy(&detail, TrafficCopyKind::FetchBrowser).unwrap();
    assert!(browser.starts_with("fetch(\"https://example.test/1\""));
    assert!(browser.contains("authorization\": \"Bearer secret\""));
}

#[test]
fn exchange_copy_rejects_binary_bodies_instead_of_corrupting_them() {
    let store = Store::open_in_memory(10, 1024).unwrap();
    let mut exchange = draft(1, vec![0, 159, 146, 150]);
    exchange.request_body = vec![0, 159, 146, 150];
    store.insert_exchange(exchange).unwrap();
    let detail = store.get_traffic_detail("traffic-1").unwrap();

    assert_eq!(
        build_exchange_copy(&detail, TrafficCopyKind::Curl)
            .unwrap_err()
            .code,
        "BINARY_REQUEST_BODY"
    );
    assert_eq!(
        build_exchange_copy(&detail, TrafficCopyKind::ResponseBody)
            .unwrap_err()
            .code,
        "BINARY_RESPONSE_BODY"
    );
}

fn rule(id: &str, priority: i64, kind: MatchKind, pattern: &str) -> MockRule {
    MockRule {
        id: id.into(),
        name: id.into(),
        enabled: true,
        priority,
        methods: vec!["GET".into()],
        match_kind: kind,
        url_pattern: pattern.into(),
        status: 200,
        response_headers: BTreeMap::new(),
        response_body: "{}".into(),
        delay_ms: 0,
        created_at: "2026-10-23T10:00:00Z".into(),
    }
}

#[test]
fn mock_selection_uses_priority_then_creation_order() {
    let low = rule("low", 1, MatchKind::Contains, "/users");
    let high = rule("high", 10, MatchKind::Regex, r"/users/\d+$");
    let rules = [low, high];
    let selected = select_mock(&rules, "GET", "https://api.test/users/42").unwrap();
    assert_eq!(selected.id, "high");
}

#[test]
fn invalid_regex_is_rejected_before_runtime() {
    let invalid = rule("bad", 1, MatchKind::Regex, "[");
    assert_eq!(validate_rule(&invalid).unwrap_err().code, "INVALID_REGEX");
}

#[test]
fn curl_escapes_single_quotes_in_headers_and_body() {
    let mut headers = BTreeMap::new();
    headers.insert("X-Name".into(), "O'Reilly".into());
    let result = build_curl(
        "POST",
        "https://example.test/items",
        &headers,
        Some("it's ready"),
    );
    assert_eq!(result, "curl -X 'POST' 'https://example.test/items' -H 'X-Name: O'\"'\"'Reilly' --data-raw 'it'\"'\"'s ready'");
}

#[test]
fn store_keeps_only_the_newest_rows() {
    let store = Store::open_in_memory(2, 8).unwrap();
    for index in 0..3 {
        store
            .insert_exchange(draft(index, vec![index as u8; 12]))
            .unwrap();
    }
    let rows = store.list_traffic(Default::default(), None, 20).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].url, "https://example.test/2");
    let detail = store.get_traffic_detail(&rows[0].id).unwrap();
    assert_eq!(detail.response_body.len(), 8);
    assert!(detail.summary.truncated);
    assert_eq!(detail.response_original_bytes, 12);
}

#[test]
fn store_toggles_mock_rule_and_lists_all_details() {
    let store = Store::open_in_memory(10, 1024).unwrap();
    let enabled = rule("enabled", 1, MatchKind::Contains, "/users");
    store.save_mock_rule(&enabled).unwrap();
    store.set_mock_enabled("enabled", false).unwrap();
    assert!(!store.list_mock_rules().unwrap()[0].enabled);

    store.insert_exchange(draft(1, b"ok".to_vec())).unwrap();
    let details = store.list_all_details().unwrap();
    assert_eq!(details.len(), 1);
    assert_eq!(details[0].summary.id, "traffic-1");
}

#[test]
fn traffic_detail_decodes_gzip_response_body_for_display() {
    let store = Store::open_in_memory(10, 1024).unwrap();
    let mut compressed = draft(
        1,
        vec![
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xab, 0x56, 0xca, 0x4d,
            0x2d, 0x2e, 0x4e, 0x4c, 0x4f, 0x55, 0xb2, 0x52, 0x7a, 0xb2, 0x63, 0xed, 0xb3, 0x69,
            0xed, 0xcf, 0xd6, 0x2e, 0x7e, 0xba, 0x63, 0x87, 0x52, 0x2d, 0x00, 0xda, 0x82, 0x01,
            0x0b, 0x1a, 0x00, 0x00, 0x00,
        ],
    );
    compressed.content_type = Some("application/json; charset=utf-8".into());
    compressed
        .response_headers
        .insert("content-encoding".into(), "gzip".into());
    store.insert_exchange(compressed).unwrap();

    let detail = store.get_traffic_detail("traffic-1").unwrap();

    assert_eq!(
        detail.response_body_text.as_deref(),
        Some(r#"{"message":"中文正常"}"#)
    );
}

#[test]
fn list_all_details_is_not_limited_to_the_ui_page_size() {
    let store = Store::open_in_memory(5_000, 10 * 1024 * 1024).unwrap();
    for index in 0..1_000 {
        store.insert_exchange(draft(index, Vec::new())).unwrap();
    }

    let started = std::time::Instant::now();
    let details = store.list_all_details().unwrap();
    let har = build_har(&details).unwrap();
    let encoded = serde_json::to_vec(&har).unwrap();

    assert_eq!(details.len(), 1_000);
    assert!(!encoded.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}
