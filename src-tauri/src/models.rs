use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub version: String,
    pub arch: String,
    pub certificate_trust_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyStatus {
    pub managed: bool,
    pub recovery_pending: bool,
    pub conflict: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticScope {
    Proxy,
    Mock,
    Replay,
    Script,
    Network,
    Pinning,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticLog {
    pub timestamp: String,
    pub level: DiagnosticLevel,
    pub scope: DiagnosticScope,
    pub event: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, Value>,
}

impl DiagnosticLog {
    pub fn new(
        level: DiagnosticLevel,
        scope: DiagnosticScope,
        event: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level,
            scope,
            event: event.into(),
            message: message.into(),
            exchange_id: None,
            details: BTreeMap::new(),
        }
    }

    pub fn with_exchange_id(mut self, exchange_id: impl Into<String>) -> Self {
        self.exchange_id = Some(exchange_id.into());
        self
    }

    pub fn with_detail(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.details.insert(name.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    pub port: u16,
    pub mode: ProxyMode,
    pub capture_enabled: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            port: 28086,
            mode: ProxyMode::Computer,
            capture_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    Computer,
    Mobile,
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
    pub mode: ProxyMode,
    pub bind_address: Option<String>,
    pub port: u16,
    pub lan_ip: Option<String>,
    pub error: Option<String>,
}

impl Default for ProxyStatus {
    fn default() -> Self {
        Self {
            state: ProxyState::Stopped,
            mode: ProxyMode::Computer,
            bind_address: None,
            port: 28086,
            lan_ip: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrafficOrigin {
    Computer,
    Mobile,
    Replay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    pub device_token: String,
    pub name: String,
    pub bound_ip: String,
    pub created_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingChallenge {
    pub url: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub device_token: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    Exact,
    Contains,
    Regex,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScriptKind {
    Proxy,
    Injection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyScript {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i64,
    #[serde(default)]
    pub methods: Vec<String>,
    pub match_kind: MatchKind,
    pub url_pattern: String,
    pub source: String,
    pub kind: ScriptKind,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub builtin: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkProfileKind {
    Preset,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkProfile {
    pub id: String,
    pub name: String,
    pub kind: NetworkProfileKind,
    pub latency_ms: u64,
    pub jitter_ms: u64,
    pub download_kbps: Option<u64>,
    pub upload_kbps: Option<u64>,
    pub timeout_percent: u8,
    pub disconnect_percent: u8,
    #[serde(default)]
    pub methods: Vec<String>,
    pub match_kind: MatchKind,
    pub url_pattern: String,
    #[serde(default)]
    pub builtin: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FridaDevice {
    pub id: String,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FridaApplication {
    pub pid: Option<u32>,
    pub name: String,
    pub identifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FridaEnvironmentStatus {
    pub available: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PinningLaunchMode {
    Spawn,
    Attach,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PinningSessionConfig {
    pub device_id: String,
    pub target: String,
    pub mode: PinningLaunchMode,
    #[serde(default)]
    pub script_ids: Vec<String>,
    #[serde(default)]
    pub gadget_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PinningSessionStatus {
    pub state: String,
    pub target: Option<String>,
    pub process_id: Option<u32>,
    pub error: Option<String>,
}

impl Default for PinningSessionStatus {
    fn default() -> Self {
        Self {
            state: "stopped".into(),
            target: None,
            process_id: None,
            error: None,
        }
    }
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
    #[serde(default)]
    pub hit_count: u64,
    #[serde(default)]
    pub last_hit_at: Option<String>,
    #[serde(default)]
    pub last_exchange_id: Option<String>,
    #[serde(default)]
    pub source_exchange_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MockRuleMatch {
    pub rule_id: String,
    pub name: String,
    pub priority: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MockRuleTestResult {
    pub matches: Vec<MockRuleMatch>,
    pub selected_rule_id: Option<String>,
    pub conflict: bool,
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
    pub origin: TrafficOrigin,
    pub device_token: Option<String>,
    pub device_name: Option<String>,
    pub source_exchange_id: Option<String>,
    pub matched_mock_rule_id: Option<String>,
    pub applied_script_ids: Vec<String>,
    pub network_profile_id: Option<String>,
    pub stage_timings: BTreeMap<String, u64>,
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
    pub origin: TrafficOrigin,
    pub device_token: Option<String>,
    pub device_name: Option<String>,
    pub source_exchange_id: Option<String>,
    pub matched_mock_rule_id: Option<String>,
    pub applied_script_ids: Vec<String>,
    pub network_profile_id: Option<String>,
    pub stage_timings: BTreeMap<String, u64>,
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
pub struct ReplayRequest {
    pub source_exchange_id: String,
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub mode: ReplayMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayDraft {
    pub source_exchange_id: String,
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub body_text: Option<String>,
    pub content_type: Option<String>,
    pub binary: bool,
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
