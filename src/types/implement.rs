use super::*;
use crate::*;
use anyhow::Context as _;
use std::path::Path;
use std::sync::Arc;
use tokio::{
    io::AsyncWriteExt,
    sync::{Mutex, MutexGuard},
};

impl ApiState {
    /// read watch list file and init daemon state
    #[tracing::instrument]
    pub async fn read_watch_list() -> anyhow::Result<Self> {
        let file = daemon::cache_dir()
            .context("failed to get watch list path")?
            .join(daemon::WATCH_LIST_NAME);
        tracing::trace!("read watch list file: {}", file.display());
        let data = match tokio::fs::read(&file).await {
            Ok(data) => serde_json::from_slice(&data).context("watch file format error")?,
            Err(_) => {
                tracing::trace!("watch list file reading error; create watch list file");
                let data = types::WatchListFile::default();
                let contents = serde_json::to_string(&data).unwrap();
                tokio::fs::write(&file, contents.as_bytes())
                    .await
                    .context("failed to read/create watch list file")?;
                data
            }
        };
        let watch_list: anyhow::Result<types::WatchList> = data
            .paths
            .into_iter()
            .map(|(k, v)| {
                Ok((
                    k.clone(),
                    types::WatchListEntry {
                        config: v.config.clone(),
                        watcher: watcher::RepoWatcher::new(&k, v.config)?,
                    },
                ))
            })
            .collect();
        Ok(Self {
            watch_list: Arc::new(Mutex::new(watch_list?)),
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
            .await
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    types::WatchListFileEntry {
                        config: v.config.clone(),
                    },
                )
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
        self.watch_list.lock().await
    }

    /// append new dir to watch list
    pub async fn append_watch_dir(
        &self,
        path: impl AsRef<Path>,
        config: config::Config,
    ) -> anyhow::Result<()> {
        let watcher = watcher::RepoWatcher::new(path.as_ref(), config.clone())?;
        self.watch_list.lock().await.insert(
            path.as_ref().to_path_buf(),
            types::WatchListEntry { config, watcher },
        );
        Ok(())
    }
    /// remove specified dir from watch list
    pub async fn remove_watch_dir(
        &self,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<types::WatchListEntry> {
        self.watch_list
            .lock()
            .await
            .remove(path.as_ref())
            .context("specified path is not in watch list")
    }
}
