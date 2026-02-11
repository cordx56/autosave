use crate::{config::Config, daemon, git, types};
use anyhow::Context as _;
use nix::{
    sys::{signal, wait},
    unistd,
};
use reqwest::blocking::Client;
use std::env;
use std::ffi::CString;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// get Unix socket client
pub fn get_client() -> anyhow::Result<Client> {
    let path = daemon::cache_dir()?.join(daemon::SOCK_NAME);
    tracing::trace!("create Unix socket client: {}", path.display());
    Client::builder()
        .unix_socket(path)
        .build()
        .context("failed to connect unix socket")
}

/// send get watch list request to Unix sock
#[tracing::instrument]
pub fn get_watch_list() -> anyhow::Result<Vec<PathBuf>> {
    let resp = get_client()?
        .get("http://localhost/watch")
        .send()
        .context("failed to get response")?;
    let data: types::ApiResponse<types::WatchListResponse> =
        resp.json().context("failed to read response")?;
    match data {
        types::ApiResponse::Success { data } => Ok(data.paths),
        types::ApiResponse::Failed { message } => {
            tracing::error!("{}", message);
            anyhow::bail!(message);
        }
    }
}

/// send change watch list request to Unix sock
#[tracing::instrument]
pub fn change_watch_list(change: types::ChangeWatchRequest) -> anyhow::Result<()> {
    let resp = get_client()?
        .post("http://localhost/watch")
        .json(&change)
        .send()
        .context("failed to get response")?;
    let data: types::ApiResponse<()> = resp.json().context("failed to read response")?;
    match data {
        types::ApiResponse::Success { .. } => Ok(()),
        types::ApiResponse::Failed { message } => anyhow::bail!(message),
    }
}

/// send kill request
#[tracing::instrument]
pub fn kill() -> anyhow::Result<()> {
    let resp = get_client()?
        .post("http://localhost/kill")
        .send()
        .context("failed to get response")?;
    let data: types::ApiResponse<()> = resp.json().context("failed to read response")?;
    match data {
        types::ApiResponse::Success { .. } => Ok(()),
        types::ApiResponse::Failed { message } => anyhow::bail!(message),
    }
}

pub const WORKTREES_DIR_NAME: &str = "worktrees";

fn tty_tcsetpgrp(pid: unistd::Pid) -> anyhow::Result<()> {
    use std::os::fd::AsFd;

    let tty = fs::File::open("/dev/tty").context("failed to open /dev/tty")?;
    unistd::tcsetpgrp(tty.as_fd(), pid).context("failed to tcsetpgrp")
}

/// Exec Git worktree process
pub fn do_worktree(
    args: &[String],
    branch: impl AsRef<str>,
    path: impl AsRef<Path>,
) -> anyhow::Result<i32> {
    let (worktree_name, worktree_path) = setup_worktree(&branch, &path)?;

    let config = Config {
        worktree: Some(worktree_name.clone()),
        branch: branch.as_ref().to_string(),
        ..Default::default()
    };
    change_watch_list(types::ChangeWatchRequest::Add {
        path: worktree_path.clone(),
        config,
    })
    .context("failed to add worktree to watch list")?;

    let mut iter = args.iter();
    let command = CString::new(iter.next().context("no command!")?.as_str())
        .context("failed to get C string")?;
    let rest_args = iter
        .map(|v| CString::new(v.as_str()))
        .collect::<Result<Vec<_>, _>>()
        .context("failed to get C string")?;
    // execvp requires argv[0] to be the command name
    let args: Vec<&CString> = std::iter::once(&command).chain(rest_args.iter()).collect();

    let child_pid = match unsafe { unistd::fork().context("failed to start child process")? } {
        unistd::ForkResult::Parent { child } => child,
        unistd::ForkResult::Child => {
            env::set_current_dir(&worktree_path).context("failed to change working dir")?;

            // Ignore SIGTTOU before any TTY operations to prevent blocking on macOS
            unsafe {
                let _ = signal::signal(signal::Signal::SIGTTOU, signal::SigHandler::SigIgn);
            }

            let pid = unistd::Pid::from_raw(0);
            unistd::setpgid(pid, pid).context("failed to set child's process group")?;

            // ignore tty setup error
            let _ = tty_tcsetpgrp(unistd::getpgrp());

            unistd::execvp(&command, &args).context("failed to start child process")?;
            unreachable!();
        }
    };

    unsafe {
        signal::signal(signal::Signal::SIGTTOU, signal::SigHandler::SigIgn)
            .context("failed to setup signal handling")?;
    }

    let _ = tty_tcsetpgrp(child_pid);

    let code = loop {
        match wait::waitpid(child_pid, None) {
            Ok(wait::WaitStatus::Exited(_, code)) => break code,
            Ok(wait::WaitStatus::Signaled(_, sig, _)) => break 128 + (sig as i32),
            _ => continue,
        }
    };

    let _ = tty_tcsetpgrp(unistd::getpgrp());

    change_watch_list(types::ChangeWatchRequest::Remove {
        path: worktree_path.clone(),
    })
    .context("failed to remove worktree from watch list")?;
    git::GitRepo::new(&path)
        .context("failed to open Git worktree")?
        .remove_worktree(&worktree_path)?;
    Ok(code)
}

/// Enter Git worktree dir
pub fn setup_worktree(
    branch: impl AsRef<str>,
    path: impl AsRef<Path>,
) -> anyhow::Result<(String, PathBuf)> {
    let worktree_path = worktree_path(&path, &branch)?;
    fs::create_dir_all(worktree_path.parent().unwrap())
        .context("failed to create worktree parent dir")?;
    let repo = git::GitRepo::new(&path).context("failed to setup Git repo")?;
    let name = repo
        .add_worktree(&branch, &worktree_path)
        .context("failed to setup Git worktree")?;
    tracing::info!("Git worktree setup at: {}", worktree_path.display());
    Ok((name, worktree_path))
}
/// Get Git worktree path
pub fn worktree_path(path: impl AsRef<Path>, branch: impl AsRef<str>) -> anyhow::Result<PathBuf> {
    let repo = git::GitRepo::new(&path).context("failed to setup Git repo")?;
    let repo_root = repo.get_repo_root();
    let replaced = repo_root
        .components()
        .filter_map(|v| match v {
            Component::Normal(v) => Some(v.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("-");
    let worktree_dir = daemon::cache_dir()?
        .join(WORKTREES_DIR_NAME)
        .join(&replaced)
        .join(branch.as_ref().replace("/", "-"));
    Ok(worktree_dir)
}
