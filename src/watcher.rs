use crate::config::Config;
use crate::git::GitRepo;
use anyhow::Context as _;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
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

fn debounce_worker(
    rx: mpsc::Receiver<()>,
    path: PathBuf,
    delay: u64,
    branch: String,
    commit_message: String,
    merge_message: String,
) {
    loop {
        // Wait for the first event
        if rx.recv().is_err() {
            // Channel closed, exit
            break;
        }

        // Debounce: keep receiving until no events for `delay` seconds
        loop {
            match rx.recv_timeout(Duration::from_secs(delay)) {
                Ok(()) => {
                    // Got another event, continue waiting
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // No events for `delay` seconds, perform commit
                    tracing::info!(
                        "edited and {delay} secs past; save current contents of: {}",
                        path.display()
                    );
                    if let Ok(repo) = GitRepo::new(&path)
                        && let Err(e) = repo.save(
                            branch.clone(),
                            commit_message.clone(),
                            merge_message.clone(),
                        )
                    {
                        tracing::error!("{e}");
                    }
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Channel closed, exit
                    return;
                }
            }
        }
    }
}

impl RepoWatcher {
    /// Create new watcher in specified path, specified configuration
    pub fn new(path: impl AsRef<Path>, conf: Config) -> anyhow::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();

        // Create channel for debouncing
        let (tx, rx) = mpsc::channel::<()>();

        // Spawn single debounce worker thread
        let debounce_thread = {
            let path = path_buf.clone();
            let delay = conf.delay;
            let branch = conf.branch.clone();
            let commit_message = conf.commit_message.clone();
            let merge_message = conf.merge_message.clone();
            thread::spawn(move || {
                debounce_worker(rx, path, delay, branch, commit_message, merge_message);
            })
        };

        let mut watcher =
            recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                if let Ok(ev) = result
                    && (ev.kind.is_create() || ev.kind.is_modify() || ev.kind.is_remove())
                {
                    // Just send signal to debounce worker, ignore send errors
                    let _ = tx.send(());
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
