use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{error::AppError, storage::Store};

use super::models::AssistantProvider;

const PROVIDER_SETTING: &str = "assistant.provider";
const MODEL_SETTING: &str = "assistant.model";
const OPENAI_PROVIDER_SETTING: &str = "assistant.openai.provider";
const OPENAI_BASE_URL_SETTING: &str = "assistant.openai.base_url";
const OPENAI_API_SETTING: &str = "assistant.openai.api";
const DEFAULT_OPENAI_PROVIDER: &str = "openai";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_OPENAI_API: &str = "openai-responses";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSettings {
    pub provider: AssistantProvider,
    pub model: String,
    pub open_ai_provider: String,
    pub open_ai_base_url: String,
    pub open_ai_api: String,
}

pub struct AssistantSettingsService {
    store: Arc<Store>,
}

impl AssistantSettingsService {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    pub fn get(&self) -> Result<AssistantSettings, AppError> {
        let provider = self
            .store
            .get_setting(PROVIDER_SETTING)?
            .as_deref()
            .and_then(AssistantProvider::parse)
            .unwrap_or(AssistantProvider::DeepSeek);
        let open_ai_provider = self
            .store
            .get_setting(OPENAI_PROVIDER_SETTING)?
            .filter(|value| validate_openai_route(value).is_ok())
            .unwrap_or_else(|| DEFAULT_OPENAI_PROVIDER.into());
        let model = self
            .store
            .get_setting(MODEL_SETTING)?
            .filter(|model| match provider {
                AssistantProvider::DeepSeek => provider.accepts_model(model),
                AssistantProvider::OpenAi => model
                    .strip_prefix(&format!("{open_ai_provider}/"))
                    .is_some_and(|model_id| validate_openai_model(model_id).is_ok()),
            })
            .unwrap_or_else(|| provider.default_model().into());
        let open_ai_base_url = self
            .store
            .get_setting(OPENAI_BASE_URL_SETTING)?
            .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.into());
        let open_ai_api = self
            .store
            .get_setting(OPENAI_API_SETTING)?
            .filter(|value| matches!(value.as_str(), "openai-responses" | "openai-completions"))
            .unwrap_or_else(|| DEFAULT_OPENAI_API.into());
        Ok(AssistantSettings {
            provider,
            model,
            open_ai_provider,
            open_ai_base_url,
            open_ai_api,
        })
    }

    pub fn save(
        &self,
        provider: AssistantProvider,
        model: &str,
    ) -> Result<AssistantSettings, AppError> {
        let settings = self.prepare(provider, model)?;
        self.persist(&settings)?;
        Ok(settings)
    }

    pub fn prepare(
        &self,
        provider: AssistantProvider,
        model: &str,
    ) -> Result<AssistantSettings, AppError> {
        if !provider.accepts_model(model) {
            return Err(AppError::new(
                "ASSISTANT_MODEL_PROVIDER_MISMATCH",
                "所选模型不属于当前服务商",
            ));
        }
        let mut settings = self.get()?;
        settings.provider = provider;
        settings.model = model.into();
        Ok(settings)
    }

    pub fn save_openai_compatible(
        &self,
        route: &str,
        model: &str,
        base_url: &str,
        api: &str,
    ) -> Result<AssistantSettings, AppError> {
        let settings = self.prepare_openai_compatible(route, model, base_url, api)?;
        self.persist(&settings)?;
        Ok(settings)
    }

    pub fn prepare_openai_compatible(
        &self,
        route: &str,
        model: &str,
        base_url: &str,
        api: &str,
    ) -> Result<AssistantSettings, AppError> {
        validate_openai_route(route)?;
        validate_openai_model(model)?;
        validate_openai_base_url(base_url)?;
        if !matches!(api, "openai-responses" | "openai-completions") {
            return Err(AppError::new(
                "ASSISTANT_OPENAI_API_INVALID",
                "OpenAI 兼容协议无效",
            ));
        }
        Ok(AssistantSettings {
            provider: AssistantProvider::OpenAi,
            model: format!("{route}/{model}"),
            open_ai_provider: route.into(),
            open_ai_base_url: base_url.into(),
            open_ai_api: api.into(),
        })
    }

    pub fn persist(&self, settings: &AssistantSettings) -> Result<(), AppError> {
        self.store.set_settings(&[
            (PROVIDER_SETTING, settings.provider.id()),
            (MODEL_SETTING, &settings.model),
            (OPENAI_PROVIDER_SETTING, &settings.open_ai_provider),
            (OPENAI_BASE_URL_SETTING, &settings.open_ai_base_url),
            (OPENAI_API_SETTING, &settings.open_ai_api),
        ])
    }
}

impl AssistantSettings {
    pub fn runtime_patch(&self) -> Option<String> {
        if self.provider != AssistantProvider::OpenAi {
            return None;
        }
        let (_, model) = self.model.split_once('/')?;
        Some(format!(
            "- insert:\n    - id: llm-pi-ai\n      name: '@deepseek-ai/dsh-llm-pi-ai'\n      config:\n        providers:\n          {}:\n            displayName: {}\n            apiKeyEnv: OPENAI_API_KEY\n            api: {}\n            baseURL: {}\n            models:\n              - id: {}\n",
            yaml_string(&self.open_ai_provider),
            yaml_string(&self.open_ai_provider),
            yaml_string(&self.open_ai_api),
            yaml_string(&self.open_ai_base_url),
            yaml_string(model),
        ))
    }
}

fn validate_openai_route(route: &str) -> Result<(), AppError> {
    if route.is_empty()
        || route.len() > 64
        || !route
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || route.starts_with('-')
        || route.ends_with('-')
    {
        return Err(AppError::new(
            "ASSISTANT_OPENAI_PROVIDER_INVALID",
            "供应商标识只能使用小写字母、数字和连字符",
        ));
    }
    Ok(())
}

fn validate_openai_model(model: &str) -> Result<(), AppError> {
    if model.trim() != model
        || model.is_empty()
        || model.len() > 200
        || model.chars().any(char::is_whitespace)
    {
        return Err(AppError::new(
            "ASSISTANT_OPENAI_MODEL_INVALID",
            "模型标识无效",
        ));
    }
    Ok(())
}

fn validate_openai_base_url(value: &str) -> Result<(), AppError> {
    let url = url::Url::parse(value)
        .map_err(|_| AppError::new("ASSISTANT_OPENAI_BASE_URL_INVALID", "API Base URL 无效"))?;
    let local_http = url.scheme() == "http"
        && url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
    if (url.scheme() != "https" && !local_http)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::new(
            "ASSISTANT_OPENAI_BASE_URL_INVALID",
            "API Base URL 必须使用 HTTPS；仅本机地址可使用 HTTP",
        ));
    }
    Ok(())
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        assistant::{models::AssistantProvider, settings::AssistantSettingsService},
        storage::Store,
    };

    #[test]
    fn returns_fixed_deepseek_defaults_and_persists_an_openai_route() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        let service = AssistantSettingsService::new(store.clone());

        let defaults = service.get().unwrap();
        assert_eq!(defaults.provider, AssistantProvider::DeepSeek);
        assert_eq!(defaults.model, "deepseek-official/deepseek-v4-flash");

        let updated = service
            .save(AssistantProvider::OpenAi, "openai/gpt-4.1")
            .unwrap();
        assert_eq!(updated.provider, AssistantProvider::OpenAi);
        assert_eq!(AssistantSettingsService::new(store).get().unwrap(), updated);
    }

    #[test]
    fn rejects_a_model_route_from_the_wrong_provider() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        let service = AssistantSettingsService::new(store);

        assert_eq!(
            service
                .save(AssistantProvider::DeepSeek, "openai/gpt-4.1")
                .unwrap_err()
                .code,
            "ASSISTANT_MODEL_PROVIDER_MISMATCH"
        );
    }

    #[test]
    fn persists_a_valid_openai_compatible_provider_profile() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        let service = AssistantSettingsService::new(store.clone());

        let updated = service
            .save_openai_compatible(
                "acme-gateway",
                "acme-chat",
                "https://gateway.example.com/v1",
                "openai-completions",
            )
            .unwrap();

        assert_eq!(updated.provider, AssistantProvider::OpenAi);
        assert_eq!(updated.model, "acme-gateway/acme-chat");
        assert_eq!(updated.open_ai_provider, "acme-gateway");
        assert_eq!(updated.open_ai_base_url, "https://gateway.example.com/v1");
        assert_eq!(updated.open_ai_api, "openai-completions");
        assert_eq!(AssistantSettingsService::new(store).get().unwrap(), updated);
    }

    #[test]
    fn openai_runtime_patch_inserts_the_adapter_plugin_into_the_minimal_profile() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        let service = AssistantSettingsService::new(store);
        let settings = service
            .prepare_openai_compatible(
                "acme-gateway",
                "acme-chat",
                "https://gateway.example.com/v1",
                "openai-completions",
            )
            .unwrap();

        assert_eq!(
            settings.runtime_patch().as_deref(),
            Some(concat!(
                "- insert:\n",
                "    - id: llm-pi-ai\n",
                "      name: '@deepseek-ai/dsh-llm-pi-ai'\n",
                "      config:\n",
                "        providers:\n",
                "          \"acme-gateway\":\n",
                "            displayName: \"acme-gateway\"\n",
                "            apiKeyEnv: OPENAI_API_KEY\n",
                "            api: \"openai-completions\"\n",
                "            baseURL: \"https://gateway.example.com/v1\"\n",
                "            models:\n",
                "              - id: \"acme-chat\"\n",
            ))
        );
    }

    #[test]
    fn rejects_an_insecure_remote_openai_compatible_endpoint() {
        let store = Arc::new(Store::open_in_memory(10, 1_024).unwrap());
        let service = AssistantSettingsService::new(store);

        assert_eq!(
            service
                .save_openai_compatible(
                    "acme",
                    "chat",
                    "http://gateway.example.com/v1",
                    "openai-responses",
                )
                .unwrap_err()
                .code,
            "ASSISTANT_OPENAI_BASE_URL_INVALID"
        );
    }
}
