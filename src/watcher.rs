use crate::git::GitRepo;
use crate::Error;
use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

pub struct RepoWatcher {
    watcher: RecommendedWatcher,
    path: String,
}

impl RepoWatcher {
    pub fn new(
        path: impl ToString,
        branch: impl ToString,
        message: impl ToString,
    ) -> Result<Self, Error> {
        let p = path.to_string();
        let b = branch.to_string();
        let m = message.to_string();
        let repo = GitRepo::new(&p).map_err(|e| Error::GitError(e))?;
        let watcher = recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
            if let Ok(ev) = result {
                if ev.kind.is_create() || ev.kind.is_modify() || ev.kind.is_remove() {
                    let _ = repo.save(&b, &m);
                }
            }
        })
        .map_err(|e| Error::WatchError(e))?;
        Ok(Self { watcher, path: p })
    }
    pub fn watch(&mut self) -> Result<(), Error> {
        self.watcher
            .watch(Path::new(&self.path), RecursiveMode::Recursive)
            .map_err(|e| Error::WatchError(e))
    }
}
