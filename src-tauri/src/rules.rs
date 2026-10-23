use regex::Regex;

use crate::{
    error::AppError,
    models::{MatchKind, MockRule},
};

pub fn validate_rule(rule: &MockRule) -> Result<(), AppError> {
    if rule.name.trim().is_empty() {
        return Err(AppError::new("INVALID_NAME", "Mock 名称不能为空"));
    }
    if rule.url_pattern.is_empty() {
        return Err(AppError::new("INVALID_PATTERN", "URL 匹配内容不能为空"));
    }
    if !(100..=599).contains(&rule.status) {
        return Err(AppError::new(
            "INVALID_STATUS",
            "响应状态码必须在 100 到 599 之间",
        ));
    }
    if rule.delay_ms > 120_000 {
        return Err(AppError::new("INVALID_DELAY", "Mock 延迟不能超过 120 秒"));
    }
    if rule.methods.is_empty() {
        return Err(AppError::new("INVALID_METHOD", "至少选择一个 HTTP 方法"));
    }
    for (name, value) in &rule.response_headers {
        if name.trim().is_empty() || name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(AppError::new("INVALID_HEADER", "响应头包含非法字符"));
        }
    }
    if rule.match_kind == MatchKind::Regex {
        Regex::new(&rule.url_pattern).map_err(|error| {
            AppError::new("INVALID_REGEX", "URL 正则表达式无效").with_detail(error.to_string())
        })?;
    }
    Ok(())
}

pub fn select_mock<'a>(rules: &'a [MockRule], method: &str, url: &str) -> Option<&'a MockRule> {
    let mut candidates: Vec<&MockRule> = rules
        .iter()
        .filter(|rule| rule.enabled)
        .filter(|rule| {
            rule.methods
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(method))
        })
        .filter(|rule| matches_url(rule, url))
        .collect();

    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.created_at.cmp(&right.created_at))
    });
    candidates.into_iter().next()
}

fn matches_url(rule: &MockRule, url: &str) -> bool {
    match rule.match_kind {
        MatchKind::Exact => url == rule.url_pattern,
        MatchKind::Contains => url.contains(&rule.url_pattern),
        MatchKind::Regex => Regex::new(&rule.url_pattern).is_ok_and(|regex| regex.is_match(url)),
    }
}
