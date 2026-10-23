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
    sync::{oneshot, Mutex},
    task::JoinHandle,
};

use crate::{
    error::AppError,
    export::filtered_request_headers,
    models::{
        MockRule, ProxyConfig, ProxyState, ProxyStatus, ReplayMode, ReplayResult, TrafficDraft,
    },
    rules::select_mock,
    storage::Store,
};

pub trait ProxyEventSink: Send + Sync + 'static {
    fn status_changed(&self, _status: &ProxyStatus) {}
    fn traffic_created(&self, _id: &str) {}
    fn traffic_updated(&self, _id: &str) {}
    fn storage_warning(&self, _error: &AppError) {}
}

#[derive(Default)]
pub struct NoopEventSink;

impl ProxyEventSink for NoopEventSink {}

pub struct ProxyService {
    store: Arc<Store>,
    rules: Arc<RwLock<Vec<MockRule>>>,
    capture_enabled: Arc<AtomicBool>,
    status: RwLock<ProxyStatus>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    task: Mutex<Option<JoinHandle<Result<(), hudsucker::Error>>>>,
    event_sink: Arc<dyn ProxyEventSink>,
}

impl ProxyService {
    pub fn new(store: Arc<Store>) -> Self {
        Self::with_event_sink(store, Arc::new(NoopEventSink))
    }

    pub fn with_event_sink(store: Arc<Store>, event_sink: Arc<dyn ProxyEventSink>) -> Self {
        Self {
            store,
            rules: Arc::new(RwLock::new(Vec::new())),
            capture_enabled: Arc::new(AtomicBool::new(true)),
            status: RwLock::new(ProxyStatus::default()),
            shutdown: Mutex::new(None),
            task: Mutex::new(None),
            event_sink,
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
        *self.rules.write().expect("proxy rules lock poisoned") = self.store.list_mock_rules()?;
        Ok(())
    }

    pub async fn replay(
        &self,
        exchange_id: &str,
        mode: ReplayMode,
    ) -> Result<ReplayResult, AppError> {
        let original = self.store.get_traffic_detail(exchange_id)?;
        if original.summary.method.eq_ignore_ascii_case("CONNECT") {
            return Err(
                AppError::new("REPLAY_UNSUPPORTED_METHOD", "CONNECT 请求不能重新发送")
                    .with_action("请选择普通 HTTP 或 HTTPS 请求"),
            );
        }
        let parsed_url = url::Url::parse(&original.summary.url).map_err(|error| {
            AppError::new("REPLAY_INVALID_URL", "原请求 URL 无效").with_detail(error.to_string())
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
        let base = TrafficDraft {
            id: id.clone(),
            started_at: started_at.clone(),
            method: original.summary.method.clone(),
            url: original.summary.url.clone(),
            host: parsed_url.host_str().unwrap_or_default().to_string(),
            status: None,
            duration_ms: None,
            request_headers: original.request_headers.clone(),
            response_headers: BTreeMap::new(),
            request_body: original.request_body.clone(),
            response_body: Vec::new(),
            content_type: None,
            mocked: false,
            error: None,
        };
        self.store.insert_exchange(base.clone())?;
        self.event_sink.traffic_created(&id);

        if mode == ReplayMode::MockAware {
            let rules = self.store.list_mock_rules()?;
            if let Some(rule) = select_mock(&rules, &base.method, &base.url).cloned() {
                if rule.delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(rule.delay_ms)).await;
                }
                let response = build_mock_response(&rule)?;
                let mut completed = base;
                completed.status = Some(response.status().as_u16());
                completed.duration_ms = Some(started.elapsed().as_millis() as u64);
                completed.response_headers = headers_to_map(response.headers());
                completed.response_body = rule.response_body.into_bytes();
                completed.content_type = completed.response_headers.get("content-type").cloned();
                completed.mocked = true;
                self.store.insert_exchange(completed)?;
                self.event_sink.traffic_updated(&id);
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
                        self.event_sink.traffic_updated(&id);
                        return Err(
                            AppError::new("REPLAY_RESPONSE_ERROR", "无法读取重发响应正文")
                                .with_detail(error.to_string())
                                .with_action("请检查上游响应后重试"),
                        );
                    }
                };
                let mut completed = base;
                completed.status = Some(status);
                completed.duration_ms = Some(started.elapsed().as_millis() as u64);
                completed.response_headers = headers;
                completed.response_body = body;
                completed.content_type = content_type;
                self.store.insert_exchange(completed)?;
                self.event_sink.traffic_updated(&id);
                self.replay_result(&id, false)
            }
            Err(error) => {
                let mut failed = base;
                failed.duration_ms = Some(started.elapsed().as_millis() as u64);
                failed.error = Some(error.to_string());
                self.store.insert_exchange(failed)?;
                self.event_sink.traffic_updated(&id);
                Err(AppError::new(
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
                .with_action("请检查目标服务和网络连接后重试"))
            }
        }
    }

    fn replay_result(&self, id: &str, mocked: bool) -> Result<ReplayResult, AppError> {
        let exchange = self.store.get_traffic_detail(id)?.summary;
        Ok(ReplayResult {
            exchange_id: id.to_string(),
            mocked,
            exchange,
        })
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
                self.fail_status(config.port, error.to_string());
                AppError::new("PORT_UNAVAILABLE", format!("无法监听端口 {}", config.port))
                    .with_detail(error.to_string())
                    .with_action("请更换端口或关闭占用该端口的程序")
            })?;
        let address = listener.local_addr().map_err(|error| {
            AppError::new("PROXY_START_ERROR", "无法读取代理监听地址")
                .with_detail(error.to_string())
        })?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let handler = CaptureHandler {
            store: Arc::clone(&self.store),
            rules: Arc::clone(&self.rules),
            capture_enabled: Arc::clone(&self.capture_enabled),
            allow_lan: config.allow_lan,
            certificate_pem: Arc::new(certificate_pem),
            event_sink: Arc::clone(&self.event_sink),
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
        let lan_ip = config
            .allow_lan
            .then(|| local_ip_address::local_ip().ok().map(|ip| ip.to_string()))
            .flatten();
        let status = ProxyStatus {
            state: ProxyState::Running,
            bind_address: Some(address.ip().to_string()),
            port: address.port(),
            lan_ip,
            error: None,
        };
        self.update_status(status.clone());
        Ok(status)
    }

    pub async fn stop(&self) -> Result<ProxyStatus, AppError> {
        if self.status().state == ProxyState::Stopped {
            return Ok(self.status());
        }
        let current = self.status();
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
    initial_persistence: Option<JoinHandle<()>>,
}

struct CaptureHandler {
    store: Arc<Store>,
    rules: Arc<RwLock<Vec<MockRule>>>,
    capture_enabled: Arc<AtomicBool>,
    allow_lan: bool,
    certificate_pem: Arc<Vec<u8>>,
    event_sink: Arc<dyn ProxyEventSink>,
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
        if request.uri().path() == "/__netcapture/ca.crt" {
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

        let (parts, body) = request.into_parts();
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
        let method = parts.method.to_string();
        let url = parts.uri.to_string();
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
            request_body: request_body.clone(),
            initial_persistence: None,
        };
        if self.capture_enabled.load(Ordering::Relaxed) {
            pending.initial_persistence = Some(persist_exchange(
                Arc::clone(&self.store),
                Arc::clone(&self.event_sink),
                pending.initial_draft(),
                PersistenceEvent::Created,
            ));
        }
        let request = Request::from_parts(parts, Body::from(request_body));
        let selected_rule = {
            let rules = self.rules.read().expect("proxy rules lock poisoned");
            select_mock(&rules, &method, &url).cloned()
        };
        if let Some(rule) = selected_rule {
            if rule.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(rule.delay_ms)).await;
            }
            match build_mock_response(&rule) {
                Ok(response) => {
                    self.record(
                        pending,
                        &response,
                        rule.response_body.as_bytes().to_vec(),
                        true,
                        None,
                    );
                    return response.into();
                }
                Err(error) => {
                    return simple_response(StatusCode::BAD_GATEWAY, &error.message).into()
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
                let response = Response::from_parts(parts, Body::from(bytes.clone()));
                if let Some(pending) = self.pending.take() {
                    self.record(pending, &response, bytes, false, None);
                }
                response
            }
            Err(error) => {
                if let Some(pending) = self.pending.take() {
                    let response = simple_response(StatusCode::BAD_GATEWAY, "无法读取上游响应正文");
                    self.record(
                        pending,
                        &response,
                        Vec::new(),
                        false,
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
            self.record(
                pending,
                &response,
                Vec::new(),
                false,
                Some(error.to_string()),
            );
        }
        response
    }
}

impl CaptureHandler {
    fn record(
        &self,
        pending: PendingExchange,
        response: &Response<Body>,
        response_body: Vec<u8>,
        mocked: bool,
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
        let id = pending.id.clone();
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
            error,
        };
        let store = Arc::clone(&self.store);
        let event_sink = Arc::clone(&self.event_sink);
        tokio::spawn(async move {
            let _ = initial_persistence.await;
            write_exchange(store, event_sink, draft, id, PersistenceEvent::Updated).await;
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
            error: None,
        }
    }
}

#[derive(Clone, Copy)]
enum PersistenceEvent {
    Created,
    Updated,
}

fn persist_exchange(
    store: Arc<Store>,
    event_sink: Arc<dyn ProxyEventSink>,
    draft: TrafficDraft,
    event: PersistenceEvent,
) -> JoinHandle<()> {
    let id = draft.id.clone();
    tokio::spawn(write_exchange(store, event_sink, draft, id, event))
}

async fn write_exchange(
    store: Arc<Store>,
    event_sink: Arc<dyn ProxyEventSink>,
    draft: TrafficDraft,
    id: String,
    event: PersistenceEvent,
) {
    match tokio::task::spawn_blocking(move || store.insert_exchange(draft)).await {
        Ok(Ok(())) => match event {
            PersistenceEvent::Created => event_sink.traffic_created(&id),
            PersistenceEvent::Updated => event_sink.traffic_updated(&id),
        },
        Ok(Err(error)) => event_sink.storage_warning(&error),
        Err(error) => event_sink.storage_warning(
            &AppError::new("STORAGE_TASK_ERROR", "流量写入任务异常").with_detail(error.to_string()),
        ),
    }
}

fn simple_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(message.to_string()))
        .expect("static response is valid")
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
