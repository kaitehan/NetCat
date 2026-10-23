use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    pub port: u16,
    pub allow_lan: bool,
    pub capture_enabled: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            port: 28086,
            allow_lan: false,
            capture_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub state: ProxyState,
    pub bind_address: Option<String>,
    pub port: u16,
    pub lan_ip: Option<String>,
    pub error: Option<String>,
}

impl Default for ProxyStatus {
    fn default() -> Self {
        Self {
            state: ProxyState::Stopped,
            bind_address: None,
            port: 28086,
            lan_ip: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    Exact,
    Contains,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MockRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i64,
    pub methods: Vec<String>,
    pub match_kind: MatchKind,
    pub url_pattern: String,
    pub status: u16,
    pub response_headers: BTreeMap<String, String>,
    pub response_body: String,
    pub delay_ms: u64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct TrafficDraft {
    pub id: String,
    pub started_at: String,
    pub method: String,
    pub url: String,
    pub host: String,
    pub status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub request_headers: BTreeMap<String, String>,
    pub response_headers: BTreeMap<String, String>,
    pub request_body: Vec<u8>,
    pub response_body: Vec<u8>,
    pub content_type: Option<String>,
    pub mocked: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSummary {
    pub id: String,
    pub started_at: String,
    pub method: String,
    pub url: String,
    pub host: String,
    pub status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub response_bytes: u64,
    pub content_type: Option<String>,
    pub mocked: bool,
    pub truncated: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrafficDetail {
    #[serde(flatten)]
    pub summary: TrafficSummary,
    pub request_headers: BTreeMap<String, String>,
    pub response_headers: BTreeMap<String, String>,
    pub request_body: Vec<u8>,
    pub response_body: Vec<u8>,
    pub request_body_text: Option<String>,
    pub response_body_text: Option<String>,
    pub request_original_bytes: u64,
    pub response_original_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TrafficCopyKind {
    Url,
    Curl,
    Powershell,
    FetchBrowser,
    FetchNode,
    RequestHeaders,
    ResponseHeaders,
    ResponseBody,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReplayMode {
    Upstream,
    MockAware,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayResult {
    pub exchange_id: String,
    pub mocked: bool,
    pub exchange: TrafficSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrafficFilter {
    pub query: Option<String>,
    pub method: Option<String>,
    pub status_group: Option<u16>,
    pub host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaStatus {
    pub generated: bool,
    pub certificate_path: Option<String>,
    pub fingerprint: Option<String>,
}
