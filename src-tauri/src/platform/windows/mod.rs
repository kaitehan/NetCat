pub mod secrets;

#[cfg(target_os = "windows")]
pub use secrets::DpapiSecretStore;
