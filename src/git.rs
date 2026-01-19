use anyhow::{Context as _, anyhow};
use git2::{
    self, Branch, BranchType, Commit, Diff, DiffOptions, ErrorCode, Index, IndexAddOption,
    IndexEntry, Oid, Reference, Repository, RepositoryState, ResetType,
};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Repository not found: {:?}", .0)]
    NoRepository(git2::Error),
    #[error("Unknown error: {:?}", .0)]
    Unknown(git2::Error),
}

/// Reference name object
#[derive(Debug)]
pub enum ReferenceName {
    Branch(String),
    Commit(Oid),
}

/// Git repository object
pub struct GitRepo(Repository);

impl GitRepo {
    /// Create new repository object
    pub fn new(dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let repo = Repository::open(dir).map_err(|e| {
            let code = e.code();
            if code == ErrorCode::NotFound {
                anyhow!(GitError::NoRepository(e))
            } else {
                anyhow!(GitError::Unknown(e))
            }
        })?;
        Ok(Self(repo))
    }

    fn head(&self) -> anyhow::Result<Reference<'_>> {
        self.0
            .head()
            .map_err(|e| anyhow!(GitError::Unknown(e)))
            .context("Failed to get HEAD reference")
    }

    fn get_branch(&self, name: impl AsRef<str>) -> anyhow::Result<Option<Branch<'_>>> {
        match self.0.find_branch(name.as_ref(), BranchType::Local) {
            Ok(b) => Ok(Some(b)),
            Err(e) => {
                let code = e.code();
                if code == ErrorCode::NotFound || code == ErrorCode::UnbornBranch {
                    Ok(None)
                } else {
                    Err(anyhow!(GitError::Unknown(e)))
                }
            }
        }
    }
    fn get_or_create_branch(&self, name: impl AsRef<str>) -> anyhow::Result<Branch<'_>> {
        match self.get_branch(&name)? {
            Some(b) => Ok(b),
            None => {
                let head = self.head()?;
                let commit = head
                    .peel_to_commit()
                    .map_err(|e| anyhow!(GitError::Unknown(e)))
                    .context("Failed to create branch")?;
                let branch = self
                    .0
                    .branch(name.as_ref(), &commit, false)
                    .map_err(|e| anyhow!(GitError::Unknown(e)))
                    .context("Failed to create branch")?;
                Ok(branch)
            }
        }
    }

    fn change_head_ref(
        &self,
        target: &ReferenceName,
        message: impl AsRef<str>,
    ) -> anyhow::Result<Reference<'_>> {
        let head = match target {
            ReferenceName::Branch(name) => self
                .0
                .reference_symbolic("HEAD", name, true, message.as_ref())
                .map_err(|e| anyhow!(GitError::Unknown(e)))?,
            ReferenceName::Commit(oid) => self
                .0
                .reference("HEAD", *oid, true, message.as_ref())
                .map_err(|e| anyhow!(GitError::Unknown(e)))?,
        };
        let commit = head
            .peel_to_commit()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let obj = commit.as_object();
        self.0
            .reset(obj, ResetType::Mixed, None)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        Ok(head)
    }

    fn change_head_branch(
        &self,
        name: impl AsRef<str>,
        message: impl AsRef<str>,
    ) -> anyhow::Result<Reference<'_>> {
        let branch = self.get_or_create_branch(name.as_ref())?;
        let ref_name = ReferenceName::Branch(branch.into_reference().name().unwrap().to_string());
        self.change_head_ref(&ref_name, message)
    }

    fn get_current_index(&self) -> anyhow::Result<Index> {
        self.0.index().map_err(|e| anyhow!(GitError::Unknown(e)))
    }

    fn add_cwd_all(&self) -> anyhow::Result<()> {
        let mut index = self.0.index().map_err(|e| anyhow!(GitError::Unknown(e)))?;
        index
            .add_all(["."], IndexAddOption::DEFAULT, None)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        index.write().map_err(|e| anyhow!(GitError::Unknown(e)))?;
        Ok(())
    }

    fn get_current_head_name(&self) -> anyhow::Result<ReferenceName> {
        let head = self.head()?;
        if self
            .0
            .head_detached()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?
        {
            let oid = head
                .peel_to_commit()
                .map_err(|e| anyhow!(GitError::Unknown(e)))?
                .id();
            Ok(ReferenceName::Commit(oid))
        } else {
            Ok(ReferenceName::Branch(head.name().unwrap().to_string()))
        }
    }

    fn get_ref_workdir_diff(&self, reference: &Reference<'_>) -> anyhow::Result<Diff<'_>> {
        let tree = reference
            .peel_to_tree()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        self.0
            .diff_tree_to_workdir(
                Some(&tree),
                Some(
                    DiffOptions::new()
                        .include_untracked(true)
                        .recurse_untracked_dirs(true),
                ),
            )
            .map_err(|e| anyhow!(GitError::Unknown(e)))
    }
    fn is_saved(&self, branch: impl AsRef<str>) -> anyhow::Result<bool> {
        let head = self.head()?;
        let diff = self.get_ref_workdir_diff(&head)?;
        let stats = diff.stats().map_err(|e| anyhow!(GitError::Unknown(e)))?;
        if stats.files_changed() == 0 {
            return Ok(true);
        }
        if let Some(branch) = self.get_branch(branch)? {
            let diff = self.get_ref_workdir_diff(branch.get())?;
            let stats = diff.stats().map_err(|e| anyhow!(GitError::Unknown(e)))?;
            Ok(stats.files_changed() == 0)
        } else {
            Ok(false)
        }
    }

    fn get_ref_ref_diff(
        &self,
        old: &Reference<'_>,
        new: &Reference<'_>,
    ) -> anyhow::Result<Diff<'_>> {
        let old = old
            .peel_to_tree()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let new = new
            .peel_to_tree()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        self.0
            .diff_tree_to_tree(
                Some(&old),
                Some(&new),
                Some(
                    DiffOptions::new()
                        .include_untracked(true)
                        .recurse_untracked_dirs(true),
                ),
            )
            .map_err(|e| anyhow!(GitError::Unknown(e)))
    }
    /// Merge two refs
    fn merge(
        &self,
        our: &Reference<'_>,
        their: &Reference<'_>,
        message: impl AsRef<str>,
    ) -> anyhow::Result<Option<Oid>> {
        let oc = our
            .peel_to_commit()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let tc = their
            .peel_to_commit()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let base_oid = self
            .0
            .merge_base(oc.id(), tc.id())
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        if oc.id() == base_oid || tc.id() == base_oid {
            return Ok(None);
        }
        let base_commit = self
            .0
            .find_commit(base_oid)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let ancestor = base_commit
            .tree()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let ot = our
            .peel_to_tree()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let tt = their
            .peel_to_tree()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let mut before_index = self.0.index().map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let mut index = self
            .0
            .merge_trees(&ancestor, &ot, &tt, None)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        self.0
            .set_index(&mut index)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let commit = self.commit(&[&tc, &oc], message)?;
        self.0
            .cleanup_state()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        self.0
            .set_index(&mut before_index)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        Ok(Some(commit))
    }
    /// Merge ref to HEAD if there are no diffs
    fn auto_merge(
        &self,
        from: &ReferenceName,
        message: impl AsRef<str>,
    ) -> anyhow::Result<Option<Oid>> {
        if let ReferenceName::Branch(branch_ref) = from {
            let branch = self
                .0
                .find_reference(branch_ref)
                .map_err(|e| anyhow!(GitError::Unknown(e)))?;
            let head = self.head()?;
            let diff = self.get_ref_ref_diff(&head, &branch)?;
            let stats = diff.stats().map_err(|e| anyhow!(GitError::Unknown(e)))?;
            if stats.files_changed() == 0 {
                let c = self.merge(&branch, &head, message)?;
                return Ok(c);
            }
        }
        Ok(None)
    }

    /// Backup current index to entries
    fn backup_index(&self) -> anyhow::Result<Vec<IndexEntry>> {
        let index = self.0.index().map_err(|e| anyhow!(GitError::Unknown(e)))?;
        Ok(index.iter().collect())
    }
    /// Restore index from entries
    fn restore_index(&self, entries: impl IntoIterator<Item = IndexEntry>) -> anyhow::Result<()> {
        let mut index = self.get_current_index()?;
        for entry in entries.into_iter() {
            index
                .add(&entry)
                .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        }
        index.write().map_err(|e| anyhow!(GitError::Unknown(e)))?;
        Ok(())
    }

    /// Create new commit
    fn commit(&self, parents: &[&Commit], message: impl AsRef<str>) -> anyhow::Result<Oid> {
        let mut index = self.0.index().map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let tree = self
            .0
            .find_tree(tree_oid)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let sig = self
            .0
            .signature()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        let commit = self
            .0
            .commit(Some("HEAD"), &sig, &sig, message.as_ref(), &tree, parents)
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        Ok(commit)
    }

    /// Create new commit on current HEAD
    fn commit_on_current_head(&self, message: impl AsRef<str>) -> anyhow::Result<Oid> {
        let commit = self
            .head()?
            .peel_to_commit()
            .map_err(|e| anyhow!(GitError::Unknown(e)))?;
        self.commit(&[&commit], &message)
    }

    /// Save current working directory to specified branch
    pub fn save(
        &self,
        branch_name: impl AsRef<str>,
        commit_message: impl AsRef<str>,
        merge_message: impl AsRef<str>,
    ) -> anyhow::Result<()> {
        let state = self.0.state();
        if state != RepositoryState::Clean {
            return Ok(());
        }

        if self.is_saved(&branch_name)? {
            return Ok(());
        }

        let current_head = self
            .get_current_head_name()
            .context("Failed to get current HEAD")?;
        let current_index_entries = self
            .backup_index()
            .context("Failed to get index entries backup")?;

        self.change_head_branch(&branch_name, "")
            .context("Failed to change branch")?;

        // Run auto merge
        if let Err(e) = self.auto_merge(&current_head, &merge_message) {
            // Restore current HEAD if error occurred
            self.change_head_ref(&current_head, "").with_context(|| {
                format!(
                    "Failed to restore HEAD reference from recovering error: {}",
                    &e
                )
            })?;
            return Err(e);
        }

        if let Err(e) = self.add_cwd_all() {
            // Restore current HEAD if error occurred
            self.change_head_ref(&current_head, "").with_context(|| {
                format!(
                    "Failed to restore HEAD reference from recovering error: {}",
                    &e
                )
            })?;
            return Err(e);
        }
        if let Err(e) = self.commit_on_current_head(&commit_message) {
            // Restore current HEAD if error occurred
            self.change_head_ref(&current_head, "").with_context(|| {
                format!(
                    "Failed to restore HEAD reference from recovering error: {}",
                    &e
                )
            })?;
            return Err(e);
        }

        self.change_head_ref(&current_head, "")
            .context("Failed to restore HEAD reference")?;

        self.restore_index(current_index_entries)
            .context("Failed to restore index entries")?;

        Ok(())
    }

    /// Add Git worktree at the specified path
    pub fn add_worktree(
        &self,
        branch_name: impl AsRef<str>,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        self.0
            .worktree(branch_name.as_ref(), path.as_ref(), None)
            .context("failed to add new worktree")?;
        Ok(())
    }
}
