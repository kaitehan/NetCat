use std::{path::PathBuf, sync::Arc};

use chrono::Utc;
use tauri::{Emitter, Manager, State};

use crate::{
    ca::{CaManager, KeychainSecretStore, SecretStore},
    diagnostics::build_diagnostic_archive,
    error::AppError,
    export::{build_exchange_copy as create_exchange_copy, build_har},
    migration::migrate_legacy_macos_database,
    models::{
        CaStatus, DiagnosticLevel, DiagnosticLog, DiagnosticScope, MockRule, MockRuleMatch,
        MockRuleTestResult, ProxyConfig, ProxyStatus, ReplayDraft, ReplayMode, ReplayRequest,
        ReplayResult, TrafficCopyKind, TrafficDetail, TrafficFilter, TrafficSummary,
    },
    proxy::{ProxyEventSink, ProxyService},
    rules::RuntimeMockRules,
    storage::Store,
    system_proxy::SystemProxyManager,
};

const DISCLAIMER_KEY: &str = "disclaimer_accepted";

struct AppState {
    store: Arc<Store>,
    proxy: Arc<ProxyService>,
    ca: CaManager<KeychainSecretStore>,
    system_proxy: SystemProxyManager,
}

struct TauriEventSink(tauri::AppHandle);

impl ProxyEventSink for TauriEventSink {
    fn status_changed(&self, status: &ProxyStatus) {
        let _ = self.0.emit("proxy-status-changed", status);
    }

    fn traffic_created(&self, exchange: &TrafficSummary) {
        let _ = self.0.emit("traffic-created", exchange);
    }

    fn traffic_updated(&self, exchange: &TrafficSummary) {
        let _ = self.0.emit("traffic-updated", exchange);
    }

    fn storage_warning(&self, error: &AppError) {
        let _ = self.0.emit("storage-warning", error);
        let _ = self.0.emit(
            "diagnostic-log",
            DiagnosticLog::new(
                DiagnosticLevel::Error,
                DiagnosticScope::System,
                "storage.failed",
                error.message.clone(),
            )
            .with_detail("errorCode", error.code.clone()),
        );
    }

    fn diagnostic_log(&self, log: &DiagnosticLog) {
        let _ = self.0.emit("diagnostic-log", log);
    }
}

fn emit_diagnostic(app: &tauri::AppHandle, log: DiagnosticLog) {
    let _ = app.emit("diagnostic-log", log);
}

fn initialize_migrated_ca<S: SecretStore>(migrated: bool, ca: &CaManager<S>) -> bool {
    if !migrated {
        return false;
    }
    match ca.status() {
        Ok(status) if status.generated => false,
        Ok(_) => match ca.generate() {
            Ok(_) => true,
            Err(error) => {
                eprintln!("NetCat CA generation after migration failed: {error}");
                false
            }
        },
        Err(error) => {
            eprintln!("NetCat CA status after migration failed: {error}");
            false
        }
    }
}

#[tauri::command]
async fn start_proxy(
    state: State<'_, AppState>,
    config: ProxyConfig,
) -> Result<ProxyStatus, AppError> {
    if !state.ca.status()?.generated {
        return Err(AppError::new("CA_NOT_GENERATED", "请先生成本地 CA 证书")
            .with_action("打开证书面板并点击“生成证书”"));
    }
    let issuer = state.ca.load_issuer()?;
    let certificate = std::fs::read(state.ca.certificate_path()).map_err(|error| {
        AppError::new("FILE_ERROR", "无法读取 CA 证书").with_detail(error.to_string())
    })?;
    state.proxy.start(config, issuer, certificate).await
}

#[tauri::command]
async fn stop_proxy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ProxyStatus, AppError> {
    let status = state.proxy.stop().await?;
    restore_system_proxy_inner(&app, &state.system_proxy)?;
    Ok(status)
}

#[tauri::command]
fn get_proxy_status(state: State<'_, AppState>) -> ProxyStatus {
    state.proxy.status()
}

#[tauri::command]
fn set_capture_enabled(state: State<'_, AppState>, enabled: bool) {
    state.proxy.set_capture_enabled(enabled);
}

#[tauri::command]
fn generate_ca(state: State<'_, AppState>) -> Result<CaStatus, AppError> {
    state.ca.generate()
}

#[tauri::command]
fn get_ca_status(state: State<'_, AppState>) -> Result<CaStatus, AppError> {
    state.ca.status()
}

#[tauri::command]
fn export_ca(state: State<'_, AppState>, destination: String) -> Result<(), AppError> {
    state
        .ca
        .export_public_certificate(PathBuf::from(destination))
}

#[tauri::command]
fn enable_system_proxy(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let status = state.proxy.status();
    if status.state != crate::models::ProxyState::Running {
        return Err(AppError::new("PROXY_NOT_RUNNING", "请先启动代理"));
    }
    match state.system_proxy.enable(status.port) {
        Ok(()) => {
            emit_diagnostic(
                &app,
                DiagnosticLog::new(
                    DiagnosticLevel::Info,
                    DiagnosticScope::System,
                    "system_proxy.enabled",
                    "系统代理已启用",
                )
                .with_detail("port", status.port),
            );
            Ok(())
        }
        Err(error) => {
            emit_diagnostic(
                &app,
                DiagnosticLog::new(
                    DiagnosticLevel::Error,
                    DiagnosticScope::System,
                    "system_proxy.failed",
                    error.message.clone(),
                )
                .with_detail("errorCode", error.code.clone()),
            );
            Err(error)
        }
    }
}

#[tauri::command]
fn restore_system_proxy(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    restore_system_proxy_inner(&app, &state.system_proxy)
}

fn restore_system_proxy_inner(
    app: &tauri::AppHandle,
    system_proxy: &SystemProxyManager,
) -> Result<(), AppError> {
    match system_proxy.restore() {
        Ok(()) => {
            emit_diagnostic(
                app,
                DiagnosticLog::new(
                    DiagnosticLevel::Info,
                    DiagnosticScope::System,
                    "system_proxy.restored",
                    "系统代理已恢复",
                ),
            );
            Ok(())
        }
        Err(error) => {
            emit_diagnostic(
                app,
                DiagnosticLog::new(
                    DiagnosticLevel::Error,
                    DiagnosticScope::System,
                    "system_proxy.restore_failed",
                    error.message.clone(),
                )
                .with_detail("errorCode", error.code.clone()),
            );
            Err(error)
        }
    }
}

#[tauri::command]
fn is_system_proxy_enabled(state: State<'_, AppState>) -> Result<bool, AppError> {
    state.system_proxy.is_managed()
}

#[tauri::command]
fn list_traffic(
    state: State<'_, AppState>,
    filter: TrafficFilter,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<TrafficSummary>, AppError> {
    state
        .store
        .list_traffic(filter, cursor, limit.unwrap_or(500))
}

#[tauri::command]
fn get_traffic_detail(state: State<'_, AppState>, id: String) -> Result<TrafficDetail, AppError> {
    state.store.get_traffic_detail(&id)
}

#[tauri::command]
fn clear_traffic(state: State<'_, AppState>) -> Result<(), AppError> {
    state.store.clear_traffic()
}

#[tauri::command]
fn check_database_integrity(state: State<'_, AppState>) -> Result<String, AppError> {
    state.store.quick_check()
}

#[tauri::command]
fn export_diagnostic_bundle(
    state: State<'_, AppState>,
    destination: String,
    logs: Vec<DiagnosticLog>,
) -> Result<(), AppError> {
    let macos_version = std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let ca = state.ca.status()?;
    let metadata = serde_json::json!({
        "application": { "name": "NetCat", "version": env!("CARGO_PKG_VERSION") },
        "macOS": macos_version,
        "proxy": state.proxy.status(),
        "systemProxyManaged": state.system_proxy.is_managed()?,
        "ca": { "generated": ca.generated, "fingerprint": ca.fingerprint },
        "databaseQuickCheck": state.store.quick_check()?,
        "generatedAt": Utc::now().to_rfc3339(),
    });
    let archive = build_diagnostic_archive(metadata, &logs)?;
    std::fs::write(destination, archive).map_err(|error| {
        AppError::new("DIAGNOSTIC_EXPORT_ERROR", "无法写入诊断包").with_detail(error.to_string())
    })
}

#[tauri::command]
fn list_mock_rules(state: State<'_, AppState>) -> Result<Vec<MockRule>, AppError> {
    state.store.list_mock_rules()
}

#[tauri::command]
fn save_mock_rule(state: State<'_, AppState>, mut rule: MockRule) -> Result<MockRule, AppError> {
    if rule.id.trim().is_empty() {
        rule.id = uuid::Uuid::new_v4().to_string();
    }
    if rule.created_at.trim().is_empty() {
        rule.created_at = Utc::now().to_rfc3339();
    }
    state.store.save_mock_rule(&rule)?;
    state.proxy.refresh_rules()?;
    Ok(rule)
}

#[tauri::command]
fn delete_mock_rule(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.store.delete_mock_rule(&id)?;
    state.proxy.refresh_rules()
}

#[tauri::command]
fn set_mock_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> Result<(), AppError> {
    state.store.set_mock_enabled(&id, enabled)?;
    state.proxy.refresh_rules()
}

#[tauri::command]
fn duplicate_mock_rule(state: State<'_, AppState>, id: String) -> Result<MockRule, AppError> {
    let mut rule = state
        .store
        .list_mock_rules()?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| AppError::new("NOT_FOUND", "未找到该 Mock 规则"))?;
    rule.id = uuid::Uuid::new_v4().to_string();
    rule.name = format!("{} 副本", rule.name);
    rule.created_at = Utc::now().to_rfc3339();
    rule.priority += 1;
    rule.hit_count = 0;
    rule.last_hit_at = None;
    rule.last_exchange_id = None;
    state.store.save_mock_rule(&rule)?;
    state.proxy.refresh_rules()?;
    Ok(rule)
}

#[tauri::command]
fn reorder_mock_rules(
    state: State<'_, AppState>,
    ordered_ids: Vec<String>,
) -> Result<(), AppError> {
    state.store.reorder_mock_rules(&ordered_ids)?;
    state.proxy.refresh_rules()
}

#[tauri::command]
fn test_mock_rules(
    state: State<'_, AppState>,
    method: String,
    url: String,
) -> Result<MockRuleTestResult, AppError> {
    let parsed = url::Url::parse(&url).map_err(|error| {
        AppError::new("INVALID_URL", "测试 URL 无效").with_detail(error.to_string())
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::new("INVALID_URL", "仅支持 HTTP/HTTPS URL"));
    }
    let runtime = RuntimeMockRules::compile(state.store.list_mock_rules()?)?;
    let result = runtime.test(&method, &url);
    let matches = result
        .matches
        .iter()
        .map(|rule| MockRuleMatch {
            rule_id: rule.id.clone(),
            name: rule.name.clone(),
            priority: rule.priority,
        })
        .collect::<Vec<_>>();
    Ok(MockRuleTestResult {
        selected_rule_id: result.selected.map(|rule| rule.id.clone()),
        conflict: matches.len() > 1,
        matches,
    })
}

#[tauri::command]
fn export_har(state: State<'_, AppState>, destination: String) -> Result<(), AppError> {
    let har = build_har(&state.store.list_all_details()?)?;
    let body = serde_json::to_vec_pretty(&har).map_err(|error| {
        AppError::new("EXPORT_ERROR", "无法生成 HAR 文件").with_detail(error.to_string())
    })?;
    std::fs::write(destination, body).map_err(|error| {
        AppError::new("EXPORT_ERROR", "无法写入 HAR 文件").with_detail(error.to_string())
    })
}

#[tauri::command]
fn build_exchange_copy(
    state: State<'_, AppState>,
    exchange_id: String,
    kind: TrafficCopyKind,
) -> Result<String, AppError> {
    let detail = state.store.get_traffic_detail(&exchange_id)?;
    create_exchange_copy(&detail, kind)
}

#[tauri::command]
async fn replay_exchange(
    state: State<'_, AppState>,
    exchange_id: String,
    mode: ReplayMode,
) -> Result<ReplayResult, AppError> {
    state.proxy.replay(&exchange_id, mode).await
}

#[tauri::command]
fn get_replay_draft(
    state: State<'_, AppState>,
    exchange_id: String,
) -> Result<ReplayDraft, AppError> {
    let detail = state.store.get_traffic_detail(&exchange_id)?;
    if detail.summary.method.eq_ignore_ascii_case("CONNECT") {
        return Err(AppError::new(
            "REPLAY_UNSUPPORTED_METHOD",
            "CONNECT 请求不能重新发送",
        ));
    }
    if detail.request_body.len() as u64 != detail.request_original_bytes {
        return Err(AppError::new(
            "REPLAY_TRUNCATED_BODY",
            "请求正文不完整，不能重新发送",
        ));
    }
    let parsed = url::Url::parse(&detail.summary.url).map_err(|error| {
        AppError::new("REPLAY_INVALID_URL", "请求 URL 无效").with_detail(error.to_string())
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::new(
            "REPLAY_UNSUPPORTED_URL",
            "仅支持重新发送 HTTP/HTTPS 请求",
        ));
    }
    let binary = !detail.request_body.is_empty() && detail.request_body_text.is_none();
    let content_type = detail
        .request_headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.clone());
    Ok(ReplayDraft {
        source_exchange_id: exchange_id,
        method: detail.summary.method,
        url: detail.summary.url,
        headers: detail.request_headers,
        body: detail.request_body,
        body_text: detail.request_body_text,
        content_type,
        binary,
    })
}

#[tauri::command]
async fn send_replay(
    state: State<'_, AppState>,
    request: ReplayRequest,
) -> Result<ReplayResult, AppError> {
    state.proxy.send_replay(request).await
}

#[tauri::command]
fn has_accepted_disclaimer(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.store.get_setting(DISCLAIMER_KEY)?.as_deref() == Some("true"))
}

#[tauri::command]
fn accept_disclaimer(state: State<'_, AppState>) -> Result<(), AppError> {
    state.store.set_setting(DISCLAIMER_KEY, "true")
}

#[tauri::command]
fn open_devtools(app: tauri::AppHandle) -> Result<(), AppError> {
    let window = app.get_webview_window("main").ok_or_else(|| {
        AppError::new("DEVTOOLS_WINDOW_NOT_FOUND", "无法找到应用主窗口")
            .with_action("请重启应用后重试")
    })?;
    if !window.is_devtools_open() {
        window.open_devtools();
    }
    Ok(())
}

pub fn run() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let data_directory = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("无法读取应用数据目录: {error}"))?;
            let migrated_from_netcapture = migrate_legacy_macos_database(&data_directory)
                .map_err(|error| error.to_string())?;
            let store = Arc::new(
                Store::open(
                    &data_directory.join("netcat.sqlite3"),
                    5_000,
                    10 * 1024 * 1024,
                )
                .map_err(|error| error.to_string())?,
            );
            let system_proxy = SystemProxyManager::new(Arc::clone(&store));
            if let Err(error) = system_proxy.restore() {
                let _ = app.emit("storage-warning", &error);
            }
            let event_sink = Arc::new(TauriEventSink(app.handle().clone()));
            let proxy = Arc::new(ProxyService::with_event_sink(
                Arc::clone(&store),
                event_sink,
            ));
            let ca = CaManager::new(data_directory.join("certificates"), KeychainSecretStore);
            initialize_migrated_ca(migrated_from_netcapture, &ca);
            app.manage(AppState {
                store,
                proxy,
                ca,
                system_proxy,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_proxy,
            stop_proxy,
            get_proxy_status,
            set_capture_enabled,
            generate_ca,
            get_ca_status,
            export_ca,
            enable_system_proxy,
            restore_system_proxy,
            is_system_proxy_enabled,
            list_traffic,
            get_traffic_detail,
            clear_traffic,
            check_database_integrity,
            export_diagnostic_bundle,
            list_mock_rules,
            save_mock_rule,
            delete_mock_rule,
            set_mock_enabled,
            duplicate_mock_rule,
            reorder_mock_rules,
            test_mock_rules,
            export_har,
            build_exchange_copy,
            replay_exchange,
            get_replay_draft,
            send_replay,
            has_accepted_disclaimer,
            accept_disclaimer,
            open_devtools,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build NetCat");

    application.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Some(state) = app_handle.try_state::<AppState>() {
                tauri::async_runtime::block_on(async {
                    let _ = state.proxy.stop().await;
                });
                let _ = state.system_proxy.restore();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::initialize_migrated_ca;
    use crate::{
        ca::{CaManager, SecretStore},
        error::AppError,
    };

    #[derive(Clone, Default)]
    struct MemorySecrets(Arc<Mutex<Option<String>>>);

    impl SecretStore for MemorySecrets {
        fn save_private_key(&self, value: &str) -> Result<(), AppError> {
            *self.0.lock().unwrap() = Some(value.to_string());
            Ok(())
        }

        fn load_private_key(&self) -> Result<Option<String>, AppError> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    #[derive(Clone)]
    struct FailingSecrets;

    impl SecretStore for FailingSecrets {
        fn save_private_key(&self, _value: &str) -> Result<(), AppError> {
            Err(AppError::new("KEYCHAIN_ERROR", "钥匙串不可用"))
        }

        fn load_private_key(&self) -> Result<Option<String>, AppError> {
            Err(AppError::new("KEYCHAIN_ERROR", "钥匙串不可用"))
        }
    }

    #[test]
    fn migrated_ca_is_generated_when_missing() {
        let directory = tempfile::tempdir().unwrap();
        let manager = CaManager::new(directory.path().to_path_buf(), MemorySecrets::default());

        assert!(initialize_migrated_ca(true, &manager));
        assert!(manager.status().unwrap().generated);
    }

    #[test]
    fn migrated_ca_failure_does_not_abort_startup() {
        let directory = tempfile::tempdir().unwrap();
        let manager = CaManager::new(directory.path().to_path_buf(), FailingSecrets);

        assert!(!initialize_migrated_ca(true, &manager));
        assert!(!manager.certificate_path().exists());
    }
}
