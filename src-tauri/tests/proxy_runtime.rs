use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use netcat_lib::{
    ca::{CaManager, SecretStore},
    models::{
        DiagnosticLog, MatchKind, MockRule, ProxyConfig, ProxyMode, ProxyScript, ProxyState,
        ReplayMode, ReplayRequest, ScriptKind, TrafficDraft, TrafficOrigin, TrafficSummary,
    },
    proxy::{ProxyEventSink, ProxyService},
    storage::Store,
};

#[derive(Clone, Default)]
struct MemorySecrets(Arc<Mutex<Option<String>>>);

#[derive(Default)]
struct CollectingEvents {
    traffic: Mutex<Vec<String>>,
    diagnostics: Mutex<Vec<DiagnosticLog>>,
}

impl ProxyEventSink for CollectingEvents {
    fn traffic_created(&self, exchange: &TrafficSummary) {
        self.traffic
            .lock()
            .unwrap()
            .push(format!("created:{}", exchange.id));
    }

    fn traffic_updated(&self, exchange: &TrafficSummary) {
        self.traffic
            .lock()
            .unwrap()
            .push(format!("updated:{}", exchange.id));
    }

    fn diagnostic_log(&self, log: &DiagnosticLog) {
        self.diagnostics.lock().unwrap().push(log.clone());
    }
}

fn replay_source(url: String) -> TrafficDraft {
    let host = url::Url::parse(&url)
        .unwrap()
        .host_str()
        .unwrap()
        .to_string();
    TrafficDraft {
        id: "source".into(),
        started_at: "2026-10-23T10:00:00Z".into(),
        method: "POST".into(),
        url,
        host,
        status: Some(201),
        duration_ms: Some(10),
        request_headers: BTreeMap::from([
            ("content-type".into(), "text/plain".into()),
            ("content-length".into(), "7".into()),
        ]),
        response_headers: BTreeMap::new(),
        request_body: b"payload".to_vec(),
        response_body: Vec::new(),
        content_type: None,
        mocked: false,
        origin: TrafficOrigin::Computer,
        device_token: None,
        device_name: None,
        source_exchange_id: None,
        matched_mock_rule_id: None,
        applied_script_ids: Vec::new(),
        network_profile_id: None,
        stage_timings: Default::default(),
        error: None,
    }
}

#[tokio::test]
async fn replay_works_while_proxy_is_stopped_and_preserves_request_body() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2048];
        let read = socket.read(&mut request).await.unwrap();
        let text = String::from_utf8_lossy(&request[..read]);
        assert!(text.contains("POST /replay"));
        assert!(text.ends_with("payload"));
        socket
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await
            .unwrap();
    });
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    store
        .insert_exchange(replay_source(format!("http://{address}/replay")))
        .unwrap();
    let events = Arc::new(CollectingEvents::default());
    let service = ProxyService::with_event_sink(
        Arc::clone(&store),
        events.clone(),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );

    let result = service
        .replay("source", ReplayMode::Upstream)
        .await
        .unwrap();

    upstream.await.unwrap();
    assert_eq!(result.exchange.status, Some(202));
    assert!(!result.mocked);
    assert_eq!(
        store
            .get_traffic_detail(&result.exchange_id)
            .unwrap()
            .response_body,
        b"ok"
    );
    assert_eq!(events.traffic.lock().unwrap().len(), 2);
    let diagnostics = events.diagnostics.lock().unwrap();
    assert_eq!(diagnostics[0].event, "replay.started");
    assert_eq!(diagnostics.last().unwrap().event, "replay.completed");
}

#[tokio::test]
async fn edited_replay_uses_modified_request_and_links_the_source() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2048];
        let read = socket.read(&mut request).await.unwrap();
        let bytes = &request[..read];
        let text = String::from_utf8_lossy(bytes);
        assert!(text.contains("PUT /edited?mode=fast"));
        assert!(text.to_ascii_lowercase().contains("x-edited: yes"));
        assert!(bytes.ends_with(&[0, 1, 2, 255]));
        socket
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    store
        .insert_exchange(replay_source(format!("http://{address}/original")))
        .unwrap();
    let service = ProxyService::new(
        Arc::clone(&store),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );

    let result = service
        .send_replay(ReplayRequest {
            source_exchange_id: "source".into(),
            method: "PUT".into(),
            url: format!("http://{address}/edited?mode=fast"),
            headers: BTreeMap::from([("x-edited".into(), "yes".into())]),
            body: vec![0, 1, 2, 255],
            mode: ReplayMode::Upstream,
        })
        .await
        .unwrap();

    upstream.await.unwrap();
    let saved = store.get_traffic_detail(&result.exchange_id).unwrap();
    assert_eq!(saved.summary.source_exchange_id.as_deref(), Some("source"));
    assert_eq!(saved.summary.method, "PUT");
    assert_eq!(saved.request_body, vec![0, 1, 2, 255]);
}

#[tokio::test]
async fn replay_failure_emits_a_sanitized_diagnostic_event() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    store
        .insert_exchange(replay_source(format!("http://{address}/replay")))
        .unwrap();
    let events = Arc::new(CollectingEvents::default());
    let service = ProxyService::with_event_sink(
        Arc::clone(&store),
        events.clone(),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );

    let error = service
        .replay("source", ReplayMode::Upstream)
        .await
        .unwrap_err();

    assert_eq!(error.code, "REPLAY_UPSTREAM_ERROR");
    let diagnostics = events.diagnostics.lock().unwrap();
    let failed = diagnostics
        .iter()
        .find(|log| log.event == "replay.failed")
        .unwrap();
    assert_eq!(failed.level, netcat_lib::models::DiagnosticLevel::Error);
    assert_eq!(failed.details["errorCode"], "REPLAY_UPSTREAM_ERROR");
    assert!(!failed.details.contains_key("url"));
    assert!(!failed.details.contains_key("headers"));
    assert!(!failed.details.contains_key("body"));
}

#[tokio::test]
async fn mock_aware_replay_uses_current_rule_without_running_proxy() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    store
        .insert_exchange(replay_source("http://unresolvable.invalid/replay".into()))
        .unwrap();
    store
        .save_mock_rule(&MockRule {
            id: "replay-mock".into(),
            name: "replay".into(),
            enabled: true,
            priority: 1,
            methods: vec!["POST".into()],
            match_kind: MatchKind::Exact,
            url_pattern: "http://unresolvable.invalid/replay".into(),
            status: 203,
            response_headers: BTreeMap::from([("content-type".into(), "text/plain".into())]),
            response_body: "mocked".into(),
            delay_ms: 0,
            created_at: "2026-10-23T10:00:00Z".into(),
            hit_count: 0,
            last_hit_at: None,
            last_exchange_id: None,
            source_exchange_id: None,
        })
        .unwrap();
    let events = Arc::new(CollectingEvents::default());
    let service = ProxyService::with_event_sink(
        Arc::clone(&store),
        events.clone(),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );

    let result = service
        .replay("source", ReplayMode::MockAware)
        .await
        .unwrap();

    assert!(result.mocked);
    assert_eq!(result.exchange.status, Some(203));
    assert_eq!(
        store
            .get_traffic_detail(&result.exchange_id)
            .unwrap()
            .response_body,
        b"mocked"
    );
    let diagnostics = events.diagnostics.lock().unwrap();
    assert!(diagnostics.iter().any(|log| log.event == "mock.matched"));
    assert_eq!(diagnostics.last().unwrap().event, "replay.completed");
}

#[tokio::test]
async fn mock_aware_replay_runs_request_scripts_before_mock_matching() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    store
        .insert_exchange(replay_source("http://unresolvable.invalid/original".into()))
        .unwrap();
    store
        .save_script(&ProxyScript {
            id: "rewrite".into(),
            name: "rewrite".into(),
            enabled: true,
            priority: 10,
            methods: vec!["POST".into()],
            match_kind: MatchKind::Contains,
            url_pattern: "/original".into(),
            source: "function onRequest(ctx) { return { url: ctx.url.replace('/original', '/scripted') }; }".into(),
            kind: ScriptKind::Proxy,
            platform: None,
            builtin: false,
            created_at: "2026-10-26T10:00:00Z".into(),
        })
        .unwrap();
    store
        .save_mock_rule(&MockRule {
            id: "scripted-mock".into(),
            name: "scripted".into(),
            enabled: true,
            priority: 1,
            methods: vec!["POST".into()],
            match_kind: MatchKind::Exact,
            url_pattern: "http://unresolvable.invalid/scripted".into(),
            status: 209,
            response_headers: BTreeMap::new(),
            response_body: "scripted".into(),
            delay_ms: 0,
            created_at: "2026-10-26T10:00:00Z".into(),
            hit_count: 0,
            last_hit_at: None,
            last_exchange_id: None,
            source_exchange_id: None,
        })
        .unwrap();
    let service = ProxyService::new(
        Arc::clone(&store),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );

    let result = service
        .replay("source", ReplayMode::MockAware)
        .await
        .unwrap();

    assert!(result.mocked);
    assert_eq!(result.exchange.status, Some(209));
    let saved = store.get_traffic_detail(&result.exchange_id).unwrap();
    assert_eq!(saved.summary.url, "http://unresolvable.invalid/scripted");
    assert_eq!(saved.response_body, b"scripted");
}

impl SecretStore for MemorySecrets {
    fn save_private_key(&self, value: &str) -> Result<(), netcat_lib::error::AppError> {
        *self.0.lock().unwrap() = Some(value.to_string());
        Ok(())
    }

    fn load_private_key(&self) -> Result<Option<String>, netcat_lib::error::AppError> {
        Ok(self.0.lock().unwrap().clone())
    }
}

#[tokio::test]
async fn proxy_serves_mock_and_persists_exchange() {
    let directory = tempfile::tempdir().unwrap();
    let ca = CaManager::new(directory.path().to_path_buf(), MemorySecrets::default());
    ca.generate().unwrap();
    let store = Arc::new(Store::open_in_memory(5000, 10 * 1024 * 1024).unwrap());
    store
        .save_mock_rule(&MockRule {
            id: "mock-1".into(),
            name: "users".into(),
            enabled: true,
            priority: 10,
            methods: vec!["GET".into()],
            match_kind: MatchKind::Contains,
            url_pattern: "/users".into(),
            status: 200,
            response_headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
            response_body: "{\"source\":\"mock\"}".into(),
            delay_ms: 0,
            created_at: "2026-10-23T10:00:00Z".into(),
            hit_count: 0,
            last_hit_at: None,
            last_exchange_id: None,
            source_exchange_id: None,
        })
        .unwrap();
    let events = Arc::new(CollectingEvents::default());
    let service = ProxyService::with_event_sink(
        Arc::clone(&store),
        events.clone(),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );

    let status = service
        .start(
            ProxyConfig {
                port: 0,
                mode: ProxyMode::Computer,
                capture_enabled: true,
            },
            ca.load_issuer().unwrap(),
            std::fs::read(ca.certificate_path()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.state, ProxyState::Running);

    let proxy_url = format!("http://127.0.0.1:{}", status.port);
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy_url).unwrap())
        .build()
        .unwrap();
    let health = client
        .get("http://netcat.local/__netcat/health")
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), 200);
    assert_eq!(
        health.text().await.unwrap(),
        r#"{"status":"ok","service":"netcat"}"#
    );
    let response = client
        .get("http://unresolvable.invalid/users")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "{\"source\":\"mock\"}");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let rows = store.list_traffic(Default::default(), None, 20).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].mocked);
    assert_eq!(rows[0].host, "unresolvable.invalid");
    let event_names = events.traffic.lock().unwrap().clone();
    assert_eq!(event_names.len(), 2);
    assert!(event_names[0].starts_with("created:"));
    assert!(event_names[1].starts_with("updated:"));
    let diagnostic_names = events
        .diagnostics
        .lock()
        .unwrap()
        .iter()
        .map(|log| log.event.clone())
        .collect::<Vec<_>>();
    assert!(diagnostic_names.contains(&"proxy.started".to_string()));
    assert!(diagnostic_names.contains(&"mock.matched".to_string()));

    let stopped = service.stop().await.unwrap();
    assert_eq!(stopped.state, ProxyState::Stopped);
    assert_eq!(
        events.diagnostics.lock().unwrap().last().unwrap().event,
        "proxy.stopped"
    );
}

#[tokio::test]
async fn live_proxy_can_return_a_synthetic_response_from_a_request_script() {
    let directory = tempfile::tempdir().unwrap();
    let ca = CaManager::new(directory.path().to_path_buf(), MemorySecrets::default());
    ca.generate().unwrap();
    let store = Arc::new(Store::open_in_memory(5000, 10 * 1024 * 1024).unwrap());
    store
        .save_script(&ProxyScript {
            id: "local-response".into(),
            name: "local response".into(),
            enabled: true,
            priority: 10,
            methods: vec!["GET".into()],
            match_kind: MatchKind::Contains,
            url_pattern: "/script".into(),
            source: r#"function onRequest() { return { response: { status: 202, headers: { "content-type": "text/plain" }, bodyText: "from-script" } }; }"#.into(),
            kind: ScriptKind::Proxy,
            platform: None,
            builtin: false,
            created_at: "2026-10-26T10:00:00Z".into(),
        })
        .unwrap();
    let service = ProxyService::new(
        Arc::clone(&store),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );
    let status = service
        .start(
            ProxyConfig {
                port: 0,
                mode: ProxyMode::Computer,
                capture_enabled: true,
            },
            ca.load_issuer().unwrap(),
            std::fs::read(ca.certificate_path()).unwrap(),
        )
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", status.port)).unwrap())
        .build()
        .unwrap();

    let response = client
        .get("http://unresolvable.invalid/script")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 202);
    assert_eq!(response.text().await.unwrap(), "from-script");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let rows = store.list_traffic(Default::default(), None, 20).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, Some(202));
    service.stop().await.unwrap();
}

#[tokio::test]
async fn proxy_forwards_a_real_http_upstream_response() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2048];
        let read = socket.read(&mut request).await.unwrap();
        assert!(String::from_utf8_lossy(&request[..read]).contains("GET /health"));
        socket.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
        ).await.unwrap();
    });

    let directory = tempfile::tempdir().unwrap();
    let ca = CaManager::new(directory.path().to_path_buf(), MemorySecrets::default());
    ca.generate().unwrap();
    let store = Arc::new(Store::open_in_memory(5000, 10 * 1024 * 1024).unwrap());
    let service = ProxyService::new(
        Arc::clone(&store),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );
    let status = service
        .start(
            ProxyConfig {
                port: 0,
                mode: ProxyMode::Computer,
                capture_enabled: true,
            },
            ca.load_issuer().unwrap(),
            std::fs::read(ca.certificate_path()).unwrap(),
        )
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", status.port)).unwrap())
        .build()
        .unwrap();

    let response = client
        .get(format!("http://{upstream_address}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "{\"ok\":true}");
    upstream.await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let rows = store.list_traffic(Default::default(), None, 20).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].mocked);
    service.stop().await.unwrap();
}

#[tokio::test]
async fn https_mock_uses_a_leaf_certificate_trusted_by_the_generated_ca() {
    let directory = tempfile::tempdir().unwrap();
    let ca = CaManager::new(directory.path().to_path_buf(), MemorySecrets::default());
    ca.generate().unwrap();
    let certificate_pem = std::fs::read(ca.certificate_path()).unwrap();
    let store = Arc::new(Store::open_in_memory(5000, 10 * 1024 * 1024).unwrap());
    store
        .save_mock_rule(&MockRule {
            id: "https-mock".into(),
            name: "secure users".into(),
            enabled: true,
            priority: 20,
            methods: vec!["GET".into()],
            match_kind: MatchKind::Contains,
            url_pattern: "/secure".into(),
            status: 200,
            response_headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
            response_body: "{\"tls\":true}".into(),
            delay_ms: 0,
            created_at: "2026-10-23T10:00:00Z".into(),
            hit_count: 0,
            last_hit_at: None,
            last_exchange_id: None,
            source_exchange_id: None,
        })
        .unwrap();
    let service = ProxyService::new(
        Arc::clone(&store),
        Arc::new(netcat_lib::platform::NativePortInspector),
    );
    let status = service
        .start(
            ProxyConfig {
                port: 0,
                mode: ProxyMode::Computer,
                capture_enabled: true,
            },
            ca.load_issuer().unwrap(),
            certificate_pem.clone(),
        )
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", status.port)).unwrap())
        .add_root_certificate(reqwest::Certificate::from_pem(&certificate_pem).unwrap())
        .build()
        .unwrap();

    let response = client
        .get("https://unresolvable.invalid/secure")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "{\"tls\":true}");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let rows = store.list_traffic(Default::default(), None, 20).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].method, "GET");
    assert_eq!(rows[0].status, Some(200));
    service.stop().await.unwrap();
}
