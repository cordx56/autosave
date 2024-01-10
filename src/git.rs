use git2::{self, BranchType, Reference, Repository};
use std::env::current_dir;
use std::io;

#[derive(Debug)]
pub enum GitError {
    Cwd(io::Error),
    NoRepository,
    UnbornBranch,
    InvalidName,
    Fatal(git2::Error),
}

pub struct GitRepo(Repository);

impl GitRepo {
    pub fn new(dir: impl AsRef<str>) -> Result<Self, GitError> {
        let repo = Repository::open(dir.as_ref()).map_err(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                GitError::NoRepository
            } else {
                GitError::Fatal(e)
            }
        })?;
        Ok(Self(repo))
    }

    pub fn change_head_ref(
        &self,
        target: impl AsRef<str>,
        message: impl AsRef<str>,
    ) -> Result<Reference<'_>, GitError> {
        self.0
            .reference_symbolic("HEAD", target.as_ref(), true, message.as_ref())
            .map_err(|e| GitError::Fatal(e))
    }

    pub fn change_head_branch(
        &self,
        branch: impl AsRef<str>,
        branch_type: BranchType,
        message: impl AsRef<str>,
    ) -> Result<Reference<'_>, GitError> {
        let branch = self
            .0
            .find_branch(branch.as_ref(), branch_type)
            .map_err(|e| GitError::Fatal(e))?;
        if let Some(name) = branch.into_reference().name() {
            self.change_head_ref(name, message)
        } else {
            Err(GitError::InvalidName)
        }
    }

    pub fn save(&self, name: impl AsRef<str>) -> Result<(), GitError> {
        self.change_head_branch(name, BranchType::Local);
        let commit = self
            .0
            .head()
            .map_err(|e| {
                if e.code() == git2::ErrorCode::UnbornBranch {
                    GitError::UnbornBranch
                } else {
                    GitError::Fatal(e)
                }
            })?
            .peel_to_commit()
            .map_err(|e| GitError::Fatal(e))?;
        let branch = self
            .0
            .branch(name.as_ref(), &commit, false)
            .map_err(|e| GitError::Fatal(e))?;
        let obj = branch
            .into_reference()
            .peel(ObjectType::Any)
            .map_err(|e| GitError::Fatal(e))?;
        self.0
            .checkout_tree(&obj, None)
            .map_err(|e| GitError::Fatal(e))?;
        Ok(())
    }
}

pub fn get_current_head_name() -> Result<Vec<u8>, GitError> {
    let cwd = current_dir().map_err(|e| GitError::Cwd(e))?;
    let repo = Repository::open(cwd).map_err(|e| {
        if e.code() == git2::ErrorCode::NotFound {
            GitError::NoRepository
        } else {
            GitError::Fatal(e)
        }
    })?;
    let head = repo.head().map_err(|e| {
        if e.code() == git2::ErrorCode::UnbornBranch {
            GitError::UnbornBranch
        } else {
            GitError::Fatal(e)
        }
    })?;
    let resolved = head.resolve().unwrap();
    Ok(resolved.name_bytes().to_vec())
}
