use crate::config::Config;
use crate::git::GitRepo;
use anyhow::{Context as _, Result};
use log::{error, info};
use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

/// Repository watcher
///
/// This object watches file changes and perform auto save when file is saved
pub struct RepoWatcher(RecommendedWatcher);

impl RepoWatcher {
    /// Create new watcher in specified path, specified configuration
    pub fn new(path: impl ToString, conf: Config) -> Result<Self> {
        let p = path.to_string();
        let branch = conf.branch();
        let commit_message = conf.commit_message();
        let merge_message = conf.merge_message();
        let mut watcher =
            recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                if let Ok(ev) = result {
                    if ev.kind.is_create() || ev.kind.is_modify() || ev.kind.is_remove() {
                        if let Ok(repo) = GitRepo::new(&p) {
                            if let Err(e) = repo.save(&branch, &commit_message, &merge_message) {
                                error!("{}", e);
                            }
                        }
                    }
                }
            })
            .context("Watcher create error")?;
        let p = path.to_string();
        watcher
            .watch(Path::new(&p), RecursiveMode::Recursive)
            .context("Watch start error")?;
        info!("Start watching: {}", &p);
        Ok(Self(watcher))
    }
}
