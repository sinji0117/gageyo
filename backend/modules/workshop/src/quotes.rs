//! 견적: 고객이 요청서를 한 번 쓰면 분야·지역이 맞는 업체들이 견적을 보내고, 고객이 하나를 골라 예약합니다.
//!
//! 상태: `open`(받는 중) → `accepted`(고객이 선택) / `cancelled`(고객 취소) / `closed`(14일 지나 마감·운영자 마감)
//! 견적 제안 상태: `sent` → `accepted` / `declined`(다른 업체 선택) / `withdrawn`(업체가 철회)
//!
//! 비회원 요청은 요청 번호(UUID)와 연락처로 확인합니다. 회원 요청은 마이페이지에 쌓입니다.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::{client_ip, Admin, ApprovedPartner, Customer, MaybeCustomer, SessionUser},
    ratelimit,
    util::{kst_now, like_term, new_id, normalize_phone, opt_text, phone_digits, req_line, req_text, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

/// 견적 요청을 받는 기간 (일)
pub const OPEN_DAYS: i64 = 14;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/quotes", post(create_quote))
        .route("/quotes/{id}", get(quote_detail))
        .route("/quotes/{id}/offers", get(list_offers))
        .route("/quotes/{id}/accept", post(accept_offer))
        .route("/quotes/{id}/cancel", post(cancel_quote))
        .route("/me/quotes", get(my_quotes))
        .route("/partner/quotes", get(partner_quotes))
        .route("/partner/quotes/{id}/offers", post(partner_create_offer))
        .route("/partner/offers", get(partner_offers))
        .route("/partner/offers/{id}", delete(withdraw_offer))
        .route("/admin/quotes", get(admin_quotes))
        .route("/admin/quotes/{id}", patch(admin_set_status))
}

#[derive(Debug, Deserialize)]
pub struct QuoteRequest {
    pub vehicle: String,
    pub service_category: String,
    pub description: String,
    pub district: String,
    pub customer_phone: String,
    pub customer_name: Option<String>,
}

async fn create_quote(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    headers: HeaderMap,
    Json(req): Json<QuoteRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let vehicle = req_line(&req.vehicle, "차량 정보", 160)?;
    let description = req_text(&req.description, "요청 내용", 2000)?;
    let district = req_line(&req.district, "지역", 100)?;
    let phone = normalize_phone(&req.customer_phone)?;
    let name = opt_text(req.customer_name.as_deref(), "이름", 100)?.or_else(|| user.as_ref().map(|u| u.display_name.clone()));
    let category = req.service_category.trim().to_string();
    if category != "기타" && !crate::categories::is_active_category(&s, &category).await? {
        return Err(AppError::bad("서비스 종류를 다시 골라 주세요"));
    }
    let ip = client_ip(&headers);
    if !ip.is_empty() && !ratelimit::hit(&s.db, &format!("quote:{ip}"), 3600, 20).await? {
        return Err(AppError::TooMany("견적 요청이 너무 많습니다. 잠시 후 다시 시도해 주세요".into()));
    }
    let id = new_id();
    sqlx::query(
        "INSERT INTO quote_requests (id, customer_user_id, customer_name, vehicle, service_category, description, district, customer_phone, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'open')",
    )
    .bind(&id)
    .bind(user.as_ref().map(|u| u.id.clone()))
    .bind(&name)
    .bind(&vehicle)
    .bind(&category)
    .bind(&description)
    .bind(&district)
    .bind(&phone)
    .execute(&s.db)
    .await?;
    crate::notify::quote_created(&s, &category, &district, &vehicle).await;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id, "status": "open" }))))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct QuoteRow {
    pub id: String,
    pub customer_user_id: Option<String>,
    pub customer_name: Option<String>,
    pub vehicle: String,
    pub service_category: String,
    pub description: String,
    pub district: String,
    pub customer_phone: String,
    pub status: String,
    pub accepted_offer_id: Option<String>,
    pub created_at: NaiveDateTime,
}

const QUOTE_COLS: &str = "id, customer_user_id, customer_name, vehicle, service_category, description, district, customer_phone, \
    status, accepted_offer_id, created_at";

/// 오래된 요청은 읽을 때 마감 처리
fn effective_status(q: &QuoteRow) -> &str {
    if q.status == "open" && q.created_at < kst_now() - chrono::Duration::days(OPEN_DAYS) {
        "closed"
    } else {
        &q.status
    }
}

/// 요청을 볼 수 있는지: 회원 요청은 그 회원, 비회원 요청은 연락처가 맞을 때
async fn load_with_access(s: &AppState, id: &str, user: Option<&SessionUser>, phone: Option<&str>) -> AppResult<QuoteRow> {
    let q = sqlx::query_as::<_, QuoteRow>(&format!("SELECT {QUOTE_COLS} FROM quote_requests WHERE id = ?"))
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| AppError::not_found("견적 요청을 찾을 수 없습니다"))?;
    let owner_ok = matches!((&q.customer_user_id, user), (Some(owner), Some(u)) if owner == &u.id);
    let phone_ok = phone.map(phone_digits).is_some_and(|p| !p.is_empty() && p == phone_digits(&q.customer_phone));
    if !owner_ok && !phone_ok {
        return Err(AppError::Forbidden("요청할 때 적은 연락처를 입력해 주세요".into()));
    }
    Ok(q)
}

#[derive(Debug, Deserialize)]
pub struct PhoneQuery {
    pub phone: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Offer {
    pub id: String,
    pub workshop_id: String,
    pub workshop_name: String,
    pub workshop_district: String,
    pub workshop_rating: f32,
    pub workshop_review_count: i32,
    pub amount: u64,
    pub description: String,
    pub valid_until: Option<NaiveDateTime>,
    pub status: String,
    pub created_at: NaiveDateTime,
}

async fn offers_of(s: &AppState, quote_id: &str) -> AppResult<Vec<Offer>> {
    Ok(sqlx::query_as::<_, Offer>(
        "SELECT o.id, o.workshop_id, w.name AS workshop_name, w.district AS workshop_district, w.rating AS workshop_rating, \
         w.review_count AS workshop_review_count, o.amount, o.description, o.valid_until, o.status, o.created_at \
         FROM quote_offers o JOIN workshops w ON w.id = o.workshop_id \
         WHERE o.quote_id = ? AND o.status <> 'withdrawn' ORDER BY o.amount, o.created_at",
    )
    .bind(quote_id)
    .fetch_all(&s.db)
    .await?)
}

/// 요청 번호만으로 받은 견적 목록 보기 (예전 화면 호환 — 연락처·요청 내용은 보이지 않음)
async fn list_offers(State(s): State<AppState>, Path(id): Path<String>) -> AppResult<Json<Vec<Offer>>> {
    Ok(Json(offers_of(&s, &id).await?))
}

async fn quote_detail(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Query(q): Query<PhoneQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let quote = load_with_access(&s, &id, user.as_ref(), q.phone.as_deref()).await?;
    let offers = offers_of(&s, &id).await?;
    let mut v = serde_json::to_value(&quote)?;
    v["status"] = serde_json::json!(effective_status(&quote));
    v["offers"] = serde_json::to_value(&offers)?;
    Ok(Json(v))
}

#[derive(Debug, Deserialize)]
pub struct AcceptInput {
    pub offer_id: String,
    pub phone: Option<String>,
}

/// 견적 하나를 고릅니다. 고른 업체 정보를 돌려주면 화면이 바로 예약 신청서를 엽니다.
async fn accept_offer(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Json(b): Json<AcceptInput>,
) -> AppResult<Json<serde_json::Value>> {
    let quote = load_with_access(&s, &id, user.as_ref(), b.phone.as_deref()).await?;
    if effective_status(&quote) != "open" {
        return Err(AppError::Conflict("이미 마감된 견적 요청입니다".into()));
    }
    let mut tx = s.db.begin().await?;
    let offer: Option<(String, String, u64)> = sqlx::query_as(
        "SELECT o.workshop_id, w.name, o.amount FROM quote_offers o JOIN workshops w ON w.id = o.workshop_id \
         WHERE o.id = ? AND o.quote_id = ? AND o.status = 'sent' AND w.status = 'approved' FOR UPDATE",
    )
    .bind(&b.offer_id)
    .bind(&id)
    .fetch_optional(&mut *tx)
    .await?;
    let (workshop_id, workshop_name, amount) = offer.ok_or_else(|| AppError::not_found("선택할 수 있는 견적이 아닙니다"))?;
    let r = sqlx::query("UPDATE quote_requests SET status = 'accepted', accepted_offer_id = ? WHERE id = ? AND status = 'open'")
        .bind(&b.offer_id)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict("이미 마감된 견적 요청입니다".into()));
    }
    sqlx::query("UPDATE quote_offers SET status = IF(id = ?, 'accepted', 'declined') WHERE quote_id = ? AND status = 'sent'")
        .bind(&b.offer_id)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::notify::offer_accepted(&s, &workshop_id, &quote.vehicle, &quote.customer_phone).await;
    Ok(Json(serde_json::json!({
        "quote_id": id, "offer_id": b.offer_id, "workshop_id": workshop_id, "workshop_name": workshop_name, "amount": amount,
        "vehicle": quote.vehicle, "customer_name": quote.customer_name, "customer_phone": quote.customer_phone,
    })))
}

async fn cancel_quote(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Json(b): Json<PhoneQuery>,
) -> AppResult<StatusCode> {
    load_with_access(&s, &id, user.as_ref(), b.phone.as_deref()).await?;
    let r = sqlx::query("UPDATE quote_requests SET status = 'cancelled' WHERE id = ? AND status = 'open'")
        .bind(&id)
        .execute(&s.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict("받는 중인 요청만 취소할 수 있습니다".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn my_quotes(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<Vec<serde_json::Value>>> {
    let rows: Vec<(String, String, String, String, String, NaiveDateTime, i64, Option<u64>)> = sqlx::query_as(
        "SELECT q.id, q.vehicle, q.service_category, q.district, q.status, q.created_at, \
           (SELECT COUNT(*) FROM quote_offers o WHERE o.quote_id = q.id AND o.status <> 'withdrawn'), \
           (SELECT MIN(o.amount) FROM quote_offers o WHERE o.quote_id = q.id AND o.status <> 'withdrawn') \
         FROM quote_requests q WHERE q.customer_user_id = ? ORDER BY q.created_at DESC LIMIT 100",
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    let cutoff = kst_now() - chrono::Duration::days(OPEN_DAYS);
    Ok(Json(
        rows.into_iter()
            .map(|(id, vehicle, category, district, status, created_at, offers, min_amount)| {
                let status = if status == "open" && created_at < cutoff { "closed".to_string() } else { status };
                serde_json::json!({"id": id, "vehicle": vehicle, "service_category": category, "district": district,
                    "status": status, "created_at": created_at, "offer_count": offers, "min_amount": min_amount})
            })
            .collect(),
    ))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PartnerQuote {
    pub id: String,
    pub vehicle: String,
    pub service_category: String,
    pub description: String,
    pub district: String,
    pub customer_phone: String,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub offer_count: i64,
}

/// 우리 업체 분야·지역에 맞는, 아직 답하지 않은 받는 중 요청
async fn partner_quotes(State(s): State<AppState>, ApprovedPartner(p): ApprovedPartner) -> AppResult<Json<Vec<serde_json::Value>>> {
    let rows = sqlx::query_as::<_, PartnerQuote>(
        "SELECT q.id, q.vehicle, q.service_category, q.description, q.district, q.customer_phone, q.status, q.created_at, \
           (SELECT COUNT(*) FROM quote_offers o2 WHERE o2.quote_id = q.id AND o2.status <> 'withdrawn') AS offer_count \
         FROM quote_requests q JOIN workshops w ON w.id = ? \
         WHERE q.status = 'open' AND q.created_at >= NOW() - INTERVAL ? DAY \
           AND q.district LIKE CONCAT('%', w.district, '%') \
           AND (q.service_category = '기타' OR EXISTS (SELECT 1 FROM workshop_categories c WHERE c.workshop_id = w.id AND c.category = q.service_category)) \
           AND NOT EXISTS (SELECT 1 FROM quote_offers o WHERE o.quote_id = q.id AND o.workshop_id = w.id) \
         ORDER BY q.created_at DESC LIMIT 200",
    )
    .bind(&p.workshop_id)
    .bind(OPEN_DAYS)
    .fetch_all(&s.db)
    .await?;
    // 견적을 보내기 전에는 연락처를 가립니다 (고객 개인정보 보호)
    Ok(Json(
        rows.into_iter()
            .map(|r| {
                let mut v = serde_json::to_value(&r).unwrap_or_default();
                v["customer_phone"] = serde_json::json!(gageyo_core::util::mask_phone(&r.customer_phone));
                v
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct OfferRequest {
    pub amount: i64,
    pub description: String,
    pub valid_until: Option<NaiveDateTime>,
    /// 예전 화면이 보내던 값 (무시함 — 업체는 로그인 정보로 정함)
    #[allow(dead_code)]
    pub workshop_id: Option<String>,
}

async fn partner_create_offer(
    State(s): State<AppState>,
    ApprovedPartner(p): ApprovedPartner,
    Path(quote_id): Path<String>,
    Json(b): Json<OfferRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if !(0..=100_000_000).contains(&b.amount) {
        return Err(AppError::bad("견적 금액은 0원 ~ 1억 원 사이로 입력해 주세요"));
    }
    let description = req_text(&b.description, "작업 내용", 2000)?;
    if let Some(v) = b.valid_until {
        if v < kst_now() {
            return Err(AppError::bad("견적 유효 기간은 오늘 이후로 정해 주세요"));
        }
    }
    let offer_id = new_id();
    let r = sqlx::query(
        "INSERT INTO quote_offers (id, quote_id, workshop_id, amount, description, valid_until, status) \
         SELECT ?, q.id, ?, ?, ?, ?, 'sent' FROM quote_requests q \
         WHERE q.id = ? AND q.status = 'open' AND q.created_at >= NOW() - INTERVAL ? DAY",
    )
    .bind(&offer_id)
    .bind(&p.workshop_id)
    .bind(b.amount as u64)
    .bind(&description)
    .bind(b.valid_until)
    .bind(&quote_id)
    .bind(OPEN_DAYS)
    .execute(&s.db)
    .await;
    match r {
        Ok(r) if r.rows_affected() == 0 => return Err(AppError::not_found("견적 요청을 찾을 수 없거나 마감되었습니다")),
        Ok(_) => {}
        Err(e) if gageyo_core::error::is_duplicate(&e) => return Err(AppError::Conflict("이미 이 요청에 견적을 보냈습니다".into())),
        Err(e) => return Err(e.into()),
    }
    crate::notify::offer_received(&s, &quote_id, &p.workshop_name, b.amount as u64).await;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": offer_id }))))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PartnerOffer {
    pub id: String,
    pub quote_id: String,
    pub vehicle: String,
    pub service_category: String,
    pub district: String,
    pub customer_name: Option<String>,
    pub customer_phone: String,
    pub quote_status: String,
    pub amount: u64,
    pub description: String,
    pub status: String,
    pub created_at: NaiveDateTime,
}

/// 우리가 보낸 견적들. 고객이 우리 견적을 고른 경우에만 연락처 전체를 보여 줍니다.
async fn partner_offers(State(s): State<AppState>, ApprovedPartner(p): ApprovedPartner, Query(pg): Query<Page>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = pg.limit_offset(50, 200);
    let rows = sqlx::query_as::<_, PartnerOffer>(
        "SELECT o.id, o.quote_id, q.vehicle, q.service_category, q.district, q.customer_name, q.customer_phone, q.status AS quote_status, \
         o.amount, o.description, o.status, o.created_at \
         FROM quote_offers o JOIN quote_requests q ON q.id = o.quote_id WHERE o.workshop_id = ? \
         ORDER BY o.created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&p.workshop_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| {
                let mut v = serde_json::to_value(&r).unwrap_or_default();
                if r.status != "accepted" {
                    v["customer_phone"] = serde_json::json!(gageyo_core::util::mask_phone(&r.customer_phone));
                    v["customer_name"] = serde_json::Value::Null;
                }
                v
            })
            .collect(),
    ))
}

async fn withdraw_offer(State(s): State<AppState>, ApprovedPartner(p): ApprovedPartner, Path(id): Path<String>) -> AppResult<StatusCode> {
    let r = sqlx::query("UPDATE quote_offers SET status = 'withdrawn' WHERE id = ? AND workshop_id = ? AND status = 'sent'")
        .bind(&id)
        .bind(&p.workshop_id)
        .execute(&s.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict("고객이 아직 고르지 않은 견적만 철회할 수 있습니다".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AdminQuoteQuery {
    pub status: Option<String>,
    pub q: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn admin_quotes(State(s): State<AppState>, _a: Admin, Query(q): Query<AdminQuoteQuery>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 200);
    let term = like_term(q.q.as_deref());
    let rows: Vec<(String, String, String, String, String, String, String, NaiveDateTime, i64)> = sqlx::query_as(
        "SELECT q.id, q.vehicle, q.service_category, q.description, q.district, q.customer_phone, q.status, q.created_at, \
           (SELECT COUNT(*) FROM quote_offers o WHERE o.quote_id = q.id AND o.status <> 'withdrawn') \
         FROM quote_requests q WHERE (? IS NULL OR q.status = ?) \
           AND (? IS NULL OR q.vehicle LIKE CONCAT('%', ?, '%') OR q.district LIKE CONCAT('%', ?, '%') OR q.customer_phone LIKE CONCAT('%', ?, '%')) \
         ORDER BY q.created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&q.status)
    .bind(&q.status)
    .bind(&term)
    .bind(&term)
    .bind(&term)
    .bind(&term)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, vehicle, category, description, district, phone, status, created_at, offers)| {
                serde_json::json!({"id": id, "vehicle": vehicle, "service_category": category, "description": description,
                    "district": district, "customer_phone": phone, "status": status, "created_at": created_at, "offer_count": offers})
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct StatusInput {
    pub status: String,
}

async fn admin_set_status(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<StatusInput>) -> AppResult<StatusCode> {
    if !["open", "closed", "cancelled"].contains(&b.status.as_str()) {
        return Err(AppError::bad("견적 요청 상태 값이 올바르지 않습니다"));
    }
    let r = sqlx::query("UPDATE quote_requests SET status = ? WHERE id = ?").bind(&b.status).bind(&id).execute(&s.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::not_found("견적 요청을 찾을 수 없습니다"));
    }
    audit::log(&s, &admin, &format!("quote.{}", b.status), "quote", &id, None).await;
    Ok(StatusCode::NO_CONTENT)
}
