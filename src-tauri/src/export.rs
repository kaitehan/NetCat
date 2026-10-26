use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

use crate::{
    error::AppError,
    models::{TrafficCopyKind, TrafficDetail},
};

const MANAGED_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "proxy-connection",
    "keep-alive",
    "upgrade",
    "te",
    "trailer",
];

const BROWSER_FORBIDDEN_HEADERS: &[&str] = &[
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "expect",
    "origin",
    "permissions-policy",
    "referer",
    "sec-fetch-dest",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-user",
    "user-agent",
    "via",
];

pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub fn build_curl(
    method: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: Option<&str>,
) -> String {
    let mut parts = vec![
        "curl".to_string(),
        "-X".to_string(),
        shell_quote(method),
        shell_quote(url),
    ];
    for (name, value) in headers {
        parts.push("-H".to_string());
        parts.push(shell_quote(&format!("{name}: {value}")));
    }
    if let Some(body) = body.filter(|body| !body.is_empty()) {
        parts.push("--data-raw".to_string());
        parts.push(shell_quote(body));
    }
    parts.join(" ")
}

pub fn build_exchange_copy(
    detail: &TrafficDetail,
    kind: TrafficCopyKind,
) -> Result<String, AppError> {
    match kind {
        TrafficCopyKind::Url => Ok(detail.summary.url.clone()),
        TrafficCopyKind::RequestHeaders => Ok(format_headers(&detail.request_headers)),
        TrafficCopyKind::ResponseHeaders => Ok(format_headers(&detail.response_headers)),
        TrafficCopyKind::ResponseBody => detail.response_body_text.clone().ok_or_else(|| {
            AppError::new("BINARY_RESPONSE_BODY", "二进制响应正文不能复制为文本")
                .with_action("请在响应详情中查看 Hex 预览或导出 HAR")
        }),
        TrafficCopyKind::Curl
        | TrafficCopyKind::Powershell
        | TrafficCopyKind::FetchBrowser
        | TrafficCopyKind::FetchNode => {
            let body = request_body_text(detail)?;
            let headers = filtered_request_headers(
                &detail.request_headers,
                kind == TrafficCopyKind::FetchBrowser,
            );
            Ok(match kind {
                TrafficCopyKind::Curl => {
                    build_curl(&detail.summary.method, &detail.summary.url, &headers, body)
                }
                TrafficCopyKind::Powershell => {
                    build_powershell(&detail.summary.method, &detail.summary.url, &headers, body)
                }
                TrafficCopyKind::FetchBrowser | TrafficCopyKind::FetchNode => {
                    build_fetch(&detail.summary.method, &detail.summary.url, &headers, body)
                }
                _ => unreachable!(),
            })
        }
    }
}

fn request_body_text(detail: &TrafficDetail) -> Result<Option<&str>, AppError> {
    if detail.request_body.is_empty() {
        return Ok(None);
    }
    detail
        .request_body_text
        .as_deref()
        .map(Some)
        .ok_or_else(|| {
            AppError::new("BINARY_REQUEST_BODY", "二进制请求正文不能生成请求命令")
                .with_action("请使用原始客户端重新发送该请求")
        })
}

pub fn filtered_request_headers(
    headers: &BTreeMap<String, String>,
    for_browser: bool,
) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter(|(name, _)| {
            let name = name.to_ascii_lowercase();
            !MANAGED_HEADERS.contains(&name.as_str())
                && (!for_browser || !BROWSER_FORBIDDEN_HEADERS.contains(&name.as_str()))
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

fn format_headers(headers: &BTreeMap<String, String>) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn build_powershell(
    method: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: Option<&str>,
) -> String {
    let header_entries = headers
        .iter()
        .map(|(name, value)| format!("{} = {}", powershell_quote(name), powershell_quote(value)))
        .collect::<Vec<_>>()
        .join("; ");
    let mut command = format!(
        "Invoke-WebRequest -Method {} -Uri {}",
        powershell_quote(method),
        powershell_quote(url)
    );
    if !header_entries.is_empty() {
        command.push_str(&format!(" -Headers @{{ {header_entries} }}"));
    }
    if let Some(body) = body.filter(|body| !body.is_empty()) {
        command.push_str(&format!(" -Body {}", powershell_quote(body)));
    }
    command
}

fn build_fetch(
    method: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: Option<&str>,
) -> String {
    let mut options = serde_json::Map::new();
    options.insert("method".into(), json!(method));
    if !headers.is_empty() {
        options.insert("headers".into(), json!(headers));
    }
    if let Some(body) = body.filter(|body| !body.is_empty()) {
        options.insert("body".into(), json!(body));
    }
    format!(
        "fetch({}, {})",
        serde_json::to_string(url).expect("URL serialization cannot fail"),
        serde_json::to_string_pretty(&options).expect("fetch options serialization cannot fail")
    )
}

pub fn build_har(entries: &[TrafficDetail]) -> Result<Value, AppError> {
    let entries = entries.iter().map(har_entry).collect::<Vec<_>>();
    Ok(json!({
        "log": {
            "version": "1.2",
            "creator": {
                "name": "NetCat",
                "version": env!("CARGO_PKG_VERSION")
            },
            "entries": entries
        }
    }))
}

fn har_entry(detail: &TrafficDetail) -> Value {
    let response_text = String::from_utf8(detail.response_body.clone());
    let (text, encoding) = match response_text {
        Ok(text) => (text, None),
        Err(_) => (STANDARD.encode(&detail.response_body), Some("base64")),
    };
    let request_text = String::from_utf8_lossy(&detail.request_body).into_owned();
    json!({
        "startedDateTime": detail.summary.started_at,
        "time": detail.summary.duration_ms.unwrap_or_default(),
        "request": {
            "method": detail.summary.method,
            "url": detail.summary.url,
            "httpVersion": "HTTP/1.1",
            "headers": har_headers(&detail.request_headers),
            "queryString": [],
            "cookies": [],
            "headersSize": -1,
            "bodySize": detail.request_original_bytes,
            "postData": {
                "mimeType": detail.request_headers.get("content-type").cloned().unwrap_or_default(),
                "text": request_text
            }
        },
        "response": {
            "status": detail.summary.status.unwrap_or_default(),
            "statusText": "",
            "httpVersion": "HTTP/1.1",
            "headers": har_headers(&detail.response_headers),
            "cookies": [],
            "content": {
                "size": detail.response_original_bytes,
                "mimeType": detail.summary.content_type.clone().unwrap_or_default(),
                "text": text,
                "encoding": encoding
            },
            "redirectURL": "",
            "headersSize": -1,
            "bodySize": detail.response_original_bytes
        },
        "cache": {},
        "timings": {
            "send": 0,
            "wait": detail.summary.duration_ms.unwrap_or_default(),
            "receive": 0
        },
        "_netcat_mocked": detail.summary.mocked,
        "_netcat_truncated": detail.summary.truncated,
        "_netcat_error": detail.summary.error,
        "_netcat_origin": detail.summary.origin,
        "_netcat_device_token": detail.summary.device_token,
        "_netcat_device_name": detail.summary.device_name,
        "_netcat_appliedScriptIds": detail.summary.applied_script_ids,
        "_netcat_networkProfileId": detail.summary.network_profile_id,
        "_netcat_stageTimings": detail.summary.stage_timings,
    })
}

fn har_headers(headers: &BTreeMap<String, String>) -> Vec<Value> {
    headers
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect()
}
