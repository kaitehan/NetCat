use std::io::{Cursor, Write};

use serde_json::{json, Value};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{
    error::AppError,
    models::{DiagnosticLevel, DiagnosticLog},
};

pub fn build_diagnostic_archive(
    metadata: Value,
    logs: &[DiagnosticLog],
) -> Result<Vec<u8>, AppError> {
    let sanitized_logs = logs.iter().map(sanitize_log).collect::<Vec<_>>();
    let errors = logs
        .iter()
        .filter(|log| log.level == DiagnosticLevel::Error)
        .map(sanitize_log)
        .collect::<Vec<_>>();
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_json(&mut archive, "diagnostics.json", &metadata, options)?;
    write_json(&mut archive, "logs.json", &json!(sanitized_logs), options)?;
    write_json(&mut archive, "errors.json", &json!(errors), options)?;
    archive
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(zip_error)
}

fn sanitize_log(log: &DiagnosticLog) -> Value {
    json!({
        "timestamp": log.timestamp,
        "level": log.level,
        "scope": log.scope,
        "event": log.event,
        "message": sanitize_message(&log.message),
        "exchangeId": log.exchange_id,
    })
}

fn sanitize_message(message: &str) -> &str {
    let lowered = message.to_ascii_lowercase();
    if [
        "authorization",
        "cookie",
        "token",
        "request body",
        "response body",
        "private key",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
    {
        "[REDACTED]"
    } else {
        message
    }
}

fn write_json<W: Write + std::io::Seek>(
    archive: &mut ZipWriter<W>,
    name: &str,
    value: &Value,
    options: SimpleFileOptions,
) -> Result<(), AppError> {
    archive.start_file(name, options).map_err(zip_error)?;
    let body = serde_json::to_vec_pretty(value).map_err(|error| {
        AppError::new("DIAGNOSTIC_EXPORT_ERROR", "无法序列化诊断信息")
            .with_detail(error.to_string())
    })?;
    archive.write_all(&body).map_err(|error| {
        AppError::new("DIAGNOSTIC_EXPORT_ERROR", "无法生成诊断包").with_detail(error.to_string())
    })
}

fn zip_error(error: zip::result::ZipError) -> AppError {
    AppError::new("DIAGNOSTIC_EXPORT_ERROR", "无法生成诊断包").with_detail(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;
    use crate::models::DiagnosticScope;

    #[test]
    fn archive_omits_details_and_redacts_sensitive_messages() {
        let log = DiagnosticLog::new(
            DiagnosticLevel::Error,
            DiagnosticScope::System,
            "test.failed",
            "Authorization token leaked",
        )
        .with_detail("cookie", "secret")
        .with_detail("body", "private payload");
        let bytes = build_diagnostic_archive(json!({ "version": "0.3.0" }), &[log]).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut contents = String::new();
        for index in 0..archive.len() {
            archive
                .by_index(index)
                .unwrap()
                .read_to_string(&mut contents)
                .unwrap();
        }
        assert!(contents.contains("[REDACTED]"));
        for forbidden in ["secret", "private payload", "Authorization token"] {
            assert!(!contents.contains(forbidden));
        }
    }
}
