//! The "recent projects" list, persisted to
//! `<config-dir>/Rustle/recent.json` (on Windows: `%APPDATA%\Rustle\`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One entry in the recent-projects list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentEntry {
    pub name: String,
    pub path: String,
}

/// The recent-projects list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentProjects {
    pub entries: Vec<RecentEntry>,
}

const MAX_ENTRIES: usize = 15;

impl RecentProjects {
    /// Location of `recent.json`, if a config directory is known.
    pub fn file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("Rustle").join("recent.json"))
    }

    /// Load the list, returning an empty list if it is missing or corrupt.
    pub fn load() -> Self {
        Self::file_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Write the list back to disk (best effort — errors are ignored).
    pub fn save(&self) {
        let Some(path) = Self::file_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Move `path` to the front of the list (de-duplicating), cap the
    /// length, and persist.
    pub fn record(&mut self, name: &str, path: &str) {
        self.entries.retain(|e| e.path != path);
        self.entries.insert(
            0,
            RecentEntry {
                name: name.to_string(),
                path: path.to_string(),
            },
        );
        self.entries.truncate(MAX_ENTRIES);
        self.save();
    }
}
