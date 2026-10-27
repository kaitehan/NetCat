use netcat_lib::{
    models::{MatchKind, NetworkProfile, NetworkProfileKind, ProxyScript},
    network::{builtin_network_profiles, NetworkConditioner, NetworkFailure, TransferDirection},
    script::{validate_script_source, PipelineRequest, PipelineResponse, ScriptEngine},
    storage::Store,
};
use std::collections::BTreeMap;

fn proxy_script() -> ProxyScript {
    ProxyScript {
        id: "script-1".into(),
        name: "rewrite api".into(),
        enabled: true,
        priority: 10,
        methods: vec!["GET".into()],
        match_kind: MatchKind::Contains,
        url_pattern: "/api".into(),
        source: "function onRequest(ctx) { return { url: ctx.url + '?debug=1' }; }".into(),
        created_at: "2026-10-26T10:00:00Z".into(),
    }
}

#[test]
fn store_round_trips_scripts_and_active_network_profile() {
    let store = Store::open_in_memory(10, 1024).unwrap();
    let script = proxy_script();
    store.save_script(&script).unwrap();

    let profile = NetworkProfile {
        id: "custom-office".into(),
        name: "办公室弱网".into(),
        kind: NetworkProfileKind::Custom,
        latency_ms: 180,
        jitter_ms: 20,
        download_kbps: Some(800),
        upload_kbps: Some(300),
        timeout_percent: 5,
        disconnect_percent: 2,
        methods: vec!["POST".into()],
        match_kind: MatchKind::Contains,
        url_pattern: "/upload".into(),
        builtin: false,
        created_at: "2026-10-26T10:00:00Z".into(),
    };
    store.save_network_profile(&profile).unwrap();
    store.set_active_network_profile(Some(&profile.id)).unwrap();

    assert_eq!(store.list_scripts().unwrap(), vec![script]);
    assert_eq!(store.list_network_profiles().unwrap(), vec![profile]);
    assert_eq!(
        store.active_network_profile_id().unwrap().as_deref(),
        Some("custom-office")
    );
}

#[test]
fn chrome_network_presets_have_stable_transport_values() {
    let profiles = builtin_network_profiles();
    let fast = profiles
        .iter()
        .find(|profile| profile.id == "fast-4g")
        .unwrap();
    let slow = profiles
        .iter()
        .find(|profile| profile.id == "slow-4g")
        .unwrap();
    let three_g = profiles.iter().find(|profile| profile.id == "3g").unwrap();
    let offline = profiles
        .iter()
        .find(|profile| profile.id == "offline")
        .unwrap();

    assert_eq!(
        (fast.download_kbps, fast.upload_kbps, fast.latency_ms),
        (Some(9_000), Some(1_500), 20)
    );
    assert_eq!(
        (slow.download_kbps, slow.upload_kbps, slow.latency_ms),
        (Some(1_600), Some(750), 150)
    );
    assert_eq!(
        (
            three_g.download_kbps,
            three_g.upload_kbps,
            three_g.latency_ms
        ),
        (Some(750), Some(250), 100)
    );
    assert_eq!(offline.timeout_percent, 100);
}

#[test]
fn proxy_scripts_apply_in_priority_order_and_fail_open() {
    let engine = ScriptEngine::default();
    let mut high = proxy_script();
    high.id = "high".into();
    high.priority = 20;
    high.source = r#"function onRequest(ctx) {
        return { headers: { ...ctx.headers, "x-script": "high" }, bodyText: ctx.bodyText + "-high" };
    }"#.into();
    let mut broken = proxy_script();
    broken.id = "broken".into();
    broken.priority = 10;
    broken.source = "function onRequest() { throw new Error('nope'); }".into();
    let mut low = proxy_script();
    low.id = "low".into();
    low.priority = 1;
    low.source = r#"function onRequest(ctx) {
        return { url: ctx.url + "?scripted=1", bodyText: ctx.bodyText + "-low" };
    }"#
    .into();
    let mut request = PipelineRequest {
        method: "GET".into(),
        url: "https://example.test/api".into(),
        headers: BTreeMap::from([("content-type".into(), "text/plain".into())]),
        body: b"start".to_vec(),
    };

    let report = engine.apply_request(&mut request, &[low, broken, high]);

    assert_eq!(report.applied_script_ids, vec!["high", "low"]);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].script_id, "broken");
    assert_eq!(request.url, "https://example.test/api?scripted=1");
    assert_eq!(request.headers["x-script"], "high");
    assert_eq!(request.body, b"start-high-low");
}

#[test]
fn network_conditioner_matches_rules_and_plans_transport_effects() {
    let profile = NetworkProfile {
        id: "uploads".into(),
        name: "上传弱网".into(),
        kind: NetworkProfileKind::Custom,
        latency_ms: 100,
        jitter_ms: 0,
        download_kbps: Some(800),
        upload_kbps: Some(100),
        timeout_percent: 10,
        disconnect_percent: 20,
        methods: vec!["POST".into()],
        match_kind: MatchKind::Contains,
        url_pattern: "/upload".into(),
        builtin: false,
        created_at: "2026-10-26T10:00:00Z".into(),
    };
    let conditioner = NetworkConditioner::new(Some(profile));

    assert!(conditioner.matches("POST", "https://example.test/upload/1"));
    assert!(!conditioner.matches("GET", "https://example.test/upload/1"));
    assert_eq!(
        conditioner.delay_ms(TransferDirection::Upload, 12_500, 0),
        1_100
    );
    assert_eq!(
        conditioner.failure_for_sample(5),
        Some(NetworkFailure::Timeout)
    );
    assert_eq!(
        conditioner.failure_for_sample(15),
        Some(NetworkFailure::Disconnect)
    );
    assert_eq!(conditioner.failure_for_sample(50), None);
}

#[test]
fn scripts_can_short_circuit_requests_and_rewrite_responses() {
    let engine = ScriptEngine::default();
    let mut short_circuit = proxy_script();
    short_circuit.source = r#"function onRequest() {
        return { response: { status: 202, headers: { "content-type": "application/json" }, bodyText: "{\"local\":true}" } };
    }"#.into();
    let mut request = PipelineRequest {
        method: "GET".into(),
        url: "https://example.test/api".into(),
        headers: BTreeMap::new(),
        body: Vec::new(),
    };

    let request_report = engine.apply_request(&mut request, &[short_circuit]);
    let synthetic = request_report.synthetic_response.unwrap();
    assert_eq!(synthetic.status, 202);
    assert_eq!(synthetic.body, br#"{"local":true}"#);

    let mut response_script = proxy_script();
    response_script.source = r#"function onResponse(ctx) {
        return { status: 203, headers: { ...ctx.headers, "x-scripted": "yes" }, bodyText: ctx.bodyText + "!" };
    }"#.into();
    let mut response = PipelineResponse {
        status: 200,
        headers: BTreeMap::from([("content-type".into(), "text/plain".into())]),
        body: b"ok".to_vec(),
    };

    let response_report = engine.apply_response(&request, &mut response, &[response_script]);
    assert_eq!(response_report.applied_script_ids, vec!["script-1"]);
    assert_eq!(response.status, 203);
    assert_eq!(response.headers["x-scripted"], "yes");
    assert_eq!(response.headers["content-length"], "3");
    assert_eq!(response.body, b"ok!");
}

#[test]
fn script_validation_rejects_invalid_javascript_without_executing_top_level_code() {
    assert!(validate_script_source("function onRequest(ctx) { return {}; }").is_ok());
    assert!(validate_script_source("throw new Error('must not execute');").is_ok());
    assert!(validate_script_source("function broken( {").is_err());
}

#[test]
fn scripts_cannot_rewrite_unsupported_or_oversized_bodies() {
    let mut script = proxy_script();
    script.id = "guard".into();
    script.methods = vec!["POST".into()];
    script.source = r#"function onRequest(ctx) {
          return { headers: { "x-script": "applied" }, bodyText: "replaced" };
        }"#
    .into();
    let mut encoded = PipelineRequest {
        method: "POST".into(),
        url: "https://example.test/api".into(),
        headers: BTreeMap::from([("content-encoding".into(), "gzip".into())]),
        body: b"encoded".to_vec(),
    };
    ScriptEngine.apply_request(&mut encoded, &[script.clone()]);
    assert_eq!(encoded.body, b"encoded");
    assert_eq!(
        encoded.headers.get("x-script").map(String::as_str),
        Some("applied")
    );

    let mut oversized = PipelineRequest {
        method: "POST".into(),
        url: "https://example.test/api".into(),
        headers: BTreeMap::new(),
        body: vec![b'a'; 10 * 1024 * 1024 + 1],
    };
    ScriptEngine.apply_request(&mut oversized, &[script]);
    assert_eq!(oversized.body.len(), 10 * 1024 * 1024 + 1);
    assert_eq!(oversized.body[0], b'a');
}
