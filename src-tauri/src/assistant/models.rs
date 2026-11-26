use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AssistantInput {
    Message {
        message: String,
    },
    Curl {
        curl: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_exchange_id: Option<String>,
    },
    Exchange {
        exchange_id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssistantProvider {
    DeepSeek,
    OpenAi,
}

impl AssistantProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::OpenAi => "openai",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "deepseek" => Some(Self::DeepSeek),
            "openai" => Some(Self::OpenAi),
            _ => None,
        }
    }

    pub fn accepts_model(self, model: &str) -> bool {
        match self {
            Self::DeepSeek => model.starts_with("deepseek-official/"),
            Self::OpenAi => model.starts_with("openai/"),
        }
    }

    pub fn credential_id(self) -> &'static str {
        match self {
            Self::DeepSeek => "netcat.assistant.deepseek",
            Self::OpenAi => "netcat.assistant.openai",
        }
    }
}

impl AssistantProvider {
    pub fn default_model(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek-official/deepseek-v4-flash",
            Self::OpenAi => "openai/gpt-4.1",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConversation {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssistantStatus {
    Starting,
    Ready,
    WaitingModel,
    CallingTool,
    WaitingApproval,
    RunningDiagnosis,
    Completed,
    Stopped,
    Recovering,
    CredentialRequired,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    #[serde(default = "default_message_type")]
    pub message_type: String,
    #[serde(default)]
    pub model_visible: bool,
}

fn default_message_type() -> String {
    "normal".into()
}

#[cfg(test)]
mod message_tests {
    use super::{AssistantDraft, AssistantMessage};

    #[test]
    fn old_messages_default_to_normal_type() {
        let message: AssistantMessage = serde_json::from_value(serde_json::json!({
            "id": "message-1",
            "role": "assistant",
            "content": "你好",
            "createdAt": "2026-11-25T10:00:00Z"
        }))
        .unwrap();

        assert_eq!(message.message_type, "normal");
        assert!(!message.model_visible);
    }

    #[test]
    fn old_drafts_default_to_unbound_timeline_metadata() {
        let draft: AssistantDraft = serde_json::from_value(serde_json::json!({
            "id": "draft-1",
            "kind": "trafficSearch",
            "payload": {},
            "status": "pending"
        }))
        .unwrap();

        assert_eq!(draft.message_id, None);
        assert_eq!(draft.created_at, None);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantActivity {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantDraft {
    pub id: String,
    pub kind: String,
    pub payload: serde_json::Value,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantRequestContext {
    pub method: String,
    pub url: String,
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_exchange_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSnapshot {
    pub conversation: AssistantConversation,
    pub messages: Vec<AssistantMessage>,
    pub activities: Vec<AssistantActivity>,
    pub drafts: Vec<AssistantDraft>,
    pub status: AssistantStatus,
}

impl AssistantConversation {
    pub fn new(
        id: impl Into<String>,
        session_id: impl Into<String>,
        title: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let timestamp = timestamp.into();
        Self {
            id: id.into(),
            session_id: session_id.into(),
            title: title.into(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            hidden: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AssistantInput, AssistantProvider};

    #[test]
    fn assistant_input_serializes_with_stable_kind_tag() {
        let input = AssistantInput::Exchange {
            exchange_id: "exchange-1".into(),
        };

        assert_eq!(
            serde_json::to_value(input).unwrap(),
            serde_json::json!({ "kind": "exchange", "exchangeId": "exchange-1" })
        );
    }

    #[test]
    fn providers_expose_fixed_default_routes() {
        assert_eq!(
            AssistantProvider::DeepSeek.default_model(),
            "deepseek-official/deepseek-v4-flash"
        );
        assert_eq!(AssistantProvider::OpenAi.default_model(), "openai/gpt-4.1");
    }
}
