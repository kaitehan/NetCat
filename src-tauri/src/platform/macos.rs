use std::{process::Command, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{SecretStore, SystemProxyBackend};
use crate::{
    ca::KEYCHAIN_SERVICE,
    error::AppError,
    storage::Store,
    system_proxy::{
        parse_default_interface, parse_hardware_service, parse_proxy_state, SystemProxyState,
        SNAPSHOT_KEY,
    },
};

const KEYCHAIN_USER: &str = "root-ca-private-key";

#[derive(Clone, Default)]
pub struct KeychainSecretStore;

impl SecretStore for KeychainSecretStore {
    fn save_private_key(&self, value: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(keychain_error)?;
        entry.set_password(value).map_err(keychain_error)
    }

    fn load_private_key(&self) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(keychain_error)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keychain_error(error)),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MacOsProxySnapshot {
    service: String,
    web: SystemProxyState,
    secure_web: SystemProxyState,
}

pub struct MacOsSystemProxyBackend {
    store: Arc<Store>,
}

impl MacOsSystemProxyBackend {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    fn managed_service(&self) -> Result<String, AppError> {
        // Use the captured service even after a restart or default-route change.
        let payload = self
            .store
            .get_setting(SNAPSHOT_KEY)?
            .ok_or_else(|| AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "找不到系统代理恢复记录"))?;
        let value: Value = serde_json::from_str(&payload).map_err(snapshot_error)?;
        let snapshot: MacOsProxySnapshot =
            serde_json::from_value(value.get("payload").unwrap_or(&value).clone())
                .map_err(snapshot_error)?;
        Ok(snapshot.service)
    }
}

impl SystemProxyBackend for MacOsSystemProxyBackend {
    fn platform(&self) -> &'static str {
        "macos"
    }

    fn capture(&self) -> Result<Value, AppError> {
        let service = detect_active_service()?;
        serde_json::to_value(MacOsProxySnapshot {
            web: get_proxy_state(&service, "-getwebproxy")?,
            secure_web: get_proxy_state(&service, "-getsecurewebproxy")?,
            service,
        })
        .map_err(snapshot_error)
    }

    fn apply(&self, port: u16) -> Result<(), AppError> {
        let service = self.managed_service()?;
        let state = SystemProxyState {
            enabled: true,
            server: "127.0.0.1".into(),
            port,
        };
        set_proxy(&service, "web", &state)?;
        set_proxy(&service, "secureweb", &state)
    }

    fn is_managed(&self, port: u16) -> Result<bool, AppError> {
        let service = self.managed_service()?;
        let expected = SystemProxyState {
            enabled: true,
            server: "127.0.0.1".into(),
            port,
        };
        Ok(get_proxy_state(&service, "-getwebproxy")? == expected
            && get_proxy_state(&service, "-getsecurewebproxy")? == expected)
    }

    fn restore(&self, snapshot: &Value) -> Result<(), AppError> {
        let snapshot: MacOsProxySnapshot =
            serde_json::from_value(snapshot.clone()).map_err(snapshot_error)?;
        set_proxy(&snapshot.service, "web", &snapshot.web)?;
        set_proxy(&snapshot.service, "secureweb", &snapshot.secure_web)
    }
}

fn snapshot_error(error: serde_json::Error) -> AppError {
    AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "系统代理恢复记录损坏")
        .with_detail(error.to_string())
}

fn keychain_error(error: keyring::Error) -> AppError {
    AppError::new("KEYCHAIN_ERROR", "无法访问 macOS 钥匙串")
        .with_detail(error.to_string())
        .with_action("请允许 NetCat 访问钥匙串后重试")
}

fn detect_active_service() -> Result<String, AppError> {
    let route = run("/sbin/route", &["-n", "get", "default"])?;
    let device = parse_default_interface(&route)
        .ok_or_else(|| AppError::new("NETWORK_SERVICE_NOT_FOUND", "无法识别当前默认网络接口"))?;
    let services = run("/usr/sbin/networksetup", &["-listnetworkserviceorder"])?;
    parse_hardware_service(&services, &device).ok_or_else(|| {
        AppError::new(
            "NETWORK_SERVICE_NOT_FOUND",
            format!("找不到接口 {device} 对应的网络服务"),
        )
        .with_action("请在系统设置中确认网络服务已启用")
    })
}

fn get_proxy_state(service: &str, operation: &str) -> Result<SystemProxyState, AppError> {
    parse_proxy_state(&run("/usr/sbin/networksetup", &[operation, service])?)
}

fn set_proxy(service: &str, kind: &str, state: &SystemProxyState) -> Result<(), AppError> {
    let set_operation = format!("-set{kind}proxy");
    let state_operation = format!("-set{kind}proxystate");
    if state.enabled {
        run(
            "/usr/sbin/networksetup",
            &[
                &set_operation,
                service,
                &state.server,
                &state.port.to_string(),
            ],
        )?;
        run("/usr/sbin/networksetup", &[&state_operation, service, "on"])?;
    } else {
        run(
            "/usr/sbin/networksetup",
            &[&state_operation, service, "off"],
        )?;
    }
    Ok(())
}

fn run(program: &str, arguments: &[&str]) -> Result<String, AppError> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| {
            AppError::new("SYSTEM_PROXY_ERROR", "无法执行 macOS 网络配置命令")
                .with_detail(error.to_string())
        })?;
    if !output.status.success() {
        return Err(
            AppError::new("SYSTEM_PROXY_ERROR", "macOS 拒绝修改网络代理")
                .with_detail(String::from_utf8_lossy(&output.stderr).into_owned())
                .with_action("请确认当前账户有权修改网络设置"),
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn managed_service_uses_original_service_from_persisted_snapshots() {
        let original = json!({
            "service": "Captured Network Service",
            "web": {"enabled": false, "server": "", "port": 0},
            "secureWeb": {"enabled": false, "server": "", "port": 0}
        });
        for snapshot in [
            original.clone(),
            json!({"version": 1, "platform": "macos", "port": 8888, "payload": original}),
        ] {
            let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
            store
                .set_setting(SNAPSHOT_KEY, &snapshot.to_string())
                .unwrap();
            let backend = MacOsSystemProxyBackend::new(store);

            assert_eq!(
                backend.managed_service().unwrap(),
                "Captured Network Service"
            );
        }
    }
}
