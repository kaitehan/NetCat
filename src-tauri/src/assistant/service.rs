use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{error::AppError, storage::Store};

use super::{
    curl::parse_curl,
    models::{
        AssistantActivity, AssistantConversation, AssistantDraft, AssistantInput, AssistantMessage,
        AssistantSnapshot, AssistantStatus,
    },
    redaction::{redact_headers, redact_json, redact_url_string},
};

pub struct AssistantService {
    store: Arc<Store>,
}

impl AssistantService {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    pub fn list_conversations(&self) -> Result<Vec<AssistantConversation>, AppError> {
        self.store.list_assistant_conversations()
    }

    pub fn create_conversation(&self) -> Result<AssistantConversation, AppError> {
        let timestamp = Utc::now().to_rfc3339();
        let conversation = AssistantConversation::new(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            "新对话",
            timestamp,
        );
        self.store.save_assistant_conversation(&conversation)?;
        self.save_snapshot(&AssistantSnapshot {
            conversation: conversation.clone(),
            messages: Vec::new(),
            activities: Vec::new(),
            drafts: Vec::new(),
            status: AssistantStatus::Ready,
        })?;
        Ok(conversation)
    }

    pub fn snapshot(&self, conversation_id: &str) -> Result<AssistantSnapshot, AppError> {
        let projection = self
            .store
            .load_assistant_projection(conversation_id)?
            .ok_or_else(|| AppError::new("NOT_FOUND", "未找到智奈会话"))?;
        serde_json::from_value(projection).map_err(|error| {
            AppError::new("ASSISTANT_PROJECTION_INVALID", "智奈会话投影无法读取")
                .with_detail(error.to_string())
        })
    }

    pub fn rename_conversation(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> Result<AssistantConversation, AppError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::new("ASSISTANT_TITLE_EMPTY", "会话名称不能为空"));
        }
        let mut snapshot = self.snapshot(conversation_id)?;
        snapshot.conversation.title = title.chars().take(60).collect();
        snapshot.conversation.updated_at = Utc::now().to_rfc3339();
        self.store
            .save_assistant_conversation(&snapshot.conversation)?;
        self.save_snapshot(&snapshot)?;
        Ok(snapshot.conversation)
    }

    pub fn hide_conversation(&self, conversation_id: &str) -> Result<(), AppError> {
        self.store.hide_assistant_conversation(conversation_id)
    }

    pub fn mark_stopped(&self, conversation_id: &str) -> Result<AssistantSnapshot, AppError> {
        let mut snapshot = self.snapshot(conversation_id)?;
        snapshot.status = AssistantStatus::Stopped;
        snapshot.conversation.updated_at = Utc::now().to_rfc3339();
        self.store
            .save_assistant_conversation(&snapshot.conversation)?;
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn submit(
        &self,
        conversation_id: &str,
        input: AssistantInput,
    ) -> Result<AssistantSnapshot, AppError> {
        let mut snapshot = self.snapshot(conversation_id)?;
        let timestamp = Utc::now().to_rfc3339();
        match input {
            AssistantInput::Curl { curl } => {
                let request = parse_curl(&curl).map_err(|error| {
                    AppError::new("ASSISTANT_CURL_INVALID", "无法解析 cURL")
                        .with_detail(format!("位置 {}: {}", error.position, error.message))
                        .with_action("请修正 cURL 后重试")
                })?;
                let projection = serde_json::to_string_pretty(&request).map_err(|error| {
                    AppError::new("SERIALIZE_ERROR", "无法生成 cURL 脱敏投影")
                        .with_detail(error.to_string())
                })?;
                snapshot.messages.push(message(
                    "user",
                    "分析 cURL（原始敏感值仅保留在本次操作内存中）",
                    &timestamp,
                ));
                snapshot.messages.push(message(
                    "assistant",
                    format!("已完成离线解析，尚未发起网络请求。\n{projection}"),
                    &timestamp,
                ));
                snapshot.status = AssistantStatus::Completed;
            }
            AssistantInput::Message { message: content } => {
                snapshot
                    .messages
                    .push(message("user", content.trim(), &timestamp));
                upsert_model_activity(&mut snapshot, "正在准备模型请求", "running", &timestamp);
                snapshot.status = AssistantStatus::WaitingModel;
            }
            AssistantInput::Exchange { exchange_id } => {
                snapshot.messages.push(message(
                    "user",
                    format!("分析抓包 {exchange_id}"),
                    &timestamp,
                ));
                upsert_model_activity(&mut snapshot, "正在准备模型请求", "running", &timestamp);
                snapshot.status = AssistantStatus::WaitingModel;
            }
        }
        snapshot.conversation.updated_at = timestamp;
        if snapshot.conversation.title == "新对话" {
            snapshot.conversation.title = snapshot
                .messages
                .first()
                .map(|message| message.content.chars().take(28).collect())
                .unwrap_or_else(|| "新对话".into());
        }
        self.store
            .save_assistant_conversation(&snapshot.conversation)?;
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn model_input(&self, input: &AssistantInput) -> Result<Option<String>, AppError> {
        match input {
            AssistantInput::Message { message } => Ok(Some(message.trim().to_owned())),
            AssistantInput::Curl { .. } => Ok(None),
            AssistantInput::Exchange { exchange_id } => {
                let exchange = self.store.get_traffic_detail(exchange_id)?;
                let projection = json!({
                    "exchangeId": exchange.summary.id,
                    "startedAt": exchange.summary.started_at,
                    "method": exchange.summary.method,
                    "url": redact_url_string(&exchange.summary.url),
                    "status": exchange.summary.status,
                    "durationMs": exchange.summary.duration_ms,
                    "contentType": exchange.summary.content_type,
                    "mocked": exchange.summary.mocked,
                    "truncated": exchange.summary.truncated,
                    "error": exchange.summary.error,
                    "requestHeaders": redact_headers(&exchange.request_headers),
                    "responseHeaders": redact_headers(&exchange.response_headers),
                    "requestBody": body_projection(
                        exchange.request_body_text.as_deref(),
                        &exchange.request_body,
                        exchange.request_original_bytes,
                    ),
                    "responseBody": body_projection(
                        exchange.response_body_text.as_deref(),
                        &exchange.response_body,
                        exchange.response_original_bytes,
                    ),
                    "stageTimings": exchange.summary.stage_timings,
                });
                serde_json::to_string_pretty(&projection)
                    .map(Some)
                    .map_err(|error| {
                        AppError::new("SERIALIZE_ERROR", "无法生成抓包脱敏投影")
                            .with_detail(error.to_string())
                    })
            }
        }
    }

    pub fn complete(
        &self,
        conversation_id: &str,
        response: &str,
    ) -> Result<AssistantSnapshot, AppError> {
        let mut snapshot = self.snapshot(conversation_id)?;
        let timestamp = Utc::now().to_rfc3339();
        snapshot
            .messages
            .push(message("assistant", response, &timestamp));
        snapshot.status = AssistantStatus::Completed;
        snapshot.conversation.updated_at = timestamp;
        self.store
            .save_assistant_conversation(&snapshot.conversation)?;
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn add_network_draft(
        &self,
        conversation_id: &str,
        payload: Value,
    ) -> Result<AssistantSnapshot, AppError> {
        let mut snapshot = self.snapshot(conversation_id)?;
        snapshot
            .drafts
            .retain(|draft| draft.kind != "networkRequest");
        snapshot.drafts.push(AssistantDraft {
            id: Uuid::new_v4().to_string(),
            kind: "networkRequest".into(),
            payload,
            status: "pending".into(),
        });
        snapshot.status = AssistantStatus::WaitingApproval;
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn complete_network_request(
        &self,
        conversation_id: &str,
        response: &str,
    ) -> Result<AssistantSnapshot, AppError> {
        let mut snapshot = self.snapshot(conversation_id)?;
        snapshot
            .drafts
            .retain(|draft| draft.kind != "networkRequest");
        self.save_snapshot(&snapshot)?;
        self.complete(conversation_id, response)
    }

    pub fn apply_bridge_event(
        &self,
        conversation_id: &str,
        event_type: &str,
        payload: &Value,
    ) -> Result<AssistantSnapshot, AppError> {
        let mut snapshot = self.snapshot(conversation_id)?;
        let timestamp = Utc::now().to_rfc3339();
        match event_type {
            "tool/started" => {
                let id = event_string(payload, "toolCallId")
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let tool_name =
                    event_string(payload, "toolName").unwrap_or_else(|| "netcat_tool".into());
                snapshot.activities.retain(|activity| activity.id != id);
                snapshot.activities.push(AssistantActivity {
                    id,
                    kind: tool_name.clone(),
                    label: tool_label(&tool_name),
                    status: "running".into(),
                    created_at: timestamp,
                });
                snapshot.status = AssistantStatus::CallingTool;
            }
            "tool/completed" => {
                if let Some(id) = event_string(payload, "toolCallId") {
                    if let Some(activity) =
                        snapshot.activities.iter_mut().find(|item| item.id == id)
                    {
                        activity.status = "completed".into();
                    }
                }
                if let Some(result) = payload.get("result") {
                    if let Ok(draft) = serde_json::from_value::<AssistantDraft>(result.clone()) {
                        if matches!(draft.kind.as_str(), "mock" | "script") {
                            snapshot.drafts.retain(|item| item.id != draft.id);
                            snapshot.drafts.push(draft);
                        }
                    }
                }
                snapshot.status = AssistantStatus::WaitingModel;
            }
            "approval/requested" => snapshot.status = AssistantStatus::WaitingApproval,
            "session/completed" => {
                upsert_model_activity(&mut snapshot, "模型处理完成", "completed", &timestamp);
                snapshot.status = AssistantStatus::Completed;
            }
            "session/failed" => {
                upsert_model_activity(&mut snapshot, "模型请求失败", "failed", &timestamp);
                snapshot.status =
                    if payload.get("code").and_then(Value::as_str) == Some("MISSING_CREDENTIAL") {
                        AssistantStatus::CredentialRequired
                    } else {
                        AssistantStatus::Error
                    };
            }
            "session/status" => {
                snapshot.status = match payload.get("status").and_then(Value::as_str) {
                    Some("stopped") => AssistantStatus::Stopped,
                    Some("recovering") => AssistantStatus::Recovering,
                    Some("runningDiagnosis") => AssistantStatus::RunningDiagnosis,
                    _ => AssistantStatus::WaitingModel,
                };
            }
            "session/activity" => {
                if let (Some(id), Some(kind), Some(label), Some(status)) = (
                    event_string(payload, "activityId"),
                    event_string(payload, "kind"),
                    event_string(payload, "label"),
                    event_string(payload, "status"),
                ) {
                    if matches!(
                        status.as_str(),
                        "pending" | "running" | "completed" | "failed"
                    ) {
                        snapshot.activities.retain(|activity| activity.id != id);
                        snapshot.activities.push(AssistantActivity {
                            id,
                            kind,
                            label,
                            status,
                            created_at: timestamp,
                        });
                    }
                }
                snapshot.status = AssistantStatus::WaitingModel;
            }
            _ => return Ok(snapshot),
        }
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn resolve_draft(
        &self,
        conversation_id: &str,
        draft_id: &str,
        status: &str,
    ) -> Result<AssistantSnapshot, AppError> {
        if !matches!(status, "accepted" | "rejected") {
            return Err(AppError::new(
                "ASSISTANT_DRAFT_STATUS_INVALID",
                "智奈草稿状态无效",
            ));
        }
        let mut snapshot = self.snapshot(conversation_id)?;
        let draft = snapshot
            .drafts
            .iter_mut()
            .find(|draft| draft.id == draft_id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "未找到智奈草稿"))?;
        draft.status = status.into();
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn save_snapshot(&self, snapshot: &AssistantSnapshot) -> Result<(), AppError> {
        let projection = serde_json::to_value(snapshot).map_err(|error| {
            AppError::new("SERIALIZE_ERROR", "无法保存智奈会话投影").with_detail(error.to_string())
        })?;
        self.store
            .save_assistant_projection(&snapshot.conversation.id, &projection)
    }
}

fn event_string(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn tool_label(tool_name: &str) -> String {
    match tool_name {
        "netcat_get_exchange" => "正在读取抓包".into(),
        "netcat_find_similar_exchanges" => "正在查找相似请求".into(),
        "netcat_compare_exchanges" => "正在比较请求".into(),
        "netcat_get_diagnostics" => "正在生成诊断".into(),
        "netcat_create_mock_draft" => "正在生成 Mock 草稿".into(),
        "netcat_create_script_draft" => "正在生成 QuickJS 草稿".into(),
        _ => "正在调用 NetCat 工具".into(),
    }
}

fn upsert_model_activity(
    snapshot: &mut AssistantSnapshot,
    label: &str,
    status: &str,
    timestamp: &str,
) {
    snapshot
        .activities
        .retain(|activity| activity.id != "model-progress");
    snapshot.activities.push(AssistantActivity {
        id: "model-progress".into(),
        kind: "model".into(),
        label: label.into(),
        status: status.into(),
        created_at: timestamp.into(),
    });
}

fn body_projection(text: Option<&str>, bytes: &[u8], original_bytes: u64) -> Value {
    if let Some(text) = text {
        if let Ok(json) = serde_json::from_str::<Value>(text) {
            return json!({
                "kind": "json",
                "content": redact_json(&json),
                "originalBytes": original_bytes,
            });
        }
    }
    json!({
        "kind": if text.is_some() { "text" } else { "binary" },
        "originalBytes": original_bytes,
        "sha256": format!("{:x}", Sha256::digest(bytes)),
    })
}

fn message(
    role: impl Into<String>,
    content: impl Into<String>,
    created_at: &str,
) -> AssistantMessage {
    AssistantMessage {
        id: Uuid::new_v4().to_string(),
        role: role.into(),
        content: content.into(),
        created_at: created_at.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use crate::{
        assistant::{
            models::{AssistantInput, AssistantStatus},
            service::AssistantService,
        },
        models::{TrafficDraft, TrafficOrigin},
        storage::Store,
    };

    fn service() -> AssistantService {
        AssistantService::new(Arc::new(Store::open_in_memory(10, 1_024).unwrap()))
    }

    #[test]
    fn creates_and_restores_an_empty_session_snapshot() {
        let service = service();

        let conversation = service.create_conversation().unwrap();
        let snapshot = service.snapshot(&conversation.id).unwrap();

        assert_eq!(snapshot.conversation, conversation);
        assert!(snapshot.messages.is_empty());
        assert_eq!(snapshot.status, AssistantStatus::Ready);
        assert_eq!(service.list_conversations().unwrap(), vec![conversation]);
    }

    #[test]
    fn curl_submission_persists_only_the_redacted_projection() {
        let service = service();
        let conversation = service.create_conversation().unwrap();

        let snapshot = service
            .submit(
                &conversation.id,
                AssistantInput::Curl {
                    curl: concat!(
                        "curl https://api.example.com/login -H 'Authorization: Bearer raw-secret' ",
                        "-H 'Content-Type: application/json' --data '{\"password\":\"hunter2\"}'"
                    )
                    .into(),
                },
            )
            .unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();

        assert!(!serialized.contains("raw-secret"));
        assert!(!serialized.contains("hunter2"));
        assert!(serialized.contains("[REDACTED]"));
        assert!(serialized.contains("api.example.com"));
        assert_eq!(snapshot.status, AssistantStatus::Completed);
    }

    #[test]
    fn exchange_model_input_uses_real_capture_data_without_sensitive_values() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        store
            .insert_exchange(TrafficDraft {
                id: "exchange-1".into(),
                started_at: "2026-11-24T10:00:00Z".into(),
                method: "POST".into(),
                url: "https://api.example.com/login?access_token=query-secret&page=2".into(),
                host: "api.example.com".into(),
                status: Some(401),
                duration_ms: Some(120),
                request_headers: BTreeMap::from([
                    ("Authorization".into(), "Bearer raw-secret".into()),
                    ("Content-Type".into(), "application/json".into()),
                ]),
                response_headers: BTreeMap::new(),
                request_body: br#"{"profile":{"accessToken":"nested-secret"}}"#.to_vec(),
                response_body: br#"{"error":"unauthorized"}"#.to_vec(),
                content_type: Some("application/json".into()),
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
        let service = AssistantService::new(store);

        let input = service
            .model_input(&AssistantInput::Exchange {
                exchange_id: "exchange-1".into(),
            })
            .unwrap()
            .unwrap();

        assert!(input.contains("api.example.com/login"));
        assert!(input.contains("unauthorized"));
        assert!(input.contains("[REDACTED]"));
        assert!(!input.contains("raw-secret"));
        assert!(!input.contains("nested-secret"));
        assert!(!input.contains("query-secret"));
        assert!(input.contains("page=2"));
    }

    #[test]
    fn completed_model_reply_is_persisted_in_the_session_projection() {
        let service = service();
        let conversation = service.create_conversation().unwrap();
        service
            .submit(
                &conversation.id,
                AssistantInput::Message {
                    message: "检查接口".into(),
                },
            )
            .unwrap();

        let snapshot = service
            .complete(&conversation.id, "发现服务端返回 401")
            .unwrap();

        assert_eq!(snapshot.status, AssistantStatus::Completed);
        assert_eq!(snapshot.messages.last().unwrap().role, "assistant");
        assert_eq!(
            service
                .snapshot(&conversation.id)
                .unwrap()
                .messages
                .last()
                .unwrap()
                .content,
            "发现服务端返回 401"
        );
    }

    #[test]
    fn bridge_tool_events_update_activities_and_pending_drafts() {
        let service = service();
        let conversation = service.create_conversation().unwrap();

        service
            .apply_bridge_event(
                &conversation.id,
                "tool/started",
                &serde_json::json!({
                    "toolCallId": "call-1",
                    "toolName": "netcat_create_mock_draft"
                }),
            )
            .unwrap();
        service
            .apply_bridge_event(
                &conversation.id,
                "tool/completed",
                &serde_json::json!({
                    "toolCallId": "call-1",
                    "toolName": "netcat_create_mock_draft",
                    "result": {
                        "id": "draft-1",
                        "kind": "mock",
                        "status": "pending",
                        "payload": { "name": "登录失败" }
                    }
                }),
            )
            .unwrap();

        let snapshot = service.snapshot(&conversation.id).unwrap();
        assert_eq!(snapshot.activities.len(), 1);
        assert_eq!(snapshot.activities[0].status, "completed");
        assert_eq!(snapshot.drafts.len(), 1);
        assert_eq!(snapshot.drafts[0].kind, "mock");
    }

    #[test]
    fn model_progress_and_failures_are_persisted_as_safe_user_visible_activity() {
        let service = service();
        let conversation = service.create_conversation().unwrap();

        let submitted = service
            .submit(
                &conversation.id,
                AssistantInput::Message {
                    message: "你好".into(),
                },
            )
            .unwrap();
        assert_eq!(submitted.activities[0].label, "正在准备模型请求");
        assert_eq!(submitted.activities[0].status, "running");

        service
            .apply_bridge_event(
                &conversation.id,
                "session/activity",
                &serde_json::json!({
                    "activityId": "model-progress",
                    "kind": "model",
                    "label": "模型正在生成回复",
                    "status": "running"
                }),
            )
            .unwrap();
        let failed = service
            .apply_bridge_event(
                &conversation.id,
                "session/failed",
                &serde_json::json!({
                    "code": "MISSING_CREDENTIAL",
                    "message": "OpenAI API Key is not configured"
                }),
            )
            .unwrap();

        assert_eq!(failed.status, AssistantStatus::CredentialRequired);
        assert_eq!(failed.activities.len(), 1);
        assert_eq!(failed.activities[0].label, "模型请求失败");
        assert_eq!(failed.activities[0].status, "failed");
        assert!(failed
            .messages
            .iter()
            .all(|message| !message.content.is_empty()));
    }

    #[test]
    fn completed_network_requests_remove_the_single_use_draft() {
        let service = service();
        let conversation = service.create_conversation().unwrap();
        service
            .add_network_draft(
                &conversation.id,
                serde_json::json!({ "domain": "example.com" }),
            )
            .unwrap();

        let snapshot = service
            .complete_network_request(&conversation.id, "诊断完成")
            .unwrap();

        assert!(snapshot
            .drafts
            .iter()
            .all(|draft| draft.kind != "networkRequest"));
        assert_eq!(snapshot.messages.last().unwrap().content, "诊断完成");
    }

    #[test]
    fn stopped_sessions_persist_their_terminal_status() {
        let service = service();
        let conversation = service.create_conversation().unwrap();
        service
            .submit(
                &conversation.id,
                AssistantInput::Message {
                    message: "分析失败".into(),
                },
            )
            .unwrap();

        service.mark_stopped(&conversation.id).unwrap();

        assert_eq!(
            service.snapshot(&conversation.id).unwrap().status,
            AssistantStatus::Stopped
        );
    }
}
