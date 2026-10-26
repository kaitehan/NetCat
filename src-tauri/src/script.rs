use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use rquickjs::{Context, Runtime};
use serde::{Deserialize, Serialize};

use crate::models::{MatchKind, ProxyScript, ScriptKind};

const SCRIPT_MEMORY_LIMIT: usize = 64 * 1024 * 1024;
const SCRIPT_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_SCRIPT_BODY_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    #[serde(skip)]
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    #[serde(skip)]
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyntheticResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(skip)]
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptRequestContext<'a> {
    method: &'a str,
    url: &'a str,
    headers: &'a BTreeMap<String, String>,
    body_text: Option<&'a str>,
    body_base64: String,
    body_editable: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RequestPatch {
    method: Option<String>,
    url: Option<String>,
    headers: Option<BTreeMap<String, String>>,
    body_text: Option<String>,
    body_base64: Option<String>,
    response: Option<ResponsePatch>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ResponsePatch {
    status: Option<u16>,
    headers: Option<BTreeMap<String, String>>,
    body_text: Option<String>,
    body_base64: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptExecutionError {
    pub script_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptExecutionReport {
    pub applied_script_ids: Vec<String>,
    pub errors: Vec<ScriptExecutionError>,
    pub synthetic_response: Option<SyntheticResponse>,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptEngine;

impl ScriptEngine {
    pub fn apply_request(
        &self,
        request: &mut PipelineRequest,
        scripts: &[ProxyScript],
    ) -> ScriptExecutionReport {
        let mut ordered = scripts
            .iter()
            .filter(|script| script.enabled && script.kind == ScriptKind::Proxy)
            .collect::<Vec<_>>();
        ordered.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        let mut report = ScriptExecutionReport::default();
        for script in ordered {
            if !matches_script(script, &request.method, &request.url) {
                continue;
            }
            match execute_request_script(script, request) {
                Ok(patch) => {
                    let body_editable = body_is_editable(&request.body, &request.headers);
                    let response = apply_request_patch(request, patch, body_editable);
                    report.applied_script_ids.push(script.id.clone());
                    if response.is_some() {
                        report.synthetic_response = response;
                        break;
                    }
                }
                Err(message) => report.errors.push(ScriptExecutionError {
                    script_id: script.id.clone(),
                    message,
                }),
            }
        }
        report
    }

    pub fn apply_response(
        &self,
        request: &PipelineRequest,
        response: &mut PipelineResponse,
        scripts: &[ProxyScript],
    ) -> ScriptExecutionReport {
        let mut ordered = scripts
            .iter()
            .filter(|script| script.enabled && script.kind == ScriptKind::Proxy)
            .collect::<Vec<_>>();
        ordered.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        let mut report = ScriptExecutionReport::default();
        for script in ordered {
            if !matches_script(script, &request.method, &request.url) {
                continue;
            }
            match execute_response_script(script, request, response) {
                Ok(patch) => {
                    let body_editable = body_is_editable(&response.body, &response.headers);
                    apply_response_patch(response, patch, body_editable);
                    report.applied_script_ids.push(script.id.clone());
                }
                Err(message) => report.errors.push(ScriptExecutionError {
                    script_id: script.id.clone(),
                    message,
                }),
            }
        }
        report
    }
}

pub fn validate_script_source(source: &str) -> Result<(), String> {
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT);
    let started = Instant::now();
    runtime.set_interrupt_handler(Some(Box::new(move || started.elapsed() > SCRIPT_TIMEOUT)));
    let context = Context::full(&runtime).map_err(|error| error.to_string())?;
    let literal = serde_json::to_string(source).map_err(|error| error.to_string())?;
    context
        .with(|ctx| ctx.eval::<(), _>(format!("new Function({literal});")))
        .map_err(|error| error.to_string())
}

fn execute_request_script(
    script: &ProxyScript,
    request: &PipelineRequest,
) -> Result<RequestPatch, String> {
    let body_editable = body_is_editable(&request.body, &request.headers);
    let body_text = body_editable
        .then(|| std::str::from_utf8(&request.body).ok())
        .flatten();
    let context = ScriptRequestContext {
        method: &request.method,
        url: &request.url,
        headers: &request.headers,
        body_text,
        body_base64: base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &request.body,
        ),
        body_editable,
    };
    let context_json = serde_json::to_string(&context).map_err(|error| error.to_string())?;
    let context_literal =
        serde_json::to_string(&context_json).map_err(|error| error.to_string())?;
    let source = format!(
        "(() => {{ 'use strict'; {}\nif (typeof onRequest !== 'function') throw new Error('onRequest is required'); const value = onRequest(JSON.parse({})); return JSON.stringify(value ?? {{}}); }})()",
        script.source, context_literal
    );
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT);
    let started = Instant::now();
    runtime.set_interrupt_handler(Some(Box::new(move || started.elapsed() > SCRIPT_TIMEOUT)));
    let context = Context::full(&runtime).map_err(|error| error.to_string())?;
    let output = context
        .with(|ctx| ctx.eval::<String, _>(source))
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&output).map_err(|error| error.to_string())
}

fn execute_response_script(
    script: &ProxyScript,
    request: &PipelineRequest,
    response: &PipelineResponse,
) -> Result<ResponsePatch, String> {
    let body_editable = body_is_editable(&response.body, &response.headers);
    let body_text = body_editable
        .then(|| std::str::from_utf8(&response.body).ok())
        .flatten();
    let value = serde_json::json!({
        "method": request.method,
        "url": request.url,
        "status": response.status,
        "headers": response.headers,
        "bodyText": body_text,
        "bodyBase64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &response.body),
        "bodyEditable": body_editable,
    });
    execute_hook(&script.source, "onResponse", &value)
}

fn execute_hook<T: serde::de::DeserializeOwned>(
    script_source: &str,
    hook: &str,
    value: &impl Serialize,
) -> Result<T, String> {
    let value_json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    let value_literal = serde_json::to_string(&value_json).map_err(|error| error.to_string())?;
    let source = format!(
        "(() => {{ 'use strict'; {}\nif (typeof {} !== 'function') throw new Error('{} is required'); const value = {}(JSON.parse({})); return JSON.stringify(value ?? {{}}); }})()",
        script_source, hook, hook, hook, value_literal
    );
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT);
    let started = Instant::now();
    runtime.set_interrupt_handler(Some(Box::new(move || started.elapsed() > SCRIPT_TIMEOUT)));
    let context = Context::full(&runtime).map_err(|error| error.to_string())?;
    let output = context
        .with(|ctx| ctx.eval::<String, _>(source))
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&output).map_err(|error| error.to_string())
}

fn apply_request_patch(
    request: &mut PipelineRequest,
    patch: RequestPatch,
    body_editable: bool,
) -> Option<SyntheticResponse> {
    if let Some(method) = patch.method.filter(|value| !value.trim().is_empty()) {
        request.method = method;
    }
    if let Some(url) = patch.url.filter(|value| !value.trim().is_empty()) {
        request.url = url;
    }
    if let Some(headers) = patch.headers {
        request.headers = headers;
    }
    let body = patch.body_text.map(String::into_bytes).or_else(|| {
        patch.body_base64.and_then(|value| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, value).ok()
        })
    });
    if body_editable {
        if let Some(body) = body {
            request.body = body;
            remove_header(&mut request.headers, "content-encoding");
            remove_header(&mut request.headers, "content-length");
            request
                .headers
                .insert("content-length".into(), request.body.len().to_string());
        }
    }
    patch.response.map(|patch| {
        let mut response = PipelineResponse {
            status: patch.status.unwrap_or(200),
            headers: patch.headers.unwrap_or_default(),
            body: Vec::new(),
        };
        apply_response_patch(
            &mut response,
            ResponsePatch {
                status: None,
                headers: None,
                body_text: patch.body_text,
                body_base64: patch.body_base64,
            },
            true,
        );
        SyntheticResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
        }
    })
}

fn apply_response_patch(
    response: &mut PipelineResponse,
    patch: ResponsePatch,
    body_editable: bool,
) {
    if let Some(status) = patch.status.filter(|status| (100..=599).contains(status)) {
        response.status = status;
    }
    if let Some(headers) = patch.headers {
        response.headers = headers;
    }
    let body = patch.body_text.map(String::into_bytes).or_else(|| {
        patch.body_base64.and_then(|value| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, value).ok()
        })
    });
    if body_editable {
        if let Some(body) = body {
            response.body = body;
            remove_header(&mut response.headers, "content-encoding");
            remove_header(&mut response.headers, "content-length");
            response
                .headers
                .insert("content-length".into(), response.body.len().to_string());
        }
    }
}

fn body_is_editable(body: &[u8], headers: &BTreeMap<String, String>) -> bool {
    if body.len() > MAX_SCRIPT_BODY_BYTES || std::str::from_utf8(body).is_err() {
        return false;
    }
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-encoding"))
        .map(|(_, value)| value.trim().is_empty() || value.eq_ignore_ascii_case("identity"))
        .unwrap_or(true)
}

fn matches_script(script: &ProxyScript, method: &str, url: &str) -> bool {
    let method_matches = script.methods.is_empty()
        || script
            .methods
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(method));
    method_matches
        && match script.match_kind {
            MatchKind::Exact => url == script.url_pattern,
            MatchKind::Contains => url.contains(&script.url_pattern),
            MatchKind::Regex => regex::Regex::new(&script.url_pattern)
                .map(|pattern| pattern.is_match(url))
                .unwrap_or(false),
        }
}

fn remove_header(headers: &mut BTreeMap<String, String>, name: &str) {
    if let Some(key) = headers
        .keys()
        .find(|key| key.eq_ignore_ascii_case(name))
        .cloned()
    {
        headers.remove(&key);
    }
}
