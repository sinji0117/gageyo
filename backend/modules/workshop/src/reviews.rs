//! 리뷰: 완료된 예약에 고객이 별점·후기를 남기고, 파트너가 답글을 달고, 운영자가 숨길 수 있습니다.
//! 업체 평점(`workshops.rating`, `review_count`)은 보이는 리뷰로 다시 계산합니다.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, put},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::{Admin, Customer, Partner},
    util::{new_id, req_text, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workshops/{id}/reviews", get(workshop_reviews))
        .route("/me/reviews", get(my_reviews).post(create_review))
        .route("/me/reviews/{id}", put(update_review).delete(delete_review))
        .route("/partner/reviews", get(partner_reviews))
        .route("/partner/reviews/{id}/reply", put(reply_review))
        .route("/admin/reviews", get(admin_reviews))
        .route("/admin/reviews/{id}", patch(admin_set_status))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PublicReview {
    pub id: String,
    #[serde(skip)]
    pub author_name: String,
    pub rating: i8,
    pub content: String,
    pub reply: Option<String>,
    pub replied_at: Option<NaiveDateTime>,
    pub service_name: Option<String>,
    pub created_at: NaiveDateTime,
}

pub async fn public_reviews(s: &AppState, workshop_id: &str, limit: i64, offset: i64) -> AppResult<Vec<serde_json::Value>> {
    let rows = sqlx::query_as::<_, PublicReview>(
        "SELECT r.id, u.display_name AS author_name, r.rating, r.content, r.reply, r.replied_at, ws.name AS service_name, r.created_at \
         FROM reviews r JOIN users u ON u.id = r.user_id \
         LEFT JOIN bookings b ON b.id = r.booking_id LEFT JOIN workshop_services ws ON ws.id = b.service_id \
         WHERE r.workshop_id = ? AND r.status = 'visible' ORDER BY r.created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(workshop_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let mut v = serde_json::to_value(&r).unwrap_or_default();
            v["author"] = serde_json::json!(crate::mask_name(&r.author_name));
            v
        })
        .collect())
}

async fn workshop_reviews(State(s): State<AppState>, Path(id): Path<String>, Query(p): Query<Page>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = p.limit_offset(10, 50);
    Ok(Json(public_reviews(&s, &id, limit, offset).await?))
}

/// 보이는 리뷰로 업체 평점·리뷰 수 다시 계산
pub async fn recompute_rating(s: &AppState, workshop_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE workshops w SET \
           rating = COALESCE((SELECT AVG(r.rating) FROM reviews r WHERE r.workshop_id = w.id AND r.status = 'visible'), 0), \
           review_count = (SELECT COUNT(*) FROM reviews r WHERE r.workshop_id = w.id AND r.status = 'visible') \
         WHERE w.id = ?",
    )
    .bind(workshop_id)
    .execute(&s.db)
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ReviewInput {
    pub booking_id: Option<String>,
    pub rating: i32,
    pub content: String,
}

fn check_review(b: &ReviewInput) -> AppResult<(i8, String)> {
    if !(1..=5).contains(&b.rating) {
        return Err(AppError::bad("별점은 1~5점으로 골라 주세요"));
    }
    let content = req_text(&b.content, "후기", 1000)?;
    if content.chars().count() < 5 {
        return Err(AppError::bad("후기는 5자 이상 적어 주세요"));
    }
    Ok((b.rating as i8, content))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MyReview {
    pub id: String,
    pub workshop_id: String,
    pub workshop_name: String,
    pub booking_id: Option<String>,
    pub rating: i8,
    pub content: String,
    pub reply: Option<String>,
    pub status: String,
    pub created_at: NaiveDateTime,
}

async fn my_reviews(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<Vec<MyReview>>> {
    Ok(Json(
        sqlx::query_as::<_, MyReview>(
            "SELECT r.id, r.workshop_id, w.name AS workshop_name, r.booking_id, r.rating, r.content, r.reply, r.status, r.created_at \
             FROM reviews r JOIN workshops w ON w.id = r.workshop_id WHERE r.user_id = ? ORDER BY r.created_at DESC",
        )
        .bind(&u.id)
        .fetch_all(&s.db)
        .await?,
    ))
}

/// 완료된 내 예약에만, 예약당 한 번 쓸 수 있습니다.
async fn create_review(
    State(s): State<AppState>,
    Customer(u): Customer,
    Json(b): Json<ReviewInput>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let (rating, content) = check_review(&b)?;
    let booking_id = b.booking_id.as_deref().ok_or_else(|| AppError::bad("후기를 남길 예약을 골라 주세요"))?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT workshop_id, status FROM bookings WHERE id = ? AND customer_user_id = ?")
            .bind(booking_id)
            .bind(&u.id)
            .fetch_optional(&s.db)
            .await?;
    let (workshop_id, status) = row.ok_or_else(|| AppError::not_found("예약을 찾을 수 없습니다"))?;
    if status != "completed" {
        return Err(AppError::bad("작업이 완료된 예약에만 후기를 남길 수 있습니다"));
    }
    let id = new_id();
    let r = sqlx::query("INSERT INTO reviews (id, workshop_id, booking_id, user_id, rating, content) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&workshop_id)
        .bind(booking_id)
        .bind(&u.id)
        .bind(rating)
        .bind(&content)
        .execute(&s.db)
        .await;
    if let Err(e) = r {
        if gageyo_core::error::is_duplicate(&e) {
            return Err(AppError::Conflict("이 예약에는 이미 후기를 남겼습니다".into()));
        }
        return Err(e.into());
    }
    recompute_rating(&s, &workshop_id).await?;
    crate::notify::review_posted(&s, &workshop_id, rating).await;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn update_review(
    State(s): State<AppState>,
    Customer(u): Customer,
    Path(id): Path<String>,
    Json(b): Json<ReviewInput>,
) -> AppResult<StatusCode> {
    let (rating, content) = check_review(&b)?;
    let workshop_id: Option<String> = sqlx::query_scalar("SELECT workshop_id FROM reviews WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&u.id)
        .fetch_optional(&s.db)
        .await?;
    let workshop_id = workshop_id.ok_or_else(|| AppError::not_found("후기를 찾을 수 없습니다"))?;
    sqlx::query("UPDATE reviews SET rating = ?, content = ? WHERE id = ? AND user_id = ?")
        .bind(rating)
        .bind(&content)
        .bind(&id)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    recompute_rating(&s, &workshop_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_review(State(s): State<AppState>, Customer(u): Customer, Path(id): Path<String>) -> AppResult<StatusCode> {
    let workshop_id: Option<String> = sqlx::query_scalar("SELECT workshop_id FROM reviews WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&u.id)
        .fetch_optional(&s.db)
        .await?;
    let workshop_id = workshop_id.ok_or_else(|| AppError::not_found("후기를 찾을 수 없습니다"))?;
    sqlx::query("DELETE FROM reviews WHERE id = ? AND user_id = ?").bind(&id).bind(&u.id).execute(&s.db).await?;
    recompute_rating(&s, &workshop_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ManagedReview {
    pub id: String,
    pub workshop_id: String,
    pub workshop_name: String,
    #[serde(skip)]
    pub author_name: String,
    pub rating: i8,
    pub content: String,
    pub reply: Option<String>,
    pub status: String,
    pub created_at: NaiveDateTime,
}

const MANAGED_SQL: &str = "SELECT r.id, r.workshop_id, w.name AS workshop_name, u.display_name AS author_name, r.rating, r.content, \
    r.reply, r.status, r.created_at FROM reviews r JOIN workshops w ON w.id = r.workshop_id JOIN users u ON u.id = r.user_id";

fn managed_json(rows: Vec<ManagedReview>) -> Vec<serde_json::Value> {
    rows.into_iter()
        .map(|r| {
            let mut v = serde_json::to_value(&r).unwrap_or_default();
            v["author"] = serde_json::json!(crate::mask_name(&r.author_name));
            v
        })
        .collect()
}

async fn partner_reviews(State(s): State<AppState>, p: Partner, Query(pg): Query<Page>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = pg.limit_offset(30, 100);
    let rows = sqlx::query_as::<_, ManagedReview>(&format!(
        "{MANAGED_SQL} WHERE r.workshop_id = ? ORDER BY r.created_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(&p.workshop_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(managed_json(rows)))
}

#[derive(Debug, Deserialize)]
pub struct ReplyInput {
    pub reply: String,
}

/// 답글 달기 (비우면 답글 지움)
async fn reply_review(State(s): State<AppState>, p: Partner, Path(id): Path<String>, Json(b): Json<ReplyInput>) -> AppResult<StatusCode> {
    let reply = b.reply.trim();
    let r = if reply.is_empty() {
        sqlx::query("UPDATE reviews SET reply = NULL, replied_at = NULL WHERE id = ? AND workshop_id = ?")
            .bind(&id)
            .bind(&p.workshop_id)
            .execute(&s.db)
            .await?
    } else {
        let reply = req_text(reply, "답글", 1000)?;
        sqlx::query("UPDATE reviews SET reply = ?, replied_at = NOW() WHERE id = ? AND workshop_id = ?")
            .bind(reply)
            .bind(&id)
            .bind(&p.workshop_id)
            .execute(&s.db)
            .await?
    };
    if r.rows_affected() == 0 {
        // 같은 내용으로 다시 저장하면 0 이 나올 수 있어 존재 여부를 다시 확인
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reviews WHERE id = ? AND workshop_id = ?")
            .bind(&id)
            .bind(&p.workshop_id)
            .fetch_one(&s.db)
            .await?;
        if n == 0 {
            return Err(AppError::not_found("후기를 찾을 수 없습니다"));
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AdminReviewQuery {
    pub status: Option<String>,
    pub workshop_id: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn admin_reviews(State(s): State<AppState>, _a: Admin, Query(q): Query<AdminReviewQuery>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 200);
    let rows = sqlx::query_as::<_, ManagedReview>(&format!(
        "{MANAGED_SQL} WHERE (? IS NULL OR r.status = ?) AND (? IS NULL OR r.workshop_id = ?) ORDER BY r.created_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(&q.status)
    .bind(&q.status)
    .bind(&q.workshop_id)
    .bind(&q.workshop_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(managed_json(rows)))
}

#[derive(Debug, Deserialize)]
pub struct StatusInput {
    pub status: String,
    pub reason: Option<String>,
}

async fn admin_set_status(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<StatusInput>) -> AppResult<StatusCode> {
    if !["visible", "hidden"].contains(&b.status.as_str()) {
        return Err(AppError::bad("리뷰 상태 값이 올바르지 않습니다"));
    }
    let workshop_id: Option<String> =
        sqlx::query_scalar("SELECT workshop_id FROM reviews WHERE id = ?").bind(&id).fetch_optional(&s.db).await?;
    let workshop_id = workshop_id.ok_or_else(|| AppError::not_found("후기를 찾을 수 없습니다"))?;
    sqlx::query("UPDATE reviews SET status = ? WHERE id = ?").bind(&b.status).bind(&id).execute(&s.db).await?;
    recompute_rating(&s, &workshop_id).await?;
    audit::log(&s, &admin, &format!("review.{}", b.status), "review", &id, b.reason.clone()).await;
    Ok(StatusCode::NO_CONTENT)
}
