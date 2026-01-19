use crate::types;
use anyhow::Context as _;
use daemonize::{Daemonize, Outcome};
use std::env;
use std::io;
use std::path::{Path, PathBuf};

mod api;

pub const SOCK_NAME: &str = "daemon.sock";
pub const PID_NAME: &str = "daemon.pid";
pub const LOG_NAME: &str = "daemon.log";
pub const WATCH_LIST_NAME: &str = "watch.json";

/// get cache directory
pub fn cache_dir() -> anyhow::Result<PathBuf> {
    if let Ok(path) = env::var("AUTOSAVE_CACHE") {
        Ok(PathBuf::from(path))
    } else {
        let home = env::home_dir().context("failed to get home dir")?;
        Ok(home.join(".cache").join("autosave"))
    }
}

/// Check daemon liveness
#[tracing::instrument]
pub fn check_daemon() -> anyhow::Result<bool> {
    Ok(std::os::unix::net::UnixStream::connect(cache_dir()?.join(SOCK_NAME)).is_ok())
}

/// Start daemon process and exit current process
#[tracing::instrument(skip(tracing_handle))]
pub fn start_daemon(tracing_handle: types::TracingReloadHandle) -> anyhow::Result<()> {
    let cache_dir = cache_dir()?;
    tracing::debug!("use cache dir: {}", cache_dir.display());
    std::fs::create_dir_all(&cache_dir).context("failed to create cache dir")?;

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cache_dir.join(LOG_NAME))
        .context("failed to open log file")?;

    let sock_path = cache_dir.join(SOCK_NAME);
    if sock_path.exists() {
        std::fs::remove_file(&sock_path).context("failed to remove old sock file")?;
    }

    let daemonize = Daemonize::new()
        .pid_file(cache_dir.join(PID_NAME))
        .chown_pid_file(true)
        .stdout(daemonize::Stdio::devnull())
        .stderr(log_file);
    match daemonize.execute() {
        Outcome::Parent(res) => {
            res.context("failed to start daemon; error from parent")?;
            // wait daemon start
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if sock_path.exists() {
                    return Ok(());
                }
            }
            anyhow::bail!("failed to wait for the daemon");
        }
        Outcome::Child(res) => {
            tracing_handle
                .modify(|layer| {
                    use tracing_subscriber::{filter, fmt, prelude::*};
                    *layer = fmt::layer()
                        .with_writer(io::stderr)
                        .with_ansi(false)
                        .with_filter(
                            filter::EnvFilter::builder()
                                .with_default_directive(filter::LevelFilter::INFO.into())
                                .from_env_lossy(),
                        )
                        .boxed();
                })
                .context("failed to update tracing logger")?;
            res.context("failed to start daemon; error from child")?;
            run_server(&sock_path)
        }
    }
}

/// Run HTTP server
///
/// The service is served via Unix socket.
#[tokio::main]
async fn run_server(sock_path: &Path) -> anyhow::Result<()> {
    let sock = tokio::net::UnixListener::bind(sock_path).context("failed to create socket")?;
    let state = types::ApiState::read_watch_list().await?;
    let app = api::routes().with_state(state);
    tracing::info!("daemon setup finished; start daemon");
    axum::serve(sock, app)
        .await
        .context("failed to serve API service")
}
