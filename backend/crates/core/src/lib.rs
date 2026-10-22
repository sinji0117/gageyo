use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;

pub mod response {
    use super::*;
    #[derive(Serialize)]
    pub struct ApiError { pub error: String }
    pub fn error(status: StatusCode, message: impl Into<String>) -> Response {
        (status, Json(ApiError { error: message.into() })).into_response()
    }
}
