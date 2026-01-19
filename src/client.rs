use crate::{daemon, types};
use anyhow::Context as _;
use reqwest::blocking::Client;
use std::path::PathBuf;

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
