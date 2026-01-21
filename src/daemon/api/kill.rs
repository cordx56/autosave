use crate::types;
use axum::{Json, extract::State};
use std::sync::LazyLock;
use tokio::sync::Notify;

static NOTIFY: LazyLock<Notify> = LazyLock::new(Notify::new);

/// API endpoint to kill the daemon
pub async fn kill(State(state): State<types::ApiState>) -> Json<types::ApiResponse<()>> {
    NOTIFY.notify_waiters();
    if let Err(e) = state.write_watch_list().await {
        Json(types::ApiResponse::Failed {
            message: e.to_string(),
        })
    } else {
        Json(types::ApiResponse::Success { data: () })
    }
}

pub async fn kill_signal() {
    NOTIFY.notified().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
}
