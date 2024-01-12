use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    branch: Option<String>,
    message: Option<String>,
}

impl Config {
    pub fn branch(&self) -> String {
        self.branch.clone().unwrap_or("tmp/autosave".to_string())
    }
    pub fn message(&self) -> String {
        self.message.clone().unwrap_or("auto save".to_string())
    }
}
