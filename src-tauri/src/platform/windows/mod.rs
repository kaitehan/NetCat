pub mod certificates;
pub mod diagnostics;
pub mod secrets;
pub mod system_proxy;

#[cfg(target_os = "windows")]
pub use certificates::{CurrentUserRootStore, WindowsCertificateTrust};

#[cfg(target_os = "windows")]
pub use secrets::DpapiSecretStore;

#[cfg(target_os = "windows")]
pub use system_proxy::{CurrentUserInternetSettings, WindowsSystemProxyBackend};
