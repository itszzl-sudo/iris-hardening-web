use rusqlite::{Connection, params, OptionalExtension};
use std::sync::Mutex;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, rusqlite::Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfig {
    pub id: String,
    pub domain: String,
    pub nginx_port: u16,
    pub gateway_port: u16,
    pub gateway_host: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: ConfigStatus,
    pub nginx_config: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigStatus {
    Pending,
    Synced,
    Failed,
}

impl std::fmt::Display for ConfigStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigStatus::Pending => write!(f, "pending"),
            ConfigStatus::Synced => write!(f, "synced"),
            ConfigStatus::Failed => write!(f, "failed"),
        }
    }
}

impl From<&str> for ConfigStatus {
    fn from(s: &str) -> Self {
        match s {
            "synced" => ConfigStatus::Synced,
            "failed" => ConfigStatus::Failed,
            _ => ConfigStatus::Pending,
        }
    }
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS domain_configs (
                id TEXT PRIMARY KEY,
                domain TEXT NOT NULL UNIQUE,
                nginx_port INTEGER NOT NULL DEFAULT 80,
                gateway_port INTEGER NOT NULL DEFAULT 9001,
                gateway_host TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                nginx_config TEXT
            )",
            [],
        )?;

        // 添加 gateway_host 列（如果不存在）
        conn.execute(
            "ALTER TABLE domain_configs ADD COLUMN gateway_host TEXT",
            [],
        ).ok(); // 忽略错误（如果列已存在）

        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                config_id TEXT NOT NULL,
                action TEXT NOT NULL,
                result TEXT,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (config_id) REFERENCES domain_configs(id)
            )",
            [],
        )?;

        Ok(())
    }

    pub fn create_domain(&self, domain: &str, nginx_port: u16, gateway_port: u16, gateway_host: Option<&str>) -> Result<DomainConfig> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO domain_configs (id, domain, nginx_port, gateway_port, gateway_host, created_at, updated_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
            params![id, domain, nginx_port, gateway_port, gateway_host, now.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(DomainConfig {
            id,
            domain: domain.to_string(),
            nginx_port,
            gateway_port,
            gateway_host: gateway_host.map(String::from),
            created_at: now,
            updated_at: now,
            status: ConfigStatus::Pending,
            nginx_config: None,
        })
    }

    pub fn get_domain(&self, domain: &str) -> Result<Option<DomainConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, domain, nginx_port, gateway_port, gateway_host, created_at, updated_at, status, nginx_config
             FROM domain_configs WHERE domain = ?1"
        )?;

        let config = stmt.query_row(params![domain], |row| {
            let created_str: String = row.get(5)?;
            let updated_str: String = row.get(6)?;
            let status_str: String = row.get(7)?;

            Ok(DomainConfig {
                id: row.get(0)?,
                domain: row.get(1)?,
                nginx_port: row.get(2)?,
                gateway_port: row.get(3)?,
                gateway_host: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                status: ConfigStatus::from(status_str.as_str()),
                nginx_config: row.get(8)?,
            })
        }).optional()?;

        Ok(config)
    }

    pub fn get_all_domains(&self) -> Result<Vec<DomainConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, domain, nginx_port, gateway_port, gateway_host, created_at, updated_at, status, nginx_config
             FROM domain_configs ORDER BY created_at DESC"
        )?;

        let configs = stmt.query_map([], |row| {
            let created_str: String = row.get(5)?;
            let updated_str: String = row.get(6)?;
            let status_str: String = row.get(7)?;

            Ok(DomainConfig {
                id: row.get(0)?,
                domain: row.get(1)?,
                nginx_port: row.get(2)?,
                gateway_port: row.get(3)?,
                gateway_host: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                status: ConfigStatus::from(status_str.as_str()),
                nginx_config: row.get(8)?,
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(configs)
    }

    pub fn update_nginx_config(&self, domain: &str, nginx_config: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE domain_configs SET nginx_config = ?1, updated_at = ?2, status = 'synced' WHERE domain = ?3",
            params![nginx_config, now, domain],
        )?;

        Ok(())
    }

    pub fn update_status(&self, domain: &str, status: ConfigStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE domain_configs SET status = ?1, updated_at = ?2 WHERE domain = ?3",
            params![status.to_string(), now, domain],
        )?;

        Ok(())
    }

    pub fn delete_domain(&self, domain: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM domain_configs WHERE domain = ?1", params![domain])?;
        Ok(rows > 0)
    }

    pub fn log_sync(&self, config_id: &str, action: &str, result: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO sync_log (config_id, action, result, timestamp) VALUES (?1, ?2, ?3, ?4)",
            params![config_id, action, result, now],
        )?;

        Ok(())
    }
}