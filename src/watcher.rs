use crate::config::Config;
use crate::git::GitRepo;
use anyhow::Context as _;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

/// Repository watcher
///
/// This object watches file changes and perform auto save when file is saved
pub struct RepoWatcher {
    #[allow(unused)]
    watcher: RecommendedWatcher,
}

impl RepoWatcher {
    /// Create new watcher in specified path, specified configuration
    pub fn new(path: impl AsRef<Path>, conf: Config) -> anyhow::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let last_edit = Arc::new(Mutex::new(SystemTime::now()));
        let mut watcher =
            recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                let branch = conf.branch();
                let commit_message = conf.commit_message();
                let merge_message = conf.merge_message();
                let delay = conf.delay();

                if let Ok(ev) = result
                    && (ev.kind.is_create() || ev.kind.is_modify() || ev.kind.is_remove())
                {
                    let path_buf = path_buf.clone();
                    let last_edit = last_edit.clone();

                    let branch = branch.to_string();
                    let commit_message = commit_message.to_string();
                    let merge_message = merge_message.to_string();

                    let now = SystemTime::now();
                    *last_edit.lock().unwrap() = now;
                    thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(delay));
                        let edited = last_edit
                            .lock()
                            .unwrap()
                            .duration_since(now)
                            .map(|v| v.as_nanos())
                            .unwrap_or(0);
                        tracing::trace!("{edited} nano secs past from last edit");
                        if edited == 0 {
                            tracing::info!(
                                "edited and {delay} secs past; save current contents of: {}",
                                path_buf.display()
                            );
                            if let Ok(repo) = GitRepo::new(&path_buf)
                                && let Err(e) = repo.save(branch, commit_message, merge_message)
                            {
                                tracing::error!("{e}");
                            }
                        }
                    });
                }
            })
            .context("Watcher create error")?;
        watcher
            .watch(path.as_ref(), RecursiveMode::Recursive)
            .context("Watch start error")?;
        tracing::info!("Start watching: {}", &path.as_ref().display());
        Ok(Self { watcher })
    }
}
