use std::{path::PathBuf, sync::Arc};

use chrono::Utc;
use tauri::{Emitter, Manager, State};

use crate::{
    ca::{CaManager, KeychainSecretStore},
    error::AppError,
    export::{build_exchange_copy as create_exchange_copy, build_har},
    models::{
        CaStatus, DiagnosticLevel, DiagnosticLog, DiagnosticScope, MockRule, ProxyConfig,
        ProxyStatus, ReplayMode, ReplayResult, TrafficCopyKind, TrafficDetail, TrafficFilter,
        TrafficSummary,
    },
    proxy::{ProxyEventSink, ProxyService},
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

    fn traffic_created(&self, id: &str) {
        let _ = self.0.emit("traffic-created", id);
    }

    fn traffic_updated(&self, id: &str) {
        let _ = self.0.emit("traffic-updated", id);
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
            let store = Arc::new(
                Store::open(
                    &data_directory.join("netcapture.sqlite3"),
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
            list_mock_rules,
            save_mock_rule,
            delete_mock_rule,
            set_mock_enabled,
            export_har,
            build_exchange_copy,
            replay_exchange,
            has_accepted_disclaimer,
            accept_disclaimer,
            open_devtools,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build NetCapture");

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
