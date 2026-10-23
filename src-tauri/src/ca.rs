use std::{fs, path::PathBuf, sync::Arc};

use hudsucker::rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, PublicKeyData,
};
use sha2::{Digest, Sha256};

use crate::{error::AppError, models::CaStatus};

pub use crate::platform::SecretStore;

pub const KEYCHAIN_SERVICE: &str = "com.kaitehan.netcat";
pub const CA_COMMON_NAME: &str = "NetCat Local Root CA";
pub const CA_ORGANIZATION_NAME: &str = "NetCat Local Development";
pub struct CaManager {
    directory: PathBuf,
    secrets: Arc<dyn SecretStore>,
}

impl CaManager {
    pub fn new<S: SecretStore + 'static>(directory: PathBuf, secrets: S) -> Self {
        Self {
            directory,
            secrets: Arc::new(secrets),
        }
    }

    pub fn certificate_path(&self) -> PathBuf {
        self.directory.join("netcat-ca.crt")
    }

    pub fn generate(&self) -> Result<CaStatus, AppError> {
        fs::create_dir_all(&self.directory).map_err(file_error)?;
        let key_pair = KeyPair::generate().map_err(certificate_error)?;
        let mut params = CertificateParams::default();
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, CA_COMMON_NAME);
        distinguished_name.push(DnType::OrganizationName, CA_ORGANIZATION_NAME);
        params.distinguished_name = distinguished_name;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::CrlSign,
        ];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let certificate = params.self_signed(&key_pair).map_err(certificate_error)?;
        let certificate_pem = certificate.pem();
        let private_key_pem = key_pair.serialize_pem();
        self.secrets.save_private_key(&private_key_pem)?;
        fs::write(self.certificate_path(), certificate_pem.as_bytes()).map_err(file_error)?;
        self.status()
    }

    pub fn status(&self) -> Result<CaStatus, AppError> {
        let path = self.certificate_path();
        if !path.try_exists().map_err(file_error)? {
            return Ok(CaStatus {
                generated: false,
                certificate_path: None,
                fingerprint: None,
            });
        }
        let certificate_pem = fs::read_to_string(&path).map_err(file_error)?;
        let fingerprint = fingerprint(&certificate_pem)?;
        Ok(CaStatus {
            generated: true,
            certificate_path: Some(path.to_string_lossy().into_owned()),
            fingerprint: Some(fingerprint),
        })
    }

    pub fn load_issuer(&self) -> Result<Issuer<'static, KeyPair>, AppError> {
        let certificate_pem = fs::read_to_string(self.certificate_path()).map_err(file_error)?;
        let key_pair = self.matching_key_pair(&certificate_pem)?;
        Issuer::from_ca_cert_pem(&certificate_pem, key_pair).map_err(certificate_error)
    }

    pub fn check_private_key(&self) -> Result<(), AppError> {
        let certificate_pem = fs::read_to_string(self.certificate_path()).map_err(file_error)?;
        self.matching_key_pair(&certificate_pem).map(|_| ())
    }

    pub fn private_key_state(&self) -> crate::platform::CaPrivateKeyState {
        use crate::platform::CaPrivateKeyState::*;
        let private_key = match self.secrets.load_private_key() {
            Ok(Some(key)) => key,
            Ok(None) => return Missing,
            Err(_) => return Unavailable,
        };
        let certificate = match fs::read_to_string(self.certificate_path()) {
            Ok(certificate) => certificate,
            Err(_) => return Unavailable,
        };
        match matching_key_pair(&certificate, &private_key) {
            Ok(_) => Ready,
            Err(error) if error.code == "CA_KEY_MISMATCH" => Mismatch,
            Err(_) => Unavailable,
        }
    }

    fn matching_key_pair(&self, certificate_pem: &str) -> Result<KeyPair, AppError> {
        let unavailable = || {
            AppError::new("CA_KEY_UNAVAILABLE", "无法读取受保护的 CA 私钥")
                .with_action("请检查私钥访问权限或重新生成本地 CA")
        };
        let mismatch = || {
            AppError::new("CA_KEY_MISMATCH", "CA 证书与私钥不匹配或私钥缺失")
                .with_action("请重新生成本地 CA")
        };
        let private_key = self
            .secrets
            .load_private_key()
            .map_err(|_| unavailable())?
            .ok_or_else(mismatch)?;
        matching_key_pair(certificate_pem, &private_key)
    }

    pub fn export_public_certificate(&self, destination: PathBuf) -> Result<(), AppError> {
        fs::copy(self.certificate_path(), destination).map_err(file_error)?;
        Ok(())
    }
}

fn matching_key_pair(certificate_pem: &str, private_key_pem: &str) -> Result<KeyPair, AppError> {
    let key = KeyPair::from_pem(private_key_pem)
        .map_err(|_| AppError::new("CA_KEY_UNAVAILABLE", "无法读取受保护的 CA 私钥"))?;
    let der = crate::platform::certificate_der(certificate_pem.as_bytes())?;
    let (remaining, certificate) = x509_parser::parse_x509_certificate(&der)
        .map_err(|_| AppError::new("CA_CERTIFICATE_INVALID", "CA 公钥证书无效"))?;
    if !remaining.is_empty() {
        return Err(AppError::new("CA_CERTIFICATE_INVALID", "CA 公钥证书无效"));
    }
    if certificate.public_key().raw != key.subject_public_key_info() {
        return Err(AppError::new("CA_KEY_MISMATCH", "CA 证书与私钥不匹配")
            .with_action("请重新生成本地 CA"));
    }
    Ok(key)
}

fn fingerprint(certificate_pem: &str) -> Result<String, AppError> {
    let der = crate::platform::certificate_der(certificate_pem.as_bytes())?;
    let digest = Sha256::digest(&der);
    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":"))
}

fn certificate_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("CA_ERROR", "无法生成或加载本地 CA").with_detail(error.to_string())
}

fn file_error(error: std::io::Error) -> AppError {
    AppError::new("FILE_ERROR", "无法读写 CA 证书文件")
        .with_detail(error.to_string())
        .with_action("请检查应用数据目录权限")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // Deliberately not Clone: private-key storage must support trait objects.
    #[derive(Default)]
    struct MemorySecrets(Mutex<Option<String>>);

    impl SecretStore for MemorySecrets {
        fn save_private_key(&self, value: &str) -> Result<(), AppError> {
            *self.0.lock().unwrap() = Some(value.to_owned());
            Ok(())
        }

        fn load_private_key(&self) -> Result<Option<String>, AppError> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    #[test]
    fn issuer_rejects_a_private_key_from_a_different_generated_ca() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemorySecrets::default());
        let manager = CaManager::new(directory.path().join("first"), secrets.clone());
        manager.generate().unwrap();
        CaManager::new(directory.path().join("second"), secrets)
            .generate()
            .unwrap();
        assert_eq!(manager.load_issuer().err().unwrap().code, "CA_KEY_MISMATCH");
    }

    #[test]
    fn public_identity_remains_available_without_a_private_key() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemorySecrets::default());
        let manager = CaManager::new(directory.path().to_path_buf(), secrets.clone());
        let original = manager.generate().unwrap();
        *secrets.0.lock().unwrap() = None;
        assert_eq!(manager.status().unwrap().fingerprint, original.fingerprint);
        assert!(manager.status().unwrap().generated);
        assert_eq!(manager.load_issuer().err().unwrap().code, "CA_KEY_MISMATCH");
    }

    #[test]
    fn malformed_private_key_is_unavailable_and_sanitized() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemorySecrets::default());
        let manager = CaManager::new(directory.path().to_path_buf(), secrets.clone());
        manager.generate().unwrap();
        *secrets.0.lock().unwrap() = Some("PRIVATE KEY SECRET SENTINEL".into());
        let error = manager.load_issuer().err().unwrap();
        assert_eq!(error.code, "CA_KEY_UNAVAILABLE");
        assert!(error.detail.is_none());
        assert!(!serde_json::to_string(&error).unwrap().contains("SENTINEL"));
    }

    #[test]
    fn fingerprint_uses_der_and_is_independent_of_pem_line_endings() {
        let pem = "-----BEGIN CERTIFICATE-----\nQQ==\n-----END CERTIFICATE-----\n";
        let expected = "55:9A:EA:D0:82:64:D5:79:5D:39:09:71:8C:DD:05:AB:D4:95:72:E8:4F:E5:55:90:EE:F3:1A:88:A0:8F:DF:FD";
        assert_eq!(fingerprint(pem).unwrap(), expected);
        assert_eq!(fingerprint(&pem.replace('\n', "\r\n")).unwrap(), expected);
        assert_eq!(
            fingerprint("invalid").unwrap_err().code,
            "CA_CERTIFICATE_INVALID"
        );
    }

    #[test]
    fn ca_uses_object_safe_secret_store_without_writing_private_key_to_disk() {
        let directory = tempfile::tempdir().unwrap();
        let secrets: Arc<dyn SecretStore> = Arc::new(MemorySecrets::default());
        let manager = CaManager::new(directory.path().to_path_buf(), secrets.clone());

        let status = manager.generate().unwrap();
        let public_certificate = fs::read_to_string(manager.certificate_path()).unwrap();

        assert!(status.generated);
        assert!(public_certificate.contains("BEGIN CERTIFICATE"));
        assert!(!public_certificate.contains("PRIVATE KEY"));
        assert!(secrets
            .load_private_key()
            .unwrap()
            .unwrap()
            .contains("PRIVATE KEY"));
        assert!(manager.load_issuer().is_ok());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
