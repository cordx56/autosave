use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Format(toml::de::Error),
}

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    branch: Option<String>,
    message: Option<String>,
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
    pub fn branch(&self) -> String {
        self.branch.clone().unwrap_or("tmp/autosave".to_string())
    }
    pub fn message(&self) -> String {
        self.message.clone().unwrap_or("auto save".to_string())
    }
}
