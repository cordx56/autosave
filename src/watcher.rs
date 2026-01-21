use crate::config::Config;
use crate::git::GitRepo;
use anyhow::Context as _;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Repository watcher
///
/// This object watches file changes and perform auto save when file is saved
pub struct RepoWatcher {
    #[allow(unused)]
    watcher: RecommendedWatcher,
    #[allow(unused)]
    debounce_thread: thread::JoinHandle<()>,
}

fn debounce_worker(rx: mpsc::Receiver<Event>, path: PathBuf, config: Arc<Mutex<Vec<Config>>>) {
    loop {
        // Wait for the first event
        if rx.recv().is_err() {
            tracing::debug!("channel closed");
            break;
        }

        let confs = config.lock().unwrap();
        if confs.is_empty() {
            return;
        }
        let relative_delays = confs.iter().zip(
            std::iter::once(confs[0].delay)
                .chain(confs.windows(2).map(|v| v[1].delay - v[0].delay)),
        );
        for (conf, relative_delay) in relative_delays {
            // Debounce: keep receiving until no events for `delay` seconds
            loop {
                match rx.recv_timeout(Duration::from_secs(relative_delay)) {
                    Ok(_) => {
                        // Got another event, continue waiting
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // No events for `delay` seconds, perform commit
                        tracing::info!(
                            "edited and {} secs past; save current contents of {} to branch {}",
                            conf.delay,
                            path.display(),
                            &conf.branch,
                        );
                        if let Ok(repo) = GitRepo::new(&path)
                            && let Err(e) = repo.save(
                                conf.branch.clone(),
                                conf.commit_message.clone(),
                                conf.merge_message.clone(),
                            )
                        {
                            tracing::error!("{e}");
                        }
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        // Channel closed, exit
                        tracing::debug!("channel closed");
                        return;
                    }
                }
            }
        }
    }
}

impl RepoWatcher {
    /// Create new watcher in specified path, specified configuration
    pub fn new(path: impl AsRef<Path>, conf: Arc<Mutex<Vec<Config>>>) -> anyhow::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();

        // Create channel for debouncing
        let (tx, rx) = mpsc::channel();

        // Spawn single debounce worker thread
        let debounce_thread = {
            let path = path_buf.clone();
            thread::spawn(move || {
                debounce_worker(rx, path, conf);
            })
        };

        let mut watcher =
            recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                if let Ok(ev) = result
                    && (ev.kind.is_create() || ev.kind.is_modify() || ev.kind.is_remove())
                {
                    // Just send signal to debounce worker, ignore send errors
                    if let Err(e) = tx.send(ev).context("send file change event error") {
                        tracing::warn!("{e:?}");
                    }
                }
            })
            .context("Watcher create error")?;
        watcher
            .watch(path.as_ref(), RecursiveMode::Recursive)
            .context("Watch start error")?;
        tracing::info!("Start watching: {}", &path.as_ref().display());
        Ok(Self {
            watcher,
            debounce_thread,
        })
    }
}
