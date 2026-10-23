use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{error::AppError, platform::SecretStore};

const FORMAT_V1: &[u8] = b"NC-DPAPI\x01";

trait DataProtector: Send + Sync {
    fn protect(&self, value: &[u8]) -> Result<Vec<u8>, ()>;
    fn unprotect(&self, value: &[u8]) -> Result<Vec<u8>, ()>;
}

/// A versioned ciphertext file protected for the current Windows user.
pub struct DpapiSecretStore {
    path: PathBuf,
    protector: Arc<dyn DataProtector>,
}

impl DpapiSecretStore {
    #[cfg(target_os = "windows")]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            protector: Arc::new(CurrentUserProtector),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(test)]
    fn with_protector(path: PathBuf, protector: Arc<dyn DataProtector>) -> Self {
        Self { path, protector }
    }
}

impl SecretStore for DpapiSecretStore {
    fn save_private_key(&self, value: &str) -> Result<(), AppError> {
        if value.is_empty() {
            return Err(AppError::new("DPAPI_EMPTY_KEY", "CA 私钥不能为空"));
        }
        let encrypted = self.protector.protect(value.as_bytes()).map_err(|_| {
            AppError::new(
                "DPAPI_ENCRYPT_ERROR",
                "无法使用当前 Windows 用户保护 CA 私钥",
            )
        })?;
        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| storage_error())?;
        temporary
            .write_all(FORMAT_V1)
            .and_then(|_| temporary.write_all(&encrypted))
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|_| storage_error())?;
        // Same-directory persist atomically replaces an existing file on Windows too.
        temporary.persist(&self.path).map_err(|_| storage_error())?;
        Ok(())
    }

    fn load_private_key(&self) -> Result<Option<String>, AppError> {
        let encoded = match fs::read(&self.path) {
            Ok(encoded) => encoded,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(storage_error()),
        };
        let encrypted = encoded
            .strip_prefix(FORMAT_V1)
            .filter(|encrypted| !encrypted.is_empty())
            .ok_or_else(decrypt_error)?;
        let plaintext = self
            .protector
            .unprotect(encrypted)
            .map_err(|_| decrypt_error())?;
        let value = String::from_utf8(plaintext).map_err(|_| decrypt_error())?;
        if value.is_empty() {
            return Err(decrypt_error());
        }
        Ok(Some(value))
    }
}

fn storage_error() -> AppError {
    AppError::new("DPAPI_STORAGE_ERROR", "无法读写本地 CA 私钥文件")
        .with_action("请检查应用数据目录权限后重试")
}

fn decrypt_error() -> AppError {
    AppError::new(
        "DPAPI_DECRYPT_ERROR",
        "无法使用当前 Windows 用户解密 CA 私钥",
    )
}

#[cfg(target_os = "windows")]
struct CurrentUserProtector;

#[cfg(target_os = "windows")]
impl DataProtector for CurrentUserProtector {
    fn protect(&self, value: &[u8]) -> Result<Vec<u8>, ()> {
        protect_for_current_user(value).map_err(|_| ())
    }

    fn unprotect(&self, value: &[u8]) -> Result<Vec<u8>, ()> {
        unprotect_for_current_user(value).map_err(|_| ())
    }
}

/// Protect bytes for the current user, without machine-wide scope or UI prompts.
#[cfg(target_os = "windows")]
pub fn protect_for_current_user(value: &[u8]) -> Result<Vec<u8>, AppError> {
    native::transform(value, false).map_err(|_| {
        AppError::new(
            "DPAPI_ENCRYPT_ERROR",
            "无法使用当前 Windows 用户保护 CA 私钥",
        )
    })
}

/// Unprotect bytes for the current user. Native errors never include key data.
#[cfg(target_os = "windows")]
pub fn unprotect_for_current_user(value: &[u8]) -> Result<Vec<u8>, AppError> {
    native::transform(value, true).map_err(|_| decrypt_error())
}

#[cfg(target_os = "windows")]
mod native {
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{LocalFree, HLOCAL},
            Security::Cryptography::{
                CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
            },
        },
    };

    /// DPAPI owns this allocation, including any allocation on a failed call.
    struct LocalBlob(CRYPT_INTEGER_BLOB);

    impl Drop for LocalBlob {
        fn drop(&mut self) {
            if !self.0.pbData.is_null() {
                // SAFETY: CryptProtectData/CryptUnprotectData allocate with LocalAlloc.
                // This guard is the sole owner and releases the buffer exactly once.
                unsafe {
                    let _ = LocalFree(Some(HLOCAL(self.0.pbData.cast())));
                }
            }
        }
    }

    pub(super) fn transform(value: &[u8], decrypt: bool) -> Result<Vec<u8>, ()> {
        if value.is_empty() {
            return Err(());
        }
        let input = CRYPT_INTEGER_BLOB {
            cbData: value.len().try_into().map_err(|_| ())?,
            // The Windows API declares a mutable pointer but only reads this input.
            pbData: value.as_ptr().cast_mut(),
        };
        let mut output = LocalBlob(CRYPT_INTEGER_BLOB::default());
        // SAFETY: Both blobs live for the call, the input points to cbData readable
        // bytes, and output is an initialized out parameter owned by the guard.
        // No optional entropy, description allocation, or interactive UI is used.
        unsafe {
            let result = if decrypt {
                CryptUnprotectData(
                    &input,
                    None,
                    None,
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output.0,
                )
            } else {
                CryptProtectData(
                    &input,
                    PCWSTR::null(),
                    None,
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output.0,
                )
            };
            result.map_err(|_| ())?;
            if output.0.pbData.is_null() || output.0.cbData == 0 {
                return Err(());
            }
            // SAFETY: On success DPAPI returns cbData initialized bytes; copy them
            // before LocalBlob drops and frees the native allocation.
            Ok(std::slice::from_raw_parts(output.0.pbData, output.0.cbData as usize).to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc};

    use super::{DataProtector, DpapiSecretStore};
    use crate::platform::SecretStore;

    struct FakeProtector;

    impl DataProtector for FakeProtector {
        fn protect(&self, value: &[u8]) -> Result<Vec<u8>, ()> {
            let mut encrypted = b"fake-protected:".to_vec();
            encrypted.extend(value.iter().map(|byte| byte ^ 0xa5));
            Ok(encrypted)
        }

        fn unprotect(&self, value: &[u8]) -> Result<Vec<u8>, ()> {
            value
                .strip_prefix(b"fake-protected:")
                .map(|value| value.iter().map(|byte| byte ^ 0xa5).collect())
                .ok_or(())
        }
    }

    struct RejectingProtector;

    impl DataProtector for RejectingProtector {
        fn protect(&self, _: &[u8]) -> Result<Vec<u8>, ()> {
            Err(())
        }

        fn unprotect(&self, _: &[u8]) -> Result<Vec<u8>, ()> {
            Err(())
        }
    }

    fn store(path: std::path::PathBuf) -> DpapiSecretStore {
        DpapiSecretStore::with_protector(path, Arc::new(FakeProtector))
    }

    #[test]
    fn saves_versioned_ciphertext_and_loads_plaintext() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(directory.path().join("ca-key.dpapi"));

        store.save_private_key("test-key-one").unwrap();

        // Boolean assertions keep both plaintext and ciphertext out of failure output.
        assert!(
            store.load_private_key().unwrap().as_deref() == Some("test-key-one"),
            "decrypted value mismatch"
        );
        let saved = fs::read(store.path()).unwrap();
        assert!(saved.starts_with(b"NC-DPAPI\x01"));
        assert!(
            !saved.windows(12).any(|bytes| bytes == b"test-key-one"),
            "plaintext must not be written"
        );
    }

    #[test]
    fn missing_file_returns_none_without_creating_it() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(directory.path().join("missing").join("ca-key.dpapi"));

        assert!(store.load_private_key().unwrap().is_none());
        assert!(!store.path().exists());
    }

    #[test]
    fn rejects_corrupt_truncated_and_unknown_version_files() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(directory.path().join("ca-key.dpapi"));

        for invalid in [
            b"".as_slice(),
            b"NC-DPAPI",
            b"NC-DPAPI\x01",
            b"NC-DPAPI\x02fake-protected:data",
            b"NC-DPAPI\x01corrupt",
            b"unexpected-format",
        ] {
            fs::write(store.path(), invalid).unwrap();
            let error = store
                .load_private_key()
                .err()
                .expect("corrupt file accepted");
            assert_eq!(error.code, "DPAPI_DECRYPT_ERROR");
            assert!(error.detail.is_none());
        }
    }

    #[test]
    fn rejects_empty_plaintext_before_replacing_existing_key() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(directory.path().join("ca-key.dpapi"));
        store.save_private_key("test-key-one").unwrap();

        assert_eq!(
            store.save_private_key("").unwrap_err().code,
            "DPAPI_EMPTY_KEY"
        );
        assert!(
            store.load_private_key().unwrap().as_deref() == Some("test-key-one"),
            "previous key must survive rejected input"
        );
    }

    #[test]
    fn rejects_empty_or_non_utf8_decrypted_keys() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(directory.path().join("ca-key.dpapi"));

        for invalid in [
            b"NC-DPAPI\x01fake-protected:".as_slice(),
            b"NC-DPAPI\x01fake-protected:\x5a",
        ] {
            fs::write(store.path(), invalid).unwrap();
            assert_eq!(
                store
                    .load_private_key()
                    .err()
                    .expect("invalid key accepted")
                    .code,
                "DPAPI_DECRYPT_ERROR"
            );
        }
    }

    #[test]
    fn overwrite_replaces_key_and_cleans_up_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(directory.path().join("ca-key.dpapi"));
        store.save_private_key("test-key-one").unwrap();
        store.save_private_key("test-key-two").unwrap();

        assert!(
            store.load_private_key().unwrap().as_deref() == Some("test-key-two"),
            "replacement key was not loaded"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn encryption_failure_preserves_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ca-key.dpapi");
        store(path.clone())
            .save_private_key("test-key-one")
            .unwrap();
        let failing = DpapiSecretStore::with_protector(path.clone(), Arc::new(RejectingProtector));

        let error = failing.save_private_key("test-key-two").unwrap_err();
        assert_eq!(error.code, "DPAPI_ENCRYPT_ERROR");
        assert!(error.detail.is_none());
        assert!(
            store(path).load_private_key().unwrap().as_deref() == Some("test-key-one"),
            "previous key must survive encryption failure"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn decryption_failure_returns_sanitized_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ca-key.dpapi");
        store(path.clone())
            .save_private_key("test-key-one")
            .unwrap();
        let failing = DpapiSecretStore::with_protector(path, Arc::new(RejectingProtector));

        let error = failing
            .load_private_key()
            .err()
            .expect("decryption failure ignored");
        assert_eq!(error.code, "DPAPI_DECRYPT_ERROR");
        assert!(error.detail.is_none());
    }

    #[test]
    fn io_errors_do_not_become_missing_keys_and_failed_replace_cleans_up() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("ca-key.dpapi");
        fs::create_dir(&destination).unwrap();
        let store = store(destination);

        assert_eq!(
            store
                .load_private_key()
                .err()
                .expect("read failure ignored")
                .code,
            "DPAPI_STORAGE_ERROR"
        );
        assert_eq!(
            store.save_private_key("test-key-one").unwrap_err().code,
            "DPAPI_STORAGE_ERROR"
        );
        assert!(store.path().is_dir());
        // The temporary file is a sibling of the destination directory.
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_saves_encrypted_key_and_loads_plaintext_after_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let store = DpapiSecretStore::new(directory.path().join("ca-key.dpapi"));

        store.save_private_key("test-key-one").unwrap();
        assert!(
            store.load_private_key().unwrap().as_deref() == Some("test-key-one"),
            "decrypted value mismatch"
        );
        assert!(
            !fs::read(store.path())
                .unwrap()
                .windows(12)
                .any(|bytes| bytes == b"test-key-one"),
            "plaintext must not be written"
        );
        store.save_private_key("test-key-two").unwrap();
        assert!(
            store.load_private_key().unwrap().as_deref() == Some("test-key-two"),
            "replacement key was not loaded"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_corrupted_ciphertext_returns_decrypt_error() {
        let directory = tempfile::tempdir().unwrap();
        let store = DpapiSecretStore::new(directory.path().join("ca-key.dpapi"));
        store.save_private_key("test-key-one").unwrap();
        let mut corrupted = fs::read(store.path()).unwrap();
        *corrupted.last_mut().unwrap() ^= 0xff;
        fs::write(store.path(), corrupted).unwrap();

        assert_eq!(
            store
                .load_private_key()
                .err()
                .expect("corrupt file accepted")
                .code,
            "DPAPI_DECRYPT_ERROR"
        );
    }
}
