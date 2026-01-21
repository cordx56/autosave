use super::*;
use crate::*;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::io::AsyncWriteExt;

impl ApiState {
    /// read watch list file and init daemon state
    pub async fn read_watch_list() -> anyhow::Result<Self> {
        let file = daemon::cache_dir()
            .context("failed to get watch list path")?
            .join(daemon::WATCH_LIST_NAME);
        tracing::trace!("read watch list file: {}", file.display());
        let data = match tokio::fs::read(&file)
            .await
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
        {
            Some(data) => data,
            None => {
                tracing::warn!("watch list file reading error; create watch list file");
                let data = types::WatchListFile::default();
                let contents = serde_json::to_string(&data).unwrap();
                tokio::fs::write(&file, contents.as_bytes())
                    .await
                    .context("failed to read/create watch list file")?;
                data
            }
        };
        let watch_list: types::WatchList = data
            .paths
            .into_iter()
            .filter_map(|(k, v)| {
                let configs = Arc::new(Mutex::new(v.configs));
                watcher::RepoWatcher::new(&k, configs.clone())
                    .map(|watcher| (k.clone(), types::WatchListEntry { configs, watcher }))
                    .ok()
            })
            .collect();
        Ok(Self {
            watch_list: Arc::new(Mutex::new(watch_list)),
        })
    }
    /// write current watch list state into file
    pub async fn write_watch_list(&self) -> anyhow::Result<()> {
        let path = daemon::cache_dir()
            .context("failed to get watch list path")?
            .join(daemon::WATCH_LIST_NAME);
        tracing::trace!("write watch list file: {}", path.display());
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .await
            .context("failed to open watch list file")?;
        let paths = self
            .watch_list
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let configs = v.configs.lock().unwrap().clone();
                (k.clone(), types::WatchListFileEntry { configs })
            })
            .collect();
        let data = types::WatchListFile { paths };
        let data = serde_json::to_string(&data).context("failed to create watch file format")?;
        file.write_all(data.as_bytes())
            .await
            .context("failed to write watch list file")
    }

    /// get current watch list
    pub async fn watch_list(&self) -> MutexGuard<'_, WatchList> {
        self.watch_list.lock().unwrap()
    }

    /// append new dir to watch list
    pub async fn append_watch_dir(
        &self,
        path: impl AsRef<Path>,
        config: config::Config,
    ) -> anyhow::Result<()> {
        self.watch_list
            .lock()
            .unwrap()
            .entry(path.as_ref().to_path_buf())
            .or_insert({
                let configs = Arc::new(Mutex::new(vec![]));
                types::WatchListEntry {
                    watcher: watcher::RepoWatcher::new(path.as_ref(), configs.clone())?,
                    configs,
                }
            })
            .configs
            .lock()
            .unwrap()
            .push(config);
        Ok(())
    }
    /// remove specified dir from watch list
    pub async fn remove_watch_dir(
        &self,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<types::WatchListEntry> {
        self.watch_list
            .lock()
            .unwrap()
            .remove(path.as_ref())
            .context("specified path is not in watch list")
    }
}
