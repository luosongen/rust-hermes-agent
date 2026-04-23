use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;

use crate::hub::error::HubError;
use crate::hub::types::{Category, SkillIndexEntry, SkillSource};

#[derive(Clone)]
pub struct SkillIndex {
    pub conn: Arc<Mutex<Connection>>,
}

impl SkillIndex {
    pub fn new(db_path: PathBuf) -> Result<Self, HubError> {
        let conn = Connection::open(&db_path)?;
        let index = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        index.init_schema()?;
        Ok(index)
    }

    pub fn init_schema(&self) -> Result<(), HubError> {
        let conn = self.conn.lock();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                category TEXT NOT NULL,
                version TEXT DEFAULT '1.0.0',
                source_type TEXT NOT NULL,
                source_url TEXT,
                file_path TEXT NOT NULL,
                checksum TEXT,
                installed_at TEXT,
                updated_at TEXT,
                UNIQUE(category, name)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS categories (
                name TEXT PRIMARY KEY,
                description TEXT,
                icon TEXT,
                sort_order INTEGER DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hub_url TEXT NOT NULL,
                synced_at TEXT NOT NULL,
                skills_count INTEGER,
                status TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS trusted_skills (
                skill_id TEXT PRIMARY KEY,
                trusted_at TEXT NOT NULL,
                trusted_by TEXT
            )",
            [],
        )?;

        conn.execute("CREATE INDEX IF NOT EXISTS idx_skills_category ON skills(category)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name)", [])?;

        Ok(())
    }

    pub fn add_skill(&self, entry: &SkillIndexEntry) -> Result<(), HubError> {
        let conn = self.conn.lock();

        let (source_type, source_url) = match &entry.source {
            SkillSource::Local => ("local".to_string(), None),
            SkillSource::Remote { url } => ("remote".to_string(), Some(url.clone())),
            SkillSource::Git { url, branch } => {
                ("git".to_string(), Some(format!("{}#{}", url, branch)))
            }
        };

        conn.execute(
            "INSERT OR REPLACE INTO skills
             (id, name, description, category, version, source_type, source_url, file_path, checksum, installed_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                entry.id,
                entry.name,
                entry.description,
                entry.category,
                entry.version,
                source_type,
                source_url,
                entry.file_path,
                entry.checksum,
                entry.installed_at.to_rfc3339(),
                entry.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn get_skill(&self, id: &str) -> Result<Option<SkillIndexEntry>, HubError> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, name, description, category, version, source_type, source_url,
                    file_path, checksum, installed_at, updated_at
             FROM skills WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(self.row_to_entry(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_skills(&self) -> Result<Vec<SkillIndexEntry>, HubError> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, name, description, category, version, source_type, source_url,
                    file_path, checksum, installed_at, updated_at
             FROM skills ORDER BY category, name",
        )?;

        let mut entries = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            entries.push(self.row_to_entry(row)?);
        }

        Ok(entries)
    }

    pub fn list_skills_by_category(&self, category: &str) -> Result<Vec<SkillIndexEntry>, HubError> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, name, description, category, version, source_type, source_url,
                    file_path, checksum, installed_at, updated_at
             FROM skills WHERE category = ?1 ORDER BY name",
        )?;

        let mut entries = Vec::new();
        let mut rows = stmt.query(params![category])?;

        while let Some(row) = rows.next()? {
            entries.push(self.row_to_entry(row)?);
        }

        Ok(entries)
    }

    pub fn remove_skill(&self, id: &str) -> Result<(), HubError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM skills WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_categories(&self) -> Result<Vec<Category>, HubError> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT c.name, c.description, c.icon, c.sort_order,
                    (SELECT COUNT(*) FROM skills WHERE category = c.name) as skill_count
             FROM categories c
             ORDER BY c.sort_order, c.name",
        )?;

        let mut categories = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let description: String = row.get(1)?;
            let icon: Option<String> = row.get(2)?;
            let _sort_order: i64 = row.get(3)?;
            let skill_count: i64 = row.get(4)?;

            categories.push(Category {
                name,
                description,
                icon,
                skill_count: skill_count as usize,
            });
        }

        Ok(categories)
    }

    pub fn add_category(&self, cat: &Category) -> Result<(), HubError> {
        let conn = self.conn.lock();

        conn.execute(
            "INSERT OR REPLACE INTO categories (name, description, icon, sort_order)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                cat.name,
                cat.description,
                cat.icon,
                cat.skill_count as i64,
            ],
        )?;

        Ok(())
    }

    fn row_to_entry(&self, row: &rusqlite::Row) -> SqliteResult<SkillIndexEntry> {
        let source_type: String = row.get(5)?;
        let source_url: Option<String> = row.get(6)?;

        let source = match source_type.as_str() {
            "local" => SkillSource::Local,
            "remote" => SkillSource::Remote {
                url: source_url.unwrap_or_default(),
            },
            "git" => {
                if let Some(url) = source_url {
                    let parts: Vec<&str> = url.splitn(2, '#').collect();
                    if parts.len() == 2 {
                        SkillSource::Git {
                            url: parts[0].to_string(),
                            branch: parts[1].to_string(),
                        }
                    } else {
                        SkillSource::Git {
                            url: url,
                            branch: "main".to_string(),
                        }
                    }
                } else {
                    SkillSource::Local
                }
            }
            _ => SkillSource::Local,
        };

        let installed_at_str: String = row.get(9)?;
        let updated_at_str: String = row.get(10)?;

        let installed_at = DateTime::parse_from_rfc3339(&installed_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(SkillIndexEntry {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            category: row.get(3)?,
            version: row.get(4)?,
            source,
            file_path: row.get(7)?,
            checksum: row.get(8)?,
            installed_at,
            updated_at,
        })
    }
}
