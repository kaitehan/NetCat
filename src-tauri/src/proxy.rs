use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use chrono::Utc;
use http_body_util::BodyExt;
use hudsucker::{
    certificate_authority::RcgenAuthority,
    hyper::{
        header::{HeaderName, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE, HOST, TRANSFER_ENCODING},
        Request, Response, StatusCode,
    },
    rcgen::{Issuer, KeyPair},
    rustls::crypto::aws_lc_rs,
    Body, HttpContext, HttpHandler, Proxy, RequestOrResponse,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
};

use crate::{
    error::AppError,
    export::filtered_request_headers,
    models::{
        DiagnosticLevel, DiagnosticLog, DiagnosticScope, MockRule, ProxyConfig, ProxyScript,
        ProxyState, ProxyStatus, ReplayMode, ReplayRequest, ReplayResult, TrafficDraft,
        TrafficSummary,
    },
    network::{builtin_network_profiles, NetworkConditioner, NetworkFailure, TransferDirection},
    platform::PortInspector,
    rules::RuntimeMockRules,
    script::{PipelineRequest, PipelineResponse, ScriptEngine},
    storage::Store,
};

pub const CA_DOWNLOAD_PATH: &str = "/__netcat/ca.crt";
pub const HEALTH_PATH: &str = "/__netcat/health";

pub fn local_lan_ip() -> Option<String> {
    local_ip_address::local_ip()
        .ok()
        .filter(|ip| is_private_client(*ip) && !ip.is_loopback())
        .map(|ip| ip.to_string())
}

pub trait ProxyEventSink: Send + Sync + 'static {
    fn status_changed(&self, _status: &ProxyStatus) {}
    fn traffic_created(&self, _exchange: &TrafficSummary) {}
    fn traffic_updated(&self, _exchange: &TrafficSummary) {}
    fn storage_warning(&self, _error: &AppError) {}
    fn diagnostic_log(&self, _log: &DiagnosticLog) {}
}

#[derive(Default)]
pub struct NoopEventSink;

impl ProxyEventSink for NoopEventSink {}

pub struct ProxyService {
    store: Arc<Store>,
    rules: Arc<RwLock<RuntimeMockRules>>,
    capture_enabled: Arc<AtomicBool>,
    status: RwLock<ProxyStatus>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    task: Mutex<Option<JoinHandle<Result<(), hudsucker::Error>>>>,
    event_sink: Arc<dyn ProxyEventSink>,
    port_inspector: Arc<dyn PortInspector>,
}

impl ProxyService {
    pub fn new(store: Arc<Store>, port_inspector: Arc<dyn PortInspector>) -> Self {
        Self::with_event_sink(store, Arc::new(NoopEventSink), port_inspector)
    }

    pub fn with_event_sink(
        store: Arc<Store>,
        event_sink: Arc<dyn ProxyEventSink>,
        port_inspector: Arc<dyn PortInspector>,
    ) -> Self {
        Self {
            store,
            rules: Arc::new(RwLock::new(RuntimeMockRules::default())),
            capture_enabled: Arc::new(AtomicBool::new(true)),
            status: RwLock::new(ProxyStatus::default()),
            shutdown: Mutex::new(None),
            task: Mutex::new(None),
            event_sink,
            port_inspector,
        }
    }

    pub fn status(&self) -> ProxyStatus {
        self.status
            .read()
            .expect("proxy status lock poisoned")
            .clone()
    }

    pub fn set_capture_enabled(&self, enabled: bool) {
        self.capture_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn refresh_rules(&self) -> Result<(), AppError> {
        *self.rules.write().expect("proxy rules lock poisoned") =
            RuntimeMockRules::compile(self.store.list_mock_rules()?)?;
        Ok(())
    }

    fn queue_mock_hit(&self, rule_id: String, exchange_id: String) {
        let store = Arc::clone(&self.store);
        let event_sink = Arc::clone(&self.event_sink);
        let timestamp = Utc::now().to_rfc3339();
        tokio::spawn(async move {
            match tokio::task::spawn_blocking(move || {
                store.record_mock_hit(&rule_id, &exchange_id, &timestamp)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => event_sink.storage_warning(&error),
                Err(error) => event_sink.storage_warning(
                    &AppError::new("STORAGE_TASK_ERROR", "Mock 命中统计写入任务异常")
                        .with_detail(error.to_string()),
                ),
            }
        });
    }

    pub async fn replay(
        &self,
        exchange_id: &str,
        mode: ReplayMode,
    ) -> Result<ReplayResult, AppError> {
        let original = self.store.get_traffic_detail(exchange_id)?;
        self.send_replay(ReplayRequest {
            source_exchange_id: exchange_id.to_string(),
            method: original.summary.method,
            url: original.summary.url,
            headers: original.request_headers,
            body: original.request_body,
            mode,
        })
        .await
    }

    pub async fn send_replay(&self, request: ReplayRequest) -> Result<ReplayResult, AppError> {
        self.event_sink.diagnostic_log(
            &DiagnosticLog::new(
                DiagnosticLevel::Info,
                DiagnosticScope::Replay,
                "replay.started",
                "开始重新发送请求",
            )
            .with_exchange_id(&request.source_exchange_id)
            .with_detail("mode", replay_mode_name(request.mode)),
        );
        let original = self.store.get_traffic_detail(&request.source_exchange_id)?;
        if original.summary.method.eq_ignore_ascii_case("CONNECT") {
            return Err(
                AppError::new("REPLAY_UNSUPPORTED_METHOD", "CONNECT 请求不能重新发送")
                    .with_action("请选择普通 HTTP 或 HTTPS 请求"),
            );
        }
        let mut pipeline_request = PipelineRequest {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
        };
        let scripts = self.store.list_scripts()?;
        let network = active_network_conditioner(&self.store)?;
        let request_script_started = Instant::now();
        let script_report = ScriptEngine.apply_request(&mut pipeline_request, &scripts);
        let mut stage_timings = BTreeMap::from([(
            "requestScript".to_string(),
            request_script_started.elapsed().as_millis() as u64,
        )]);
        for failure in &script_report.errors {
            self.event_sink.diagnostic_log(
                &DiagnosticLog::new(
                    DiagnosticLevel::Warn,
                    DiagnosticScope::Script,
                    "script.failed",
                    "请求脚本执行失败，已放行当前流量",
                )
                .with_exchange_id(&request.source_exchange_id)
                .with_detail("scriptId", failure.script_id.clone()),
            );
        }
        let parsed_url = url::Url::parse(&pipeline_request.url).map_err(|error| {
            AppError::new("REPLAY_INVALID_URL", "请求 URL 无效").with_detail(error.to_string())
        })?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(AppError::new(
                "REPLAY_UNSUPPORTED_URL",
                "仅支持重新发送 HTTP/HTTPS 请求",
            ));
        }
        if original.request_body.len() as u64 != original.request_original_bytes {
            return Err(
                AppError::new("REPLAY_TRUNCATED_BODY", "请求正文不完整，不能重新发送")
                    .with_action("请使用原始客户端重新发起该请求"),
            );
        }

        let id = uuid::Uuid::new_v4().to_string();
        let started = Instant::now();
        let started_at = Utc::now().to_rfc3339();
        let mut base = TrafficDraft {
            id: id.clone(),
            started_at: started_at.clone(),
            method: pipeline_request.method.clone(),
            url: pipeline_request.url.clone(),
            host: parsed_url.host_str().unwrap_or_default().to_string(),
            status: None,
            duration_ms: None,
            request_headers: pipeline_request.headers.clone(),
            response_headers: BTreeMap::new(),
            request_body: pipeline_request.body.clone(),
            response_body: Vec::new(),
            content_type: None,
            mocked: false,
            source_exchange_id: Some(request.source_exchange_id),
            matched_mock_rule_id: None,
            applied_script_ids: script_report.applied_script_ids.clone(),
            network_profile_id: network.profile().map(|profile| profile.id.clone()),
            stage_timings: stage_timings.clone(),
            error: None,
        };
        self.store.insert_exchange(base.clone())?;
        self.event_sink
            .traffic_created(&self.store.get_traffic_detail(&id)?.summary);

        let upload_started = Instant::now();
        let upload_result = apply_network_stage(
            &network,
            TransferDirection::Upload,
            &pipeline_request.method,
            &pipeline_request.url,
            pipeline_request.body.len(),
        )
        .await;
        stage_timings.insert(
            "networkUpload".into(),
            upload_started.elapsed().as_millis() as u64,
        );
        base.stage_timings = stage_timings.clone();
        if let Err(error) = upload_result {
            let mut failed = base;
            failed.stage_timings = stage_timings;
            failed.duration_ms = Some(started.elapsed().as_millis() as u64);
            failed.error = Some(error.message.clone());
            self.store.insert_exchange(failed)?;
            self.event_sink
                .traffic_updated(&self.store.get_traffic_detail(&id)?.summary);
            return Err(error);
        }

        if let Some(synthetic) = script_report.synthetic_response {
            let (response, body) = apply_response_pipeline(
                &pipeline_request,
                synthetic.status,
                synthetic.headers,
                synthetic.body,
                &scripts,
                &network,
                &mut base.applied_script_ids,
                &mut base.stage_timings,
            )
            .await?;
            let mut completed = base;
            completed.status = Some(response.status().as_u16());
            completed.duration_ms = Some(started.elapsed().as_millis() as u64);
            completed.response_headers = headers_to_map(response.headers());
            completed.response_body = body;
            completed.content_type = completed.response_headers.get("content-type").cloned();
            self.store.insert_exchange(completed)?;
            self.event_sink
                .traffic_updated(&self.store.get_traffic_detail(&id)?.summary);
            return self.replay_result(&id, false);
        }

        if request.mode == ReplayMode::MockAware {
            self.refresh_rules()?;
            let rule = self
                .rules
                .read()
                .expect("proxy rules lock poisoned")
                .test(&base.method, &base.url)
                .selected
                .cloned();
            if let Some(rule) = rule {
                self.event_sink.diagnostic_log(
                    &DiagnosticLog::new(
                        DiagnosticLevel::Info,
                        DiagnosticScope::Mock,
                        "mock.matched",
                        "重新发送请求命中 Mock 规则",
                    )
                    .with_exchange_id(&id)
                    .with_detail("ruleId", rule.id.clone())
                    .with_detail("ruleName", rule.name.clone())
                    .with_detail("priority", rule.priority)
                    .with_detail("delayMs", rule.delay_ms),
                );
                if rule.delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(rule.delay_ms)).await;
                }
                let response = build_mock_response(&rule).map_err(|error| {
                    self.event_sink.diagnostic_log(
                        &DiagnosticLog::new(
                            DiagnosticLevel::Error,
                            DiagnosticScope::Mock,
                            "mock.failed",
                            "Mock 响应生成失败",
                        )
                        .with_exchange_id(&id)
                        .with_detail("ruleId", rule.id.clone())
                        .with_detail("errorCode", error.code.clone()),
                    );
                    error
                })?;
                let (response, body) = apply_response_pipeline(
                    &pipeline_request,
                    response.status().as_u16(),
                    headers_to_map(response.headers()),
                    rule.response_body.as_bytes().to_vec(),
                    &scripts,
                    &network,
                    &mut base.applied_script_ids,
                    &mut base.stage_timings,
                )
                .await?;
                let mut completed = base;
                completed.status = Some(response.status().as_u16());
                completed.duration_ms = Some(started.elapsed().as_millis() as u64);
                completed.response_headers = headers_to_map(response.headers());
                completed.response_body = body;
                completed.content_type = completed.response_headers.get("content-type").cloned();
                completed.mocked = true;
                completed.matched_mock_rule_id = Some(rule.id.clone());
                self.queue_mock_hit(rule.id.clone(), id.clone());
                self.store.insert_exchange(completed)?;
                self.event_sink
                    .traffic_updated(&self.store.get_traffic_detail(&id)?.summary);
                return self.replay_result(&id, true);
            }
        }

        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| {
                AppError::new("REPLAY_CLIENT_ERROR", "无法创建重发客户端")
                    .with_detail(error.to_string())
            })?;
        let method = reqwest::Method::from_bytes(base.method.as_bytes()).map_err(|error| {
            AppError::new("REPLAY_UNSUPPORTED_METHOD", "原请求方法无效")
                .with_detail(error.to_string())
        })?;
        let mut request = client.request(method, &base.url);
        for (name, value) in filtered_request_headers(&base.request_headers, false) {
            request = request.header(&name, &value);
        }
        if !base.request_body.is_empty() {
            request = request.body(base.request_body.clone());
        }
        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let headers = response
                    .headers()
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|value| (name.as_str().to_string(), value.to_string()))
                    })
                    .collect::<BTreeMap<_, _>>();
                let content_type = headers.get("content-type").cloned();
                let body = match response.bytes().await {
                    Ok(body) => body.to_vec(),
                    Err(error) => {
                        let mut failed = base;
                        failed.status = Some(status);
                        failed.duration_ms = Some(started.elapsed().as_millis() as u64);
                        failed.response_headers = headers;
                        failed.content_type = content_type;
                        failed.error = Some(error.to_string());
                        self.store.insert_exchange(failed)?;
                        self.event_sink
                            .traffic_updated(&self.store.get_traffic_detail(&id)?.summary);
                        let app_error =
                            AppError::new("REPLAY_RESPONSE_ERROR", "无法读取重发响应正文")
                                .with_detail(error.to_string())
                                .with_action("请检查上游响应后重试");
                        self.log_replay_failure(&id, &app_error);
                        return Err(app_error);
                    }
                };
                let (response, body) = apply_response_pipeline(
                    &pipeline_request,
                    status,
                    headers,
                    body,
                    &scripts,
                    &network,
                    &mut base.applied_script_ids,
                    &mut base.stage_timings,
                )
                .await?;
                let mut completed = base;
                completed.status = Some(response.status().as_u16());
                completed.duration_ms = Some(started.elapsed().as_millis() as u64);
                completed.response_headers = headers_to_map(response.headers());
                completed.response_body = body;
                completed.content_type = completed.response_headers.get("content-type").cloned();
                self.store.insert_exchange(completed)?;
                self.event_sink
                    .traffic_updated(&self.store.get_traffic_detail(&id)?.summary);
                self.replay_result(&id, false)
            }
            Err(error) => {
                let mut failed = base;
                failed.duration_ms = Some(started.elapsed().as_millis() as u64);
                failed.error = Some(error.to_string());
                self.store.insert_exchange(failed)?;
                self.event_sink
                    .traffic_updated(&self.store.get_traffic_detail(&id)?.summary);
                let app_error = AppError::new(
                    if error.is_timeout() {
                        "REPLAY_TIMEOUT"
                    } else {
                        "REPLAY_UPSTREAM_ERROR"
                    },
                    if error.is_timeout() {
                        "重新发送请求超时"
                    } else {
                        "重新发送请求失败"
                    },
                )
                .with_detail(error.to_string())
                .with_action("请检查目标服务和网络连接后重试");
                self.log_replay_failure(&id, &app_error);
                Err(app_error)
            }
        }
    }

    fn replay_result(&self, id: &str, mocked: bool) -> Result<ReplayResult, AppError> {
        let exchange = self.store.get_traffic_detail(id)?.summary;
        self.event_sink.diagnostic_log(
            &DiagnosticLog::new(
                DiagnosticLevel::Info,
                DiagnosticScope::Replay,
                "replay.completed",
                "重新发送请求完成",
            )
            .with_exchange_id(id)
            .with_detail("mocked", mocked)
            .with_detail("status", exchange.status.unwrap_or_default())
            .with_detail("durationMs", exchange.duration_ms.unwrap_or_default()),
        );
        Ok(ReplayResult {
            exchange_id: id.to_string(),
            mocked,
            exchange,
        })
    }

    fn log_replay_failure(&self, id: &str, error: &AppError) {
        self.event_sink.diagnostic_log(
            &DiagnosticLog::new(
                DiagnosticLevel::Error,
                DiagnosticScope::Replay,
                "replay.failed",
                error.message.clone(),
            )
            .with_exchange_id(id)
            .with_detail("errorCode", error.code.clone()),
        );
    }

    pub async fn start(
        &self,
        config: ProxyConfig,
        issuer: Issuer<'static, KeyPair>,
        certificate_pem: Vec<u8>,
    ) -> Result<ProxyStatus, AppError> {
        if self.status().state != ProxyState::Stopped {
            return Err(AppError::new("PROXY_ALREADY_RUNNING", "代理已经启动"));
        }
        self.event_sink.diagnostic_log(
            &DiagnosticLog::new(
                DiagnosticLevel::Info,
                DiagnosticScope::Proxy,
                "proxy.starting",
                "正在启动代理",
            )
            .with_detail("port", config.port)
            .with_detail("allowLan", config.allow_lan)
            .with_detail("captureEnabled", config.capture_enabled),
        );
        self.update_status(ProxyStatus {
            state: ProxyState::Starting,
            bind_address: None,
            port: config.port,
            lan_ip: None,
            error: None,
        });
        self.refresh_rules()?;
        self.set_capture_enabled(config.capture_enabled);

        let bind_ip = if config.allow_lan {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        let listener = TcpListener::bind(SocketAddr::from((bind_ip, config.port)))
            .await
            .map_err(|error| {
                let occupant = self
                    .port_inspector
                    .listener(config.port)
                    .map(|process| process.to_string());
                self.fail_status(config.port, error.to_string());
                self.event_sink.diagnostic_log(
                    &DiagnosticLog::new(
                        DiagnosticLevel::Error,
                        DiagnosticScope::Proxy,
                        "proxy.failed",
                        "代理监听端口失败",
                    )
                    .with_detail("port", config.port)
                    .with_detail("errorCode", "PORT_UNAVAILABLE"),
                );
                let detail = occupant
                    .as_ref()
                    .map(|process| format!("{error}; 占用进程: {process}"))
                    .unwrap_or_else(|| error.to_string());
                let action = occupant
                    .map(|process| format!("请退出 {process} 或更换代理端口"))
                    .unwrap_or_else(|| "请更换端口或关闭占用该端口的程序".to_string());
                AppError::new("PORT_UNAVAILABLE", format!("无法监听端口 {}", config.port))
                    .with_detail(detail)
                    .with_action(action)
            })?;
        let address = listener.local_addr().map_err(|error| {
            AppError::new("PROXY_START_ERROR", "无法读取代理监听地址")
                .with_detail(error.to_string())
        })?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let persistence =
            spawn_persistence_writer(Arc::clone(&self.store), Arc::clone(&self.event_sink));
        let handler = CaptureHandler {
            store: Arc::clone(&self.store),
            rules: Arc::clone(&self.rules),
            capture_enabled: Arc::clone(&self.capture_enabled),
            allow_lan: config.allow_lan,
            certificate_pem: Arc::new(certificate_pem),
            event_sink: Arc::clone(&self.event_sink),
            persistence,
            pending: None,
        };
        let authority = RcgenAuthority::new(issuer, 1_000, aws_lc_rs::default_provider());
        let proxy = Proxy::builder()
            .with_listener(listener)
            .with_ca(authority)
            .with_rustls_connector(aws_lc_rs::default_provider())
            .with_http_handler(handler)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .build()
            .map_err(|error| {
                AppError::new("PROXY_START_ERROR", "无法创建代理服务")
                    .with_detail(error.to_string())
            })?;
        *self.shutdown.lock().await = Some(shutdown_tx);
        *self.task.lock().await = Some(tokio::spawn(proxy.start()));
        let lan_ip = config.allow_lan.then(local_lan_ip).flatten();
        let status = ProxyStatus {
            state: ProxyState::Running,
            bind_address: Some(address.ip().to_string()),
            port: address.port(),
            lan_ip,
            error: None,
        };
        self.update_status(status.clone());
        self.event_sink.diagnostic_log(
            &DiagnosticLog::new(
                DiagnosticLevel::Info,
                DiagnosticScope::Proxy,
                "proxy.started",
                "代理已启动",
            )
            .with_detail("bindAddress", address.ip().to_string())
            .with_detail("port", address.port())
            .with_detail("allowLan", config.allow_lan),
        );
        Ok(status)
    }

    pub async fn stop(&self) -> Result<ProxyStatus, AppError> {
        if self.status().state == ProxyState::Stopped {
            return Ok(self.status());
        }
        let current = self.status();
        self.event_sink.diagnostic_log(&DiagnosticLog::new(
            DiagnosticLevel::Info,
            DiagnosticScope::Proxy,
            "proxy.stopping",
            "正在停止代理",
        ));
        self.update_status(ProxyStatus {
            state: ProxyState::Stopping,
            ..current
        });
        if let Some(shutdown) = self.shutdown.lock().await.take() {
            let _ = shutdown.send(());
        }
        if let Some(mut task) = self.task.lock().await.take() {
            if tokio::time::timeout(Duration::from_secs(3), &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
        let status = ProxyStatus::default();
        self.update_status(status.clone());
        self.event_sink.diagnostic_log(&DiagnosticLog::new(
            DiagnosticLevel::Info,
            DiagnosticScope::Proxy,
            "proxy.stopped",
            "代理已停止",
        ));
        Ok(status)
    }

    fn update_status(&self, status: ProxyStatus) {
        *self.status.write().expect("proxy status lock poisoned") = status.clone();
        self.event_sink.status_changed(&status);
    }

    fn fail_status(&self, port: u16, error: String) {
        self.update_status(ProxyStatus {
            state: ProxyState::Error,
            bind_address: None,
            port,
            lan_ip: None,
            error: Some(error),
        });
    }
}

struct PendingExchange {
    id: String,
    started_at: String,
    started: Instant,
    method: String,
    url: String,
    host: String,
    request_headers: BTreeMap<String, String>,
    request_body: Vec<u8>,
    scripts: Vec<ProxyScript>,
    network: NetworkConditioner,
    applied_script_ids: Vec<String>,
    network_profile_id: Option<String>,
    stage_timings: BTreeMap<String, u64>,
    initial_persistence: Option<JoinHandle<()>>,
}

struct CaptureHandler {
    store: Arc<Store>,
    rules: Arc<RwLock<RuntimeMockRules>>,
    capture_enabled: Arc<AtomicBool>,
    allow_lan: bool,
    certificate_pem: Arc<Vec<u8>>,
    event_sink: Arc<dyn ProxyEventSink>,
    persistence: mpsc::Sender<PersistenceJob>,
    pending: Option<PendingExchange>,
}

impl Clone for CaptureHandler {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            rules: Arc::clone(&self.rules),
            capture_enabled: Arc::clone(&self.capture_enabled),
            allow_lan: self.allow_lan,
            certificate_pem: Arc::clone(&self.certificate_pem),
            event_sink: Arc::clone(&self.event_sink),
            persistence: self.persistence.clone(),
            pending: None,
        }
    }
}

impl HttpHandler for CaptureHandler {
    async fn handle_request(
        &mut self,
        context: &HttpContext,
        request: Request<Body>,
    ) -> RequestOrResponse {
        if self.allow_lan && !is_private_client(context.client_addr.ip()) {
            return simple_response(StatusCode::FORBIDDEN, "仅允许同一私有网络中的设备连接").into();
        }
        if request.uri().path() == HEALTH_PATH {
            let body = r#"{"status":"ok","service":"netcat"}"#;
            let mut response = Response::new(Body::from(body));
            *response.status_mut() = StatusCode::OK;
            response
                .headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            response.headers_mut().insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).expect("valid length"),
            );
            return response.into();
        }
        if request.uri().path() == CA_DOWNLOAD_PATH {
            let mut response = Response::new(Body::from(self.certificate_pem.as_ref().clone()));
            *response.status_mut() = StatusCode::OK;
            response.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-x509-ca-cert"),
            );
            response.headers_mut().insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&self.certificate_pem.len().to_string())
                    .expect("valid length"),
            );
            return response.into();
        }

        let (mut parts, body) = request.into_parts();
        let request_body = match body.collect().await {
            Ok(body) => body.to_bytes().to_vec(),
            Err(error) => {
                return simple_response(
                    StatusCode::BAD_REQUEST,
                    &format!("无法读取请求正文: {error}"),
                )
                .into()
            }
        };
        let original_method = parts.method.to_string();
        let original_url = parts.uri.to_string();
        let original_headers = headers_to_map(&parts.headers);
        let mut pipeline_request = PipelineRequest {
            method: original_method.clone(),
            url: original_url.clone(),
            headers: original_headers.clone(),
            body: request_body,
        };
        let scripts = self.store.list_scripts().unwrap_or_else(|error| {
            self.event_sink.storage_warning(&error);
            Vec::new()
        });
        let network = active_network_conditioner(&self.store).unwrap_or_else(|error| {
            self.event_sink.storage_warning(&error);
            NetworkConditioner::default()
        });
        let request_script_started = Instant::now();
        let script_report = ScriptEngine.apply_request(&mut pipeline_request, &scripts);
        let mut stage_timings = BTreeMap::from([(
            "requestScript".to_string(),
            request_script_started.elapsed().as_millis() as u64,
        )]);
        for failure in &script_report.errors {
            self.event_sink.diagnostic_log(
                &DiagnosticLog::new(
                    DiagnosticLevel::Warn,
                    DiagnosticScope::Script,
                    "script.failed",
                    "请求脚本执行失败，已放行当前流量",
                )
                .with_detail("scriptId", failure.script_id.clone()),
            );
        }
        let method = match pipeline_request.method.parse() {
            Ok(method) => {
                parts.method = method;
                pipeline_request.method.clone()
            }
            Err(_) => original_method,
        };
        let url = match pipeline_request.url.parse() {
            Ok(uri) => {
                parts.uri = uri;
                pipeline_request.url.clone()
            }
            Err(_) => original_url,
        };
        if let Ok(headers) = map_to_headers(&pipeline_request.headers) {
            parts.headers = headers;
        } else {
            pipeline_request.headers = original_headers;
        }
        let host = parts
            .uri
            .host()
            .map(str::to_string)
            .or_else(|| {
                parts
                    .headers
                    .get(HOST)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let request_headers = headers_to_map(&parts.headers);
        let mut pending = PendingExchange {
            id: uuid::Uuid::new_v4().to_string(),
            started_at: Utc::now().to_rfc3339(),
            started: Instant::now(),
            method: method.clone(),
            url: url.clone(),
            host,
            request_headers,
            request_body: pipeline_request.body.clone(),
            scripts: scripts.clone(),
            network: network.clone(),
            applied_script_ids: script_report.applied_script_ids.clone(),
            network_profile_id: network.profile().map(|profile| profile.id.clone()),
            stage_timings: stage_timings.clone(),
            initial_persistence: None,
        };
        if self.capture_enabled.load(Ordering::Relaxed) {
            pending.initial_persistence = Some(persist_exchange(
                self.persistence.clone(),
                Arc::clone(&self.event_sink),
                pending.initial_draft(),
                PersistenceEvent::Created,
            ));
        }
        let upload_started = Instant::now();
        let upload_result = apply_network_stage(
            &network,
            TransferDirection::Upload,
            &method,
            &url,
            pipeline_request.body.len(),
        )
        .await;
        stage_timings.insert(
            "networkUpload".into(),
            upload_started.elapsed().as_millis() as u64,
        );
        pending.stage_timings = stage_timings;
        if let Err(error) = upload_result {
            let status = if error.code == "NETWORK_TIMEOUT" {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::BAD_GATEWAY
            };
            let response = simple_response(status, &error.message);
            self.record(
                pending,
                &response,
                Vec::new(),
                false,
                None,
                Some(error.message),
            );
            return response.into();
        }
        if let Some(synthetic) = script_report.synthetic_response {
            let request_context = pipeline_request.clone();
            let (response, response_body) = apply_response_pipeline(
                &request_context,
                synthetic.status,
                synthetic.headers,
                synthetic.body,
                &scripts,
                &network,
                &mut pending.applied_script_ids,
                &mut pending.stage_timings,
            )
            .await
            .unwrap_or_else(|error| {
                (
                    simple_response(StatusCode::BAD_GATEWAY, &error.message),
                    Vec::new(),
                )
            });
            self.record(pending, &response, response_body, false, None, None);
            return response.into();
        }
        let request = Request::from_parts(parts, Body::from(pipeline_request.body.clone()));
        let selected_rule = {
            self.rules
                .read()
                .expect("proxy rules lock poisoned")
                .test(&method, &url)
                .selected
                .cloned()
        };
        if let Some(rule) = selected_rule {
            self.queue_mock_hit(rule.id.clone(), pending.id.clone());
            self.event_sink.diagnostic_log(
                &DiagnosticLog::new(
                    DiagnosticLevel::Info,
                    DiagnosticScope::Mock,
                    "mock.matched",
                    "请求命中 Mock 规则",
                )
                .with_exchange_id(&pending.id)
                .with_detail("ruleId", rule.id.clone())
                .with_detail("ruleName", rule.name.clone())
                .with_detail("priority", rule.priority)
                .with_detail("delayMs", rule.delay_ms),
            );
            if rule.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(rule.delay_ms)).await;
            }
            match build_mock_response(&rule) {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let headers = headers_to_map(response.headers());
                    let body = rule.response_body.as_bytes().to_vec();
                    let (response, body) = apply_response_pipeline(
                        &pipeline_request,
                        status,
                        headers,
                        body,
                        &scripts,
                        &network,
                        &mut pending.applied_script_ids,
                        &mut pending.stage_timings,
                    )
                    .await
                    .unwrap_or_else(|error| {
                        (
                            simple_response(StatusCode::BAD_GATEWAY, &error.message),
                            Vec::new(),
                        )
                    });
                    self.record(pending, &response, body, true, Some(rule.id.clone()), None);
                    return response.into();
                }
                Err(error) => {
                    self.event_sink.diagnostic_log(
                        &DiagnosticLog::new(
                            DiagnosticLevel::Error,
                            DiagnosticScope::Mock,
                            "mock.failed",
                            "Mock 响应生成失败",
                        )
                        .with_exchange_id(&pending.id)
                        .with_detail("ruleId", rule.id.clone())
                        .with_detail("errorCode", error.code.clone()),
                    );
                    return simple_response(StatusCode::BAD_GATEWAY, &error.message).into();
                }
            }
        }
        self.pending = Some(pending);
        request.into()
    }

    async fn handle_response(
        &mut self,
        _context: &HttpContext,
        response: Response<Body>,
    ) -> Response<Body> {
        let (parts, body) = response.into_parts();
        match body.collect().await {
            Ok(body) => {
                let bytes = body.to_bytes().to_vec();
                if let Some(mut pending) = self.pending.take() {
                    let request = PipelineRequest {
                        method: pending.method.clone(),
                        url: pending.url.clone(),
                        headers: pending.request_headers.clone(),
                        body: pending.request_body.clone(),
                    };
                    let status = parts.status.as_u16();
                    let headers = headers_to_map(&parts.headers);
                    match apply_response_pipeline(
                        &request,
                        status,
                        headers,
                        bytes,
                        &pending.scripts,
                        &pending.network,
                        &mut pending.applied_script_ids,
                        &mut pending.stage_timings,
                    )
                    .await
                    {
                        Ok((response, body)) => {
                            self.record(pending, &response, body, false, None, None);
                            response
                        }
                        Err(error) => {
                            let response = simple_response(StatusCode::BAD_GATEWAY, &error.message);
                            self.record(
                                pending,
                                &response,
                                Vec::new(),
                                false,
                                None,
                                Some(error.message),
                            );
                            response
                        }
                    }
                } else {
                    Response::from_parts(parts, Body::from(bytes))
                }
            }
            Err(error) => {
                if let Some(pending) = self.pending.take() {
                    let response = simple_response(StatusCode::BAD_GATEWAY, "无法读取上游响应正文");
                    self.record(
                        pending,
                        &response,
                        Vec::new(),
                        false,
                        None,
                        Some(error.to_string()),
                    );
                    response
                } else {
                    simple_response(StatusCode::BAD_GATEWAY, "无法读取上游响应正文")
                }
            }
        }
    }

    async fn handle_error(
        &mut self,
        _context: &HttpContext,
        error: hudsucker::hyper_util::client::legacy::Error,
    ) -> Response<Body> {
        let response = simple_response(StatusCode::BAD_GATEWAY, "上游连接失败");
        if let Some(pending) = self.pending.take() {
            self.event_sink.diagnostic_log(
                &DiagnosticLog::new(
                    DiagnosticLevel::Error,
                    DiagnosticScope::Proxy,
                    "proxy.upstream_failed",
                    "上游连接失败",
                )
                .with_exchange_id(&pending.id),
            );
            self.record(
                pending,
                &response,
                Vec::new(),
                false,
                None,
                Some(error.to_string()),
            );
        }
        response
    }
}

fn replay_mode_name(mode: ReplayMode) -> &'static str {
    match mode {
        ReplayMode::Upstream => "upstream",
        ReplayMode::MockAware => "mockAware",
    }
}

fn active_network_conditioner(store: &Store) -> Result<NetworkConditioner, AppError> {
    let Some(active_id) = store.active_network_profile_id()? else {
        return Ok(NetworkConditioner::default());
    };
    let profile = builtin_network_profiles()
        .into_iter()
        .chain(store.list_network_profiles()?)
        .find(|profile| profile.id == active_id);
    Ok(NetworkConditioner::new(profile))
}

async fn apply_network_stage(
    network: &NetworkConditioner,
    direction: TransferDirection,
    method: &str,
    url: &str,
    bytes: usize,
) -> Result<(), AppError> {
    if !network.matches(method, url) {
        return Ok(());
    }
    let sample = pseudo_random(100) as u8;
    if let Some(failure) = network.failure_for_sample(sample) {
        return Err(match failure {
            NetworkFailure::Timeout => AppError::new("NETWORK_TIMEOUT", "弱网配置模拟请求超时")
                .with_action("可在网络请求工具栏停用或调整弱网配置"),
            NetworkFailure::Disconnect => {
                AppError::new("NETWORK_DISCONNECTED", "弱网配置模拟连接中断")
                    .with_action("可在网络请求工具栏停用或调整弱网配置")
            }
        });
    }
    let jitter = network
        .profile()
        .map(|profile| pseudo_random(profile.jitter_ms.saturating_add(1)))
        .unwrap_or(0);
    let delay = network.delay_ms(direction, bytes, jitter);
    if delay > 0 {
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    Ok(())
}

fn pseudo_random(upper_bound: u64) -> u64 {
    if upper_bound == 0 {
        return 0;
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as u64 % upper_bound)
        .unwrap_or(0)
}

async fn apply_response_pipeline(
    request: &PipelineRequest,
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
    scripts: &[ProxyScript],
    network: &NetworkConditioner,
    applied_script_ids: &mut Vec<String>,
    stage_timings: &mut BTreeMap<String, u64>,
) -> Result<(Response<Body>, Vec<u8>), AppError> {
    let mut pipeline_response = PipelineResponse {
        status,
        headers,
        body,
    };
    let response_script_started = Instant::now();
    let report = ScriptEngine.apply_response(request, &mut pipeline_response, scripts);
    stage_timings.insert(
        "responseScript".into(),
        response_script_started.elapsed().as_millis() as u64,
    );
    for id in report.applied_script_ids {
        if !applied_script_ids.contains(&id) {
            applied_script_ids.push(id);
        }
    }
    let download_started = Instant::now();
    let download_result = apply_network_stage(
        network,
        TransferDirection::Download,
        &request.method,
        &request.url,
        pipeline_response.body.len(),
    )
    .await;
    stage_timings.insert(
        "networkDownload".into(),
        download_started.elapsed().as_millis() as u64,
    );
    download_result?;
    let body = pipeline_response.body.clone();
    let response = pipeline_response_to_http(
        pipeline_response.status,
        pipeline_response.headers,
        pipeline_response.body,
    )?;
    Ok((response, body))
}

impl CaptureHandler {
    fn record(
        &self,
        pending: PendingExchange,
        response: &Response<Body>,
        response_body: Vec<u8>,
        mocked: bool,
        matched_mock_rule_id: Option<String>,
        error: Option<String>,
    ) {
        let Some(initial_persistence) = pending.initial_persistence else {
            return;
        };
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let draft = TrafficDraft {
            id: pending.id,
            started_at: pending.started_at,
            method: pending.method,
            url: pending.url,
            host: pending.host,
            status: Some(response.status().as_u16()),
            duration_ms: Some(pending.started.elapsed().as_millis() as u64),
            request_headers: pending.request_headers,
            response_headers: headers_to_map(response.headers()),
            request_body: pending.request_body,
            response_body,
            content_type,
            mocked,
            source_exchange_id: None,
            matched_mock_rule_id,
            applied_script_ids: pending.applied_script_ids,
            network_profile_id: pending.network_profile_id,
            stage_timings: pending.stage_timings,
            error,
        };
        let persistence = self.persistence.clone();
        let event_sink = Arc::clone(&self.event_sink);
        tokio::spawn(async move {
            let _ = initial_persistence.await;
            let _ =
                persist_exchange(persistence, event_sink, draft, PersistenceEvent::Updated).await;
        });
    }

    fn queue_mock_hit(&self, rule_id: String, exchange_id: String) {
        let store = Arc::clone(&self.store);
        let event_sink = Arc::clone(&self.event_sink);
        let timestamp = Utc::now().to_rfc3339();
        tokio::spawn(async move {
            match tokio::task::spawn_blocking(move || {
                store.record_mock_hit(&rule_id, &exchange_id, &timestamp)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => event_sink.storage_warning(&error),
                Err(error) => event_sink.storage_warning(
                    &AppError::new("STORAGE_TASK_ERROR", "Mock 命中统计写入任务异常")
                        .with_detail(error.to_string()),
                ),
            }
        });
    }
}

impl PendingExchange {
    fn initial_draft(&self) -> TrafficDraft {
        TrafficDraft {
            id: self.id.clone(),
            started_at: self.started_at.clone(),
            method: self.method.clone(),
            url: self.url.clone(),
            host: self.host.clone(),
            status: None,
            duration_ms: None,
            request_headers: self.request_headers.clone(),
            response_headers: BTreeMap::new(),
            request_body: self.request_body.clone(),
            response_body: Vec::new(),
            content_type: None,
            mocked: false,
            source_exchange_id: None,
            matched_mock_rule_id: None,
            applied_script_ids: self.applied_script_ids.clone(),
            network_profile_id: self.network_profile_id.clone(),
            stage_timings: self.stage_timings.clone(),
            error: None,
        }
    }
}

#[derive(Clone, Copy)]
enum PersistenceEvent {
    Created,
    Updated,
}

struct PersistenceJob {
    draft: TrafficDraft,
    id: String,
    event: PersistenceEvent,
    completed: oneshot::Sender<()>,
}

const PERSISTENCE_QUEUE_CAPACITY: usize = 1_000;
const PERSISTENCE_BATCH_SIZE: usize = 50;
const PERSISTENCE_BATCH_DELAY: Duration = Duration::from_millis(25);

fn spawn_persistence_writer(
    store: Arc<Store>,
    event_sink: Arc<dyn ProxyEventSink>,
) -> mpsc::Sender<PersistenceJob> {
    let (sender, mut receiver) = mpsc::channel::<PersistenceJob>(PERSISTENCE_QUEUE_CAPACITY);
    tokio::spawn(async move {
        while let Some(first) = receiver.recv().await {
            let mut jobs = vec![first];
            let deadline = tokio::time::Instant::now() + PERSISTENCE_BATCH_DELAY;
            while jobs.len() < PERSISTENCE_BATCH_SIZE {
                match tokio::time::timeout_at(deadline, receiver.recv()).await {
                    Ok(Some(job)) => jobs.push(job),
                    _ => break,
                }
            }
            let drafts = jobs.iter().map(|job| job.draft.clone()).collect();
            let write_store = Arc::clone(&store);
            match tokio::task::spawn_blocking(move || write_store.insert_exchanges(drafts)).await {
                Ok(Ok(())) => {
                    for job in jobs {
                        match store.get_traffic_detail(&job.id) {
                            Ok(detail) => match job.event {
                                PersistenceEvent::Created => {
                                    event_sink.traffic_created(&detail.summary)
                                }
                                PersistenceEvent::Updated => {
                                    event_sink.traffic_updated(&detail.summary)
                                }
                            },
                            Err(error) => event_sink.storage_warning(&error),
                        }
                        let _ = job.completed.send(());
                    }
                }
                Ok(Err(error)) => {
                    event_sink.storage_warning(&error);
                    for job in jobs {
                        let _ = job.completed.send(());
                    }
                }
                Err(error) => {
                    event_sink.storage_warning(
                        &AppError::new("STORAGE_TASK_ERROR", "流量批量写入任务异常")
                            .with_detail(error.to_string()),
                    );
                    for job in jobs {
                        let _ = job.completed.send(());
                    }
                }
            }
        }
    });
    sender
}

fn persist_exchange(
    sender: mpsc::Sender<PersistenceJob>,
    event_sink: Arc<dyn ProxyEventSink>,
    draft: TrafficDraft,
    event: PersistenceEvent,
) -> JoinHandle<()> {
    let id = draft.id.clone();
    let (completed, receiver) = oneshot::channel();
    let job = PersistenceJob {
        draft,
        id,
        event,
        completed,
    };
    if let Err(error) = sender.try_send(job) {
        let reason = match error {
            mpsc::error::TrySendError::Full(_) => "数据库写入队列已满，当前记录未保存",
            mpsc::error::TrySendError::Closed(_) => "数据库写入器已停止，当前记录未保存",
        };
        event_sink.storage_warning(
            &AppError::new("STORAGE_QUEUE_UNAVAILABLE", reason)
                .with_action("代理转发将继续，请检查诊断日志"),
        );
        return tokio::spawn(async {});
    }
    tokio::spawn(async move {
        let _ = receiver.await;
    })
}

fn simple_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(message.to_string()))
        .expect("static response is valid")
}

pub fn parse_lsof_listener(output: &str) -> Option<String> {
    let mut lines = output.lines();
    lines.next()?;
    let columns = lines.next()?.split_whitespace().collect::<Vec<_>>();
    (columns.len() >= 2).then(|| format!("{} (PID {})", columns[0], columns[1]))
}

fn headers_to_map(headers: &hudsucker::hyper::HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect()
}

fn map_to_headers(
    headers: &BTreeMap<String, String>,
) -> Result<hudsucker::hyper::HeaderMap, AppError> {
    let mut output = hudsucker::hyper::HeaderMap::new();
    for (name, value) in headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
            AppError::new("SCRIPT_INVALID_HEADER", "脚本生成了无效的 Header 名称")
                .with_detail(error.to_string())
        })?;
        let value = HeaderValue::from_str(value).map_err(|error| {
            AppError::new("SCRIPT_INVALID_HEADER", "脚本生成了无效的 Header 内容")
                .with_detail(error.to_string())
        })?;
        output.insert(name, value);
    }
    Ok(output)
}

fn pipeline_response_to_http(
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
) -> Result<Response<Body>, AppError> {
    let status = StatusCode::from_u16(status).map_err(|error| {
        AppError::new("SCRIPT_INVALID_STATUS", "脚本生成了无效的响应状态码")
            .with_detail(error.to_string())
    })?;
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    *response.headers_mut() = map_to_headers(&headers)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{PortInspector, PortProcess};

    struct FixturePorts {
        port: u16,
        name: Option<String>,
    }
    impl PortInspector for FixturePorts {
        fn listener(&self, port: u16) -> Option<PortProcess> {
            assert_eq!(port, self.port);
            Some(PortProcess {
                pid: 42,
                name: self.name.clone(),
            })
        }
    }

    #[tokio::test]
    async fn occupied_port_reports_injected_process_or_pid_when_name_is_unavailable() {
        for (name, expected) in [
            (Some("process.exe".to_string()), "process.exe (PID 42)"),
            (None, "PID 42"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let store = Arc::new(Store::open_in_memory(100, 1024).unwrap());
            let service = ProxyService::new(store, Arc::new(FixturePorts { port, name }));
            let key = KeyPair::generate().unwrap();
            let issuer = hudsucker::rcgen::CertificateParams::default()
                .self_signed(&key)
                .unwrap();
            let issuer = Issuer::from_ca_cert_pem(&issuer.pem(), key).unwrap();
            let error = service
                .start(
                    ProxyConfig {
                        port,
                        ..Default::default()
                    },
                    issuer,
                    Vec::new(),
                )
                .await
                .unwrap_err();
            assert_eq!(error.code, "PORT_UNAVAILABLE");
            assert!(
                error.detail.as_deref().unwrap().contains(expected),
                "{error:?}"
            );
            assert!(
                error.action.as_deref().unwrap().contains(expected),
                "{error:?}"
            );
        }
    }
}

fn is_private_client(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00,
    }
}

pub fn build_mock_response(rule: &MockRule) -> Result<Response<Body>, AppError> {
    let status = StatusCode::from_u16(rule.status).map_err(|error| {
        AppError::new("INVALID_STATUS", "Mock 状态码无效").with_detail(error.to_string())
    })?;
    let mut response = Response::builder()
        .status(status)
        .body(Body::from(rule.response_body.clone()))
        .map_err(|error| {
            AppError::new("MOCK_RESPONSE_ERROR", "无法构建 Mock 响应")
                .with_detail(error.to_string())
        })?;

    for (name, value) in &rule.response_headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
            AppError::new("INVALID_HEADER", "Mock 响应头名称无效").with_detail(error.to_string())
        })?;
        if name == CONTENT_LENGTH || name == TRANSFER_ENCODING {
            continue;
        }
        let value = HeaderValue::from_str(value).map_err(|error| {
            AppError::new("INVALID_HEADER", "Mock 响应头内容无效").with_detail(error.to_string())
        })?;
        response.headers_mut().insert(name, value);
    }
    response.headers_mut().remove(TRANSFER_ENCODING);
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&rule.response_body.len().to_string())
            .expect("body length is always a valid header value"),
    );
    Ok(response)
}
