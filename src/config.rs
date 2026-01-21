use serde::{Deserialize, Serialize};

/// Configuration object
///
/// Config file is deserialized to this object
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub worktree: Option<String>,
    pub branch: String,
    pub commit_message: String,
    pub merge_message: String,
    pub delay: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            worktree: None,
            branch: "tmp/autosave".to_string(),
            commit_message: "autosave commit".to_string(),
            merge_message: "autosave merge".to_string(),
            delay: 3,
        }
    }
}
