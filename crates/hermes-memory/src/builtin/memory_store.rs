//! Built-in memory store - MEMORY.md/USER.md file storage

use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use once_cell::sync::Lazy;
use regex::Regex;

const MEMORY_LIMIT: usize = 2200;
const USER_LIMIT: usize = 1375;
const DELIMITER: &str = "\n§\n";

// Injection patterns
static INJECTION_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"[\x{200B}-\x{200F}]").unwrap(),
        Regex::new(r"(?i)ignore[_\s]+previous").unwrap(),
        Regex::new(r"(?i)disregard[_\s]+all").unwrap(),
    ]
});

static EXFIL_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"curl.*\$\{?[A-Z_]+}?").unwrap(),
        Regex::new(r"wget.*\$\{?[A-Z_]+}?").unwrap(),
    ]
});

pub struct MemoryStore {
    memory_path: PathBuf,
    user_path: PathBuf,
    memory: RwLock<String>,
    user: RwLock<String>,
    snapshot: RwLock<String>,
}

impl MemoryStore {
    pub fn new(home_dir: &std::path::Path) -> Result<Self, std::io::Error> {
        let memory_path = home_dir.join("MEMORY.md");
        let user_path = home_dir.join("USER.md");

        if !memory_path.exists() {
            fs::write(&memory_path, "§\n")?;
        }
        if !user_path.exists() {
            fs::write(&user_path, "§\n")?;
        }

        let memory = fs::read_to_string(&memory_path).unwrap_or_else(|_| "§\n".to_string());
        let user = fs::read_to_string(&user_path).unwrap_or_else(|_| "§\n".to_string());

        Ok(Self {
            memory_path,
            user_path,
            memory: RwLock::new(memory),
            user: RwLock::new(user),
            snapshot: RwLock::new(String::new()),
        })
    }

    pub fn load(&self) -> Result<(), String> {
        let memory = fs::read_to_string(&self.memory_path).map_err(|e| e.to_string())?;
        let user = fs::read_to_string(&self.user_path).map_err(|e| e.to_string())?;

        *self.memory.write().map_err(|_| "Lock poisoned")? = memory;
        *self.user.write().map_err(|_| "Lock poisoned")? = user;
        self.update_snapshot();
        Ok(())
    }

    fn update_snapshot(&self) {
        let memory = self.memory.read().ok();
        let user = self.user.read().ok();
        if let (Some(m), Some(u)) = (memory, user) {
            let snapshot = format!("{}\n§\n{}", m.trim(), u.trim());
            if let Ok(mut guard) = self.snapshot.write() {
                *guard = snapshot;
            }
        }
    }

    pub fn get_snapshot(&self) -> String {
        self.snapshot.read().ok().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn add(&self, entry: &str, memory_type: MemoryType) -> Result<(), String> {
        self.scan_entry(entry)?;

        let path = match memory_type {
            MemoryType::Memory => &self.memory_path,
            MemoryType::User => &self.user_path,
        };

        let limit = match memory_type {
            MemoryType::Memory => MEMORY_LIMIT,
            MemoryType::User => USER_LIMIT,
        };

        let mut content = fs::read_to_string(path).map_err(|e| e.to_string())?;

        if content.len() + entry.len() > limit {
            return Err(format!("{} limit exceeded ({} chars)", memory_type, limit));
        }

        if content.contains(entry) {
            return Ok(());
        }

        if !content.ends_with(DELIMITER) {
            content.push_str(DELIMITER);
        }
        content.push_str(entry);

        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &content).map_err(|e| e.to_string())?;
        fs::rename(&temp_path, path).map_err(|e| e.to_string())?;

        match memory_type {
            MemoryType::Memory => *self.memory.write().map_err(|_| "Lock poisoned")? = content,
            MemoryType::User => *self.user.write().map_err(|_| "Lock poisoned")? = content,
        }
        self.update_snapshot();
        Ok(())
    }

    pub fn remove(&self, entry: &str, memory_type: MemoryType) -> Result<(), String> {
        let path = match memory_type {
            MemoryType::Memory => &self.memory_path,
            MemoryType::User => &self.user_path,
        };

        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        if !content.contains(entry) {
            return Err("Entry not found".to_string());
        }

        let new_content = content.replace(entry, "").replace("\n§\n§\n", "\n§\n");
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &new_content).map_err(|e| e.to_string())?;
        fs::rename(&temp_path, path).map_err(|e| e.to_string())?;

        match memory_type {
            MemoryType::Memory => *self.memory.write().map_err(|_| "Lock poisoned")? = new_content.clone(),
            MemoryType::User => *self.user.write().map_err(|_| "Lock poisoned")? = new_content.clone(),
        }
        self.update_snapshot();
        Ok(())
    }

    fn scan_entry(&self, entry: &str) -> Result<(), String> {
        for pattern in INJECTION_PATTERNS.iter() {
            if pattern.is_match(entry) {
                return Err("Injection pattern detected".to_string());
            }
        }
        for pattern in EXFIL_PATTERNS.iter() {
            if pattern.is_match(entry) {
                return Err("Exfiltration pattern detected".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryType {
    Memory,
    User,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Memory => write!(f, "memory"),
            MemoryType::User => write!(f, "user"),
        }
    }
}