use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use url::Url;

use super::redaction::{
    is_sensitive_name, redact_headers, redact_json, redact_url, sensitive_json_paths,
    REDACTED_VALUE,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum CurlBody {
    Empty,
    Json(serde_json::Value),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurlRequest {
    pub method: String,
    pub url: Url,
    pub query: Vec<(String, String)>,
    pub headers: BTreeMap<String, String>,
    pub body: CurlBody,
    pub follow_redirects: bool,
    pub tls_verification: bool,
    pub timeout_seconds: Option<u64>,
    pub sensitive_fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExecutableCurlRequest {
    pub method: String,
    pub url: Url,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub follow_redirects: bool,
    pub tls_verification: bool,
    pub timeout_seconds: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CurlOperation {
    pub raw: ExecutableCurlRequest,
    pub projection: CurlRequest,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurlParseError {
    pub position: usize,
    pub message: String,
}

pub fn parse_curl(input: &str) -> Result<CurlRequest, CurlParseError> {
    Ok(parse_curl_operation(input)?.projection)
}

pub fn parse_curl_operation(input: &str) -> Result<CurlOperation, CurlParseError> {
    let tokens = tokenize(input)?;
    if tokens.first().map(String::as_str) != Some("curl") {
        return Err(parse_error(0, "输入必须以 curl 开头"));
    }

    let mut method = None;
    let mut url = None;
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    let mut data = None;
    let mut follow_redirects = false;
    let mut tls_verification = true;
    let mut timeout_seconds = None;
    let mut description = String::new();
    let mut index = 1;
    while index < tokens.len() {
        let token = &tokens[index];
        match token.as_str() {
            "-X" | "--request" => {
                method = Some(next_value(&tokens, &mut index, token)?.to_uppercase())
            }
            "-H" | "--header" => {
                let header = next_value(&tokens, &mut index, token)?;
                let Some((name, value)) = header.split_once(':') else {
                    return Err(parse_error(index, "请求头必须使用 Name: Value 格式"));
                };
                headers.insert(name.trim().into(), value.trim().into());
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode" => {
                data = Some(next_value(&tokens, &mut index, token)?);
            }
            "--url" => url = Some(next_value(&tokens, &mut index, token)?),
            "-b" | "--cookie" => {
                headers.insert("Cookie".into(), next_value(&tokens, &mut index, token)?);
            }
            "--compressed" | "--http1.1" | "--http2" => {}
            "-L" | "--location" => follow_redirects = true,
            "-k" | "--insecure" => tls_verification = false,
            "--max-time" => {
                let value = next_value(&tokens, &mut index, token)?;
                timeout_seconds = Some(
                    value
                        .parse()
                        .map_err(|_| parse_error(index, "--max-time 必须是整数秒数"))?,
                );
            }
            value if value.starts_with("http://") || value.starts_with("https://") => {
                url = Some(value.to_owned());
            }
            value if value.starts_with('-') => {
                return Err(parse_error(index, format!("暂不支持参数 {value}")));
            }
            _ if url.is_some() => {
                description = tokens[index..].join(" ");
                break;
            }
            value => return Err(parse_error(index, format!("无法识别参数 {value}"))),
        }
        index += 1;
    }

    let url = url.ok_or_else(|| parse_error(input.len(), "缺少请求 URL"))?;
    let url = Url::parse(&url).map_err(|error| parse_error(0, format!("URL 无效: {error}")))?;
    let query = url
        .query_pairs()
        .map(|(name, value)| {
            let sensitive = is_sensitive_name(&name);
            (
                name.into_owned(),
                if sensitive {
                    REDACTED_VALUE.into()
                } else {
                    value.into_owned()
                },
            )
        })
        .collect();
    let mut sensitive_fields = headers
        .keys()
        .filter(|name| is_sensitive_name(name))
        .map(|name| format!("header.{}", name.to_lowercase()))
        .collect::<Vec<_>>();
    sensitive_fields.extend(
        url.query_pairs()
            .filter(|(name, _)| is_sensitive_name(name))
            .map(|(name, _)| format!("query.{name}")),
    );
    let method = method.unwrap_or_else(|| {
        if data.is_none() {
            "GET".into()
        } else {
            "POST".into()
        }
    });
    let body = match data.as_deref() {
        Some(data) => match serde_json::from_str::<serde_json::Value>(data) {
            Ok(json) => {
                sensitive_fields.extend(
                    sensitive_json_paths(&json)
                        .into_iter()
                        .map(|path| format!("body.{path}")),
                );
                CurlBody::Json(redact_json(&json))
            }
            Err(_) => {
                sensitive_fields.push("body.text".into());
                CurlBody::Text(format!("[OMITTED TEXT BODY: {} bytes]", data.len()))
            }
        },
        None => CurlBody::Empty,
    };

    Ok(CurlOperation {
        raw: ExecutableCurlRequest {
            method: method.clone(),
            url: url.clone(),
            headers: headers.clone(),
            body: data.unwrap_or_default().into_bytes(),
            follow_redirects,
            tls_verification,
            timeout_seconds,
        },
        projection: CurlRequest {
            method,
            url: redact_url(&url),
            query,
            headers: redact_headers(&headers),
            body,
            follow_redirects,
            tls_verification,
            timeout_seconds,
            sensitive_fields,
        },
        description,
    })
}

fn next_value(
    tokens: &[String],
    index: &mut usize,
    option: &str,
) -> Result<String, CurlParseError> {
    *index += 1;
    tokens
        .get(*index)
        .cloned()
        .ok_or_else(|| parse_error(*index, format!("{option} 缺少参数值")))
}

fn tokenize(input: &str) -> Result<Vec<String>, CurlParseError> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut token_start = 0;
    let mut escaped = false;
    for (position, character) in input.char_indices() {
        if escaped {
            if character != '\n' {
                token.push(character);
            }
            escaped = false;
            continue;
        }
        match (quote, character) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), character) | (Some('"'), character) => token.push(character),
            (None, '\'') | (None, '"') => {
                if token.is_empty() {
                    token_start = position;
                }
                quote = Some(character);
            }
            (None, '\\') => escaped = true,
            (None, character) if character.is_whitespace() => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            (None, character) => {
                if token.is_empty() {
                    token_start = position;
                }
                token.push(character);
            }
            _ => unreachable!(),
        }
    }
    if quote.is_some() {
        return Err(parse_error(token_start, "存在未闭合的引号"));
    }
    if escaped {
        return Err(parse_error(
            input.len().saturating_sub(1),
            "反斜杠后缺少内容",
        ));
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn parse_error(position: usize, message: impl Into<String>) -> CurlParseError {
    CurlParseError {
        position,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_curl, parse_curl_operation, CurlBody};

    #[test]
    fn parses_method_headers_query_and_json_without_executing_curl() {
        let parsed = parse_curl(concat!(
            "curl 'https://api.example.com/users?active=true' -X POST",
            " -H 'Authorization: Bearer top-secret'",
            " -H 'Content-Type: application/json'",
            " --data '{\"password\":\"secret\",\"name\":\"Ada\"}'"
        ))
        .unwrap();

        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.url.host_str(), Some("api.example.com"));
        assert_eq!(parsed.query, vec![("active".into(), "true".into())]);
        assert!(parsed
            .sensitive_fields
            .contains(&"header.authorization".into()));
        assert!(parsed.sensitive_fields.contains(&"body.password".into()));
        assert_eq!(parsed.headers["Authorization"], "[REDACTED]");
        assert_eq!(
            parsed.body,
            CurlBody::Json(serde_json::json!({"password": "[REDACTED]", "name": "Ada"}))
        );
    }

    #[test]
    fn reports_the_token_position_for_an_unterminated_quote() {
        let error = parse_curl("curl 'https://api.example.com").unwrap_err();

        assert_eq!(error.position, 5);
        assert!(error.message.contains("引号"));
    }

    #[test]
    fn operation_keeps_raw_values_only_in_the_non_serializable_request() {
        let operation = parse_curl_operation(concat!(
            "curl 'https://api.example.com/login?access_token=query-secret&page=2' ",
            "-H 'Authorization: Bearer raw-secret' ",
            "--data '{\"password\":\"hunter2\"}'"
        ))
        .unwrap();

        assert_eq!(operation.raw.headers["Authorization"], "Bearer raw-secret");
        assert!(operation.raw.url.as_str().contains("query-secret"));
        assert_eq!(operation.raw.body, br#"{"password":"hunter2"}"#);
        let projection = serde_json::to_string(&operation.projection).unwrap();
        assert!(!projection.contains("raw-secret"));
        assert!(!projection.contains("hunter2"));
        assert!(!projection.contains("query-secret"));
        assert!(projection.contains("query.access_token"));
    }

    #[test]
    fn parses_chrome_curl_options_cookie_and_trailing_description() {
        let operation = parse_curl_operation(concat!(
            "curl --url 'https://api.example.com/login' \\\n",
            "  -H 'accept: application/json' \\\n",
            "  -b 'session=raw-secret; theme=light' \\\n",
            "  --data-raw '{\"name\":\"Ada\"}' --compressed --http2 ",
            "请分析为什么登录失败"
        ))
        .unwrap();

        assert_eq!(operation.raw.method, "POST");
        assert_eq!(
            operation.raw.headers["Cookie"],
            "session=raw-secret; theme=light"
        );
        assert_eq!(operation.description, "请分析为什么登录失败");
        assert_eq!(operation.projection.headers["Cookie"], "[REDACTED]");
    }

    #[test]
    fn treats_text_on_the_next_line_as_description() {
        let operation = parse_curl_operation(
            "curl 'https://api.example.com/users' --http1.1\n帮我检查这个接口",
        )
        .unwrap();

        assert_eq!(operation.description, "帮我检查这个接口");
    }

    #[test]
    fn still_rejects_unknown_options_instead_of_treating_them_as_description() {
        let error =
            parse_curl_operation("curl 'https://api.example.com/users' --unknown-option 帮我分析")
                .unwrap_err();

        assert!(error.message.contains("--unknown-option"));
    }
}
