use std::time::Duration;

use futures_util::StreamExt;

use crate::error::AppError;

use super::settings::AssistantSettings;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

pub async fn test_openai_connection(
    settings: &AssistantSettings,
    key: &str,
) -> Result<String, AppError> {
    let mut endpoint = url::Url::parse(&settings.open_ai_base_url)
        .map_err(|_| AppError::new("ASSISTANT_OPENAI_BASE_URL_INVALID", "API Base URL 无效"))?;
    let model = settings
        .model
        .split_once('/')
        .map(|(_, model)| model)
        .unwrap_or(settings.model.as_str());
    let (protocol_path, protocol_label, payload) = match settings.open_ai_api.as_str() {
        "openai-responses" => (
            "responses",
            "Responses API",
            serde_json::json!({
                "model": model,
                "input": "Reply with OK.",
                "max_output_tokens": 1,
            }),
        ),
        "openai-completions" => (
            "chat/completions",
            "Chat Completions",
            serde_json::json!({
                "model": model,
                "messages": [{ "role": "user", "content": "Reply with OK." }],
                "max_tokens": 1,
                "stream": false,
            }),
        ),
        _ => {
            return Err(AppError::new(
                "ASSISTANT_OPENAI_API_INVALID",
                "OpenAI 兼容协议无效",
            ))
        }
    };
    let path = format!("{}/{protocol_path}", endpoint.path().trim_end_matches('/'));
    endpoint.set_path(&path);
    endpoint.set_query(None);
    endpoint.set_fragment(None);

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(connection_error)?;
    let response = client
        .post(endpoint)
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await
        .map_err(connection_error)?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::new(
            "ASSISTANT_CONNECTION_TEST_FAILED",
            "协议测试响应超过 1 MiB 限制",
        ));
    }
    let bytes = read_bounded_response(response).await?;
    if !status.is_success() {
        let detail = safe_error_detail(&bytes, key);
        let message = detail.map_or_else(
            || format!("协议测试请求失败（HTTP {}）", status.as_u16()),
            |detail| format!("协议测试请求失败（HTTP {}）：{detail}", status.as_u16()),
        );
        let mut error = AppError::new("ASSISTANT_CONNECTION_TEST_FAILED", message);
        error = match status.as_u16() {
            401 | 403 => error.with_action("请确认 API Key 权限、模型授权以及网关访问策略"),
            404 | 405 => error.with_action(
                "请确认网关支持当前 API 协议，或切换 Responses / Chat Completions 后重试",
            ),
            _ => error.with_action("请检查服务商状态和模型配置后重试"),
        };
        return Err(error);
    }

    let _: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        AppError::new(
            "ASSISTANT_CONNECTION_TEST_FAILED",
            "协议测试响应不是有效 JSON",
        )
        .with_detail(error.to_string())
    })?;
    Ok(format!(
        "连接成功，模型 {model} 已通过 {protocol_label} 测试。"
    ))
}

async fn read_bounded_response(response: reqwest::Response) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(connection_error)?;
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::new(
                "ASSISTANT_CONNECTION_TEST_FAILED",
                "协议测试响应超过 1 MiB 限制",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn safe_error_detail(bytes: &[u8], key: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let message = value
        .pointer("/error/message")
        .or_else(|| value.get("message"))?
        .as_str()?;
    let normalized = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    let redacted = normalized
        .replace(key, "[REDACTED]")
        .split_whitespace()
        .map(|word| {
            if word.starts_with("sk-") {
                "[REDACTED]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(redacted.chars().take(300).collect())
}

fn connection_error(error: impl std::fmt::Display) -> AppError {
    AppError::new(
        "ASSISTANT_CONNECTION_TEST_FAILED",
        "无法连接 OpenAI 兼容服务",
    )
    .with_detail(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::assistant::{
        connection::test_openai_connection, models::AssistantProvider, settings::AssistantSettings,
    };

    async fn serve_response(status: &'static str) -> (String, Arc<Mutex<String>>) {
        serve_response_with_body(status, r#"{"id":"test-response"}"#).await
    }

    async fn serve_response_with_body(
        status: &'static str,
        body: &'static str,
    ) -> (String, Arc<Mutex<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let request = Arc::new(Mutex::new(String::new()));
        let captured = Arc::clone(&request);
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0_u8; 4096];
            let count = stream.read(&mut bytes).await.unwrap();
            *captured.lock().unwrap() = String::from_utf8_lossy(&bytes[..count]).into_owned();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        (format!("http://{address}/v1"), request)
    }

    fn settings(base_url: String, api: &str) -> AssistantSettings {
        AssistantSettings {
            provider: AssistantProvider::OpenAi,
            model: "acme-gateway/acme-chat".into(),
            open_ai_provider: "acme-gateway".into(),
            open_ai_base_url: base_url,
            open_ai_api: api.into(),
        }
    }

    #[tokio::test]
    async fn tests_chat_completions_with_the_key_and_selected_model() {
        let (base_url, request) = serve_response("200 OK").await;

        let result =
            test_openai_connection(&settings(base_url, "openai-completions"), "sk-local-test")
                .await
                .unwrap();

        assert!(result.contains("Chat Completions"));
        let request = request.lock().unwrap();
        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-local-test"));
        assert!(request.contains(r#""model":"acme-chat""#));
    }

    #[tokio::test]
    async fn tests_the_selected_responses_protocol() {
        let (base_url, request) = serve_response("200 OK").await;

        let result =
            test_openai_connection(&settings(base_url, "openai-responses"), "sk-local-test")
                .await
                .unwrap();

        assert!(result.contains("Responses API"));
        assert!(request
            .lock()
            .unwrap()
            .starts_with("POST /v1/responses HTTP/1.1\r\n"));
    }

    #[tokio::test]
    async fn reports_protocol_http_failures() {
        let (base_url, _) = serve_response_with_body(
            "403 Forbidden",
            r#"{"error":{"message":"API key sk-secret-token has no access to this model"}}"#,
        )
        .await;

        let error =
            test_openai_connection(&settings(base_url, "openai-responses"), "sk-local-test")
                .await
                .unwrap_err();

        assert_eq!(error.code, "ASSISTANT_CONNECTION_TEST_FAILED");
        assert_eq!(
            error.message,
            "协议测试请求失败（HTTP 403）：API key [REDACTED] has no access to this model"
        );
        assert_eq!(
            error.action.as_deref(),
            Some("请确认 API Key 权限、模型授权以及网关访问策略")
        );
        assert!(!error.message.contains("sk-secret-token"));
    }
}
