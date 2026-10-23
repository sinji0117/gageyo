//! 이미지 올리기(파트너·운영자)와 내려주기.

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use gageyo_core::{
    auth::{partner_from_headers, session_user, Scope},
    files::{self, disk_path, mime_of},
    ratelimit, AppError, AppResult, AppState,
};

pub fn routes(max_bytes: usize) -> Router<AppState> {
    Router::new()
        .route("/files", post(upload).layer(DefaultBodyLimit::max(max_bytes + 64 * 1024)))
        .route("/files/{month}/{name}", get(serve))
}

/// 파트너(업체 사진·상품 이미지) 또는 운영자만 올릴 수 있습니다.
async fn upload(State(s): State<AppState>, headers: HeaderMap, mp: Multipart) -> AppResult<(StatusCode, Json<files::FileInfo>)> {
    let (user_id, workshop_id) = match partner_from_headers(&s, &headers).await {
        Ok(p) => (p.user.map(|u| u.id), Some(p.workshop_id)),
        Err(_) => match session_user(&s, &headers, Scope::Admin).await? {
            Some(u) if s.cfg.admin_emails.contains(&u.email) => (Some(u.id), None),
            _ => return Err(AppError::login_required()),
        },
    };
    let bucket = format!("upload:{}", workshop_id.as_deref().or(user_id.as_deref()).unwrap_or("-"));
    if !ratelimit::hit(&s.db, &bucket, 3600, 300).await? {
        return Err(AppError::TooMany("이미지를 너무 많이 올렸습니다. 잠시 후 다시 시도해 주세요".into()));
    }
    let (name, data) = files::read_file_field(mp, s.cfg.upload_max_bytes).await?;
    let info = files::store_image(&s, user_id.as_deref(), workshop_id.as_deref(), &name, data).await?;
    Ok((StatusCode::CREATED, Json(info)))
}

async fn serve(State(s): State<AppState>, Path((month, name)): Path<(String, String)>) -> Response {
    let rel = format!("{month}/{name}");
    let Some(path) = disk_path(&s.cfg.upload_dir, &rel) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, mime_of(&rel)),
                // 이름이 무작위라 내용이 바뀌지 않음
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            Body::from(bytes),
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
