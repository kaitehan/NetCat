use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    body::decode_body_text,
    error::AppError,
    models::{MockRule, TrafficDetail, TrafficDraft, TrafficFilter, TrafficSummary},
    rules::validate_rule,
};

pub struct Store {
    connection: Mutex<Connection>,
    max_entries: usize,
    max_body_bytes: usize,
}

impl Store {
    pub fn open(path: &Path, max_entries: usize, max_body_bytes: usize) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                AppError::new("STORAGE_ERROR", "无法创建应用数据目录")
                    .with_detail(error.to_string())
            })?;
        }
        let connection = Connection::open(path)?;
        Self::from_connection(connection, max_entries, max_body_bytes)
    }

    pub fn open_in_memory(max_entries: usize, max_body_bytes: usize) -> Result<Self, AppError> {
        Self::from_connection(Connection::open_in_memory()?, max_entries, max_body_bytes)
    }

    fn from_connection(
        connection: Connection,
        max_entries: usize,
        max_body_bytes: usize,
    ) -> Result<Self, AppError> {
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY);
             INSERT OR IGNORE INTO schema_migrations(version) VALUES (1);
             CREATE TABLE IF NOT EXISTS exchanges(
               id TEXT PRIMARY KEY,
               started_at TEXT NOT NULL,
               method TEXT NOT NULL,
               url TEXT NOT NULL,
               host TEXT NOT NULL,
               status INTEGER,
               duration_ms INTEGER,
               request_headers TEXT NOT NULL,
               response_headers TEXT NOT NULL,
               request_body BLOB NOT NULL,
               response_body BLOB NOT NULL,
               request_original_bytes INTEGER NOT NULL,
               response_original_bytes INTEGER NOT NULL,
               content_type TEXT,
               mocked INTEGER NOT NULL,
               truncated INTEGER NOT NULL,
               error TEXT
             );
             CREATE INDEX IF NOT EXISTS exchanges_started_at_idx ON exchanges(started_at DESC);
             CREATE TABLE IF NOT EXISTS mock_rules(
               id TEXT PRIMARY KEY,
               payload TEXT NOT NULL,
               priority INTEGER NOT NULL,
               created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
            max_entries: max_entries.max(1),
            max_body_bytes: max_body_bytes.max(1),
        })
    }

    pub fn insert_exchange(&self, draft: TrafficDraft) -> Result<(), AppError> {
        let request_original_bytes = draft.request_body.len();
        let response_original_bytes = draft.response_body.len();
        let request_body = truncate(draft.request_body, self.max_body_bytes);
        let response_body = truncate(draft.response_body, self.max_body_bytes);
        let truncated = request_body.len() < request_original_bytes
            || response_body.len() < response_original_bytes;
        let request_headers = serde_json::to_string(&draft.request_headers).map_err(|error| {
            AppError::new("SERIALIZE_ERROR", "无法保存请求头").with_detail(error.to_string())
        })?;
        let response_headers = serde_json::to_string(&draft.response_headers).map_err(|error| {
            AppError::new("SERIALIZE_ERROR", "无法保存响应头").with_detail(error.to_string())
        })?;
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO exchanges(
                id, started_at, method, url, host, status, duration_ms,
                request_headers, response_headers, request_body, response_body,
                request_original_bytes, response_original_bytes, content_type,
                mocked, truncated, error
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                draft.id,
                draft.started_at,
                draft.method,
                draft.url,
                draft.host,
                draft.status,
                draft.duration_ms.map(to_db_integer).transpose()?,
                request_headers,
                response_headers,
                request_body,
                response_body,
                to_db_integer(request_original_bytes as u64)?,
                to_db_integer(response_original_bytes as u64)?,
                draft.content_type,
                draft.mocked,
                truncated,
                draft.error,
            ],
        )?;
        transaction.execute(
            "DELETE FROM exchanges WHERE id IN (
               SELECT id FROM exchanges ORDER BY rowid DESC LIMIT -1 OFFSET ?1
             )",
            params![to_db_integer(self.max_entries as u64)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_traffic(
        &self,
        filter: TrafficFilter,
        cursor: Option<String>,
        limit: usize,
    ) -> Result<Vec<TrafficSummary>, AppError> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, started_at, method, url, host, status, duration_ms,
                    response_original_bytes, content_type, mocked, truncated, error
             FROM exchanges
             WHERE (?1 IS NULL OR lower(url) LIKE '%' || lower(?1) || '%' OR lower(host) LIKE '%' || lower(?1) || '%')
               AND (?2 IS NULL OR method = ?2)
               AND (?3 IS NULL OR status / 100 = ?3)
               AND (?4 IS NULL OR host = ?4)
               AND (?5 IS NULL OR rowid < (SELECT rowid FROM exchanges WHERE id = ?5))
             ORDER BY rowid DESC LIMIT ?6"
        )?;
        let rows = statement.query_map(
            params![
                filter.query,
                filter.method,
                filter.status_group,
                filter.host,
                cursor,
                limit.min(500) as i64
            ],
            |row| {
                let duration_ms: Option<i64> = row.get(6)?;
                let response_bytes: i64 = row.get(7)?;
                Ok(TrafficSummary {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    method: row.get(2)?,
                    url: row.get(3)?,
                    host: row.get(4)?,
                    status: row.get(5)?,
                    duration_ms: duration_ms.map(|value| value as u64),
                    response_bytes: response_bytes as u64,
                    content_type: row.get(8)?,
                    mocked: row.get(9)?,
                    truncated: row.get(10)?,
                    error: row.get(11)?,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_traffic_detail(&self, id: &str) -> Result<TrafficDetail, AppError> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection
            .query_row(
                "SELECT id, started_at, method, url, host, status, duration_ms,
                    response_original_bytes, content_type, mocked, truncated, error,
                    request_headers, response_headers, request_body, response_body,
                    request_original_bytes
             FROM exchanges WHERE id = ?1",
                params![id],
                |row| {
                    let request_headers_json: String = row.get(12)?;
                    let response_headers_json: String = row.get(13)?;
                    let duration_ms: Option<i64> = row.get(6)?;
                    let response_original_bytes: i64 = row.get(7)?;
                    let request_original_bytes: i64 = row.get(16)?;
                    let request_headers =
                        serde_json::from_str(&request_headers_json).unwrap_or_default();
                    let response_headers =
                        serde_json::from_str(&response_headers_json).unwrap_or_default();
                    let request_body: Vec<u8> = row.get(14)?;
                    let response_body: Vec<u8> = row.get(15)?;
                    Ok(TrafficDetail {
                        summary: TrafficSummary {
                            id: row.get(0)?,
                            started_at: row.get(1)?,
                            method: row.get(2)?,
                            url: row.get(3)?,
                            host: row.get(4)?,
                            status: row.get(5)?,
                            duration_ms: duration_ms.map(|value| value as u64),
                            response_bytes: response_original_bytes as u64,
                            content_type: row.get(8)?,
                            mocked: row.get(9)?,
                            truncated: row.get(10)?,
                            error: row.get(11)?,
                        },
                        request_body_text: decode_body_text(&request_body, &request_headers),
                        response_body_text: decode_body_text(&response_body, &response_headers),
                        request_headers,
                        response_headers,
                        request_body,
                        response_body,
                        request_original_bytes: request_original_bytes as u64,
                        response_original_bytes: response_original_bytes as u64,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::new("NOT_FOUND", "未找到该流量记录"))
    }

    pub fn clear_traffic(&self) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute("DELETE FROM exchanges", [])?;
        Ok(())
    }

    pub fn save_mock_rule(&self, rule: &MockRule) -> Result<(), AppError> {
        validate_rule(rule)?;
        let payload = serde_json::to_string(rule).map_err(|error| {
            AppError::new("SERIALIZE_ERROR", "无法保存 Mock 规则").with_detail(error.to_string())
        })?;
        self.connection.lock().expect("store mutex poisoned").execute(
            "INSERT OR REPLACE INTO mock_rules(id, payload, priority, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![rule.id, payload, rule.priority, rule.created_at],
        )?;
        Ok(())
    }

    pub fn list_mock_rules(&self) -> Result<Vec<MockRule>, AppError> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection
            .prepare("SELECT payload FROM mock_rules ORDER BY priority DESC, created_at ASC")?;
        let payloads = statement.query_map([], |row| row.get::<_, String>(0))?;
        payloads
            .map(|payload| {
                let payload = payload?;
                serde_json::from_str(&payload).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        payload.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_mock_rule(&self, id: &str) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute("DELETE FROM mock_rules WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_mock_enabled(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        let mut rule = self
            .list_mock_rules()?
            .into_iter()
            .find(|rule| rule.id == id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "未找到该 Mock 规则"))?;
        rule.enabled = enabled;
        self.save_mock_rule(&rule)
    }

    pub fn list_all_details(&self) -> Result<Vec<TrafficDetail>, AppError> {
        let mut details = Vec::new();
        let mut cursor = None;
        loop {
            let rows = self.list_traffic(TrafficFilter::default(), cursor.take(), 500)?;
            if rows.is_empty() {
                break;
            }
            cursor = rows.last().map(|row| row.id.clone());
            for row in rows {
                details.push(self.get_traffic_detail(&row.id)?);
            }
        }
        Ok(details)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute(
                "INSERT OR REPLACE INTO app_settings(key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute("DELETE FROM app_settings WHERE key = ?1", params![key])?;
        Ok(())
    }
}

fn truncate(mut body: Vec<u8>, limit: usize) -> Vec<u8> {
    body.truncate(limit);
    body
}

fn to_db_integer(value: u64) -> Result<i64, AppError> {
    i64::try_from(value)
        .map_err(|_| AppError::new("VALUE_TOO_LARGE", "数值超过本地数据库可保存范围"))
}
