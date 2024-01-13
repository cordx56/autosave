use git2::{
    self, Branch, BranchType, Commit, Diff, DiffOptions, ErrorCode, IndexAddOption, IndexEntry,
    Oid, Reference, Repository, RepositoryState, ResetType,
};

#[derive(Debug)]
pub enum GitError {
    NoRepository(&'static str, u32, git2::Error),
    BadState(&'static str, u32, RepositoryState),
    BranchCreation(&'static str, u32, git2::Error),
    Reset(&'static str, u32, git2::Error),
    Commit(&'static str, u32, git2::Error),
    Add(&'static str, u32, git2::Error),
    Diff(&'static str, u32, git2::Error),
    Merge(&'static str, u32, git2::Error),
    Unknown(&'static str, u32, git2::Error),
}

#[derive(Debug)]
pub enum ReferenceName {
    Branch(String),
    Commit(Oid),
}

pub struct GitRepo(Repository);

impl GitRepo {
    pub fn new(dir: impl AsRef<str>) -> Result<Self, GitError> {
        let repo = Repository::open(dir.as_ref()).map_err(|e| {
            let code = e.code();
            if code == ErrorCode::NotFound {
                GitError::NoRepository(file!(), line!(), e)
            } else {
                GitError::Unknown(file!(), line!(), e)
            }
        })?;
        Ok(Self(repo))
    }

    fn head(&self) -> Result<Reference<'_>, GitError> {
        self.0
            .head()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))
    }

    pub fn get_branch(&self, name: impl AsRef<str>) -> Result<Option<Branch>, GitError> {
        match self.0.find_branch(name.as_ref(), BranchType::Local) {
            Ok(b) => Ok(Some(b)),
            Err(e) => {
                let code = e.code();
                if code == ErrorCode::NotFound || code == ErrorCode::UnbornBranch {
                    Ok(None)
                } else {
                    Err(GitError::Unknown(file!(), line!(), e))
                }
            }
        }
    }
    pub fn get_or_create_branch(&self, name: impl AsRef<str>) -> Result<Branch, GitError> {
        match self.get_branch(&name)? {
            Some(b) => Ok(b),
            None => {
                let head = self.head()?;
                let commit = head
                    .peel_to_commit()
                    .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
                let branch = self
                    .0
                    .branch(name.as_ref(), &commit, false)
                    .map_err(|e| GitError::BranchCreation(file!(), line!(), e))?;
                Ok(branch)
            }
        }
    }

    pub fn change_head_ref(
        &self,
        target: &ReferenceName,
        message: impl AsRef<str>,
    ) -> Result<Reference<'_>, GitError> {
        let head = match target {
            ReferenceName::Branch(name) => self
                .0
                .reference_symbolic("HEAD", name, true, message.as_ref())
                .map_err(|e| GitError::Unknown(file!(), line!(), e))?,
            ReferenceName::Commit(oid) => self
                .0
                .reference("HEAD", *oid, true, message.as_ref())
                .map_err(|e| GitError::Unknown(file!(), line!(), e))?,
        };
        let commit = head
            .peel_to_commit()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        let obj = commit.as_object();
        self.0
            .reset(obj, ResetType::Mixed, None)
            .map_err(|e| GitError::Reset(file!(), line!(), e))?;
        Ok(head)
    }

    pub fn change_head_branch(
        &self,
        name: impl AsRef<str>,
        message: impl AsRef<str>,
    ) -> Result<Reference<'_>, GitError> {
        let branch = self.get_or_create_branch(name.as_ref())?;
        let ref_name = ReferenceName::Branch(branch.into_reference().name().unwrap().to_string());
        self.change_head_ref(&ref_name, message)
    }

    pub fn add_cwd_all(&self) -> Result<(), GitError> {
        let mut index = self
            .0
            .index()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        index
            .add_all(["."], IndexAddOption::DEFAULT, None)
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        index
            .write()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        Ok(())
    }

    pub fn get_current_head_name(&self) -> Result<ReferenceName, GitError> {
        let head = self.head()?;
        if self
            .0
            .head_detached()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?
        {
            let oid = head
                .peel_to_commit()
                .map_err(|e| GitError::Unknown(file!(), line!(), e))?
                .id();
            Ok(ReferenceName::Commit(oid))
        } else {
            Ok(ReferenceName::Branch(head.name().unwrap().to_string()))
        }
    }

    pub fn get_ref_workdir_diff(&self, reference: &Reference<'_>) -> Result<Diff<'_>, GitError> {
        let tree = reference
            .peel_to_tree()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        self.0
            .diff_tree_to_workdir(
                Some(&tree),
                Some(
                    DiffOptions::new()
                        .include_untracked(true)
                        .recurse_untracked_dirs(true),
                ),
            )
            .map_err(|e| GitError::Diff(file!(), line!(), e))
    }
    pub fn is_saved(&self, branch: impl AsRef<str>) -> Result<bool, GitError> {
        let head = self.head()?;
        let diff = self.get_ref_workdir_diff(&head)?;
        let stats = diff
            .stats()
            .map_err(|e| GitError::Diff(file!(), line!(), e))?;
        if stats.files_changed() == 0 {
            return Ok(true);
        }
        if let Some(branch) = self.get_branch(branch)? {
            let diff = self.get_ref_workdir_diff(branch.get())?;
            let stats = diff
                .stats()
                .map_err(|e| GitError::Diff(file!(), line!(), e))?;
            Ok(stats.files_changed() == 0)
        } else {
            Ok(false)
        }
    }

    pub fn get_ref_ref_diff(
        &self,
        old: &Reference<'_>,
        new: &Reference<'_>,
    ) -> Result<Diff<'_>, GitError> {
        let old = old
            .peel_to_tree()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        let new = new
            .peel_to_tree()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
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
            .map_err(|e| GitError::Diff(file!(), line!(), e))
    }
    /// Merge two refs
    pub fn merge(
        &self,
        our: &Reference<'_>,
        their: &Reference<'_>,
        message: impl AsRef<str>,
    ) -> Result<Option<Oid>, GitError> {
        let oc = our
            .peel_to_commit()
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let tc = their
            .peel_to_commit()
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let base_oid = self
            .0
            .merge_base(oc.id(), tc.id())
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        if oc.id() == base_oid || tc.id() == base_oid {
            return Ok(None);
        }
        let base_commit = self
            .0
            .find_commit(base_oid)
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let ancestor = base_commit
            .tree()
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let ot = our
            .peel_to_tree()
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let tt = their
            .peel_to_tree()
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let mut before_index = self
            .0
            .index()
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let mut index = self
            .0
            .merge_trees(&ancestor, &ot, &tt, None)
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        self.0
            .set_index(&mut index)
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        let commit = self.commit(&[&tc, &oc], message)?;
        self.0
            .cleanup_state()
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        self.0
            .set_index(&mut before_index)
            .map_err(|e| GitError::Merge(file!(), line!(), e))?;
        Ok(Some(commit))
    }
    /// Merge ref to HEAD if there are no diffs
    pub fn auto_merge(
        &self,
        from: &ReferenceName,
        message: impl AsRef<str>,
    ) -> Result<Option<Oid>, GitError> {
        if let ReferenceName::Branch(branch_ref) = from {
            let branch = self
                .0
                .find_reference(branch_ref)
                .map_err(|e| GitError::Merge(file!(), line!(), e))?;
            let head = self.head()?;
            let diff = self.get_ref_ref_diff(&head, &branch)?;
            let stats = diff
                .stats()
                .map_err(|e| GitError::Diff(file!(), line!(), e))?;
            if stats.files_changed() == 0 {
                let c = self.merge(&branch, &head, message)?;
                return Ok(c);
            }
        }
        Ok(None)
    }

    /// Backup current index to entries
    pub fn backup_index(&self) -> Result<Vec<IndexEntry>, GitError> {
        let index = self
            .0
            .index()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        Ok(index.iter().collect())
    }
    /// Restore index from entries
    pub fn restore_index(
        &self,
        entries: impl IntoIterator<Item = IndexEntry>,
    ) -> Result<(), GitError> {
        let mut index = self
            .0
            .index()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        for entry in entries.into_iter() {
            index
                .add(&entry)
                .map_err(|e| GitError::Add(file!(), line!(), e))?;
        }
        index
            .write()
            .map_err(|e| GitError::Add(file!(), line!(), e))?;
        Ok(())
    }

    /// Create new commit
    pub fn commit(&self, parents: &[&Commit], message: impl AsRef<str>) -> Result<Oid, GitError> {
        let mut index = self
            .0
            .index()
            .map_err(|e| GitError::Commit(file!(), line!(), e))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| GitError::Commit(file!(), line!(), e))?;
        let tree = self
            .0
            .find_tree(tree_oid)
            .map_err(|e| GitError::Commit(file!(), line!(), e))?;
        let sig = self
            .0
            .signature()
            .map_err(|e| GitError::Commit(file!(), line!(), e))?;
        let commit = self
            .0
            .commit(Some("HEAD"), &sig, &sig, message.as_ref(), &tree, parents)
            .map_err(|e| GitError::Commit(file!(), line!(), e))?;
        Ok(commit)
    }

    /// Save current working directory to specified branch
    pub fn save(
        &self,
        branch_name: impl AsRef<str>,
        commit_message: impl AsRef<str>,
    ) -> Result<(), GitError> {
        let state = self.0.state();
        if state != RepositoryState::Clean {
            return Ok(());
            //return Err(GitError::BadState(file!(), line!(), state));
        }

        if self.is_saved(&branch_name)? {
            return Ok(());
        }

        let current_head = self.get_current_head_name()?;
        let current_index_entries = self.backup_index()?;

        self.change_head_branch(&branch_name, "")?;
        self.auto_merge(&current_head, &commit_message)?;

        let branch_ref = self.change_head_branch(&branch_name, "")?;
        let parent_commit = branch_ref
            .peel_to_commit()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;

        self.add_cwd_all()?;
        self.commit(&[&parent_commit], &commit_message)?;
        self.change_head_ref(&current_head, "")?;

        self.restore_index(current_index_entries)?;

        Ok(())
    }
}
