pub mod certificates;
pub mod secrets;

#[cfg(target_os = "windows")]
pub use certificates::{CurrentUserRootStore, WindowsCertificateTrust};

#[cfg(target_os = "windows")]
pub use secrets::DpapiSecretStore;
