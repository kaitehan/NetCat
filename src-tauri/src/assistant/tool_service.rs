use std::{collections::BTreeMap, path::Path, sync::Arc};

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{MatchKind, MockRule, ProxyScript, TrafficFilter},
    rules::validate_rule,
    script::validate_script_source,
    storage::Store,
};

use super::{
    credentials::{CredentialService, NativeCredentialBackend},
    models::AssistantProvider,
    network::PendingRequestManager,
    private_ipc::PrivateIpcService,
    redaction::redact_url_string,
    service::AssistantService,
    settings::AssistantSettingsService,
};

type CredentialLoader = dyn Fn(AssistantProvider) -> Result<Option<String>, AppError> + Send + Sync;

pub struct NetcatPrivateIpcService {
    store: Arc<Store>,
    credential_loader: Arc<CredentialLoader>,
    requests: Arc<PendingRequestManager>,
}

impl NetcatPrivateIpcService {
    pub fn new(
        store: Arc<Store>,
        data_directory: &Path,
        requests: Arc<PendingRequestManager>,
    ) -> Self {
        let data_directory = data_directory.to_path_buf();
        let settings_store = Arc::clone(&store);
        let mut service = Self::with_credential_loader(store, move |provider| {
            let route = if provider == AssistantProvider::OpenAi {
                Some(
                    AssistantSettingsService::new(Arc::clone(&settings_store))
                        .get()?
                        .open_ai_provider,
                )
            } else {
                None
            };
            CredentialService::new(NativeCredentialBackend::new(&data_directory))
                .resolve_scoped(provider, route.as_deref())
        });
        service.requests = requests;
        service
    }

    pub fn with_credential_loader(
        store: Arc<Store>,
        loader: impl Fn(AssistantProvider) -> Result<Option<String>, AppError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            store,
            credential_loader: Arc::new(loader),
            requests: Arc::new(PendingRequestManager::default()),
        }
    }

    fn provider(params: &Value) -> Result<AssistantProvider, AppError> {
        match required_string(params, "reference")? {
            "DEEPSEEK_API_KEY" => Ok(AssistantProvider::DeepSeek),
            "OPENAI_API_KEY" => Ok(AssistantProvider::OpenAi),
            _ => Err(AppError::new(
                "ASSISTANT_CREDENTIAL_REFERENCE_DENIED",
                "该凭据引用不允许访问",
            )),
        }
    }

    fn exchange(&self, params: &Value) -> Result<Value, AppError> {
        let exchange_id = required_string(params, "resourceId")?;
        let projection =
            AssistantService::new(Arc::clone(&self.store)).exchange_model_input(exchange_id)?;
        serde_json::from_str(&projection).map_err(|error| {
            AppError::new("ASSISTANT_TOOL_ERROR", "抓包投影无效").with_detail(error.to_string())
        })
    }

    fn search_exchanges(&self, params: &Value) -> Result<Value, AppError> {
        let query = optional_string(params, "query")
            .unwrap_or_default()
            .trim()
            .to_owned();
        let domain = optional_string(params, "domain")
            .unwrap_or_default()
            .trim()
            .trim_matches('.')
            .to_lowercase();
        if query.is_empty() && domain.is_empty() {
            return Err(AppError::new(
                "ASSISTANT_TOOL_ARGUMENT_INVALID",
                "query 与 domain 至少需要提供一个",
            ));
        }
        let method = optional_string(params, "method")
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_uppercase());
        let status_group = params
            .get("statusGroup")
            .and_then(Value::as_u64)
            .map(|value| value as u16);
        if status_group.is_some_and(|value| !(2..=5).contains(&value)) {
            return Err(AppError::new(
                "ASSISTANT_TOOL_ARGUMENT_INVALID",
                "statusGroup 只支持 2、3、4、5",
            ));
        }
        let limit = params
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .clamp(1, 50) as usize;
        let mut cursor = None;
        let mut matches = Vec::new();
        loop {
            let rows = self.store.list_traffic(
                TrafficFilter {
                    query: (!query.is_empty()).then_some(query.clone()),
                    method: method.clone(),
                    status_group,
                    host: None,
                },
                cursor.take(),
                500,
            )?;
            if rows.is_empty() {
                break;
            }
            cursor = rows.last().map(|row| row.id.clone());
            matches.extend(rows.into_iter().filter(|row| {
                domain.is_empty()
                    || row.host.eq_ignore_ascii_case(&domain)
                    || row.host.to_lowercase().ends_with(&format!(".{domain}"))
            }));
            if cursor.is_none() {
                break;
            }
        }
        let total = matches.len();
        let results = matches
            .into_iter()
            .take(limit)
            .map(|row| {
                json!({
                    "resourceId": row.id,
                    "method": row.method,
                    "url": redact_url_string(&row.url),
                    "status": row.status,
                    "durationMs": row.duration_ms,
                    "startedAt": row.started_at,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "filter": {
                "query": query,
                "domain": domain,
                "method": method,
                "statusGroup": status_group,
            },
            "total": total,
            "results": results,
        }))
    }

    fn similar(&self, params: &Value) -> Result<Value, AppError> {
        let exchange_id = required_string(params, "resourceId")?;
        let source = self.store.get_traffic_detail(exchange_id)?;
        let limit = params
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(10)
            .clamp(1, 20) as usize;
        let mut rows = self.store.list_traffic(
            TrafficFilter {
                method: Some(source.summary.method),
                host: Some(source.summary.host),
                ..TrafficFilter::default()
            },
            None,
            limit + 1,
        )?;
        for row in &mut rows {
            row.url = redact_url_string(&row.url);
        }
        Ok(serde_json::to_value(
            rows.into_iter()
                .filter(|row| row.id != exchange_id)
                .take(limit)
                .collect::<Vec<_>>(),
        )
        .map_err(serialization_error)?)
    }

    fn compare(&self, params: &Value) -> Result<Value, AppError> {
        let left = self
            .store
            .get_traffic_detail(required_string(params, "resourceId")?)?;
        let right = self
            .store
            .get_traffic_detail(required_string(params, "otherResourceId")?)?;
        Ok(json!({
            "method": { "left": left.summary.method, "right": right.summary.method },
            "url": {
                "left": redact_url_string(&left.summary.url),
                "right": redact_url_string(&right.summary.url),
            },
            "status": { "left": left.summary.status, "right": right.summary.status },
            "durationMs": { "left": left.summary.duration_ms, "right": right.summary.duration_ms },
            "responseBytes": {
                "left": left.summary.response_bytes,
                "right": right.summary.response_bytes,
            },
        }))
    }

    fn mock_draft(&self, params: &Value) -> Result<Value, AppError> {
        let exchange_id = required_string(params, "resourceId")?;
        let exchange = self.store.get_traffic_detail(exchange_id)?;
        let status = params
            .get("status")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(|| AppError::new("ASSISTANT_TOOL_ARGUMENT_INVALID", "Mock 状态码无效"))?;
        let (match_kind, url_pattern) = draft_url_match(&exchange.summary.url);
        let rule = MockRule {
            id: Uuid::new_v4().to_string(),
            name: required_string(params, "name")?.trim().into(),
            enabled: false,
            priority: 0,
            methods: vec![exchange.summary.method],
            match_kind,
            url_pattern,
            status,
            response_headers: BTreeMap::new(),
            response_body: required_string(params, "responseBody")?.into(),
            delay_ms: 0,
            created_at: Utc::now().to_rfc3339(),
            hit_count: 0,
            last_hit_at: None,
            last_exchange_id: None,
            source_exchange_id: Some(exchange_id.into()),
        };
        validate_rule(&rule)?;
        Ok(json!({
            "id": rule.id,
            "kind": "mock",
            "status": "pending",
            "payload": rule,
        }))
    }

    fn script_draft(&self, params: &Value) -> Result<Value, AppError> {
        let exchange_id = required_string(params, "resourceId")?;
        let exchange = self.store.get_traffic_detail(exchange_id)?;
        let source = required_string(params, "source")?.trim();
        validate_script_source(source).map_err(|error| {
            AppError::new("ASSISTANT_SCRIPT_INVALID", "QuickJS 草稿无法通过校验").with_detail(error)
        })?;
        let (match_kind, url_pattern) = draft_url_match(&exchange.summary.url);
        let script = ProxyScript {
            id: Uuid::new_v4().to_string(),
            name: required_string(params, "name")?.trim().into(),
            enabled: false,
            priority: 0,
            methods: vec![exchange.summary.method],
            match_kind,
            url_pattern,
            source: source.into(),
            created_at: Utc::now().to_rfc3339(),
        };
        Ok(json!({
            "id": script.id,
            "kind": "script",
            "status": "pending",
            "payload": script,
        }))
    }
}

fn draft_url_match(value: &str) -> (MatchKind, String) {
    let Ok(mut url) = url::Url::parse(value) else {
        return (MatchKind::Contains, "[OMITTED INVALID URL]".into());
    };
    if url.query().is_some() {
        url.set_query(None);
        url.set_fragment(None);
        return (MatchKind::Contains, url.to_string());
    }
    (MatchKind::Exact, redact_url_string(value))
}

impl PrivateIpcService for NetcatPrivateIpcService {
    fn call(&self, method: &str, params: &Value) -> Result<Value, AppError> {
        match method {
            "credentials/resolve" => {
                let value = (self.credential_loader)(Self::provider(params)?)?;
                Ok(match value {
                    Some(value) => json!({ "configured": true, "value": value }),
                    None => json!({ "configured": false }),
                })
            }
            "credentials/describe" => {
                let configured = (self.credential_loader)(Self::provider(params)?)?.is_some();
                Ok(json!({ "configured": configured }))
            }
            "netcat_search_exchanges" => self.search_exchanges(params),
            "netcat_get_exchange" => self.exchange(params),
            "netcat_parse_curl" => self
                .requests
                .projection(required_string(params, "resourceId")?),
            "netcat_find_similar_exchanges" => self.similar(params),
            "netcat_compare_exchanges" => self.compare(params),
            "netcat_get_diagnostics" => {
                let database = self.store.quick_check()?;
                let recent_exchange_count = self
                    .store
                    .list_traffic(TrafficFilter::default(), None, 100)?
                    .len();
                Ok(json!({
                    "summary": "已完成 NetCat 本地诊断检查",
                    "facts": [
                        format!("数据库检查结果: {database}"),
                        format!("最近抓包数量: {recent_exchange_count}"),
                    ],
                    "possibleCauses": [],
                    "confidence": 1.0,
                    "evidence": [
                        { "kind": "database", "value": database },
                        { "kind": "trafficCount", "value": recent_exchange_count },
                    ],
                    "verificationSteps": ["结合目标抓包的状态码、耗时和响应内容继续验证"],
                    "actions": ["选择异常抓包并使用相似请求对比"],
                }))
            }
            "netcat_create_mock_draft" => self.mock_draft(params),
            "netcat_create_script_draft" => self.script_draft(params),
            "netcat_execute_curl" => {
                let response = tauri::async_runtime::block_on(
                    self.requests
                        .execute_handle(required_string(params, "requestHandle")?),
                )?;
                serde_json::to_value(response).map_err(serialization_error)
            }
            _ => Err(AppError::new(
                "ASSISTANT_TOOL_NOT_ALLOWED",
                "该智奈工具未启用",
            )),
        }
    }
}

fn required_string<'a>(params: &'a Value, name: &str) -> Result<&'a str, AppError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "ASSISTANT_TOOL_ARGUMENT_INVALID",
                format!("缺少参数 {name}"),
            )
        })
}

fn optional_string<'a>(params: &'a Value, name: &str) -> Option<&'a str> {
    params.get(name).and_then(Value::as_str)
}

fn serialization_error(error: serde_json::Error) -> AppError {
    AppError::new("ASSISTANT_TOOL_ERROR", "无法生成智奈工具结果").with_detail(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use serde_json::json;

    use crate::{
        assistant::{private_ipc::PrivateIpcService, tool_service::NetcatPrivateIpcService},
        models::{TrafficDraft, TrafficOrigin},
        storage::Store,
    };

    #[test]
    fn private_service_resolves_credentials_and_redacts_exchange_tools() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        store
            .insert_exchange(TrafficDraft {
                id: "exchange-1".into(),
                started_at: "2026-11-24T10:00:00Z".into(),
                method: "GET".into(),
                url: "https://api.example.com/private?api_key=query-secret&page=2".into(),
                host: "api.example.com".into(),
                status: Some(200),
                duration_ms: Some(15),
                request_headers: BTreeMap::from([(
                    "Authorization".into(),
                    "Bearer capture-secret".into(),
                )]),
                response_headers: BTreeMap::new(),
                request_body: Vec::new(),
                response_body: Vec::new(),
                content_type: None,
                mocked: false,
                origin: TrafficOrigin::Computer,
                device_token: None,
                device_name: None,
                source_exchange_id: None,
                matched_mock_rule_id: None,
                applied_script_ids: Vec::new(),
                network_profile_id: None,
                stage_timings: BTreeMap::new(),
                error: None,
            })
            .unwrap();
        let service = NetcatPrivateIpcService::with_credential_loader(store, |provider| {
            Ok(Some(format!("{}-runtime-secret", provider.id())))
        });

        let credential = service
            .call(
                "credentials/resolve",
                &json!({ "reference": "DEEPSEEK_API_KEY" }),
            )
            .unwrap();
        assert_eq!(credential["value"], "deepseek-runtime-secret");

        let exchange = service
            .call(
                "netcat_get_exchange",
                &json!({ "resourceId": "exchange-1" }),
            )
            .unwrap();
        let serialized = serde_json::to_string(&exchange).unwrap();
        assert!(serialized.contains("[REDACTED]"));
        assert!(!serialized.contains("capture-secret"));
        assert!(!serialized.contains("query-secret"));
        assert!(serialized.contains("page=2"));
        assert!(!serialized.contains("runtime-secret"));
    }

    #[test]
    fn draft_tools_validate_without_persisting_or_enabling_anything() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        store
            .insert_exchange(TrafficDraft {
                id: "exchange-1".into(),
                started_at: "2026-11-24T10:00:00Z".into(),
                method: "POST".into(),
                url: "https://api.example.com/login?api_key=query-secret&page=2".into(),
                host: "api.example.com".into(),
                status: Some(401),
                duration_ms: Some(20),
                request_headers: BTreeMap::new(),
                response_headers: BTreeMap::new(),
                request_body: Vec::new(),
                response_body: Vec::new(),
                content_type: None,
                mocked: false,
                origin: TrafficOrigin::Computer,
                device_token: None,
                device_name: None,
                source_exchange_id: None,
                matched_mock_rule_id: None,
                applied_script_ids: Vec::new(),
                network_profile_id: None,
                stage_timings: BTreeMap::new(),
                error: None,
            })
            .unwrap();
        let service =
            NetcatPrivateIpcService::with_credential_loader(Arc::clone(&store), |_| Ok(None));

        let mock = service
            .call(
                "netcat_create_mock_draft",
                &json!({
                    "resourceId": "exchange-1",
                    "name": "登录失败响应",
                    "status": 401,
                    "responseBody": "{\"error\":\"unauthorized\"}"
                }),
            )
            .unwrap();
        let script = service
            .call(
                "netcat_create_script_draft",
                &json!({
                    "resourceId": "exchange-1",
                    "name": "添加诊断头",
                    "source": "function onRequest(request) { request.headers['x-debug'] = '1'; return request; }"
                }),
            )
            .unwrap();

        assert_eq!(mock["kind"], "mock");
        assert_eq!(mock["status"], "pending");
        assert_eq!(mock["payload"]["matchKind"], "contains");
        assert_eq!(
            mock["payload"]["urlPattern"],
            "https://api.example.com/login"
        );
        assert_eq!(script["kind"], "script");
        assert_eq!(script["status"], "pending");
        assert_eq!(script["payload"]["matchKind"], "contains");
        assert!(!serde_json::to_string(&script)
            .unwrap()
            .contains("query-secret"));
        assert!(store.list_mock_rules().unwrap().is_empty());
        assert!(store.list_scripts().unwrap().is_empty());
    }

    #[test]
    fn diagnostics_tool_uses_the_stable_evidence_shape() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        let service = NetcatPrivateIpcService::with_credential_loader(store, |_| Ok(None));

        let diagnosis = service.call("netcat_get_diagnostics", &json!({})).unwrap();

        assert!(diagnosis.get("summary").is_some());
        assert!(diagnosis.get("facts").unwrap().is_array());
        assert!(diagnosis.get("possibleCauses").unwrap().is_array());
        assert!(diagnosis.get("confidence").unwrap().is_number());
        assert!(diagnosis.get("evidence").unwrap().is_array());
        assert!(diagnosis.get("verificationSteps").unwrap().is_array());
        assert!(diagnosis.get("actions").unwrap().is_array());
    }

    #[test]
    fn search_tool_matches_root_and_subdomains_without_exposing_ids_in_summaries() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        for (id, host, url, status) in [
            (
                "root",
                "example.com",
                "https://example.com/users?page=1",
                200,
            ),
            (
                "sub",
                "api.example.com",
                "https://api.example.com/users?token=secret",
                401,
            ),
            (
                "lookalike",
                "notexample.com",
                "https://notexample.com/users",
                200,
            ),
        ] {
            store
                .insert_exchange(TrafficDraft {
                    id: id.into(),
                    started_at: "2026-11-25T10:00:00Z".into(),
                    method: "GET".into(),
                    url: url.into(),
                    host: host.into(),
                    status: Some(status),
                    duration_ms: Some(15),
                    request_headers: BTreeMap::new(),
                    response_headers: BTreeMap::new(),
                    request_body: Vec::new(),
                    response_body: Vec::new(),
                    content_type: None,
                    mocked: false,
                    origin: TrafficOrigin::Computer,
                    device_token: None,
                    device_name: None,
                    source_exchange_id: None,
                    matched_mock_rule_id: None,
                    applied_script_ids: Vec::new(),
                    network_profile_id: None,
                    stage_timings: BTreeMap::new(),
                    error: None,
                })
                .unwrap();
        }
        let service = NetcatPrivateIpcService::with_credential_loader(store, |_| Ok(None));

        let result = service
            .call(
                "netcat_search_exchanges",
                &json!({ "domain": "example.com", "query": "users", "limit": 1 }),
            )
            .unwrap();

        assert_eq!(result["total"], 2);
        assert_eq!(result["results"].as_array().unwrap().len(), 1);
        assert!(result["results"][0]["resourceId"].is_string());
        assert!(!result["results"][0]["url"]
            .as_str()
            .unwrap()
            .contains("secret"));
        assert_eq!(result["filter"]["domain"], "example.com");
    }
}
