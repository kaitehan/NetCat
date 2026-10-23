use std::{path::Path, sync::Arc};

use serde_json::Value;

use crate::{
    error::AppError, models::PlatformInfo, storage::Store, system_proxy::SystemProxyManager,
};

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(any(target_os = "windows", test))]
pub mod windows;

pub trait SecretStore: Send + Sync {
    fn save_private_key(&self, value: &str) -> Result<(), AppError>;
    fn load_private_key(&self) -> Result<Option<String>, AppError>;
}

pub struct PortProcess {
    pub pid: u32,
    pub name: Option<String>,
}

impl std::fmt::Display for PortProcess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.name.as_deref().filter(|name| !name.is_empty()) {
            Some(name) => write!(formatter, "{name} (PID {})", self.pid),
            None => write!(formatter, "PID {}", self.pid),
        }
    }
}

pub trait PortInspector: Send + Sync {
    fn listener(&self, port: u16) -> Option<PortProcess>;
}

pub struct NativePortInspector;

impl PortInspector for NativePortInspector {
    fn listener(&self, port: u16) -> Option<PortProcess> {
        #[cfg(target_os = "macos")]
        {
            macos::port_listener(port)
        }
        #[cfg(target_os = "windows")]
        {
            windows::diagnostics::port_listener(port)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = port;
            None
        }
    }
}

#[cfg(target_os = "macos")]
struct UnsupportedCertificateTrust;

#[cfg(target_os = "macos")]
impl CertificateTrust for UnsupportedCertificateTrust {
    fn status(&self, _: &[u8], _: &str) -> Result<CertificateTrustState, AppError> {
        Ok(CertificateTrustState::Unsupported)
    }
    fn install(&self, _: &[u8], _: &str) -> Result<CertificateTrustState, AppError> {
        Ok(CertificateTrustState::Unsupported)
    }
    fn remove(&self, _: &[u8], _: &str) -> Result<CertificateTrustState, AppError> {
        Ok(CertificateTrustState::Unsupported)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaPrivateKeyState {
    Ready,
    Missing,
    Mismatch,
    Unavailable,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTrustDetails {
    pub public_trust: CertificateTrustState,
    pub private_key_state: CaPrivateKeyState,
}

impl CertificateTrustDetails {
    pub fn summary(&self) -> CertificateTrustState {
        if matches!(
            self.public_trust,
            CertificateTrustState::Unsupported | CertificateTrustState::NotGenerated
        ) {
            return self.public_trust;
        }
        match self.private_key_state {
            CaPrivateKeyState::Ready => self.public_trust,
            CaPrivateKeyState::Missing | CaPrivateKeyState::Mismatch => {
                CertificateTrustState::Mismatch
            }
            CaPrivateKeyState::Unavailable => CertificateTrustState::Unavailable,
        }
    }
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
    fn is_original(&self, snapshot: &Value) -> Result<bool, AppError> {
        Ok(self.capture()? == *snapshot)
    }
    /// Retry any pending OS notification once the configuration already matches.
    fn finish_restore(&self) -> Result<(), AppError> {
        Ok(())
    }
    /// Accept per-field original/managed mixtures only during persisted apply/recovery.
    fn can_restore(
        &self,
        _snapshot: &Value,
        port: u16,
        _restoring: bool,
    ) -> Result<bool, AppError> {
        self.is_managed(port)
    }
    fn restore(&self, snapshot: &Value) -> Result<(), AppError>;
}

pub struct PlatformServices {
    pub secrets: Arc<dyn SecretStore>,
    pub system_proxy: SystemProxyManager,
    pub info: PlatformInfo,
    pub certificate_trust: Arc<dyn CertificateTrust>,
    pub ports: Arc<dyn PortInspector>,
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
                info: macos::platform_info(),
                certificate_trust: Arc::new(UnsupportedCertificateTrust),
                ports: Arc::new(NativePortInspector),
            })
        }
        #[cfg(target_os = "windows")]
        {
            std::fs::create_dir_all(data_dir)
                .map_err(|_| AppError::new("FILE_ERROR", "无法创建应用数据目录"))?;
            let backend = Arc::new(windows::WindowsSystemProxyBackend::new(Arc::new(
                windows::CurrentUserInternetSettings,
            )));
            Ok(Self {
                secrets: Arc::new(windows::DpapiSecretStore::new(
                    data_dir.join("ca-private-key.dpapi"),
                )),
                system_proxy: SystemProxyManager::new(store, backend),
                info: windows::diagnostics::platform_info(),
                certificate_trust: Arc::new(windows::WindowsCertificateTrust::new(
                    windows::CurrentUserRootStore,
                )),
                ports: Arc::new(NativePortInspector),
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = store;
            Err(AppError::new(
                "UNSUPPORTED_PLATFORM",
                "当前平台暂不支持系统集成",
            ))
        }
    }
}
