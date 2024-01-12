use git2::{
    self, Branch, BranchType, Diff, DiffOptions, ErrorCode, IndexAddOption, IndexEntry, Oid,
    Reference, Repository, RepositoryState, ResetType,
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

    pub fn backup_index(&self) -> Result<Vec<IndexEntry>, GitError> {
        let index = self
            .0
            .index()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        Ok(index.iter().collect())
    }
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

    pub fn commit(
        &self,
        reference: &Reference<'_>,
        message: impl AsRef<str>,
    ) -> Result<Oid, GitError> {
        let mut index = self
            .0
            .index()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        let tree = self
            .0
            .find_tree(tree_oid)
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        let sig = self
            .0
            .signature()
            .map_err(|e| GitError::Unknown(file!(), line!(), e))?;
        let commit = self
            .0
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                message.as_ref(),
                &tree,
                &[&reference
                    .peel_to_commit()
                    .map_err(|e| GitError::Unknown(file!(), line!(), e))?],
            )
            .map_err(|e| GitError::Commit(file!(), line!(), e))?;
        Ok(commit)
    }

    pub fn save(
        &self,
        name: impl AsRef<str>,
        commit_message: impl AsRef<str>,
    ) -> Result<(), GitError> {
        let state = self.0.state();
        if state != RepositoryState::Clean {
            return Err(GitError::BadState(file!(), line!(), state));
        }

        if self.is_saved(&name)? {
            return Ok(());
        }

        let current_head = self.get_current_head_name()?;
        let current_index_entries = self.backup_index()?;

        let branch_ref = self.change_head_branch(name, "")?;
        self.add_cwd_all()?;
        self.commit(&branch_ref, commit_message)?;
        self.change_head_ref(&current_head, "")?;

        self.restore_index(current_index_entries)?;

        Ok(())
    }
}
