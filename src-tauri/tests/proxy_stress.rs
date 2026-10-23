use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::future::join_all;
use netcat_lib::{
    ca::{CaManager, SecretStore},
    models::{MatchKind, MockRule, ProxyConfig},
    proxy::ProxyService,
    storage::Store,
};

#[derive(Clone, Default)]
struct MemorySecrets(Arc<std::sync::Mutex<Option<String>>>);

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
#[ignore = "60-second acceptance stress test"]
async fn sustains_200_qps_for_60_seconds_and_keeps_5000_rows() {
    let directory = tempfile::tempdir().unwrap();
    let ca = CaManager::new(directory.path().to_path_buf(), MemorySecrets::default());
    ca.generate().unwrap();
    let store = Arc::new(Store::open_in_memory(5_000, 10 * 1024 * 1024).unwrap());
    store
        .save_mock_rule(&MockRule {
            id: "load-test".into(),
            name: "load test".into(),
            enabled: true,
            priority: 100,
            methods: vec!["GET".into()],
            match_kind: MatchKind::Contains,
            url_pattern: "/load".into(),
            status: 200,
            response_headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
            response_body: "{\"ok\":true}".into(),
            delay_ms: 0,
            created_at: "2026-10-23T10:00:00Z".into(),
        })
        .unwrap();
    let service = ProxyService::new(Arc::clone(&store));
    let status = service
        .start(
            ProxyConfig {
                port: 0,
                allow_lan: false,
                capture_enabled: true,
            },
            ca.load_issuer().unwrap(),
            std::fs::read(ca.certificate_path()).unwrap(),
        )
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(256)
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", status.port)).unwrap())
        .build()
        .unwrap();

    for second in 0..60 {
        let round_started = Instant::now();
        let requests = (0..200).map(|index| {
            let client = client.clone();
            async move {
                client
                    .get(format!("http://load.invalid/load/{second}/{index}"))
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
        });
        join_all(requests).await;
        if let Some(remaining) = Duration::from_secs(1).checked_sub(round_started.elapsed()) {
            tokio::time::sleep(remaining).await;
        }
    }

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if store.list_all_details().unwrap().len() == 5_000 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for persistence tasks"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    service.stop().await.unwrap();
}
