use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::AppError;

const AUTHORIZATION_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestBinding {
    conversation_id: String,
    operation_id: String,
    method: String,
    url_fingerprint: [u8; 32],
    body_hash: [u8; 32],
}

impl RequestBinding {
    pub fn new(
        conversation_id: impl Into<String>,
        operation_id: impl Into<String>,
        method: impl AsRef<str>,
        url: impl AsRef<str>,
        body: impl AsRef<[u8]>,
    ) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            operation_id: operation_id.into(),
            method: method.as_ref().to_uppercase(),
            url_fingerprint: Sha256::digest(url.as_ref().as_bytes()).into(),
            body_hash: Sha256::digest(body.as_ref()).into(),
        }
    }
}

struct Authorization {
    binding: RequestBinding,
    expires_at: SystemTime,
    used: bool,
}

#[derive(Default)]
pub struct AuthorizationStore {
    authorizations: HashMap<String, Authorization>,
}

impl AuthorizationStore {
    pub fn issue(&mut self, binding: RequestBinding, now: SystemTime) -> Result<String, AppError> {
        let handle = Uuid::new_v4().to_string();
        self.authorizations.insert(
            handle.clone(),
            Authorization {
                binding,
                expires_at: now + AUTHORIZATION_TTL,
                used: false,
            },
        );
        Ok(handle)
    }

    pub fn consume(
        &mut self,
        handle: &str,
        binding: &RequestBinding,
        now: SystemTime,
    ) -> Result<(), AppError> {
        let authorization = self
            .authorizations
            .get_mut(handle)
            .ok_or_else(|| AppError::new("ASSISTANT_AUTHORIZATION_INVALID", "联网诊断授权无效"))?;
        if authorization.used {
            return Err(AppError::new(
                "ASSISTANT_AUTHORIZATION_USED",
                "联网诊断授权已使用",
            ));
        }
        if now > authorization.expires_at {
            return Err(AppError::new(
                "ASSISTANT_AUTHORIZATION_EXPIRED",
                "联网诊断授权已过期",
            ));
        }
        if &authorization.binding != binding {
            return Err(AppError::new(
                "ASSISTANT_AUTHORIZATION_MISMATCH",
                "联网诊断请求与授权内容不一致",
            ));
        }
        authorization.used = true;
        Ok(())
    }

    pub fn conversation_id(&self, handle: &str) -> Result<&str, AppError> {
        self.authorizations
            .get(handle)
            .map(|authorization| authorization.binding.conversation_id.as_str())
            .ok_or_else(|| AppError::new("ASSISTANT_AUTHORIZATION_INVALID", "联网诊断授权无效"))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{AuthorizationStore, RequestBinding};

    fn binding() -> RequestBinding {
        RequestBinding::new(
            "conversation-1",
            "operation-1",
            "POST",
            "https://api.example.com/users",
            br#"{"name":"Ada"}"#,
        )
    }

    #[test]
    fn request_handle_is_single_use_and_bound_to_the_exact_request() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut store = AuthorizationStore::default();
        let handle = store.issue(binding(), now).unwrap();

        assert!(store.consume(&handle, &binding(), now).is_ok());
        assert_eq!(
            store.consume(&handle, &binding(), now).unwrap_err().code,
            "ASSISTANT_AUTHORIZATION_USED"
        );
    }

    #[test]
    fn changed_method_url_or_body_cannot_reuse_a_handle() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut store = AuthorizationStore::default();
        let handle = store.issue(binding(), now).unwrap();
        let changed = RequestBinding::new(
            "conversation-1",
            "operation-1",
            "PUT",
            "https://api.example.com/users",
            br#"{"name":"Grace"}"#,
        );

        assert_eq!(
            store.consume(&handle, &changed, now).unwrap_err().code,
            "ASSISTANT_AUTHORIZATION_MISMATCH"
        );
    }

    #[test]
    fn request_handle_expires_after_five_minutes() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut store = AuthorizationStore::default();
        let handle = store.issue(binding(), now).unwrap();

        assert_eq!(
            store
                .consume(&handle, &binding(), now + Duration::from_secs(301))
                .unwrap_err()
                .code,
            "ASSISTANT_AUTHORIZATION_EXPIRED"
        );
    }
}
