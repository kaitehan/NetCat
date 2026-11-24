use std::{
    collections::{BTreeMap, HashMap},
    net::ToSocketAddrs,
    sync::Mutex,
    time::SystemTime,
};

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;

use crate::error::AppError;

use super::{
    authorization::{AuthorizationStore, RequestBinding},
    curl::{CurlOperation, ExecutableCurlRequest},
    redaction::redact_headers,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkDiagnosticResponse {
    pub url: String,
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub truncated: bool,
    pub followed_redirect: bool,
    pub redirect_location: Option<String>,
    pub tls_verification: bool,
    pub untrusted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRequestPreview {
    pub domain: String,
    pub resolved_ips: Vec<String>,
    pub method: String,
    pub body_bytes: usize,
    pub sensitive_fields: Vec<String>,
    pub tls_verification: bool,
    pub insecure_option_ignored: bool,
}

#[derive(Clone)]
struct PendingRequest {
    operation_id: String,
    request: ExecutableCurlRequest,
    preview: NetworkRequestPreview,
    projection: Value,
}

#[derive(Default)]
pub struct PendingRequestManager {
    pending: Mutex<HashMap<String, PendingRequest>>,
    authorizations: Mutex<AuthorizationStore>,
}

impl PendingRequestManager {
    pub fn remember(
        &self,
        conversation_id: &str,
        operation: CurlOperation,
    ) -> NetworkRequestPreview {
        let projection = serde_json::to_value(&operation.projection).unwrap_or(Value::Null);
        let domain = operation.raw.url.host_str().unwrap_or_default().to_owned();
        let port = operation.raw.url.port_or_known_default().unwrap_or(443);
        let mut resolved_ips = (domain.as_str(), port)
            .to_socket_addrs()
            .map(|addresses| {
                addresses
                    .map(|address| address.ip().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        resolved_ips.sort();
        resolved_ips.dedup();
        let preview = NetworkRequestPreview {
            domain,
            resolved_ips,
            method: operation.raw.method.clone(),
            body_bytes: operation.raw.body.len(),
            sensitive_fields: operation.projection.sensitive_fields.clone(),
            tls_verification: true,
            insecure_option_ignored: !operation.raw.tls_verification,
        };
        self.pending
            .lock()
            .expect("assistant pending request mutex poisoned")
            .insert(
                conversation_id.into(),
                PendingRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    request: operation.raw,
                    preview: preview.clone(),
                    projection,
                },
            );
        preview
    }

    pub fn projection(&self, conversation_id: &str) -> Result<Value, AppError> {
        self.pending
            .lock()
            .expect("assistant pending request mutex poisoned")
            .get(conversation_id)
            .map(|pending| pending.projection.clone())
            .ok_or_else(|| {
                AppError::new(
                    "ASSISTANT_REQUEST_NOT_AVAILABLE",
                    "该 cURL 的脱敏投影已不可用",
                )
            })
    }

    pub fn preview(&self, conversation_id: &str) -> Result<NetworkRequestPreview, AppError> {
        self.pending
            .lock()
            .expect("assistant pending request mutex poisoned")
            .get(conversation_id)
            .map(|pending| pending.preview.clone())
            .ok_or_else(|| {
                AppError::new(
                    "ASSISTANT_REQUEST_NOT_AVAILABLE",
                    "该 cURL 的原始请求已不可用",
                )
            })
    }

    pub fn authorize(&self, conversation_id: &str) -> Result<String, AppError> {
        let pending = self
            .pending
            .lock()
            .expect("assistant pending request mutex poisoned")
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "ASSISTANT_REQUEST_NOT_AVAILABLE",
                    "该 cURL 的原始请求已不可用",
                )
            })?;
        self.authorizations
            .lock()
            .expect("assistant authorization mutex poisoned")
            .issue(binding(conversation_id, &pending), SystemTime::now())
    }

    pub async fn execute(
        &self,
        conversation_id: &str,
        handle: &str,
    ) -> Result<NetworkDiagnosticResponse, AppError> {
        let pending = self
            .pending
            .lock()
            .expect("assistant pending request mutex poisoned")
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "ASSISTANT_REQUEST_NOT_AVAILABLE",
                    "该 cURL 的原始请求已不可用",
                )
            })?;
        self.authorizations
            .lock()
            .expect("assistant authorization mutex poisoned")
            .consume(
                handle,
                &binding(conversation_id, &pending),
                SystemTime::now(),
            )?;
        self.pending
            .lock()
            .expect("assistant pending request mutex poisoned")
            .remove(conversation_id);
        execute_request(pending.request, 1024 * 1024).await
    }

    pub async fn execute_handle(
        &self,
        handle: &str,
    ) -> Result<NetworkDiagnosticResponse, AppError> {
        let conversation_id = self
            .authorizations
            .lock()
            .expect("assistant authorization mutex poisoned")
            .conversation_id(handle)?
            .to_owned();
        self.execute(&conversation_id, handle).await
    }

    pub fn clear(&self) {
        self.pending
            .lock()
            .expect("assistant pending request mutex poisoned")
            .clear();
    }
}

fn binding(conversation_id: &str, pending: &PendingRequest) -> RequestBinding {
    RequestBinding::new(
        conversation_id,
        &pending.operation_id,
        &pending.request.method,
        pending.request.url.as_str(),
        &pending.request.body,
    )
}

pub async fn execute_authorized_request(
    authorizations: &mut AuthorizationStore,
    handle: &str,
    binding: &RequestBinding,
    request: ExecutableCurlRequest,
    max_response_bytes: usize,
) -> Result<NetworkDiagnosticResponse, AppError> {
    authorizations.consume(handle, binding, SystemTime::now())?;
    execute_request(request, max_response_bytes).await
}

async fn execute_request(
    request: ExecutableCurlRequest,
    max_response_bytes: usize,
) -> Result<NetworkDiagnosticResponse, AppError> {
    let timeout = request.timeout_seconds.unwrap_or(15).clamp(1, 30);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
        .map_err(network_error)?;
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|error| {
        AppError::new("ASSISTANT_REQUEST_INVALID", "联网诊断请求方法无效")
            .with_detail(error.to_string())
    })?;
    let mut builder = client.request(method, request.url.clone());
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    if !request.body.is_empty() {
        builder = builder.body(request.body);
    }
    let response = builder.send().await.map_err(network_error)?;
    let status = response.status();
    let redirect_location = if status.is_redirection() {
        response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    } else {
        None
    };
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut body = Vec::with_capacity(max_response_bytes.min(64 * 1024));
    let mut truncated = false;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(network_error)?;
        let remaining = max_response_bytes.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
        if body.len() == max_response_bytes {
            truncated = stream.next().await.is_some();
            break;
        }
    }
    Ok(NetworkDiagnosticResponse {
        url: request.url.to_string(),
        status: status.as_u16(),
        headers: redact_headers(&headers),
        body,
        truncated,
        followed_redirect: false,
        redirect_location,
        tls_verification: true,
        untrusted: true,
    })
}

fn network_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("ASSISTANT_NETWORK_ERROR", "联网诊断请求失败").with_detail(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::SystemTime,
    };

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::assistant::{
        authorization::{AuthorizationStore, RequestBinding},
        curl::ExecutableCurlRequest,
        network::execute_authorized_request,
    };

    #[tokio::test]
    async fn authorized_request_is_sent_once_and_response_is_bounded() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let server_hits = Arc::clone(&hits);
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 1024];
            let _ = stream.read(&mut request).await.unwrap();
            server_hits.fetch_add(1, Ordering::SeqCst);
            let body = vec![b'x'; 128];
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            stream.write_all(&body).await.unwrap();
        });
        let request = ExecutableCurlRequest {
            method: "GET".into(),
            url: format!("http://{address}/diagnose").parse().unwrap(),
            headers: BTreeMap::new(),
            body: Vec::new(),
            follow_redirects: true,
            tls_verification: false,
            timeout_seconds: Some(2),
        };
        let binding = RequestBinding::new(
            "conversation-1",
            "operation-1",
            &request.method,
            request.url.as_str(),
            &request.body,
        );
        let mut authorizations = AuthorizationStore::default();
        let handle = authorizations
            .issue(binding.clone(), SystemTime::now())
            .unwrap();

        let result =
            execute_authorized_request(&mut authorizations, &handle, &binding, request.clone(), 32)
                .await
                .unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body.len(), 32);
        assert!(result.truncated);
        assert!(!result.followed_redirect);
        assert!(result.tls_verification);

        let replay =
            execute_authorized_request(&mut authorizations, &handle, &binding, request, 32)
                .await
                .unwrap_err();
        assert_eq!(replay.code, "ASSISTANT_AUTHORIZATION_USED");
        server.await.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
