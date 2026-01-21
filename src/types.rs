use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{Layer, registry::Registry, reload::Handle};

mod implement;

pub type TracingReloadHandle = Handle<Box<dyn Layer<Registry> + Send + Sync>, Registry>;

pub struct WatchListEntry {
    pub configs: Arc<Mutex<Vec<crate::config::Config>>>,
    pub watcher: crate::watcher::RepoWatcher,
}
pub type WatchList = HashMap<PathBuf, WatchListEntry>;
#[derive(Clone)]
pub struct ApiState {
    watch_list: Arc<Mutex<WatchList>>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct WatchListFileEntry {
    pub configs: Vec<crate::config::Config>,
}
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct WatchListFile {
    pub paths: HashMap<PathBuf, WatchListFileEntry>,
}

//
// API
//
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ApiResponse<T> {
    Success { data: T },
    Failed { message: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WatchListResponse {
    pub paths: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ChangeWatchRequest {
    Add {
        path: PathBuf,
        config: crate::config::Config,
    },
    Remove {
        path: PathBuf,
    },
}
