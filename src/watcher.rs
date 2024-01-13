use crate::config::Config;
use crate::git::GitRepo;
use crate::Error;
use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

pub struct RepoWatcher(RecommendedWatcher);

impl RepoWatcher {
    pub fn new(path: impl ToString, conf: Config) -> Result<Self, Error> {
        let p = path.to_string();
        let branch = conf.branch();
        let message = conf.message();
        let mut watcher =
            recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                if let Ok(ev) = result {
                    if ev.kind.is_create() || ev.kind.is_modify() || ev.kind.is_remove() {
                        if let Ok(repo) = GitRepo::new(&p) {
                            repo.save(&branch, &message).unwrap();
                        }
                    }
                }
            })
            .map_err(|e| Error::WatchError(e))?;
        watcher
            .watch(Path::new(&path.to_string()), RecursiveMode::Recursive)
            .map_err(|e| Error::WatchError(e))?;
        Ok(Self(watcher))
    }
}
