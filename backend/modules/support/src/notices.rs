//! 공지사항

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::{Admin, Partner},
    util::{new_id, req_line, req_text},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/notices", get(public_notices))
        .route("/partner/notices", get(partner_notices))
        .route("/admin/notices", get(admin_notices).post(create_notice))
        .route("/admin/notices/{id}", put(update_notice).delete(delete_notice))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Notice {
    pub id: String,
    pub audience: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub is_published: bool,
    pub created_at: NaiveDateTime,
}

const COLS: &str = "id, audience, title, body, pinned, is_published, created_at";

#[derive(Debug, Deserialize)]
pub struct LimitQuery {
    pub limit: Option<i64>,
}

async fn public_notices(State(s): State<AppState>, Query(q): Query<LimitQuery>) -> AppResult<Json<Vec<Notice>>> {
    Ok(Json(
        sqlx::query_as::<_, Notice>(&format!(
            "SELECT {COLS} FROM notices WHERE is_published = 1 AND audience IN ('all','customer') ORDER BY pinned DESC, created_at DESC LIMIT ?"
        ))
        .bind(q.limit.unwrap_or(20).clamp(1, 100))
        .fetch_all(&s.db)
        .await?,
    ))
}

async fn partner_notices(State(s): State<AppState>, _p: Partner, Query(q): Query<LimitQuery>) -> AppResult<Json<Vec<Notice>>> {
    Ok(Json(
        sqlx::query_as::<_, Notice>(&format!(
            "SELECT {COLS} FROM notices WHERE is_published = 1 AND audience IN ('all','partner') ORDER BY pinned DESC, created_at DESC LIMIT ?"
        ))
        .bind(q.limit.unwrap_or(20).clamp(1, 100))
        .fetch_all(&s.db)
        .await?,
    ))
}

async fn admin_notices(State(s): State<AppState>, _a: Admin) -> AppResult<Json<Vec<Notice>>> {
    Ok(Json(sqlx::query_as::<_, Notice>(&format!("SELECT {COLS} FROM notices ORDER BY pinned DESC, created_at DESC LIMIT 300")).fetch_all(&s.db).await?))
}

#[derive(Debug, Deserialize)]
pub struct NoticeInput {
    pub audience: String,
    pub title: String,
    pub body: String,
    pub pinned: Option<bool>,
    pub is_published: Option<bool>,
}

fn check(b: &NoticeInput) -> AppResult<(String, String)> {
    if !["all", "customer", "partner"].contains(&b.audience.as_str()) {
        return Err(AppError::bad("공지 대상을 골라 주세요"));
    }
    Ok((req_line(&b.title, "제목", 200)?, req_text(&b.body, "내용", 10_000)?))
}

async fn create_notice(State(s): State<AppState>, admin: Admin, Json(b): Json<NoticeInput>) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let (title, body) = check(&b)?;
    let id = new_id();
    sqlx::query("INSERT INTO notices (id, audience, title, body, pinned, is_published) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&b.audience)
        .bind(&title)
        .bind(&body)
        .bind(b.pinned.unwrap_or(false))
        .bind(b.is_published.unwrap_or(true))
        .execute(&s.db)
        .await?;
    audit::log(&s, &admin, "notice.create", "notice", &id, Some(title)).await;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn update_notice(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<NoticeInput>) -> AppResult<StatusCode> {
    let (title, body) = check(&b)?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notices WHERE id = ?").bind(&id).fetch_one(&s.db).await?;
    if n == 0 {
        return Err(AppError::not_found("공지를 찾을 수 없습니다"));
    }
    sqlx::query("UPDATE notices SET audience = ?, title = ?, body = ?, pinned = ?, is_published = ? WHERE id = ?")
        .bind(&b.audience)
        .bind(&title)
        .bind(&body)
        .bind(b.pinned.unwrap_or(false))
        .bind(b.is_published.unwrap_or(true))
        .bind(&id)
        .execute(&s.db)
        .await?;
    audit::log(&s, &admin, "notice.update", "notice", &id, Some(title)).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_notice(State(s): State<AppState>, admin: Admin, Path(id): Path<String>) -> AppResult<StatusCode> {
    let r = sqlx::query("DELETE FROM notices WHERE id = ?").bind(&id).execute(&s.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::not_found("공지를 찾을 수 없습니다"));
    }
    audit::log(&s, &admin, "notice.delete", "notice", &id, None).await;
    Ok(StatusCode::NO_CONTENT)
}
