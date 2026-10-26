use std::{net::IpAddr, str::FromStr, sync::Arc};

use chrono::{Duration, Utc};
use netcat_lib::{
    models::{TrafficDraft, TrafficOrigin},
    pairing::{NoopPairingEventSink, PairingService},
    storage::Store,
};

fn mobile_exchange(id: &str, device_token: &str, device_name: &str) -> TrafficDraft {
    TrafficDraft {
        id: id.into(),
        started_at: "2026-10-26T10:00:00Z".into(),
        method: "GET".into(),
        url: "https://api.example.com/users".into(),
        host: "api.example.com".into(),
        status: Some(200),
        duration_ms: Some(12),
        request_headers: Default::default(),
        response_headers: Default::default(),
        request_body: Vec::new(),
        response_body: b"ok".to_vec(),
        content_type: Some("text/plain".into()),
        mocked: false,
        origin: TrafficOrigin::Mobile,
        device_token: Some(device_token.into()),
        device_name: Some(device_name.into()),
        source_exchange_id: None,
        matched_mock_rule_id: None,
        applied_script_ids: Vec::new(),
        network_profile_id: None,
        stage_timings: Default::default(),
        error: None,
    }
}

#[test]
fn pairing_challenge_is_single_use_and_authorizes_the_bound_ip() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    let service = PairingService::new(store, Arc::new(NoopPairingEventSink));
    let challenge = service
        .create_challenge("192.168.1.8", 28086, None)
        .unwrap();
    let token = challenge.url.rsplit('/').next().unwrap();
    let phone_ip = IpAddr::from_str("192.168.1.25").unwrap();

    let device = service.consume_challenge(token, phone_ip).unwrap();

    assert_eq!(device.name, "移动设备 1");
    assert_eq!(device.bound_ip, "192.168.1.25");
    assert_eq!(
        service.authorize(phone_ip).unwrap().device_token,
        device.device_token
    );
    assert!(service.consume_challenge(token, phone_ip).is_err());
}

#[test]
fn idle_cleanup_removes_pairing_but_preserves_captured_history() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    let service = PairingService::new(Arc::clone(&store), Arc::new(NoopPairingEventSink));
    service.set_idle_timeout_minutes(5).unwrap();
    let challenge = service
        .create_challenge("192.168.1.8", 28086, None)
        .unwrap();
    let phone_ip = IpAddr::from_str("192.168.1.25").unwrap();
    let device = service
        .consume_challenge(challenge.url.rsplit('/').next().unwrap(), phone_ip)
        .unwrap();
    store
        .insert_exchange(mobile_exchange(
            "mobile-1",
            &device.device_token,
            &device.name,
        ))
        .unwrap();

    service
        .cleanup_expired_at(Utc::now() + Duration::minutes(6))
        .unwrap();

    assert!(service.list_devices().unwrap().is_empty());
    let history = store.list_traffic(Default::default(), None, 20).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].device_token.as_deref(),
        Some(device.device_token.as_str())
    );
    assert_eq!(history[0].device_name.as_deref(), Some("移动设备 1"));
}

#[test]
fn deleting_device_data_removes_pairing_and_all_its_history() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    let service = PairingService::new(Arc::clone(&store), Arc::new(NoopPairingEventSink));
    let challenge = service
        .create_challenge("192.168.1.8", 28086, None)
        .unwrap();
    let phone_ip = IpAddr::from_str("192.168.1.25").unwrap();
    let device = service
        .consume_challenge(challenge.url.rsplit('/').next().unwrap(), phone_ip)
        .unwrap();
    store
        .insert_exchange(mobile_exchange(
            "mobile-1",
            &device.device_token,
            &device.name,
        ))
        .unwrap();

    let deleted = service.delete_device_data(&device.device_token).unwrap();

    assert_eq!(deleted, 1);
    assert!(service.authorize(phone_ip).is_none());
    assert!(store
        .list_traffic(Default::default(), None, 20)
        .unwrap()
        .is_empty());
}

#[test]
fn destructive_delete_discards_late_writes_for_the_device() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    let service = PairingService::new(Arc::clone(&store), Arc::new(NoopPairingEventSink));
    let challenge = service
        .create_challenge("192.168.1.8", 28086, None)
        .unwrap();
    let phone_ip = IpAddr::from_str("192.168.1.25").unwrap();
    let device = service
        .consume_challenge(challenge.url.rsplit('/').next().unwrap(), phone_ip)
        .unwrap();

    service.delete_device_data(&device.device_token).unwrap();
    store
        .insert_exchange(mobile_exchange(
            "late-mobile-write",
            &device.device_token,
            &device.name,
        ))
        .unwrap();

    assert!(store
        .list_traffic(Default::default(), None, 20)
        .unwrap()
        .is_empty());
}

#[test]
fn re_pairing_moves_a_device_to_the_new_ip() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    let service = PairingService::new(store, Arc::new(NoopPairingEventSink));
    let old_ip = IpAddr::from_str("192.168.1.25").unwrap();
    let first = service
        .create_challenge("192.168.1.8", 28086, None)
        .and_then(|challenge| {
            service.consume_challenge(challenge.url.rsplit('/').next().unwrap(), old_ip)
        })
        .unwrap();
    let new_ip = IpAddr::from_str("192.168.1.31").unwrap();
    let re_pair = service
        .create_challenge("192.168.1.8", 28086, Some(first.device_token.clone()))
        .unwrap();

    let moved = service
        .consume_challenge(re_pair.url.rsplit('/').next().unwrap(), new_ip)
        .unwrap();

    assert_eq!(moved.device_token, first.device_token);
    assert!(service.authorize(old_ip).is_none());
    assert_eq!(
        service.authorize(new_ip).unwrap().device_token,
        moved.device_token
    );
}

#[test]
fn disconnected_history_can_be_renamed_and_paired_again() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    let service = PairingService::new(Arc::clone(&store), Arc::new(NoopPairingEventSink));
    let old_ip = IpAddr::from_str("192.168.1.25").unwrap();
    let challenge = service
        .create_challenge("192.168.1.8", 28086, None)
        .unwrap();
    let device = service
        .consume_challenge(challenge.url.rsplit('/').next().unwrap(), old_ip)
        .unwrap();
    store
        .insert_exchange(mobile_exchange(
            "mobile-history",
            &device.device_token,
            &device.name,
        ))
        .unwrap();
    service.disconnect_device(&device.device_token).unwrap();

    let renamed = service
        .rename_device(&device.device_token, "测试手机")
        .unwrap();
    let new_ip = IpAddr::from_str("192.168.1.42").unwrap();
    let reconnect = service
        .create_challenge("192.168.1.8", 28086, Some(device.device_token.clone()))
        .unwrap();
    let reconnected = service
        .consume_challenge(reconnect.url.rsplit('/').next().unwrap(), new_ip)
        .unwrap();

    assert_eq!(renamed.name, "测试手机");
    assert_eq!(reconnected.device_token, device.device_token);
    assert_eq!(reconnected.name, "测试手机");
    assert_eq!(
        store
            .get_traffic_detail("mobile-history")
            .unwrap()
            .summary
            .device_name
            .as_deref(),
        Some("测试手机")
    );
}

#[test]
fn idle_timeout_accepts_only_five_through_one_hundred_twenty_minutes() {
    let store = Arc::new(Store::open_in_memory(5000, 1024).unwrap());
    let service = PairingService::new(store, Arc::new(NoopPairingEventSink));

    assert!(service.set_idle_timeout_minutes(4).is_err());
    assert_eq!(service.set_idle_timeout_minutes(5).unwrap(), 5);
    assert_eq!(service.set_idle_timeout_minutes(120).unwrap(), 120);
    assert!(service.set_idle_timeout_minutes(121).is_err());
}
