use crate::types;
use axum::{Json, extract::State};

/// API endpoint to get watch list
pub async fn get_watch_list(
    State(state): State<types::ApiState>,
) -> Json<types::ApiResponse<types::WatchListResponse>> {
    let paths = state.watch_list().await.keys().cloned().collect();
    Json(types::ApiResponse::Success {
        data: types::WatchListResponse { paths },
    })
}

/// API endpoint to add/remove watch list
pub async fn change_watch_list(
    State(state): State<types::ApiState>,
    Json(req): Json<types::ChangeWatchRequest>,
) -> Json<types::ApiResponse<()>> {
    match req {
        types::ChangeWatchRequest::Add { path, config } => {
            if let Err(e) = state.append_watch_dir(&path, config).await {
                return Json(types::ApiResponse::Failed {
                    message: e.to_string(),
                });
            }
        }
        types::ChangeWatchRequest::Remove { path } => {
            if let Err(e) = state.remove_watch_dir(&path).await {
                return Json(types::ApiResponse::Failed {
                    message: e.to_string(),
                });
            }
        }
    }
    if let Err(e) = state.write_watch_list().await {
        Json(types::ApiResponse::Failed {
            message: e.to_string(),
        })
    } else {
        Json(types::ApiResponse::Success { data: () })
    }
}
