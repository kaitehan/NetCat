use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{error::AppError, platform::SystemProxyBackend};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsProxyConfiguration {
    pub proxy_enable: Option<u32>,
    pub proxy_server: Option<String>,
    pub proxy_override: Option<String>,
    pub auto_config_url: Option<String>,
    pub auto_detect: Option<u32>,
}

pub enum InternetSettingValue {
    Dword(u32),
    String(String),
}

pub trait InternetSettingsApi: Send + Sync {
    fn read(&self) -> Result<WindowsProxyConfiguration, AppError>;
    fn write(
        &self,
        name: &'static str,
        value: Option<InternetSettingValue>,
    ) -> Result<(), AppError>;
    fn notify(&self) -> Result<(), AppError>;
}

pub struct WindowsSystemProxyBackend {
    registry: Arc<dyn InternetSettingsApi>,
}

impl WindowsSystemProxyBackend {
    pub fn new(registry: Arc<dyn InternetSettingsApi>) -> Self {
        Self { registry }
    }
}

impl SystemProxyBackend for WindowsSystemProxyBackend {
    fn platform(&self) -> &'static str {
        "windows"
    }

    fn capture(&self) -> Result<Value, AppError> {
        serde_json::to_value(self.registry.read()?).map_err(snapshot_error)
    }

    fn apply(&self, port: u16) -> Result<(), AppError> {
        self.registry
            .write("ProxyEnable", Some(InternetSettingValue::Dword(1)))?;
        self.registry.write(
            "ProxyServer",
            Some(InternetSettingValue::String(managed_server(port))),
        )?;
        // Preserve bypass, PAC and automatic detection while taking over manual HTTP(S).
        self.registry.notify()
    }

    fn is_managed(&self, port: u16) -> Result<bool, AppError> {
        let current = self.registry.read()?;
        Ok(current.proxy_enable == Some(1) && current.proxy_server == Some(managed_server(port)))
    }

    fn can_restore(&self, snapshot: &Value, port: u16, restoring: bool) -> Result<bool, AppError> {
        let original: WindowsProxyConfiguration =
            serde_json::from_value(snapshot.clone()).map_err(snapshot_error)?;
        let current = self.registry.read()?;
        let enable_matches = current.proxy_enable == Some(1)
            || restoring && current.proxy_enable == original.proxy_enable;
        let server_matches = current.proxy_server == Some(managed_server(port))
            || restoring && current.proxy_server == original.proxy_server;
        Ok(enable_matches
            && server_matches
            && current.proxy_override == original.proxy_override
            && current.auto_config_url == original.auto_config_url
            && current.auto_detect == original.auto_detect)
    }

    fn restore(&self, snapshot: &Value) -> Result<(), AppError> {
        let original: WindowsProxyConfiguration =
            serde_json::from_value(snapshot.clone()).map_err(snapshot_error)?;
        for (name, value) in [
            (
                "ProxyEnable",
                original.proxy_enable.map(InternetSettingValue::Dword),
            ),
            (
                "ProxyServer",
                original.proxy_server.map(InternetSettingValue::String),
            ),
            (
                "ProxyOverride",
                original.proxy_override.map(InternetSettingValue::String),
            ),
            (
                "AutoConfigURL",
                original.auto_config_url.map(InternetSettingValue::String),
            ),
            (
                "AutoDetect",
                original.auto_detect.map(InternetSettingValue::Dword),
            ),
        ] {
            self.registry.write(name, value)?;
        }
        self.registry.notify()
    }
}

fn managed_server(port: u16) -> String {
    format!("http=127.0.0.1:{port};https=127.0.0.1:{port}")
}

fn snapshot_error(error: serde_json::Error) -> AppError {
    AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "系统代理恢复记录损坏")
        .with_detail(error.to_string())
}

#[cfg(target_os = "windows")]
pub struct CurrentUserInternetSettings;

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use std::io::ErrorKind;
    use windows::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
    };
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_SZ},
        types::FromRegValue,
        RegKey,
    };

    const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    impl InternetSettingsApi for CurrentUserInternetSettings {
        fn read(&self) -> Result<WindowsProxyConfiguration, AppError> {
            let key = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(INTERNET_SETTINGS, KEY_READ)
                .map_err(registry_error)?;
            Ok(WindowsProxyConfiguration {
                proxy_enable: read_value(&key, "ProxyEnable", REG_DWORD)?,
                proxy_server: read_value(&key, "ProxyServer", REG_SZ)?,
                proxy_override: read_value(&key, "ProxyOverride", REG_SZ)?,
                auto_config_url: read_value(&key, "AutoConfigURL", REG_SZ)?,
                auto_detect: read_value(&key, "AutoDetect", REG_DWORD)?,
            })
        }

        fn write(
            &self,
            name: &'static str,
            value: Option<InternetSettingValue>,
        ) -> Result<(), AppError> {
            let key = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(INTERNET_SETTINGS, KEY_SET_VALUE)
                .map_err(registry_error)?;
            match value {
                Some(InternetSettingValue::Dword(value)) => key.set_value(name, &value),
                Some(InternetSettingValue::String(value)) => key.set_value(name, &value),
                None => match key.delete_value(name) {
                    Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                    result => result,
                },
            }
            .map_err(registry_error)
        }

        fn notify(&self) -> Result<(), AppError> {
            // These WinINet notifications require neither a buffer nor a handle.
            unsafe {
                InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0)
                    .map_err(notification_error)?;
                InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0)
                    .map_err(notification_error)
            }
        }
    }

    fn read_value<T: FromRegValue>(
        key: &RegKey,
        name: &str,
        kind: winreg::enums::RegType,
    ) -> Result<Option<T>, AppError> {
        match key.get_raw_value(name) {
            Ok(value) if value.vtype == kind => {
                T::from_reg_value(&value).map(Some).map_err(registry_error)
            }
            // Do not normalize an unsupported registry type and then overwrite it on restore.
            Ok(_) => Err(
                AppError::new("SYSTEM_PROXY_ERROR", "系统代理注册表值类型不受支持")
                    .with_detail(name),
            ),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(registry_error(error)),
        }
    }

    fn registry_error(error: std::io::Error) -> AppError {
        AppError::new("SYSTEM_PROXY_ERROR", "无法访问当前用户的系统代理设置")
            .with_detail(error.to_string())
    }

    fn notification_error(error: windows::core::Error) -> AppError {
        AppError::new("SYSTEM_PROXY_ERROR", "无法通知 Windows 更新系统代理设置")
            .with_detail(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{storage::Store, system_proxy::SystemProxyManager};
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryInternetSettings {
        values: Mutex<WindowsProxyConfiguration>,
        notifications: Mutex<usize>,
        fail_notifications: Mutex<usize>,
        fail_write: Mutex<Option<&'static str>>,
    }

    impl MemoryInternetSettings {
        fn from(values: WindowsProxyConfiguration) -> Arc<Self> {
            Arc::new(Self {
                values: Mutex::new(values),
                ..Default::default()
            })
        }
    }

    impl InternetSettingsApi for MemoryInternetSettings {
        fn read(&self) -> Result<WindowsProxyConfiguration, AppError> {
            Ok(self.values.lock().unwrap().clone())
        }

        fn write(
            &self,
            name: &'static str,
            value: Option<InternetSettingValue>,
        ) -> Result<(), AppError> {
            let mut fail = self.fail_write.lock().unwrap();
            if *fail == Some(name) {
                *fail = None;
                return Err(AppError::new("SYSTEM_PROXY_ERROR", "模拟注册表写入失败"));
            }
            let mut values = self.values.lock().unwrap();
            match (name, value) {
                ("ProxyEnable", value) => {
                    values.proxy_enable = value.map(|v| match v {
                        InternetSettingValue::Dword(v) => v,
                        _ => panic!("wrong type"),
                    })
                }
                ("ProxyServer", value) => {
                    values.proxy_server = value.map(|v| match v {
                        InternetSettingValue::String(v) => v,
                        _ => panic!("wrong type"),
                    })
                }
                ("ProxyOverride", value) => {
                    values.proxy_override = value.map(|v| match v {
                        InternetSettingValue::String(v) => v,
                        _ => panic!("wrong type"),
                    })
                }
                ("AutoConfigURL", value) => {
                    values.auto_config_url = value.map(|v| match v {
                        InternetSettingValue::String(v) => v,
                        _ => panic!("wrong type"),
                    })
                }
                ("AutoDetect", value) => {
                    values.auto_detect = value.map(|v| match v {
                        InternetSettingValue::Dword(v) => v,
                        _ => panic!("wrong type"),
                    })
                }
                _ => panic!("unexpected registry value {name}"),
            }
            Ok(())
        }

        fn notify(&self) -> Result<(), AppError> {
            *self.notifications.lock().unwrap() += 1;
            let mut failures = self.fail_notifications.lock().unwrap();
            if *failures > 0 {
                *failures -= 1;
                Err(AppError::new("SYSTEM_PROXY_ERROR", "模拟 WinINet 通知失败"))
            } else {
                Ok(())
            }
        }
    }

    fn original() -> WindowsProxyConfiguration {
        WindowsProxyConfiguration {
            proxy_enable: Some(0),
            proxy_server: Some("old-proxy:3128".into()),
            proxy_override: Some("<local>;*.internal".into()),
            auto_config_url: Some("https://example.test/proxy.pac".into()),
            auto_detect: Some(1),
        }
    }

    fn manager(registry: Arc<MemoryInternetSettings>, store: Arc<Store>) -> SystemProxyManager {
        SystemProxyManager::new(store, Arc::new(WindowsSystemProxyBackend::new(registry)))
    }

    #[test]
    fn apply_sets_http_and_https_without_changing_bypass_or_auto_configuration() {
        let registry = MemoryInternetSettings::from(original());
        WindowsSystemProxyBackend::new(registry.clone())
            .apply(28086)
            .unwrap();
        assert_eq!(
            registry.read().unwrap(),
            WindowsProxyConfiguration {
                proxy_enable: Some(1),
                proxy_server: Some("http=127.0.0.1:28086;https=127.0.0.1:28086".into()),
                ..original()
            }
        );
        assert_eq!(*registry.notifications.lock().unwrap(), 1);
    }

    #[test]
    fn is_managed_requires_both_protocols_and_requested_port_and_enable() {
        for (enabled, server, expected) in [
            (
                Some(1),
                Some("http=127.0.0.1:28086;https=127.0.0.1:28086"),
                true,
            ),
            (
                Some(1),
                Some("http=127.0.0.1:28087;https=127.0.0.1:28087"),
                false,
            ),
            (Some(1), Some("http=127.0.0.1:28086"), false),
            (
                Some(0),
                Some("http=127.0.0.1:28086;https=127.0.0.1:28086"),
                false,
            ),
            (None, None, false),
        ] {
            let registry = MemoryInternetSettings::from(WindowsProxyConfiguration {
                proxy_enable: enabled,
                proxy_server: server.map(str::to_owned),
                ..Default::default()
            });
            assert_eq!(
                WindowsSystemProxyBackend::new(registry)
                    .is_managed(28086)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn capture_and_restore_preserve_missing_empty_and_present_values() {
        for values in [
            WindowsProxyConfiguration::default(),
            WindowsProxyConfiguration {
                proxy_enable: Some(0),
                proxy_server: Some(String::new()),
                proxy_override: Some(String::new()),
                auto_config_url: Some(String::new()),
                auto_detect: Some(0),
            },
            original(),
        ] {
            let registry = MemoryInternetSettings::from(values.clone());
            let backend = WindowsSystemProxyBackend::new(registry.clone());
            let snapshot = backend.capture().unwrap();
            let persisted = serde_json::to_string(&snapshot).unwrap();
            backend.apply(28086).unwrap();
            assert_eq!(
                registry.read().unwrap(),
                WindowsProxyConfiguration {
                    proxy_enable: Some(1),
                    proxy_server: Some("http=127.0.0.1:28086;https=127.0.0.1:28086".into()),
                    ..values.clone()
                }
            );
            backend
                .restore(&serde_json::from_str(&persisted).unwrap())
                .unwrap();
            assert_eq!(registry.read().unwrap(), values);
            assert_eq!(*registry.notifications.lock().unwrap(), 2);
        }
    }

    #[test]
    fn notification_failure_rolls_back_and_clears_coordinator_snapshot() {
        let registry = MemoryInternetSettings::from(original());
        *registry.fail_notifications.lock().unwrap() = 1;
        let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
        let manager = manager(registry.clone(), store);
        assert_eq!(
            manager.enable(28086).unwrap_err().code,
            "SYSTEM_PROXY_ERROR"
        );
        assert_eq!(registry.read().unwrap(), original());
        assert!(!manager.is_managed().unwrap());
        assert_eq!(*registry.notifications.lock().unwrap(), 2);
    }

    #[test]
    fn partial_apply_failure_rolls_back_values() {
        let registry = MemoryInternetSettings::from(original());
        *registry.fail_write.lock().unwrap() = Some("ProxyServer");
        let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
        let manager = manager(registry.clone(), store);
        assert_eq!(
            manager.enable(28086).unwrap_err().message,
            "模拟注册表写入失败"
        );
        assert_eq!(registry.read().unwrap(), original());
        assert!(!manager.is_managed().unwrap());
    }

    #[test]
    fn partial_restore_retries_after_restart_without_losing_missing_values() {
        for values in [original(), WindowsProxyConfiguration::default()] {
            let registry = MemoryInternetSettings::from(values.clone());
            let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
            let first = manager(registry.clone(), store.clone());
            first.enable(28086).unwrap();
            *registry.fail_write.lock().unwrap() = Some("ProxyServer");
            assert!(first.restore().is_err());
            assert!(first.is_managed().unwrap());
            drop(first);
            let restarted = manager(registry.clone(), store);
            restarted.restore().unwrap();
            assert_eq!(registry.read().unwrap(), values);
            assert!(!restarted.is_managed().unwrap());
        }
    }

    #[test]
    fn restore_notification_failure_can_retry_after_values_are_restored() {
        let registry = MemoryInternetSettings::from(original());
        let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
        let first = manager(registry.clone(), store.clone());
        first.enable(28086).unwrap();
        *registry.fail_notifications.lock().unwrap() = 1;
        assert!(first.restore().is_err());
        assert_eq!(registry.read().unwrap(), original());
        manager(registry.clone(), store).restore().unwrap();
        assert_eq!(*registry.notifications.lock().unwrap(), 3);
    }

    #[test]
    fn restore_conflict_preserves_user_changes_and_snapshot_in_both_phases() {
        for restoring in [false, true] {
            for field in [
                "ProxyServer",
                "ProxyEnable",
                "ProxyOverride",
                "AutoConfigURL",
                "AutoDetect",
            ] {
                let registry = MemoryInternetSettings::from(original());
                let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
                let first = manager(registry.clone(), store.clone());
                first.enable(28086).unwrap();
                if restoring {
                    *registry.fail_write.lock().unwrap() = Some("ProxyServer");
                    assert!(first.restore().is_err());
                }
                let changed = match field {
                    "ProxyEnable" | "AutoDetect" => InternetSettingValue::Dword(7),
                    _ => InternetSettingValue::String("user-change".into()),
                };
                registry.write(field, Some(changed)).unwrap();
                let before = registry.read().unwrap();
                let restarted = manager(registry.clone(), store);
                assert_eq!(
                    restarted.restore().unwrap_err().code,
                    "SYSTEM_PROXY_CONFLICT",
                    "{field}, restoring={restoring}"
                );
                assert_eq!(registry.read().unwrap(), before);
                assert!(restarted.is_managed().unwrap());
            }
        }
    }

    #[test]
    fn invalid_snapshot_is_rejected_before_registry_writes() {
        let registry = MemoryInternetSettings::from(original());
        assert_eq!(
            WindowsSystemProxyBackend::new(registry.clone())
                .restore(&serde_json::json!({"proxyEnable": "invalid"}))
                .unwrap_err()
                .code,
            "SYSTEM_PROXY_RESTORE_ERROR"
        );
        assert_eq!(registry.read().unwrap(), original());
        assert_eq!(*registry.notifications.lock().unwrap(), 0);
    }

    #[test]
    fn first_restore_rejects_user_change_back_to_original_proxy_values() {
        for field in ["ProxyEnable", "ProxyServer"] {
            let registry = MemoryInternetSettings::from(original());
            let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
            let manager = manager(registry.clone(), store);
            manager.enable(28086).unwrap();
            let value = if field == "ProxyEnable" {
                InternetSettingValue::Dword(0)
            } else {
                InternetSettingValue::String("old-proxy:3128".into())
            };
            registry.write(field, Some(value)).unwrap();
            let before = registry.read().unwrap();
            assert_eq!(manager.restore().unwrap_err().code, "SYSTEM_PROXY_CONFLICT");
            assert_eq!(registry.read().unwrap(), before);
            assert!(manager.is_managed().unwrap());
        }
    }
}
