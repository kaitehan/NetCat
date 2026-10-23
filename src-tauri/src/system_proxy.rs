use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::{error::AppError, platform::SystemProxyBackend, storage::Store};

pub(crate) const SNAPSHOT_KEY: &str = "managed_system_proxy_snapshot";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyState {
    pub enabled: bool,
    pub server: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProxySnapshot {
    version: u32,
    platform: String,
    port: u16,
    payload: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restoring: Option<bool>,
}

pub struct SystemProxyManager {
    store: Arc<Store>,
    backend: Arc<dyn SystemProxyBackend>,
    operation: Mutex<()>,
}

impl SystemProxyManager {
    pub fn new(store: Arc<Store>, backend: Arc<dyn SystemProxyBackend>) -> Self {
        Self {
            store,
            backend,
            operation: Mutex::new(()),
        }
    }

    pub fn enable(&self, port: u16) -> Result<(), AppError> {
        let _operation = self.operation.lock().expect("system proxy mutex poisoned");
        if self.store.get_setting(SNAPSHOT_KEY)?.is_some() {
            return Err(AppError::new(
                "SYSTEM_PROXY_MANAGED",
                "NetCat 已接管系统代理",
            ));
        }
        let mut snapshot = ProxySnapshot {
            version: 1,
            platform: self.backend.platform().to_owned(),
            port,
            payload: self.backend.capture()?,
            restoring: None,
        };
        let payload = serde_json::to_string(&snapshot).map_err(|error| {
            AppError::new("SYSTEM_PROXY_ERROR", "无法保存系统代理原配置")
                .with_detail(error.to_string())
        })?;
        self.store.set_setting(SNAPSHOT_KEY, &payload)?;
        let applied = self.backend.apply(port).and_then(|_| {
            if self.backend.is_managed(port)? {
                Ok(())
            } else {
                Err(AppError::new("SYSTEM_PROXY_ERROR", "系统代理配置校验失败"))
            }
        });
        if let Err(error) = applied {
            // A failed apply may leave only one protocol configured. Roll back
            // directly rather than applying the user-change guard to partial state.
            // Try to record recovery progress, but a storage failure must not
            // prevent the immediate best-effort rollback of OS changes.
            let _ = self.mark_restoring(&mut snapshot);
            if self.backend.restore(&snapshot.payload).is_ok() {
                self.store.delete_setting(SNAPSHOT_KEY)?;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn restore(&self) -> Result<(), AppError> {
        let _operation = self.operation.lock().expect("system proxy mutex poisoned");
        let Some(payload) = self.store.get_setting(SNAPSHOT_KEY)? else {
            return Ok(());
        };
        let value: serde_json::Value = serde_json::from_str(&payload).map_err(|error| {
            AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "系统代理恢复记录损坏")
                .with_detail(error.to_string())
        })?;
        if value.get("version").is_none() && self.backend.platform() == "macos" {
            // Pre-platform macOS snapshots have no managed port. Preserve their
            // original recovery behavior so an upgrade can still restore them.
            self.backend.restore(&value)?;
        } else {
            let mut snapshot: ProxySnapshot = serde_json::from_value(value).map_err(|error| {
                AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "系统代理恢复记录损坏")
                    .with_detail(error.to_string())
            })?;
            if snapshot.version != 1 || snapshot.platform != self.backend.platform() {
                return Err(AppError::new(
                    "SYSTEM_PROXY_RESTORE_ERROR",
                    "系统代理恢复记录不适用于当前平台",
                ));
            }
            if !self.backend.can_restore(
                &snapshot.payload,
                snapshot.port,
                snapshot.restoring.unwrap_or(false),
            )? {
                return Err(AppError::new(
                    "SYSTEM_PROXY_CONFLICT",
                    "系统代理已被修改，无法自动恢复",
                )
                .with_action("请检查系统代理设置后重试"));
            }
            // Persist the phase before the first OS write. A retry after a
            // partial restore (including after restart) can recognize progress.
            self.mark_restoring(&mut snapshot)?;
            self.backend.restore(&snapshot.payload)?;
        }
        self.store.delete_setting(SNAPSHOT_KEY)?;
        Ok(())
    }

    pub fn is_managed(&self) -> Result<bool, AppError> {
        Ok(self.store.get_setting(SNAPSHOT_KEY)?.is_some())
    }

    fn mark_restoring(&self, snapshot: &mut ProxySnapshot) -> Result<(), AppError> {
        snapshot.restoring = Some(true);
        let payload = serde_json::to_string(snapshot).map_err(|error| {
            AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "无法保存系统代理恢复进度")
                .with_detail(error.to_string())
        })?;
        self.store.set_setting(SNAPSHOT_KEY, &payload)
    }
}

pub fn parse_default_interface(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "interface").then(|| value.trim().to_string())
    })
}

pub fn parse_hardware_service(output: &str, device: &str) -> Option<String> {
    let mut service = None;
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("Hardware Port:") {
            service = Some(value.trim().to_string());
            continue;
        }
        if trimmed.starts_with('(') && !trimmed.contains("Hardware Port:") {
            if let Some((_, value)) = trimmed.split_once(") ") {
                service = Some(value.trim_start_matches('*').trim().to_string());
            }
            continue;
        }
        if trimmed.contains(&format!("Device: {device}")) {
            return service;
        }
    }
    None
}

pub fn parse_proxy_state(output: &str) -> Result<SystemProxyState, AppError> {
    let mut enabled = false;
    let mut server = String::new();
    let mut port = 0;
    for line in output.lines() {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "Enabled" => enabled = value.trim().eq_ignore_ascii_case("yes"),
                "Server" => server = value.trim().to_string(),
                "Port" => port = value.trim().parse().unwrap_or_default(),
                _ => {}
            }
        }
    }
    Ok(SystemProxyState {
        enabled,
        server,
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::SystemProxyBackend;
    use serde_json::{json, Value};
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeProxyBackend {
        platform: &'static str,
        state: Mutex<Value>,
        fail_apply: bool,
        fail_validation: bool,
        fail_restore: bool,
        restores: Mutex<usize>,
    }

    impl FakeProxyBackend {
        fn set_external_change(&self) {
            *self.state.lock().unwrap() = json!({"server": "user-proxy", "port": 9090});
        }

        fn restore_calls(&self) -> usize {
            *self.restores.lock().unwrap()
        }
    }

    impl SystemProxyBackend for FakeProxyBackend {
        fn platform(&self) -> &'static str {
            self.platform
        }

        fn capture(&self) -> Result<Value, AppError> {
            Ok(self.state.lock().unwrap().clone())
        }

        fn apply(&self, port: u16) -> Result<(), AppError> {
            *self.state.lock().unwrap() = json!({"server": "127.0.0.1", "port": port});
            if self.fail_apply {
                Err(AppError::new("SYSTEM_PROXY_ERROR", "模拟写入失败"))
            } else {
                Ok(())
            }
        }

        fn is_managed(&self, port: u16) -> Result<bool, AppError> {
            Ok(!self.fail_validation
                && *self.state.lock().unwrap() == json!({"server": "127.0.0.1", "port": port}))
        }

        fn restore(&self, snapshot: &Value) -> Result<(), AppError> {
            *self.restores.lock().unwrap() += 1;
            if self.fail_restore {
                return Err(AppError::new("SYSTEM_PROXY_ERROR", "模拟恢复失败"));
            }
            *self.state.lock().unwrap() = snapshot.clone();
            Ok(())
        }
    }

    struct ProxyFixture {
        _directory: tempfile::TempDir,
        store: Arc<Store>,
        backend: Arc<FakeProxyBackend>,
        manager: SystemProxyManager,
    }

    impl ProxyFixture {
        fn new(fail_apply: bool, fail_validation: bool, fail_restore: bool) -> Self {
            Self::with_platform(fail_apply, fail_validation, fail_restore, "fake")
        }

        fn with_platform(
            fail_apply: bool,
            fail_validation: bool,
            fail_restore: bool,
            platform: &'static str,
        ) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let store =
                Arc::new(Store::open(&directory.path().join("proxy.sqlite"), 100, 1024).unwrap());
            let backend = Arc::new(FakeProxyBackend {
                platform,
                state: Mutex::new(json!({"server": "original-proxy", "port": 3128})),
                fail_apply,
                fail_validation,
                fail_restore,
                ..Default::default()
            });
            let manager = SystemProxyManager::new(store.clone(), backend.clone());
            Self {
                _directory: directory,
                store,
                backend,
                manager,
            }
        }

        fn managed() -> Self {
            let fixture = Self::new(false, false, false);
            fixture.manager.enable(8888).unwrap();
            fixture
        }
    }

    #[test]
    fn enable_persists_versioned_snapshot_and_applies_requested_port() {
        let fixture = ProxyFixture::managed();
        let snapshot: Value =
            serde_json::from_str(&fixture.store.get_setting(SNAPSHOT_KEY).unwrap().unwrap())
                .unwrap();

        assert_eq!(
            snapshot,
            json!({
                "version": 1,
                "platform": "fake",
                "port": 8888,
                "payload": {"server": "original-proxy", "port": 3128}
            })
        );
        assert_eq!(
            fixture.backend.capture().unwrap(),
            json!({"server": "127.0.0.1", "port": 8888})
        );
        assert!(fixture.manager.is_managed().unwrap());
    }

    #[test]
    fn apply_failure_rolls_back_partial_changes_and_removes_snapshot() {
        let fixture = ProxyFixture::new(true, false, false);

        assert_eq!(
            fixture.manager.enable(8888).unwrap_err().code,
            "SYSTEM_PROXY_ERROR"
        );
        assert_eq!(
            fixture.backend.capture().unwrap(),
            json!({"server": "original-proxy", "port": 3128})
        );
        assert!(!fixture.manager.is_managed().unwrap());
        assert_eq!(fixture.backend.restore_calls(), 1);
    }

    #[test]
    fn verification_failure_rolls_back_and_removes_snapshot() {
        let fixture = ProxyFixture::new(false, true, false);

        assert_eq!(
            fixture.manager.enable(8888).unwrap_err().code,
            "SYSTEM_PROXY_ERROR"
        );
        assert_eq!(
            fixture.backend.capture().unwrap(),
            json!({"server": "original-proxy", "port": 3128})
        );
        assert!(!fixture.manager.is_managed().unwrap());
    }

    #[test]
    fn restore_preserves_snapshot_when_current_proxy_was_changed() {
        let fixture = ProxyFixture::managed();
        fixture.backend.set_external_change();

        let error = fixture.manager.restore().unwrap_err();

        assert_eq!(error.code, "SYSTEM_PROXY_CONFLICT");
        assert!(fixture.manager.is_managed().unwrap());
        assert_eq!(fixture.backend.restore_calls(), 0);
        assert_eq!(
            fixture.backend.capture().unwrap(),
            json!({"server": "user-proxy", "port": 9090})
        );
    }

    #[test]
    fn restore_returns_original_settings_and_clears_snapshot() {
        let fixture = ProxyFixture::managed();

        fixture.manager.restore().unwrap();

        assert_eq!(
            fixture.backend.capture().unwrap(),
            json!({"server": "original-proxy", "port": 3128})
        );
        assert!(!fixture.manager.is_managed().unwrap());
    }

    #[test]
    fn rollback_failure_preserves_recovery_snapshot() {
        let fixture = ProxyFixture::new(true, false, true);

        assert!(fixture.manager.enable(8888).is_err());
        assert!(fixture.manager.is_managed().unwrap());
        let snapshot: Value =
            serde_json::from_str(&fixture.store.get_setting(SNAPSHOT_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(snapshot["restoring"], true);
    }

    #[test]
    fn failed_restore_persists_restore_phase_for_retry() {
        let fixture = ProxyFixture::new(false, false, true);
        fixture.manager.enable(8888).unwrap();

        assert_eq!(
            fixture.manager.restore().unwrap_err().code,
            "SYSTEM_PROXY_ERROR"
        );

        let snapshot: Value =
            serde_json::from_str(&fixture.store.get_setting(SNAPSHOT_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(snapshot["restoring"], true);
    }

    #[test]
    fn restore_accepts_legacy_macos_snapshot_without_managed_port() {
        let fixture = ProxyFixture::with_platform(false, false, false, "macos");
        let legacy = json!({
            "service": "Wi-Fi",
            "web": {"enabled": false, "server": "", "port": 0},
            "secureWeb": {"enabled": true, "server": "old-proxy", "port": 3128}
        });
        fixture
            .store
            .set_setting(SNAPSHOT_KEY, &legacy.to_string())
            .unwrap();
        fixture.backend.set_external_change();

        fixture.manager.restore().unwrap();

        assert_eq!(fixture.backend.capture().unwrap(), legacy);
        assert!(!fixture.manager.is_managed().unwrap());
    }

    #[test]
    fn restore_preserves_unsupported_snapshots_without_changing_proxy() {
        for snapshot in [
            json!({"version": 2, "platform": "fake", "port": 8888, "payload": {}}),
            json!({"version": 1, "platform": "other", "port": 8888, "payload": {}}),
        ] {
            let fixture = ProxyFixture::managed();
            fixture
                .store
                .set_setting(SNAPSHOT_KEY, &snapshot.to_string())
                .unwrap();

            assert_eq!(
                fixture.manager.restore().unwrap_err().code,
                "SYSTEM_PROXY_RESTORE_ERROR"
            );
            assert_eq!(fixture.backend.restore_calls(), 0);
            assert!(fixture.manager.is_managed().unwrap());
        }
    }

    #[test]
    fn snapshot_storage_failure_does_not_apply_proxy_changes() {
        let fixture = ProxyFixture::new(false, false, false);
        let database =
            rusqlite::Connection::open(fixture._directory.path().join("proxy.sqlite")).unwrap();
        database.execute_batch("CREATE TRIGGER reject_snapshot BEFORE INSERT ON app_settings BEGIN SELECT RAISE(FAIL, 'snapshot write rejected'); END;").unwrap();

        assert_eq!(
            fixture.manager.enable(8888).unwrap_err().code,
            "STORAGE_ERROR"
        );
        assert_eq!(
            fixture.backend.capture().unwrap(),
            json!({"server": "original-proxy", "port": 3128})
        );
        assert!(!fixture.manager.is_managed().unwrap());
    }
}
