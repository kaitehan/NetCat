use std::path::Path;

use serde::Serialize;

use crate::error::AppError;

use super::models::AssistantProvider;

pub trait CredentialBackend: Send + Sync {
    fn save(&self, id: &str, value: &str) -> Result<(), AppError>;
    fn load(&self, id: &str) -> Result<Option<String>, AppError>;
    fn exists(&self, id: &str) -> Result<bool, AppError>;
    fn delete(&self, id: &str) -> Result<(), AppError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialDescription {
    pub provider: AssistantProvider,
    pub configured: bool,
    pub masked_value: Option<&'static str>,
}

pub struct CredentialService<B> {
    backend: B,
}

impl<B: CredentialBackend> CredentialService<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn describe(&self, provider: AssistantProvider) -> Result<CredentialDescription, AppError> {
        self.describe_scoped(provider, None)
    }

    pub fn describe_scoped(
        &self,
        provider: AssistantProvider,
        openai_provider: Option<&str>,
    ) -> Result<CredentialDescription, AppError> {
        let configured = self
            .backend
            .exists(&credential_id(provider, openai_provider))?;
        Ok(CredentialDescription {
            provider,
            configured,
            masked_value: configured.then_some("********"),
        })
    }

    pub fn save(&self, provider: AssistantProvider, value: &str) -> Result<(), AppError> {
        self.save_scoped(provider, None, value)
    }

    pub fn save_scoped(
        &self,
        provider: AssistantProvider,
        openai_provider: Option<&str>,
        value: &str,
    ) -> Result<(), AppError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(AppError::new(
                "ASSISTANT_CREDENTIAL_EMPTY",
                "API Key 不能为空",
            ));
        }
        self.backend
            .save(&credential_id(provider, openai_provider), value)
    }

    pub fn resolve_for_runtime(
        &self,
        provider: AssistantProvider,
    ) -> Result<Option<String>, AppError> {
        self.resolve_scoped(provider, None)
    }

    pub fn resolve_scoped(
        &self,
        provider: AssistantProvider,
        openai_provider: Option<&str>,
    ) -> Result<Option<String>, AppError> {
        self.backend.load(&credential_id(provider, openai_provider))
    }

    pub fn delete(&self, provider: AssistantProvider) -> Result<(), AppError> {
        self.delete_scoped(provider, None)
    }

    pub fn delete_scoped(
        &self,
        provider: AssistantProvider,
        openai_provider: Option<&str>,
    ) -> Result<(), AppError> {
        self.backend
            .delete(&credential_id(provider, openai_provider))
    }
}

fn credential_id(provider: AssistantProvider, openai_provider: Option<&str>) -> String {
    match (provider, openai_provider) {
        (AssistantProvider::OpenAi, Some(route)) if route != "openai" => {
            format!("netcat.assistant.openai.{route}")
        }
        _ => provider.credential_id().into(),
    }
}

pub struct NativeCredentialBackend {
    #[cfg(target_os = "windows")]
    data_directory: std::path::PathBuf,
}

impl NativeCredentialBackend {
    pub fn new(data_directory: &Path) -> Self {
        let _ = data_directory;
        Self {
            #[cfg(target_os = "windows")]
            data_directory: data_directory.to_path_buf(),
        }
    }

    #[cfg(target_os = "macos")]
    fn entry(id: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(id, "api-key").map_err(credential_error)
    }

    #[cfg(target_os = "windows")]
    fn store(&self, id: &str) -> crate::platform::windows::DpapiSecretStore {
        use crate::platform::windows::DpapiSecretStore;
        DpapiSecretStore::new(self.data_directory.join(format!("{id}.dpapi")))
    }
}

#[cfg(target_os = "macos")]
impl CredentialBackend for NativeCredentialBackend {
    fn save(&self, id: &str, value: &str) -> Result<(), AppError> {
        Self::entry(id)?
            .set_password(value)
            .map_err(credential_error)
    }

    fn exists(&self, id: &str) -> Result<bool, AppError> {
        Ok(self.load(id)?.is_some())
    }

    fn load(&self, id: &str) -> Result<Option<String>, AppError> {
        match Self::entry(id)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(credential_error(error)),
        }
    }

    fn delete(&self, id: &str) -> Result<(), AppError> {
        match Self::entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(credential_error(error)),
        }
    }
}

#[cfg(target_os = "macos")]
fn credential_error(error: keyring::Error) -> AppError {
    AppError::new("ASSISTANT_CREDENTIAL_ERROR", "无法访问智奈 API Key")
        .with_detail(error.to_string())
        .with_action("请检查钥匙串访问权限后重试")
}

#[cfg(target_os = "windows")]
impl CredentialBackend for NativeCredentialBackend {
    fn save(&self, id: &str, value: &str) -> Result<(), AppError> {
        use crate::platform::SecretStore;
        self.store(id).save_private_key(value)
    }

    fn exists(&self, id: &str) -> Result<bool, AppError> {
        Ok(self.load(id)?.is_some())
    }

    fn load(&self, id: &str) -> Result<Option<String>, AppError> {
        use crate::platform::SecretStore;
        self.store(id).load_private_key()
    }

    fn delete(&self, id: &str) -> Result<(), AppError> {
        match std::fs::remove_file(self.store(id).path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(
                AppError::new("ASSISTANT_CREDENTIAL_ERROR", "无法删除智奈 API Key")
                    .with_detail(error.to_string()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use crate::assistant::{
        credentials::{CredentialBackend, CredentialService},
        models::AssistantProvider,
    };

    #[derive(Default)]
    struct MemoryBackend(Mutex<HashMap<String, String>>);

    impl CredentialBackend for MemoryBackend {
        fn save(&self, id: &str, value: &str) -> Result<(), crate::error::AppError> {
            self.0.lock().unwrap().insert(id.into(), value.into());
            Ok(())
        }

        fn exists(&self, id: &str) -> Result<bool, crate::error::AppError> {
            Ok(self.0.lock().unwrap().contains_key(id))
        }

        fn load(&self, id: &str) -> Result<Option<String>, crate::error::AppError> {
            Ok(self.0.lock().unwrap().get(id).cloned())
        }

        fn delete(&self, id: &str) -> Result<(), crate::error::AppError> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }

    #[test]
    fn provider_credentials_use_separate_ids_and_never_return_the_key() {
        let service = CredentialService::new(MemoryBackend::default());

        service
            .save(AssistantProvider::DeepSeek, "sk-deepseek")
            .unwrap();
        service
            .save(AssistantProvider::OpenAi, "sk-openai")
            .unwrap();

        assert!(
            service
                .describe(AssistantProvider::DeepSeek)
                .unwrap()
                .configured
        );
        assert_eq!(
            service
                .resolve_for_runtime(AssistantProvider::OpenAi)
                .unwrap(),
            Some("sk-openai".into())
        );
        assert!(
            service
                .describe(AssistantProvider::OpenAi)
                .unwrap()
                .configured
        );
        assert_eq!(
            AssistantProvider::DeepSeek.credential_id(),
            "netcat.assistant.deepseek"
        );
        assert_eq!(
            AssistantProvider::OpenAi.credential_id(),
            "netcat.assistant.openai"
        );

        service.delete(AssistantProvider::DeepSeek).unwrap();
        assert!(
            !service
                .describe(AssistantProvider::DeepSeek)
                .unwrap()
                .configured
        );
        assert!(
            service
                .describe(AssistantProvider::OpenAi)
                .unwrap()
                .configured
        );
    }

    #[test]
    fn openai_compatible_providers_use_isolated_credential_ids() {
        let service = CredentialService::new(MemoryBackend::default());

        service
            .save_scoped(AssistantProvider::OpenAi, Some("openai"), "sk-official")
            .unwrap();
        service
            .save_scoped(AssistantProvider::OpenAi, Some("acme-gateway"), "sk-acme")
            .unwrap();

        assert_eq!(
            service
                .resolve_scoped(AssistantProvider::OpenAi, Some("openai"))
                .unwrap(),
            Some("sk-official".into())
        );
        assert_eq!(
            service
                .resolve_scoped(AssistantProvider::OpenAi, Some("acme-gateway"))
                .unwrap(),
            Some("sk-acme".into())
        );
        assert!(
            !service
                .describe_scoped(AssistantProvider::OpenAi, Some("other-gateway"))
                .unwrap()
                .configured
        );
    }
}
