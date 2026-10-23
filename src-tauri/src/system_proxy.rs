use std::{process::Command, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{error::AppError, storage::Store};

const SNAPSHOT_KEY: &str = "managed_system_proxy_snapshot";

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
    service: String,
    web: SystemProxyState,
    secure_web: SystemProxyState,
}

pub struct SystemProxyManager {
    store: Arc<Store>,
}

impl SystemProxyManager {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    pub fn enable(&self, port: u16) -> Result<(), AppError> {
        if self.store.get_setting(SNAPSHOT_KEY)?.is_some() {
            return Err(AppError::new(
                "SYSTEM_PROXY_MANAGED",
                "NetCat 已接管系统代理",
            ));
        }
        let service = detect_active_service()?;
        let snapshot = ProxySnapshot {
            web: get_proxy_state(&service, "-getwebproxy")?,
            secure_web: get_proxy_state(&service, "-getsecurewebproxy")?,
            service: service.clone(),
        };
        let payload = serde_json::to_string(&snapshot).map_err(|error| {
            AppError::new("SYSTEM_PROXY_ERROR", "无法保存系统代理原配置")
                .with_detail(error.to_string())
        })?;
        self.store.set_setting(SNAPSHOT_KEY, &payload)?;
        if let Err(error) = set_proxy(
            &service,
            "web",
            &SystemProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port,
            },
        )
        .and_then(|_| {
            set_proxy(
                &service,
                "secureweb",
                &SystemProxyState {
                    enabled: true,
                    server: "127.0.0.1".into(),
                    port,
                },
            )
        }) {
            let _ = self.restore();
            return Err(error);
        }
        Ok(())
    }

    pub fn restore(&self) -> Result<(), AppError> {
        let Some(payload) = self.store.get_setting(SNAPSHOT_KEY)? else {
            return Ok(());
        };
        let snapshot: ProxySnapshot = serde_json::from_str(&payload).map_err(|error| {
            AppError::new("SYSTEM_PROXY_RESTORE_ERROR", "系统代理恢复记录损坏")
                .with_detail(error.to_string())
        })?;
        set_proxy(&snapshot.service, "web", &snapshot.web)?;
        set_proxy(&snapshot.service, "secureweb", &snapshot.secure_web)?;
        self.store.delete_setting(SNAPSHOT_KEY)?;
        Ok(())
    }

    pub fn is_managed(&self) -> Result<bool, AppError> {
        Ok(self.store.get_setting(SNAPSHOT_KEY)?.is_some())
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
