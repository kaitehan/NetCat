use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AssistantInput {
    Message { message: String },
    Curl { curl: String },
    Exchange { exchange_id: String },
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
