use std::{path::Path, sync::Arc};

use serde_json::Value;

use crate::{error::AppError, storage::Store, system_proxy::SystemProxyManager};

#[cfg(target_os = "macos")]
pub mod macos;

pub trait SecretStore: Send + Sync {
    fn save_private_key(&self, value: &str) -> Result<(), AppError>;
    fn load_private_key(&self) -> Result<Option<String>, AppError>;
}

impl<S: SecretStore + ?Sized> SecretStore for Arc<S> {
    fn save_private_key(&self, value: &str) -> Result<(), AppError> {
        (**self).save_private_key(value)
    }

    fn load_private_key(&self) -> Result<Option<String>, AppError> {
        (**self).load_private_key()
    }
}

pub trait SystemProxyBackend: Send + Sync {
    fn platform(&self) -> &'static str;
    fn capture(&self) -> Result<Value, AppError>;
    fn apply(&self, port: u16) -> Result<(), AppError>;
    fn is_managed(&self, port: u16) -> Result<bool, AppError>;
    fn restore(&self, snapshot: &Value) -> Result<(), AppError>;
}

pub struct PlatformServices {
    pub secrets: Arc<dyn SecretStore>,
    pub system_proxy: SystemProxyManager,
}

impl PlatformServices {
    pub fn current(store: Arc<Store>, data_dir: &Path) -> Result<Self, AppError> {
        let _ = data_dir;
        #[cfg(target_os = "macos")]
        {
            let backend = Arc::new(macos::MacOsSystemProxyBackend::new(store.clone()));
            Ok(Self {
                secrets: Arc::new(macos::KeychainSecretStore),
                system_proxy: SystemProxyManager::new(store, backend),
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = store;
            Err(AppError::new(
                "UNSUPPORTED_PLATFORM",
                "当前平台暂不支持系统集成",
            ))
        }
    }
}
