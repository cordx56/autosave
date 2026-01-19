use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Configuration object
///
/// Config file is deserialized to this object
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    branch: String,
    commit_message: String,
    merge_message: String,
    delay: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            branch: "tmp/autosave".to_string(),
            commit_message: "auto save".to_string(),
            merge_message: "auto merge".to_string(),
            delay: 3,
        }
    }
}

impl Config {
    pub fn from_file_path(p: impl AsRef<Path>) -> Result<Self> {
        let s = fs::read_to_string(p.as_ref())
            .with_context(|| format!("Config file read error: {}", p.as_ref().display()))?;
        let c = toml::from_str(&s).context("Config file format error")?;
        Ok(c)
    }
    pub fn from_dir_path(p: impl AsRef<Path>, file_name: impl AsRef<Path>) -> Result<Self> {
        let mut path = fs::canonicalize(p).context("Failed to get absolute path")?;
        let f = file_name.as_ref();
        loop {
            let file_path = path.join(f);
            if let Ok(c) = Self::from_file_path(file_path) {
                return Ok(c);
            }
            if let Some(new_path) = path.parent() {
                path = new_path.to_path_buf();
            } else {
                return Ok(Self::default());
            }
        }
    }

    /// Get branch name
    pub fn branch(&self) -> &str {
        &self.branch
    }
    /// Get commit message
    pub fn commit_message(&self) -> &str {
        &self.commit_message
    }
    /// Get merge message
    pub fn merge_message(&self) -> &str {
        &self.merge_message
    }
    pub fn delay(&self) -> u64 {
        self.delay
    }
}
