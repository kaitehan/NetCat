use std::collections::BTreeMap;

use url::Url;

pub const REDACTED_VALUE: &str = "[REDACTED]";

pub fn is_sensitive_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "password"
            | "passwd"
            | "secret"
            | "apikey"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "clientsecret"
            | "privatekey"
    ) || normalized.ends_with("token")
        || normalized.ends_with("password")
        || normalized.ends_with("secret")
}

pub fn redact_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                if is_sensitive_name(name) {
                    REDACTED_VALUE.into()
                } else {
                    value.clone()
                },
            )
        })
        .collect()
}

pub fn redact_url(url: &Url) -> Url {
    let mut redacted = url.clone();
    if url.query().is_none() {
        return redacted;
    }
    redacted.set_query(None);
    {
        let mut query = redacted.query_pairs_mut();
        for (name, value) in url.query_pairs() {
            query.append_pair(
                &name,
                if is_sensitive_name(&name) {
                    REDACTED_VALUE
                } else {
                    &value
                },
            );
        }
    }
    redacted
}

pub fn redact_url_string(value: &str) -> String {
    Url::parse(value)
        .map(|url| redact_url(&url).to_string())
        .unwrap_or_else(|_| "[OMITTED INVALID URL]".into())
}

pub fn redact_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        if is_sensitive_name(name) {
                            serde_json::Value::String(REDACTED_VALUE.into())
                        } else {
                            redact_json(value)
                        },
                    )
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(redact_json).collect())
        }
        _ => value.clone(),
    }
}

pub fn sensitive_json_paths(value: &serde_json::Value) -> Vec<String> {
    fn visit(value: &serde_json::Value, path: &str, output: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(fields) => {
                for (name, value) in fields {
                    let next = if path.is_empty() {
                        name.clone()
                    } else {
                        format!("{path}.{name}")
                    };
                    if is_sensitive_name(name) {
                        output.push(next);
                    } else {
                        visit(value, &next, output);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, path, output);
                }
            }
            _ => {}
        }
    }

    let mut paths = Vec::new();
    visit(value, "", &mut paths);
    paths
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{redact_headers, redact_json, redact_url_string, REDACTED_VALUE};

    #[test]
    fn sensitive_headers_are_replaced_without_hiding_names() {
        let headers = BTreeMap::from([
            ("Authorization".into(), "Bearer secret".into()),
            ("Cookie".into(), "sid=secret".into()),
            ("Content-Type".into(), "application/json".into()),
        ]);

        let redacted = redact_headers(&headers);

        assert_eq!(redacted["Authorization"], REDACTED_VALUE);
        assert_eq!(redacted["Cookie"], REDACTED_VALUE);
        assert_eq!(redacted["Content-Type"], "application/json");
    }

    #[test]
    fn nested_sensitive_json_fields_are_recursively_redacted() {
        let value = serde_json::json!({
            "profile": {
                "password": "hunter2",
                "nested": [{ "accessToken": "abc" }, { "name": "visible" }]
            },
            "api_key": "key-value"
        });

        let redacted = redact_json(&value);

        assert_eq!(redacted["profile"]["password"], REDACTED_VALUE);
        assert_eq!(
            redacted["profile"]["nested"][0]["accessToken"],
            REDACTED_VALUE
        );
        assert_eq!(redacted["profile"]["nested"][1]["name"], "visible");
        assert_eq!(redacted["api_key"], REDACTED_VALUE);
    }

    #[test]
    fn sensitive_query_values_are_redacted_without_hiding_safe_filters() {
        let redacted = redact_url_string(
            "https://api.example.com/items?access_token=raw-secret&page=2&api_key=raw-key",
        );

        assert!(redacted.contains("access_token=%5BREDACTED%5D"));
        assert!(redacted.contains("api_key=%5BREDACTED%5D"));
        assert!(redacted.contains("page=2"));
        assert!(!redacted.contains("raw-secret"));
        assert!(!redacted.contains("raw-key"));
    }
}
