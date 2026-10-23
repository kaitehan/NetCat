use std::{process::Command, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{PortProcess, SecretStore, SystemProxyBackend};
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

pub fn platform_info() -> crate::models::PlatformInfo {
    let version = Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    crate::models::PlatformInfo {
        os: "macos".into(),
        version,
        arch: std::env::consts::ARCH.into(),
        certificate_trust_supported: false,
    }
}

pub fn port_listener(port: u16) -> Option<PortProcess> {
    let output = Command::new("/usr/sbin/lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut columns = text.lines().nth(1)?.split_whitespace();
    let name = columns.next()?.to_owned();
    let pid = columns.next()?.parse().ok()?;
    Some(PortProcess {
        pid,
        name: Some(name),
    })
}

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
    command: Arc<NetworkCommand>,
}

type NetworkCommand = dyn Fn(&str, &[&str]) -> Result<String, AppError> + Send + Sync;

impl MacOsSystemProxyBackend {
    pub fn new(store: Arc<Store>) -> Self {
        Self {
            store,
            command: Arc::new(run),
        }
    }

    fn managed_service(&self) -> Result<String, AppError> {
        Ok(self.managed_snapshot()?.0.service)
    }

    fn managed_snapshot(&self) -> Result<(MacOsProxySnapshot, bool), AppError> {
        // Use the captured service even after a restart or default-route change.
        let payload = self
            .store
            .get_setting(SNAPSHOT_KEY)?
            .ok_or_else(|| AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "找不到系统代理恢复记录"))?;
        let value: Value = serde_json::from_str(&payload).map_err(snapshot_error)?;
        let snapshot: MacOsProxySnapshot =
            serde_json::from_value(value.get("payload").unwrap_or(&value).clone())
                .map_err(snapshot_error)?;
        let restoring = value
            .get("restoring")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok((snapshot, restoring))
    }
}

impl SystemProxyBackend for MacOsSystemProxyBackend {
    fn platform(&self) -> &'static str {
        "macos"
    }

    fn capture(&self) -> Result<Value, AppError> {
        let service = detect_active_service(self.command.as_ref())?;
        serde_json::to_value(MacOsProxySnapshot {
            web: get_proxy_state(self.command.as_ref(), &service, "-getwebproxy")?,
            secure_web: get_proxy_state(self.command.as_ref(), &service, "-getsecurewebproxy")?,
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
        set_proxy(self.command.as_ref(), &service, "web", &state)?;
        set_proxy(self.command.as_ref(), &service, "secureweb", &state)
    }

    fn is_managed(&self, port: u16) -> Result<bool, AppError> {
        let (snapshot, restoring) = self.managed_snapshot()?;
        let expected = SystemProxyState {
            enabled: true,
            server: "127.0.0.1".into(),
            port,
        };
        let matches = |current: SystemProxyState, original: &SystemProxyState| {
            current == expected
                || restoring
                    && if original.enabled {
                        current == *original
                    } else {
                        // networksetup retains the server/port when disabling a proxy.
                        !current.enabled
                    }
        };
        Ok(matches(
            get_proxy_state(self.command.as_ref(), &snapshot.service, "-getwebproxy")?,
            &snapshot.web,
        ) && matches(
            get_proxy_state(
                self.command.as_ref(),
                &snapshot.service,
                "-getsecurewebproxy",
            )?,
            &snapshot.secure_web,
        ))
    }

    fn restore(&self, snapshot: &Value) -> Result<(), AppError> {
        let snapshot: MacOsProxySnapshot =
            serde_json::from_value(snapshot.clone()).map_err(snapshot_error)?;
        set_proxy(
            self.command.as_ref(),
            &snapshot.service,
            "web",
            &snapshot.web,
        )?;
        set_proxy(
            self.command.as_ref(),
            &snapshot.service,
            "secureweb",
            &snapshot.secure_web,
        )
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

fn detect_active_service(command: &NetworkCommand) -> Result<String, AppError> {
    let route = command("/sbin/route", &["-n", "get", "default"])?;
    let device = parse_default_interface(&route)
        .ok_or_else(|| AppError::new("NETWORK_SERVICE_NOT_FOUND", "无法识别当前默认网络接口"))?;
    let services = command("/usr/sbin/networksetup", &["-listnetworkserviceorder"])?;
    parse_hardware_service(&services, &device).ok_or_else(|| {
        AppError::new(
            "NETWORK_SERVICE_NOT_FOUND",
            format!("找不到接口 {device} 对应的网络服务"),
        )
        .with_action("请在系统设置中确认网络服务已启用")
    })
}

fn get_proxy_state(
    command: &NetworkCommand,
    service: &str,
    operation: &str,
) -> Result<SystemProxyState, AppError> {
    parse_proxy_state(&command("/usr/sbin/networksetup", &[operation, service])?)
}

fn set_proxy(
    command: &NetworkCommand,
    service: &str,
    kind: &str,
    state: &SystemProxyState,
) -> Result<(), AppError> {
    let set_operation = format!("-set{kind}proxy");
    let state_operation = format!("-set{kind}proxystate");
    if state.enabled {
        command(
            "/usr/sbin/networksetup",
            &[
                &set_operation,
                service,
                &state.server,
                &state.port.to_string(),
            ],
        )?;
        command("/usr/sbin/networksetup", &[&state_operation, service, "on"])?;
    } else {
        command(
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
    use crate::system_proxy::SystemProxyManager;
    use serde_json::json;
    use std::sync::Mutex;

    struct FakeNetworkSetup {
        states: Mutex<[SystemProxyState; 2]>,
        fail_https_restore: Mutex<bool>,
    }

    impl FakeNetworkSetup {
        fn new() -> Self {
            Self::with_enabled(false)
        }

        fn with_enabled(enabled: bool) -> Self {
            Self {
                states: Mutex::new(std::array::from_fn(|_| SystemProxyState {
                    enabled,
                    server: "original-proxy".into(),
                    port: 3128,
                })),
                fail_https_restore: Mutex::new(true),
            }
        }

        fn run(&self, program: &str, arguments: &[&str]) -> Result<String, AppError> {
            if program == "/sbin/route" {
                return Ok("interface: en0".into());
            }
            assert_eq!(program, "/usr/sbin/networksetup");
            if arguments[0] == "-listnetworkserviceorder" {
                return Ok("Hardware Port: Wi-Fi\nDevice: en0".into());
            }
            assert_eq!(arguments[1], "Wi-Fi");
            let index = usize::from(arguments[0].contains("secureweb"));
            let mut states = self.states.lock().unwrap();
            let state = &mut states[index];
            match arguments[0] {
                "-getwebproxy" | "-getsecurewebproxy" => Ok(format!(
                    "Enabled: {}\nServer: {}\nPort: {}",
                    if state.enabled { "Yes" } else { "No" },
                    state.server,
                    state.port
                )),
                "-setwebproxy" | "-setsecurewebproxy" => {
                    let mut fail = self.fail_https_restore.lock().unwrap();
                    if index == 1 && arguments[2] == "original-proxy" && *fail {
                        *fail = false;
                        return Err(AppError::new("SYSTEM_PROXY_ERROR", "HTTPS 恢复临时失败"));
                    }
                    state.server = arguments[2].to_owned();
                    state.port = arguments[3].parse().unwrap();
                    Ok(String::new())
                }
                "-setwebproxystate" | "-setsecurewebproxystate" => {
                    let mut fail = self.fail_https_restore.lock().unwrap();
                    if index == 1 && arguments[2] == "off" && *fail {
                        *fail = false;
                        return Err(AppError::new("SYSTEM_PROXY_ERROR", "HTTPS 恢复临时失败"));
                    }
                    state.enabled = arguments[2] == "on";
                    Ok(String::new())
                }
                other => panic!("unexpected networksetup command: {other}"),
            }
        }
    }

    fn manager_with_commands(
        store: Arc<Store>,
        commands: Arc<FakeNetworkSetup>,
    ) -> SystemProxyManager {
        let backend = MacOsSystemProxyBackend {
            store: store.clone(),
            command: Arc::new(move |program, arguments| commands.run(program, arguments)),
        };
        SystemProxyManager::new(store, Arc::new(backend))
    }

    #[test]
    fn restore_retries_after_second_protocol_failure_and_restart() {
        for original_enabled in [false, true] {
            let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
            let commands = Arc::new(FakeNetworkSetup::with_enabled(original_enabled));
            let manager = manager_with_commands(store.clone(), commands.clone());
            manager.enable(8888).unwrap();

            assert_eq!(manager.restore().unwrap_err().code, "SYSTEM_PROXY_ERROR");
            {
                let states = commands.states.lock().unwrap();
                assert_eq!(states[0].enabled, original_enabled);
                if original_enabled {
                    assert_eq!(states[0].server, "original-proxy");
                    assert_eq!(states[0].port, 3128);
                }
                assert!(states[1].enabled);
                assert_eq!(states[1].server, "127.0.0.1");
            }
            assert!(manager.is_managed().unwrap());
            drop(manager);

            let restarted = manager_with_commands(store, commands.clone());
            restarted.restore().unwrap();

            assert!(!restarted.is_managed().unwrap());
            let states = commands.states.lock().unwrap();
            for state in states.iter() {
                assert_eq!(state.enabled, original_enabled);
                if original_enabled {
                    assert_eq!(state.server, "original-proxy");
                    assert_eq!(state.port, 3128);
                }
            }
        }
    }

    #[test]
    fn first_restore_rejects_external_change_back_to_original_disabled_state() {
        let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
        let commands = Arc::new(FakeNetworkSetup::new());
        let manager = manager_with_commands(store.clone(), commands.clone());
        manager.enable(8888).unwrap();
        commands.states.lock().unwrap()[0].enabled = false;
        let before = commands.states.lock().unwrap().clone();

        assert_eq!(manager.restore().unwrap_err().code, "SYSTEM_PROXY_CONFLICT");

        assert_eq!(*commands.states.lock().unwrap(), before);
        let snapshot: Value =
            serde_json::from_str(&store.get_setting(SNAPSHOT_KEY).unwrap().unwrap()).unwrap();
        assert_ne!(snapshot["restoring"], true);
    }

    #[test]
    fn restore_retry_preserves_external_change_to_either_protocol() {
        for protocol in 0..2 {
            let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
            let commands = Arc::new(FakeNetworkSetup::new());
            let manager = manager_with_commands(store.clone(), commands.clone());
            manager.enable(8888).unwrap();
            assert!(manager.restore().is_err());
            commands.states.lock().unwrap()[protocol] = SystemProxyState {
                enabled: true,
                server: "user-proxy".into(),
                port: 9090,
            };
            let before = commands.states.lock().unwrap().clone();
            let restarted = manager_with_commands(store, commands.clone());

            assert_eq!(
                restarted.restore().unwrap_err().code,
                "SYSTEM_PROXY_CONFLICT"
            );

            assert_eq!(*commands.states.lock().unwrap(), before);
            assert!(restarted.is_managed().unwrap());
        }
    }

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
