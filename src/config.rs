use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Format(toml::de::Error),
}

/// Configuration object
///
/// Config file is deserialized to this object
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    branch: Option<String>,
    commit_message: Option<String>,
    merge_message: Option<String>,
}

impl Config {
    pub fn from_file_path(p: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let s = fs::read_to_string(p).map_err(|e| ConfigError::Io(e))?;
        let c = toml::from_str(&s).map_err(|e| ConfigError::Format(e))?;
        Ok(c)
    }
    pub fn from_dir_path(
        p: impl AsRef<Path>,
        file_name: impl AsRef<Path>,
    ) -> Result<Self, ConfigError> {
        let mut path = fs::canonicalize(p).map_err(|e| ConfigError::Io(e))?;
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
    pub fn branch(&self) -> String {
        self.branch.clone().unwrap_or("tmp/autosave".to_string())
    }
    /// Get commit message
    pub fn commit_message(&self) -> String {
        self.commit_message
            .clone()
            .unwrap_or("auto save".to_string())
    }
    /// Get merge message
    pub fn merge_message(&self) -> String {
        self.merge_message
            .clone()
            .unwrap_or("auto merge".to_string())
    }
}
