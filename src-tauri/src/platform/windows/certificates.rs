use sha2::{Digest, Sha256};

use crate::{
    error::AppError,
    platform::{certificate_der, CertificateTrust, CertificateTrustState},
};

/// The external certificate-store boundary; implementations return complete DER certificates.
pub trait CertificateStoreApi: Send + Sync {
    fn certificates(&self) -> Result<Vec<Vec<u8>>, AppError>;
    fn add(&self, der: &[u8]) -> Result<(), AppError>;
    /// Remove only contexts with the SHA-256 fingerprint of this exact DER certificate.
    fn remove(&self, der: &[u8]) -> Result<(), AppError>;
}

pub struct WindowsCertificateTrust<S> {
    api: S,
}

impl<S: CertificateStoreApi> WindowsCertificateTrust<S> {
    pub fn new(api: S) -> Self {
        Self { api }
    }

    fn matching_certificates(&self, der: &[u8]) -> Result<Vec<Vec<u8>>, AppError> {
        let expected = Sha256::digest(der);
        Ok(self
            .api
            .certificates()?
            .into_iter()
            .filter(|stored| Sha256::digest(stored) == expected)
            .collect())
    }
}

enum CertificateInput {
    Ready(Vec<u8>),
    State(CertificateTrustState),
}

fn validate(certificate_pem: &[u8], fingerprint: &str) -> Result<CertificateInput, AppError> {
    if certificate_pem.is_empty() {
        return Ok(CertificateInput::State(CertificateTrustState::NotGenerated));
    }
    let der = certificate_der(certificate_pem)?;
    let compact = fingerprint.replace(':', "");
    if compact.len() != 64 || !compact.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::new(
            "CA_FINGERPRINT_INVALID",
            "需要完整的 SHA-256 证书指纹",
        ));
    }
    let actual = Sha256::digest(&der)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    if !compact.eq_ignore_ascii_case(&actual) {
        return Ok(CertificateInput::State(CertificateTrustState::Mismatch));
    }
    Ok(CertificateInput::Ready(der))
}

impl<S: CertificateStoreApi> CertificateTrust for WindowsCertificateTrust<S> {
    fn status(
        &self,
        certificate_pem: &[u8],
        fingerprint: &str,
    ) -> Result<CertificateTrustState, AppError> {
        let der = match validate(certificate_pem, fingerprint)? {
            CertificateInput::Ready(der) => der,
            CertificateInput::State(state) => return Ok(state),
        };
        Ok(match self.matching_certificates(&der) {
            Ok(matches) if matches.is_empty() => CertificateTrustState::NotTrusted,
            Ok(_) => CertificateTrustState::Trusted,
            Err(_) => CertificateTrustState::Unavailable,
        })
    }

    fn install(
        &self,
        certificate_pem: &[u8],
        fingerprint: &str,
    ) -> Result<CertificateTrustState, AppError> {
        let der = match validate(certificate_pem, fingerprint)? {
            CertificateInput::Ready(der) => der,
            CertificateInput::State(state) => return Ok(state),
        };
        if self.matching_certificates(&der)?.is_empty() {
            self.api.add(&der)?;
        }
        self.status(certificate_pem, fingerprint)
    }

    fn remove(
        &self,
        certificate_pem: &[u8],
        fingerprint: &str,
    ) -> Result<CertificateTrustState, AppError> {
        let der = match validate(certificate_pem, fingerprint)? {
            CertificateInput::Ready(der) => der,
            CertificateInput::State(state) => return Ok(state),
        };
        for matched in self.matching_certificates(&der)? {
            self.api.remove(&matched)?;
        }
        self.status(certificate_pem, fingerprint)
    }
}

#[cfg(target_os = "windows")]
pub use native::CurrentUserRootStore;

#[cfg(target_os = "windows")]
mod native {
    use std::{mem, ptr, slice};

    use windows::{
        core::w,
        Win32::{
            Foundation::{GetLastError, CRYPT_E_NOT_FOUND},
            Security::Cryptography::{
                CertAddEncodedCertificateToStore, CertCloseStore, CertDeleteCertificateFromStore,
                CertDuplicateCertificateContext, CertEnumCertificatesInStore,
                CertFreeCertificateContext, CertOpenStore, CERT_CONTEXT, CERT_OPEN_STORE_FLAGS,
                CERT_STORE_ADD_NEW, CERT_STORE_OPEN_EXISTING_FLAG, CERT_STORE_PROV_SYSTEM_W,
                CERT_STORE_READONLY_FLAG, CERT_SYSTEM_STORE_CURRENT_USER, HCERTSTORE,
                X509_ASN_ENCODING,
            },
        },
    };

    use super::*;

    /// Each operation opens only CurrentUser\\Root, and retains no native handles between calls.
    pub struct CurrentUserRootStore;

    struct Store(HCERTSTORE);

    impl Store {
        fn open(read_only: bool) -> Result<Self, AppError> {
            let mut flags = CERT_OPEN_STORE_FLAGS(CERT_SYSTEM_STORE_CURRENT_USER)
                | CERT_STORE_OPEN_EXISTING_FLAG;
            if read_only {
                flags |= CERT_STORE_READONLY_FLAG;
            }
            // ROOT is a static, NUL-terminated UTF-16 name; no LocalMachine flag is used.
            unsafe {
                CertOpenStore(
                    CERT_STORE_PROV_SYSTEM_W,
                    X509_ASN_ENCODING,
                    None,
                    flags,
                    Some(w!("ROOT").as_ptr().cast()),
                )
                .map(Self)
                .map_err(|_| store_error())
            }
        }

        fn visit(
            &self,
            mut visitor: impl FnMut(*const CERT_CONTEXT, &[u8]) -> Result<(), AppError>,
        ) -> Result<(), AppError> {
            let mut context = Context(ptr::null());
            loop {
                // Enumeration consumes the previous context, including on failure.
                let previous = mem::replace(&mut context.0, ptr::null());
                context.0 = unsafe {
                    CertEnumCertificatesInStore(self.0, (!previous.is_null()).then_some(previous))
                };
                if context.0.is_null() {
                    let error = unsafe { GetLastError() };
                    return if error.0 == CRYPT_E_NOT_FOUND.0 as u32 {
                        Ok(())
                    } else {
                        Err(store_error())
                    };
                }
                let encoded = unsafe {
                    let certificate = &*context.0;
                    if certificate.pbCertEncoded.is_null() || certificate.cbCertEncoded == 0 {
                        return Err(store_error());
                    }
                    slice::from_raw_parts(
                        certificate.pbCertEncoded,
                        certificate.cbCertEncoded as usize,
                    )
                };
                visitor(context.0, encoded)?;
            }
        }
    }

    impl Drop for Store {
        fn drop(&mut self) {
            let _ = unsafe { CertCloseStore(Some(self.0), 0) };
        }
    }

    struct Context(*const CERT_CONTEXT);

    impl Drop for Context {
        fn drop(&mut self) {
            if !self.0.is_null() {
                let _ = unsafe { CertFreeCertificateContext(Some(self.0)) };
            }
        }
    }

    impl CertificateStoreApi for CurrentUserRootStore {
        fn certificates(&self) -> Result<Vec<Vec<u8>>, AppError> {
            let store = Store::open(true)?;
            let mut certificates = Vec::new();
            store.visit(|_, encoded| {
                certificates.push(encoded.to_vec());
                Ok(())
            })?;
            Ok(certificates)
        }

        fn add(&self, der: &[u8]) -> Result<(), AppError> {
            if der.len() > u32::MAX as usize {
                return Err(store_error());
            }
            let store = Store::open(false)?;
            unsafe {
                CertAddEncodedCertificateToStore(
                    Some(store.0),
                    X509_ASN_ENCODING,
                    der,
                    CERT_STORE_ADD_NEW,
                    None,
                )
                .map_err(|_| store_error())
            }
        }

        fn remove(&self, der: &[u8]) -> Result<(), AppError> {
            let expected = Sha256::digest(der);
            let store = Store::open(false)?;
            store.visit(|context, encoded| {
                if Sha256::digest(encoded) == expected {
                    // Deletion consumes its context even on failure. Duplicate the enumerator's
                    // reference so enumeration can safely continue to find further exact matches.
                    let owned = unsafe { CertDuplicateCertificateContext(Some(context)) };
                    if owned.is_null() {
                        return Err(store_error());
                    }
                    unsafe { CertDeleteCertificateFromStore(owned) }.map_err(|_| store_error())?;
                }
                Ok(())
            })
        }
    }

    fn store_error() -> AppError {
        AppError::new(
            "CERTIFICATE_STORE_ERROR",
            "无法访问当前 Windows 用户的受信任根证书存储",
        )
        .with_action("请检查当前用户的证书存储访问权限后重试")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{CertificateStoreApi, WindowsCertificateTrust};
    use crate::{
        error::AppError,
        platform::{CertificateTrust, CertificateTrustState},
    };

    const PEM_A: &[u8] = b"-----BEGIN CERTIFICATE-----\nQQ==\n-----END CERTIFICATE-----\n";
    const PEM_B: &[u8] = b"-----BEGIN CERTIFICATE-----\nQg==\n-----END CERTIFICATE-----\n";
    const HASH_A: &str = "559AEAD08264D5795D3909718CDD05ABD49572E84FE55590EEF31A88A08FDFFD";
    const HASH_B: &str = "DF7E70E5021544F4834BBEE64A9E3789FEBC4BE81470DF629CAD6DDB03320A5C";

    #[derive(Clone, Default)]
    struct MemoryCertificateStore {
        certificates: Arc<Mutex<Vec<Vec<u8>>>>,
        fail: bool,
    }

    impl MemoryCertificateStore {
        fn with_certificates(certificates: &[&[u8]]) -> Self {
            Self {
                certificates: Arc::new(Mutex::new(
                    certificates.iter().map(|der| der.to_vec()).collect(),
                )),
                fail: false,
            }
        }

        fn contents(&self) -> Vec<Vec<u8>> {
            self.certificates.lock().unwrap().clone()
        }
    }

    impl CertificateStoreApi for MemoryCertificateStore {
        fn certificates(&self) -> Result<Vec<Vec<u8>>, AppError> {
            if self.fail {
                return Err(AppError::new("CERTIFICATE_STORE_ERROR", "unavailable"));
            }
            Ok(self.contents())
        }

        fn add(&self, der: &[u8]) -> Result<(), AppError> {
            self.certificates.lock().unwrap().push(der.to_vec());
            Ok(())
        }

        fn remove(&self, der: &[u8]) -> Result<(), AppError> {
            self.certificates
                .lock()
                .unwrap()
                .retain(|stored| stored != der);
            Ok(())
        }
    }

    #[test]
    fn generated_ca_fingerprint_matches_store_and_preserves_same_name_certificate() {
        struct MemorySecrets(Mutex<Option<String>>);
        impl crate::platform::SecretStore for MemorySecrets {
            fn save_private_key(&self, value: &str) -> Result<(), AppError> {
                *self.0.lock().unwrap() = Some(value.to_owned());
                Ok(())
            }
            fn load_private_key(&self) -> Result<Option<String>, AppError> {
                Ok(self.0.lock().unwrap().clone())
            }
        }
        let first_directory = tempfile::tempdir().unwrap();
        let second_directory = tempfile::tempdir().unwrap();
        let first = crate::ca::CaManager::new(
            first_directory.path().to_owned(),
            MemorySecrets(Mutex::new(None)),
        );
        let second = crate::ca::CaManager::new(
            second_directory.path().to_owned(),
            MemorySecrets(Mutex::new(None)),
        );
        let first_status = first.generate().unwrap();
        let second_status = second.generate().unwrap();
        let first_pem = std::fs::read(first.certificate_path()).unwrap();
        let second_pem = std::fs::read(second.certificate_path()).unwrap();
        let api = MemoryCertificateStore::default();
        let trust = WindowsCertificateTrust::new(api.clone());
        let first_hash = first_status.fingerprint.unwrap();
        let second_hash = second_status.fingerprint.unwrap();
        assert!(api.contents().is_empty());
        assert_ne!(first_hash, second_hash);

        assert_eq!(
            trust.install(&second_pem, &second_hash).unwrap(),
            CertificateTrustState::Trusted
        );
        assert_eq!(
            trust.status(&first_pem, &first_hash).unwrap(),
            CertificateTrustState::NotTrusted
        );
        assert_eq!(
            trust.install(&first_pem, &first_hash).unwrap(),
            CertificateTrustState::Trusted
        );
        assert_eq!(
            trust.remove(&first_pem, &first_hash).unwrap(),
            CertificateTrustState::NotTrusted
        );
        assert_eq!(
            trust.status(&second_pem, &second_hash).unwrap(),
            CertificateTrustState::Trusted
        );
        assert_eq!(api.contents().len(), 1);
    }

    #[test]
    fn remove_deletes_only_the_matching_fingerprint() {
        let api = MemoryCertificateStore::with_certificates(&[b"A", b"B", b"A"]);
        let trust = WindowsCertificateTrust::new(api.clone());

        assert_eq!(
            trust.remove(PEM_A, HASH_A).unwrap(),
            CertificateTrustState::NotTrusted
        );
        assert_eq!(api.contents(), vec![b"B".to_vec()]);
        assert_eq!(
            trust.status(PEM_B, HASH_B).unwrap(),
            CertificateTrustState::Trusted
        );
    }

    #[test]
    fn unrelated_certificate_is_not_trusted_or_deleted() {
        let api = MemoryCertificateStore::with_certificates(&[b"B"]);
        let trust = WindowsCertificateTrust::new(api.clone());

        assert_eq!(
            trust.status(PEM_A, HASH_A).unwrap(),
            CertificateTrustState::NotTrusted
        );
        assert_eq!(
            trust.remove(PEM_A, HASH_A).unwrap(),
            CertificateTrustState::NotTrusted
        );
        assert_eq!(api.contents(), vec![b"B".to_vec()]);
    }

    #[test]
    fn installation_is_explicit_and_idempotent() {
        let api = MemoryCertificateStore::default();
        let trust: Box<dyn CertificateTrust> = Box::new(WindowsCertificateTrust::new(api.clone()));
        assert_eq!(
            trust.status(PEM_A, HASH_A).unwrap(),
            CertificateTrustState::NotTrusted
        );
        assert!(api.contents().is_empty());

        assert_eq!(
            trust.install(PEM_A, HASH_A).unwrap(),
            CertificateTrustState::Trusted
        );
        assert_eq!(
            trust.install(PEM_A, HASH_A).unwrap(),
            CertificateTrustState::Trusted
        );
        assert_eq!(api.contents(), vec![b"A".to_vec()]);
    }

    #[test]
    fn mismatched_fingerprint_cannot_install_or_remove() {
        let api = MemoryCertificateStore::with_certificates(&[b"B"]);
        let trust = WindowsCertificateTrust::new(api.clone());
        assert_eq!(
            trust.status(PEM_A, HASH_B).unwrap(),
            CertificateTrustState::Mismatch
        );
        assert_eq!(
            trust.install(PEM_A, HASH_B).unwrap(),
            CertificateTrustState::Mismatch
        );
        assert_eq!(
            trust.remove(PEM_A, HASH_B).unwrap(),
            CertificateTrustState::Mismatch
        );
        assert_eq!(api.contents(), vec![b"B".to_vec()]);
    }

    #[test]
    fn empty_certificate_maps_to_not_generated_without_mutation() {
        let api = MemoryCertificateStore::with_certificates(&[b"B"]);
        let trust = WindowsCertificateTrust::new(api.clone());
        assert_eq!(
            trust.status(b"", "").unwrap(),
            CertificateTrustState::NotGenerated
        );
        assert_eq!(
            trust.install(b"", "").unwrap(),
            CertificateTrustState::NotGenerated
        );
        assert_eq!(
            trust.remove(b"", "").unwrap(),
            CertificateTrustState::NotGenerated
        );
        assert_eq!(api.contents(), vec![b"B".to_vec()]);
    }

    #[test]
    fn store_failure_is_unavailable_for_status_and_actionable_for_mutations() {
        let trust = WindowsCertificateTrust::new(MemoryCertificateStore {
            fail: true,
            ..Default::default()
        });
        assert_eq!(
            trust.status(PEM_A, HASH_A).unwrap(),
            CertificateTrustState::Unavailable
        );
        assert_eq!(
            trust.install(PEM_A, HASH_A).unwrap_err().code,
            "CERTIFICATE_STORE_ERROR"
        );
        assert_eq!(
            trust.remove(PEM_A, HASH_A).unwrap_err().code,
            "CERTIFICATE_STORE_ERROR"
        );
    }

    #[test]
    fn malformed_or_multiple_pem_blocks_cannot_change_store() {
        let api = MemoryCertificateStore::with_certificates(&[b"B"]);
        let trust = WindowsCertificateTrust::new(api.clone());
        for pem in [
            b"not a certificate".to_vec(),
            [PEM_A, PEM_B].concat(),
            b"-----BEGIN PRIVATE KEY-----\nQQ==\n-----END PRIVATE KEY-----\n".to_vec(),
            b"-----BEGIN CERTIFICATE-----\n!\n-----END CERTIFICATE-----\n".to_vec(),
        ] {
            assert_eq!(
                trust.status(&pem, HASH_A).unwrap_err().code,
                "CA_CERTIFICATE_INVALID"
            );
            assert_eq!(
                trust.install(&pem, HASH_A).unwrap_err().code,
                "CA_CERTIFICATE_INVALID"
            );
            assert_eq!(
                trust.remove(&pem, HASH_A).unwrap_err().code,
                "CA_CERTIFICATE_INVALID"
            );
        }
        assert_eq!(api.contents(), vec![b"B".to_vec()]);
    }

    #[test]
    fn fingerprint_accepts_colons_and_lowercase_but_requires_sha256() {
        let trust =
            WindowsCertificateTrust::new(MemoryCertificateStore::with_certificates(&[b"A"]));
        let fingerprint = "55:9a:ea:d0:82:64:d5:79:5d:39:09:71:8c:dd:05:ab:d4:95:72:e8:4f:e5:55:90:ee:f3:1a:88:a0:8f:df:fd";
        assert_eq!(
            trust.status(PEM_A, fingerprint).unwrap(),
            CertificateTrustState::Trusted
        );
        for invalid in [
            "",
            "AA",
            "ZZ9AEAD08264D5795D3909718CDD05ABD49572E84FE55590EEF31A88A08FDFFD",
        ] {
            assert_eq!(
                trust.status(PEM_A, invalid).unwrap_err().code,
                "CA_FINGERPRINT_INVALID"
            );
        }
    }
}
