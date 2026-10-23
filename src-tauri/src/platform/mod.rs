use std::{path::Path, sync::Arc};

use serde_json::Value;

use crate::{error::AppError, storage::Store, system_proxy::SystemProxyManager};

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(any(target_os = "windows", test))]
pub mod windows;

pub trait SecretStore: Send + Sync {
    fn save_private_key(&self, value: &str) -> Result<(), AppError>;
    fn load_private_key(&self) -> Result<Option<String>, AppError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CertificateTrustState {
    Unsupported,
    NotGenerated,
    NotTrusted,
    Trusted,
    Mismatch,
    Unavailable,
}

/// Mutations are always explicit; constructing services or checking status never installs a CA.
pub trait CertificateTrust: Send + Sync {
    fn status(
        &self,
        certificate_pem: &[u8],
        fingerprint: &str,
    ) -> Result<CertificateTrustState, AppError>;
    fn install(
        &self,
        certificate_pem: &[u8],
        fingerprint: &str,
    ) -> Result<CertificateTrustState, AppError>;
    fn remove(
        &self,
        certificate_pem: &[u8],
        fingerprint: &str,
    ) -> Result<CertificateTrustState, AppError>;
}

pub(crate) fn certificate_der(certificate_pem: &[u8]) -> Result<Vec<u8>, AppError> {
    let invalid = || {
        AppError::new(
            "CA_CERTIFICATE_INVALID",
            "需要且仅接受一个 X.509 PEM 证书块",
        )
    };
    let text = std::str::from_utf8(certificate_pem)
        .map_err(|_| invalid())?
        .trim();
    let body = text
        .strip_prefix("-----BEGIN CERTIFICATE-----")
        .and_then(|text| text.strip_suffix("-----END CERTIFICATE-----"))
        .ok_or_else(invalid)?;
    if body.contains("-----") {
        return Err(invalid());
    }
    let items = rustls_pemfile::read_all(&mut std::io::Cursor::new(certificate_pem))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    match items.as_slice() {
        [rustls_pemfile::Item::X509Certificate(der)] if !der.is_empty() => Ok(der.to_vec()),
        _ => Err(invalid()),
    }
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
