use std::{collections::HashSet, path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    body::decode_body_text,
    error::AppError,
    models::{
        MockRule, PairedDevice, TrafficDetail, TrafficDraft, TrafficFilter, TrafficOrigin,
        TrafficSummary,
    },
    rules::validate_rule,
};

pub struct Store {
    connection: Mutex<Connection>,
    deleted_device_tokens: Mutex<HashSet<String>>,
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
               source_exchange_id TEXT,
               matched_mock_rule_id TEXT,
               origin TEXT NOT NULL DEFAULT 'computer',
               device_token TEXT,
               device_name TEXT,
               error TEXT
             );
             CREATE INDEX IF NOT EXISTS exchanges_started_at_idx ON exchanges(started_at DESC);
             CREATE TABLE IF NOT EXISTS mock_rules(
               id TEXT PRIMARY KEY,
               payload TEXT NOT NULL,
               priority INTEGER NOT NULL,
               created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS mock_rule_stats(
               rule_id TEXT PRIMARY KEY,
               hit_count INTEGER NOT NULL DEFAULT 0,
               last_hit_at TEXT,
               last_exchange_id TEXT
             );
             CREATE TABLE IF NOT EXISTS app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS paired_devices(
               device_token TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               bound_ip TEXT NOT NULL UNIQUE,
               created_at TEXT NOT NULL,
               last_seen_at TEXT NOT NULL
             );
             INSERT OR IGNORE INTO schema_migrations(version) VALUES (2);
             INSERT OR IGNORE INTO schema_migrations(version) VALUES (3);",
        )?;
        ensure_column(&connection, "exchanges", "source_exchange_id", "TEXT")?;
        ensure_column(&connection, "exchanges", "matched_mock_rule_id", "TEXT")?;
        ensure_column(
            &connection,
            "exchanges",
            "origin",
            "TEXT NOT NULL DEFAULT 'computer'",
        )?;
        ensure_column(&connection, "exchanges", "device_token", "TEXT")?;
        ensure_column(&connection, "exchanges", "device_name", "TEXT")?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS exchanges_device_token_idx ON exchanges(device_token)",
            [],
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
            deleted_device_tokens: Mutex::new(HashSet::new()),
            max_entries: max_entries.max(1),
            max_body_bytes: max_body_bytes.max(1),
        })
    }

    pub fn insert_exchange(&self, draft: TrafficDraft) -> Result<(), AppError> {
        self.insert_exchanges(vec![draft])
    }

    pub fn insert_exchanges(&self, mut drafts: Vec<TrafficDraft>) -> Result<(), AppError> {
        let deleted = self
            .deleted_device_tokens
            .lock()
            .expect("deleted device token mutex poisoned");
        drafts.retain(|draft| {
            draft
                .device_token
                .as_ref()
                .is_none_or(|token| !deleted.contains(token))
        });
        drop(deleted);
        if drafts.is_empty() {
            return Ok(());
        }
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        for draft in drafts {
            let request_original_bytes = draft.request_body.len();
            let response_original_bytes = draft.response_body.len();
            let request_body = truncate(draft.request_body, self.max_body_bytes);
            let response_body = truncate(draft.response_body, self.max_body_bytes);
            let truncated = request_body.len() < request_original_bytes
                || response_body.len() < response_original_bytes;
            let request_headers =
                serde_json::to_string(&draft.request_headers).map_err(|error| {
                    AppError::new("SERIALIZE_ERROR", "无法保存请求头")
                        .with_detail(error.to_string())
                })?;
            let response_headers =
                serde_json::to_string(&draft.response_headers).map_err(|error| {
                    AppError::new("SERIALIZE_ERROR", "无法保存响应头")
                        .with_detail(error.to_string())
                })?;
            transaction.execute(
                "INSERT OR REPLACE INTO exchanges(
                    id, started_at, method, url, host, status, duration_ms,
                    request_headers, response_headers, request_body, response_body,
                    request_original_bytes, response_original_bytes, content_type,
                    mocked, truncated, source_exchange_id, matched_mock_rule_id,
                    origin, device_token, device_name, error
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
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
                    draft.source_exchange_id,
                    draft.matched_mock_rule_id,
                    traffic_origin_name(draft.origin),
                    draft.device_token,
                    draft.device_name,
                    draft.error,
                ],
            )?;
        }
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
                    response_original_bytes, content_type, mocked, truncated,
                    source_exchange_id, matched_mock_rule_id, origin, device_token, device_name, error
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
                    source_exchange_id: row.get(11)?,
                    matched_mock_rule_id: row.get(12)?,
                    origin: parse_traffic_origin(row.get::<_, String>(13)?.as_str()),
                    device_token: row.get(14)?,
                    device_name: row.get(15)?,
                    error: row.get(16)?,
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
                    response_original_bytes, content_type, mocked, truncated,
                    source_exchange_id, matched_mock_rule_id, origin, device_token, device_name, error,
                    request_headers, response_headers, request_body, response_body,
                    request_original_bytes
             FROM exchanges WHERE id = ?1",
                params![id],
                |row| {
                    let request_headers_json: String = row.get(17)?;
                    let response_headers_json: String = row.get(18)?;
                    let duration_ms: Option<i64> = row.get(6)?;
                    let response_original_bytes: i64 = row.get(7)?;
                    let request_original_bytes: i64 = row.get(21)?;
                    let request_headers =
                        serde_json::from_str(&request_headers_json).unwrap_or_default();
                    let response_headers =
                        serde_json::from_str(&response_headers_json).unwrap_or_default();
                    let request_body: Vec<u8> = row.get(19)?;
                    let response_body: Vec<u8> = row.get(20)?;
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
                            source_exchange_id: row.get(11)?,
                            matched_mock_rule_id: row.get(12)?,
                            origin: parse_traffic_origin(row.get::<_, String>(13)?.as_str()),
                            device_token: row.get(14)?,
                            device_name: row.get(15)?,
                            error: row.get(16)?,
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

    pub fn next_device_name(&self) -> Result<String, AppError> {
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        let current = transaction
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'device_name_sequence'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let next = current + 1;
        transaction.execute(
            "INSERT OR REPLACE INTO app_settings(key, value) VALUES ('device_name_sequence', ?1)",
            params![next.to_string()],
        )?;
        transaction.commit()?;
        Ok(format!("移动设备 {next}"))
    }

    pub fn save_paired_device(&self, device: &PairedDevice) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute(
                "INSERT OR REPLACE INTO paired_devices(device_token, name, bound_ip, created_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    device.device_token,
                    device.name,
                    device.bound_ip,
                    device.created_at,
                    device.last_seen_at
                ],
            )?;
        Ok(())
    }

    pub fn save_paired_devices(&self, devices: &[PairedDevice]) -> Result<(), AppError> {
        if devices.is_empty() {
            return Ok(());
        }
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        for device in devices {
            transaction.execute(
                "INSERT OR REPLACE INTO paired_devices(device_token, name, bound_ip, created_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    device.device_token,
                    device.name,
                    device.bound_ip,
                    device.created_at,
                    device.last_seen_at
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_paired_devices(&self) -> Result<Vec<PairedDevice>, AppError> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT device_token, name, bound_ip, created_at, last_seen_at
             FROM paired_devices ORDER BY last_seen_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(PairedDevice {
                device_token: row.get(0)?,
                name: row.get(1)?,
                bound_ip: row.get(2)?,
                created_at: row.get(3)?,
                last_seen_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn historical_device(&self, device_token: &str) -> Result<Option<PairedDevice>, AppError> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection
            .query_row(
                "SELECT device_token, device_name, started_at
                 FROM exchanges
                 WHERE device_token = ?1
                 ORDER BY rowid DESC LIMIT 1",
                params![device_token],
                |row| {
                    let timestamp: String = row.get(2)?;
                    Ok(PairedDevice {
                        device_token: row.get(0)?,
                        name: row
                            .get::<_, Option<String>>(1)?
                            .unwrap_or_else(|| "历史设备".to_string()),
                        bound_ip: String::new(),
                        created_at: timestamp.clone(),
                        last_seen_at: timestamp,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn remove_paired_device(&self, device_token: &str) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute(
                "DELETE FROM paired_devices WHERE device_token = ?1",
                params![device_token],
            )?;
        Ok(())
    }

    pub fn rename_paired_device(&self, device_token: &str, name: &str) -> Result<(), AppError> {
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE paired_devices SET name = ?2 WHERE device_token = ?1",
            params![device_token, name],
        )?;
        let history_changed = transaction.execute(
            "UPDATE exchanges SET device_name = ?2 WHERE device_token = ?1",
            params![device_token, name],
        )?;
        if changed == 0 && history_changed == 0 {
            return Err(AppError::new("PAIRED_DEVICE_NOT_FOUND", "未找到已配对设备"));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn clear_paired_devices(&self) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute("DELETE FROM paired_devices", [])?;
        Ok(())
    }

    pub fn delete_device_data(&self, device_token: &str) -> Result<usize, AppError> {
        self.deleted_device_tokens
            .lock()
            .expect("deleted device token mutex poisoned")
            .insert(device_token.to_string());
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM paired_devices WHERE device_token = ?1",
            params![device_token],
        )?;
        let deleted = transaction.execute(
            "DELETE FROM exchanges WHERE device_token = ?1",
            params![device_token],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn quick_check(&self) -> Result<String, AppError> {
        let result = self
            .connection
            .lock()
            .expect("store mutex poisoned")
            .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))?;
        if result == "ok" {
            Ok(result)
        } else {
            Err(
                AppError::new("DATABASE_INTEGRITY_ERROR", "数据库完整性检查失败")
                    .with_detail(result)
                    .with_action("请先备份数据库，再使用非破坏性恢复入口"),
            )
        }
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
        let mut statement = connection.prepare(
            "SELECT r.payload, COALESCE(s.hit_count, 0), s.last_hit_at, s.last_exchange_id
             FROM mock_rules r LEFT JOIN mock_rule_stats s ON s.rule_id = r.id
             ORDER BY r.priority DESC, r.created_at ASC",
        )?;
        let payloads = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        payloads
            .map(|payload| {
                let (payload, hit_count, last_hit_at, last_exchange_id) = payload?;
                let mut rule: MockRule = serde_json::from_str(&payload).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        payload.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                rule.hit_count = hit_count as u64;
                rule.last_hit_at = last_hit_at;
                rule.last_exchange_id = last_exchange_id;
                Ok::<MockRule, rusqlite::Error>(rule)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_mock_rule(&self, id: &str) -> Result<(), AppError> {
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM mock_rule_stats WHERE rule_id = ?1",
            params![id],
        )?;
        transaction.execute("DELETE FROM mock_rules WHERE id = ?1", params![id])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn record_mock_hit(
        &self,
        rule_id: &str,
        exchange_id: &str,
        timestamp: &str,
    ) -> Result<(), AppError> {
        self.connection
            .lock()
            .expect("store mutex poisoned")
            .execute(
                "INSERT INTO mock_rule_stats(rule_id, hit_count, last_hit_at, last_exchange_id)
             VALUES (?1, 1, ?2, ?3)
             ON CONFLICT(rule_id) DO UPDATE SET
               hit_count = hit_count + 1,
               last_hit_at = excluded.last_hit_at,
               last_exchange_id = excluded.last_exchange_id",
                params![rule_id, timestamp, exchange_id],
            )?;
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

    pub fn reorder_mock_rules(&self, ordered_ids: &[String]) -> Result<(), AppError> {
        let rules = self.list_mock_rules()?;
        if ordered_ids.len() != rules.len()
            || !ordered_ids
                .iter()
                .all(|id| rules.iter().any(|rule| &rule.id == id))
        {
            return Err(AppError::new(
                "INVALID_MOCK_ORDER",
                "Mock 排序必须包含全部现有规则",
            ));
        }
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        for (index, id) in ordered_ids.iter().enumerate() {
            let mut rule = rules
                .iter()
                .find(|rule| &rule.id == id)
                .cloned()
                .expect("validated rule id");
            rule.priority = (ordered_ids.len() - index) as i64;
            let payload = serde_json::to_string(&rule).map_err(|error| {
                AppError::new("SERIALIZE_ERROR", "无法保存 Mock 排序")
                    .with_detail(error.to_string())
            })?;
            transaction.execute(
                "UPDATE mock_rules SET payload = ?1, priority = ?2 WHERE id = ?3",
                params![payload, rule.priority, id],
            )?;
        }
        transaction.commit()?;
        Ok(())
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

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    column_type: &str,
) -> Result<(), AppError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !columns.iter().any(|name| name == column) {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}"),
            [],
        )?;
    }
    Ok(())
}

fn truncate(mut body: Vec<u8>, limit: usize) -> Vec<u8> {
    body.truncate(limit);
    body
}

fn to_db_integer(value: u64) -> Result<i64, AppError> {
    i64::try_from(value)
        .map_err(|_| AppError::new("VALUE_TOO_LARGE", "数值超过本地数据库可保存范围"))
}

fn traffic_origin_name(origin: TrafficOrigin) -> &'static str {
    match origin {
        TrafficOrigin::Computer => "computer",
        TrafficOrigin::Mobile => "mobile",
        TrafficOrigin::Replay => "replay",
    }
}

fn parse_traffic_origin(value: &str) -> TrafficOrigin {
    match value {
        "mobile" => TrafficOrigin::Mobile,
        "replay" => TrafficOrigin::Replay,
        _ => TrafficOrigin::Computer,
    }
}
