use std::{path::PathBuf, sync::Arc};

use chrono::Utc;
use tauri::{Emitter, Manager, State};

use crate::{
    ca::CaManager,
    diagnostics::build_diagnostic_archive,
    error::AppError,
    export::{build_exchange_copy as create_exchange_copy, build_har},
    migration::migrate_legacy_macos_database,
    models::{
        CaStatus, DiagnosticLevel, DiagnosticLog, DiagnosticScope, MockRule, MockRuleMatch,
        MockRuleTestResult, ProxyConfig, ProxyStatus, ReplayDraft, ReplayMode, ReplayRequest,
        ReplayResult, TrafficCopyKind, TrafficDetail, TrafficFilter, TrafficSummary,
    },
    platform::{CertificateTrustDetails, CertificateTrustState, PlatformServices},
    proxy::{ProxyEventSink, ProxyService},
    rules::RuntimeMockRules,
    storage::Store,
    system_proxy::SystemProxyManager,
};

const DISCLAIMER_KEY: &str = "disclaimer_accepted";

struct AppState {
    store: Arc<Store>,
    proxy: Arc<ProxyService>,
    ca: CaManager,
    platform: PlatformServices,
}

enum TrustOperation {
    Status,
    Install,
    Remove,
}

impl AppState {
    fn certificate_trust(
        &self,
        operation: TrustOperation,
    ) -> Result<CertificateTrustState, AppError> {
        match operation {
            TrustOperation::Status => Ok(self.certificate_trust_details()?.summary()),
            TrustOperation::Install => {
                if self.platform.info.certificate_trust_supported && self.ca.status()?.generated {
                    self.ca.check_private_key()?;
                }
                self.public_certificate_trust(operation)
            }
            TrustOperation::Remove => self.public_certificate_trust(operation),
        }
    }

    fn certificate_trust_details(&self) -> Result<CertificateTrustDetails, AppError> {
        Ok(CertificateTrustDetails {
            public_trust: self.public_certificate_trust(TrustOperation::Status)?,
            private_key_state: self.ca.private_key_state(),
        })
    }

    fn public_certificate_trust(
        &self,
        operation: TrustOperation,
    ) -> Result<CertificateTrustState, AppError> {
        if !self.platform.info.certificate_trust_supported {
            return Ok(CertificateTrustState::Unsupported);
        }
        let status = self.ca.status()?;
        if !status.generated {
            return Ok(CertificateTrustState::NotGenerated);
        }
        let certificate = std::fs::read(self.ca.certificate_path()).map_err(|error| {
            AppError::new("FILE_ERROR", "无法读取 CA 证书").with_detail(error.to_string())
        })?;
        let fingerprint = status
            .fingerprint
            .as_deref()
            .ok_or_else(|| AppError::new("CA_FINGERPRINT_INVALID", "CA 证书指纹不可用"))?;
        let trust = &self.platform.certificate_trust;
        match operation {
            TrustOperation::Status => trust.status(&certificate, fingerprint),
            TrustOperation::Install => trust.install(&certificate, fingerprint),
            TrustOperation::Remove => trust.remove(&certificate, fingerprint),
        }
    }
}

#[tauri::command]
fn get_platform_info(state: State<'_, AppState>) -> crate::models::PlatformInfo {
    state.platform.info.clone()
}

#[tauri::command]
fn get_system_proxy_status(
    state: State<'_, AppState>,
) -> Result<crate::models::SystemProxyStatus, AppError> {
    state.platform.system_proxy.status()
}

#[tauri::command]
fn get_certificate_trust_status(
    state: State<'_, AppState>,
) -> Result<CertificateTrustState, AppError> {
    state.certificate_trust(TrustOperation::Status)
}

#[tauri::command]
fn get_certificate_trust_details(
    state: State<'_, AppState>,
) -> Result<CertificateTrustDetails, AppError> {
    state.certificate_trust_details()
}

#[tauri::command]
fn install_ca_trust(state: State<'_, AppState>) -> Result<CertificateTrustState, AppError> {
    state.certificate_trust(TrustOperation::Install)
}

#[tauri::command]
fn remove_ca_trust(state: State<'_, AppState>) -> Result<CertificateTrustState, AppError> {
    state.certificate_trust(TrustOperation::Remove)
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

fn initialize_migrated_ca(migrated: bool, ca: &CaManager) -> bool {
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
    restore_system_proxy_inner(&app, &state.platform.system_proxy)?;
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
    match state.platform.system_proxy.enable(status.port) {
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
    restore_system_proxy_inner(&app, &state.platform.system_proxy)
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
    state.platform.system_proxy.is_managed()
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
    let metadata = diagnostic_metadata(&state)?;
    let archive = build_diagnostic_archive(metadata, &logs)?;
    std::fs::write(destination, archive).map_err(|error| {
        AppError::new("DIAGNOSTIC_EXPORT_ERROR", "无法写入诊断包").with_detail(error.to_string())
    })
}

fn diagnostic_metadata(state: &AppState) -> Result<serde_json::Value, AppError> {
    let mut ca = match state.ca.status() {
        Ok(status) => {
            serde_json::json!({"generated": status.generated, "fingerprint": status.fingerprint})
        }
        Err(_) => {
            serde_json::json!({"generated": false, "fingerprint": null, "errorCode": "CA_PUBLIC_UNAVAILABLE"})
        }
    };
    match state.certificate_trust_details() {
        Ok(details) => {
            ca["trustState"] = serde_json::json!(details.summary());
            ca["publicTrust"] = serde_json::json!(details.public_trust);
            ca["privateKeyState"] = serde_json::json!(details.private_key_state);
        }
        Err(_) => {
            ca["trustState"] = serde_json::json!("unavailable");
            ca["publicTrust"] = serde_json::json!("unavailable");
            ca["privateKeyState"] = serde_json::json!(state.ca.private_key_state());
            ca["trustErrorCode"] = serde_json::json!("CA_TRUST_UNAVAILABLE");
        }
    }
    let metadata = serde_json::json!({
        "application": { "name": "NetCat", "version": env!("CARGO_PKG_VERSION") },
        "platform": state.platform.info,
        "proxy": state.proxy.status(),
        "systemProxy": state.platform.system_proxy.status()?,
        "ca": ca,
        "databaseQuickCheck": state.store.quick_check()?,
        "generatedAt": Utc::now().to_rfc3339(),
    });
    Ok(metadata)
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
            let platform = PlatformServices::current(Arc::clone(&store), &data_directory)
                .map_err(|error| error.to_string())?;
            if let Err(error) = platform.system_proxy.restore() {
                let _ = app.emit("storage-warning", &error);
            }
            let event_sink = Arc::new(TauriEventSink(app.handle().clone()));
            let proxy = Arc::new(ProxyService::with_event_sink(
                Arc::clone(&store),
                event_sink,
                platform.ports.clone(),
            ));
            let ca = CaManager::new(
                data_directory.join("certificates"),
                platform.secrets.clone(),
            );
            initialize_migrated_ca(migrated_from_netcapture, &ca);
            app.manage(AppState {
                store,
                proxy,
                ca,
                platform,
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
            get_platform_info,
            get_system_proxy_status,
            get_certificate_trust_status,
            get_certificate_trust_details,
            install_ca_trust,
            remove_ca_trust,
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
                let _ = state.platform.system_proxy.restore();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
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

    struct FixtureBackend;

    impl crate::platform::SystemProxyBackend for FixtureBackend {
        fn platform(&self) -> &'static str {
            "windows"
        }
        fn capture(&self) -> Result<serde_json::Value, AppError> {
            Ok(serde_json::json!({}))
        }
        fn apply(&self, _: u16) -> Result<(), AppError> {
            Ok(())
        }
        fn is_managed(&self, _: u16) -> Result<bool, AppError> {
            Ok(true)
        }
        fn restore(&self, _: &serde_json::Value) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FixtureCertificateStore(Mutex<Vec<Vec<u8>>>);

    impl crate::platform::windows::certificates::CertificateStoreApi for FixtureCertificateStore {
        fn certificates(&self) -> Result<Vec<Vec<u8>>, AppError> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn add(&self, der: &[u8]) -> Result<(), AppError> {
            self.0.lock().unwrap().push(der.to_vec());
            Ok(())
        }
        fn remove(&self, der: &[u8]) -> Result<(), AppError> {
            self.0.lock().unwrap().retain(|stored| stored != der);
            Ok(())
        }
    }

    fn fixture_state(directory: &std::path::Path) -> AppState {
        let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
        let platform = PlatformServices {
            secrets: Arc::new(MemorySecrets::default()),
            system_proxy: SystemProxyManager::new(store.clone(), Arc::new(FixtureBackend)),
            info: crate::models::PlatformInfo {
                os: "windows".into(),
                version: "10.0.22631".into(),
                arch: "x86_64".into(),
                certificate_trust_supported: true,
            },
            certificate_trust: Arc::new(
                crate::platform::windows::certificates::WindowsCertificateTrust::new(
                    FixtureCertificateStore::default(),
                ),
            ),
            ports: Arc::new(crate::platform::NativePortInspector),
        };
        AppState {
            ca: CaManager::new(directory.to_path_buf(), platform.secrets.clone()),
            proxy: Arc::new(ProxyService::new(store.clone(), platform.ports.clone())),
            store,
            platform,
        }
    }

    #[test]
    fn missing_or_unreadable_key_blocks_install_but_never_public_trust_removal() {
        use CertificateTrustState::*;
        for unreadable in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let mut state = fixture_state(directory.path());
            let public = state.ca.generate().unwrap();
            assert_eq!(
                state.certificate_trust(TrustOperation::Install).unwrap(),
                Trusted
            );
            state.ca = if unreadable {
                CaManager::new(directory.path().to_path_buf(), FailingSecrets)
            } else {
                CaManager::new(directory.path().to_path_buf(), MemorySecrets::default())
            };
            assert_eq!(state.ca.status().unwrap().fingerprint, public.fingerprint);
            let details = serde_json::to_value(state.certificate_trust_details().unwrap()).unwrap();
            assert_eq!(details["publicTrust"], "trusted");
            assert_eq!(
                details["privateKeyState"],
                if unreadable { "unavailable" } else { "missing" }
            );
            let metadata = diagnostic_metadata(&state).unwrap();
            assert_eq!(metadata["ca"]["publicTrust"], "trusted");
            assert_eq!(
                metadata["ca"]["privateKeyState"],
                if unreadable { "unavailable" } else { "missing" }
            );
            assert_eq!(
                state.certificate_trust(TrustOperation::Status).unwrap(),
                if unreadable { Unavailable } else { Mismatch }
            );
            let error = state
                .certificate_trust(TrustOperation::Install)
                .unwrap_err();
            assert_eq!(
                error.code,
                if unreadable {
                    "CA_KEY_UNAVAILABLE"
                } else {
                    "CA_KEY_MISMATCH"
                }
            );
            assert!(error.detail.is_none());
            assert_eq!(
                state.certificate_trust(TrustOperation::Remove).unwrap(),
                NotTrusted
            );
            let certificate = std::fs::read(state.ca.certificate_path()).unwrap();
            assert_eq!(
                state
                    .platform
                    .certificate_trust
                    .status(&certificate, public.fingerprint.as_deref().unwrap())
                    .unwrap(),
                NotTrusted
            );
        }
    }

    #[test]
    fn malformed_key_preserves_public_trust_query_and_explicit_removal() {
        let directory = tempfile::tempdir().unwrap();
        let state = fixture_state(directory.path());
        state.ca.generate().unwrap();
        state.certificate_trust(TrustOperation::Install).unwrap();
        state
            .platform
            .secrets
            .save_private_key("PRIVATE KEY SECRET SENTINEL")
            .unwrap();
        let details = serde_json::to_value(state.certificate_trust_details().unwrap()).unwrap();
        assert_eq!(
            details,
            serde_json::json!({ "publicTrust": "trusted", "privateKeyState": "unavailable" })
        );
        assert_eq!(
            state.ca.load_issuer().err().unwrap().code,
            "CA_KEY_UNAVAILABLE"
        );
        assert_eq!(
            state
                .certificate_trust(TrustOperation::Install)
                .unwrap_err()
                .code,
            "CA_KEY_UNAVAILABLE"
        );
        assert_eq!(
            state.certificate_trust(TrustOperation::Remove).unwrap(),
            CertificateTrustState::NotTrusted
        );
        assert_eq!(
            state.certificate_trust_details().unwrap().public_trust,
            CertificateTrustState::NotTrusted
        );
    }

    struct FailingCertificateTrust;

    impl crate::platform::CertificateTrust for FailingCertificateTrust {
        fn status(&self, _: &[u8], _: &str) -> Result<CertificateTrustState, AppError> {
            Err(AppError::new(
                "SECRET_SENTINEL",
                "Cookie Authorization Token PRIVATE KEY SENTINEL",
            )
            .with_detail("NC-DPAPI"))
        }
        fn install(
            &self,
            pem: &[u8],
            fingerprint: &str,
        ) -> Result<CertificateTrustState, AppError> {
            self.status(pem, fingerprint)
        }
        fn remove(&self, pem: &[u8], fingerprint: &str) -> Result<CertificateTrustState, AppError> {
            self.status(pem, fingerprint)
        }
    }

    #[test]
    fn diagnostic_export_retains_only_stable_state_when_trust_query_fails() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = fixture_state(directory.path());
        state.ca.generate().unwrap();
        state.platform.certificate_trust = Arc::new(FailingCertificateTrust);
        let metadata = diagnostic_metadata(&state).unwrap();
        assert_eq!(metadata["ca"]["publicTrust"], "unavailable");
        assert_eq!(metadata["ca"]["privateKeyState"], "ready");
        assert_eq!(metadata["ca"]["trustErrorCode"], "CA_TRUST_UNAVAILABLE");
        let serialized = metadata.to_string();
        for sensitive in [
            "SENTINEL",
            "Cookie",
            "Authorization",
            "Token",
            "PRIVATE KEY",
            "NC-DPAPI",
        ] {
            assert!(!serialized.contains(sensitive));
        }
        assert!(build_diagnostic_archive(metadata, &[]).is_ok());
    }

    #[test]
    fn different_ca_private_key_reports_mismatch_and_blocks_install() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = fixture_state(directory.path());
        state.ca.generate().unwrap();
        let other = directory.path().join("other");
        CaManager::new(other, state.platform.secrets.clone())
            .generate()
            .unwrap();
        assert_eq!(
            state.certificate_trust(TrustOperation::Status).unwrap(),
            CertificateTrustState::Mismatch
        );
        assert_eq!(
            state
                .certificate_trust(TrustOperation::Install)
                .unwrap_err()
                .code,
            "CA_KEY_MISMATCH"
        );
        // A malformed protected key must also leave public identity readable.
        state.ca = CaManager::new(directory.path().to_path_buf(), FailingSecrets);
        assert!(state.ca.status().unwrap().generated);
    }

    #[test]
    fn diagnostic_export_survives_unreadable_key_with_only_sanitized_trust_state() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = fixture_state(directory.path());
        state.ca.generate().unwrap();
        state.ca = CaManager::new(directory.path().to_path_buf(), FailingSecrets);
        let metadata = diagnostic_metadata(&state).unwrap();
        assert_eq!(metadata["ca"]["trustState"], "unavailable");
        let serialized = metadata.to_string();
        for secret in [
            "SENTINEL",
            "PRIVATE KEY",
            "BEGIN CERTIFICATE",
            "NC-DPAPI",
            "Cookie",
            "Authorization",
            "Token",
        ] {
            assert!(!serialized.contains(secret));
        }
        assert!(build_diagnostic_archive(metadata, &[]).is_ok());
    }

    #[test]
    fn diagnostic_metadata_uses_platform_info_without_private_certificate_data() {
        let directory = tempfile::tempdir().unwrap();
        let state = fixture_state(directory.path());
        state.ca.generate().unwrap();

        let metadata = diagnostic_metadata(&state).unwrap();

        assert_eq!(metadata["platform"]["os"], "windows");
        assert_eq!(metadata["platform"]["version"], "10.0.22631");
        assert!(metadata["ca"]["fingerprint"].is_string());
        assert_eq!(metadata["ca"]["trustState"], "notTrusted");
        assert!(metadata.get("macOS").is_none());
        assert_eq!(metadata["ca"]["publicTrust"], "notTrusted");
        assert_eq!(metadata["ca"]["privateKeyState"], "ready");
        assert_eq!(metadata["ca"].as_object().unwrap().len(), 5);
        let serialized = metadata.to_string();
        for secret in [
            "PRIVATE KEY",
            "BEGIN CERTIFICATE",
            "\"privateKey\":",
            "NC-DPAPI",
        ] {
            assert!(!serialized.contains(secret));
        }
    }

    #[test]
    fn certificate_trust_changes_only_on_explicit_install_or_remove() {
        use crate::platform::CertificateTrustState::*;
        let directory = tempfile::tempdir().unwrap();
        let state = fixture_state(directory.path());
        assert_eq!(
            state.certificate_trust(TrustOperation::Status).unwrap(),
            NotGenerated
        );
        assert_eq!(
            state.certificate_trust(TrustOperation::Install).unwrap(),
            NotGenerated
        );
        state.ca.generate().unwrap();
        diagnostic_metadata(&state).unwrap();
        assert_eq!(
            state.certificate_trust(TrustOperation::Status).unwrap(),
            NotTrusted
        );
        assert_eq!(
            state.certificate_trust(TrustOperation::Install).unwrap(),
            Trusted
        );
        assert_eq!(
            state.certificate_trust(TrustOperation::Status).unwrap(),
            Trusted
        );
        assert_eq!(
            state.certificate_trust(TrustOperation::Remove).unwrap(),
            NotTrusted
        );
    }

    impl SecretStore for FailingSecrets {
        fn save_private_key(&self, _value: &str) -> Result<(), AppError> {
            Err(AppError::new("KEYCHAIN_ERROR", "钥匙串不可用"))
        }

        fn load_private_key(&self) -> Result<Option<String>, AppError> {
            Err(AppError::new("KEYCHAIN_ERROR", "钥匙串不可用")
                .with_detail("PRIVATE KEY NC-DPAPI SENTINEL"))
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
