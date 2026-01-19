use crate::types;
use axum::{
    Router,
    routing::{get, post},
};

mod watch;

pub fn routes() -> Router<types::ApiState> {
    Router::new()
        .route("/watch", get(watch::get_watch_list))
        .route("/watch", post(watch::change_watch_list))
}
