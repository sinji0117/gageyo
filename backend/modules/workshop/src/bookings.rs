//! 예약: 고객이 업체·서비스·방문 시간을 골라 신청하고, 파트너가 확정·거절·완료 처리합니다.
//!
//! 상태: `requested` → `confirmed` → `completed`
//!                    ↘ `declined`(업체 거절)   ↘ `cancelled`(고객·업체 취소)
//!
//! 예전에는 업체가 승인됐는지, 서비스가 그 업체 것인지, 지난 시간인지 확인하지 않고 저장했습니다.
//! 이제 영업시간·휴무일·방문 가능 기간까지 확인합니다.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{NaiveDateTime, NaiveTime};
use gageyo_core::{
    audit,
    auth::{client_ip, Admin, ApprovedPartner, Customer, MaybeCustomer},
    ratelimit, schedule,
    util::{kst_now, like_term, new_id, normalize_phone, opt_text, phone_digits, req_line, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/bookings", post(create_booking))
        .route("/bookings/lookup", post(lookup_booking))
        .route("/bookings/{id}/cancel", post(cancel_booking))
        .route("/me/bookings", get(my_bookings))
        .route("/partner/bookings", get(partner_bookings))
        .route("/partner/bookings/{id}", patch(partner_update_booking))
        .route("/admin/bookings", get(admin_bookings))
        .route("/admin/bookings/{id}", patch(admin_update_booking))
}

#[derive(Debug, Deserialize)]
pub struct BookingRequest {
    pub workshop_id: String,
    pub service_id: Option<String>,
    pub vehicle: String,
    pub starts_at: NaiveDateTime,
    pub customer_name: String,
    pub customer_phone: String,
    pub note: Option<String>,
    /// 견적에서 고른 경우 그 견적 번호
    pub quote_offer_id: Option<String>,
}

/// 업체 영업시간 (예약 시간 검사용)
pub struct Hours {
    pub open: Option<NaiveTime>,
    pub close: Option<NaiveTime>,
    pub closed_days: String,
}

pub async fn approved_hours(s: &AppState, workshop_id: &str) -> AppResult<Hours> {
    let row: Option<(Option<NaiveTime>, Option<NaiveTime>, String)> =
        sqlx::query_as("SELECT open_time, close_time, closed_days FROM workshops WHERE id = ? AND status = 'approved'")
            .bind(workshop_id)
            .fetch_optional(&s.db)
            .await?;
    let (open, close, closed_days) = row.ok_or_else(|| AppError::not_found("예약할 수 있는 업체가 아닙니다"))?;
    Ok(Hours { open, close, closed_days })
}

async fn create_booking(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    headers: HeaderMap,
    Json(req): Json<BookingRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let vehicle = req_line(&req.vehicle, "차량 정보", 160)?;
    let name = req_line(&req.customer_name, "이름", 100)?;
    let phone = normalize_phone(&req.customer_phone)?;
    let note = opt_text(req.note.as_deref(), "메모", 1000)?;
    let hours = approved_hours(&s, &req.workshop_id).await?;
    schedule::check_visit_time(req.starts_at, kst_now(), hours.open, hours.close, &hours.closed_days)?;
    if let Some(service_id) = req.service_id.as_deref().filter(|v| !v.is_empty()) {
        let ok: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshop_services WHERE id = ? AND workshop_id = ? AND is_active = 1")
            .bind(service_id)
            .bind(&req.workshop_id)
            .fetch_one(&s.db)
            .await?;
        if ok == 0 {
            return Err(AppError::bad("선택한 서비스를 이 업체에서 찾을 수 없습니다"));
        }
    }
    if let Some(offer_id) = req.quote_offer_id.as_deref().filter(|v| !v.is_empty()) {
        let ok: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM quote_offers o JOIN quote_requests q ON q.id = o.quote_id \
             WHERE o.id = ? AND o.workshop_id = ? AND o.status = 'accepted'",
        )
        .bind(offer_id)
        .bind(&req.workshop_id)
        .fetch_one(&s.db)
        .await?;
        if ok == 0 {
            return Err(AppError::bad("고른 견적을 확인할 수 없습니다"));
        }
    }
    let ip = client_ip(&headers);
    if !ip.is_empty() && !ratelimit::hit(&s.db, &format!("booking:{ip}"), 3600, 20).await? {
        return Err(AppError::TooMany("예약 신청이 너무 많습니다. 잠시 후 다시 시도해 주세요".into()));
    }
    // 같은 연락처로 같은 업체·같은 시간에 두 번 누른 경우
    let dup: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bookings WHERE workshop_id = ? AND starts_at = ? AND customer_phone = ? AND status IN ('requested','confirmed')",
    )
    .bind(&req.workshop_id)
    .bind(req.starts_at)
    .bind(&phone)
    .fetch_one(&s.db)
    .await?;
    if dup > 0 {
        return Err(AppError::Conflict("같은 시간에 이미 신청한 예약이 있습니다".into()));
    }
    let id = new_id();
    sqlx::query(
        "INSERT INTO bookings (id, customer_user_id, workshop_id, service_id, quote_offer_id, vehicle, starts_at, customer_name, customer_phone, note, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'requested')",
    )
    .bind(&id)
    .bind(user.as_ref().map(|u| u.id.clone()))
    .bind(&req.workshop_id)
    .bind(req.service_id.as_deref().filter(|v| !v.is_empty()))
    .bind(req.quote_offer_id.as_deref().filter(|v| !v.is_empty()))
    .bind(&vehicle)
    .bind(req.starts_at)
    .bind(&name)
    .bind(&phone)
    .bind(&note)
    .execute(&s.db)
    .await?;
    crate::notify::booking_created(&s, &req.workshop_id, &name, &vehicle, &req.starts_at.format("%Y-%m-%d %H:%M").to_string()).await;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id, "status": "requested" }))))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BookingRow {
    pub id: String,
    pub workshop_id: String,
    pub workshop_name: String,
    pub workshop_phone: String,
    pub workshop_address: String,
    pub service_id: Option<String>,
    pub service_name: Option<String>,
    pub vehicle: String,
    pub starts_at: NaiveDateTime,
    pub customer_name: String,
    pub customer_phone: String,
    pub note: Option<String>,
    pub status: String,
    pub decline_reason: Option<String>,
    pub order_id: Option<String>,
    pub review_id: Option<String>,
    pub created_at: NaiveDateTime,
}

const BOOKING_SQL: &str = "SELECT b.id, b.workshop_id, w.name AS workshop_name, w.phone AS workshop_phone, w.address AS workshop_address, \
    b.service_id, ws.name AS service_name, b.vehicle, b.starts_at, b.customer_name, b.customer_phone, b.note, b.status, b.decline_reason, \
    b.order_id, r.id AS review_id, b.created_at \
    FROM bookings b JOIN workshops w ON w.id = b.workshop_id \
    LEFT JOIN workshop_services ws ON ws.id = b.service_id \
    LEFT JOIN reviews r ON r.booking_id = b.id";

async fn my_bookings(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<Vec<BookingRow>>> {
    Ok(Json(
        sqlx::query_as::<_, BookingRow>(&format!("{BOOKING_SQL} WHERE b.customer_user_id = ? ORDER BY b.starts_at DESC LIMIT 200"))
            .bind(&u.id)
            .fetch_all(&s.db)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct LookupInput {
    pub booking_id: String,
    pub phone: String,
}

/// 비회원 예약 확인 (예약 번호 + 연락처)
async fn lookup_booking(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<LookupInput>) -> AppResult<Json<BookingRow>> {
    let ip = client_ip(&headers);
    if !ratelimit::hit(&s.db, &format!("lookup:{ip}"), 900, 30).await? {
        return Err(AppError::TooMany("잠시 후 다시 시도해 주세요".into()));
    }
    let row = sqlx::query_as::<_, BookingRow>(&format!("{BOOKING_SQL} WHERE b.id = ?"))
        .bind(b.booking_id.trim())
        .fetch_optional(&s.db)
        .await?
        .filter(|r| !phone_digits(&b.phone).is_empty() && phone_digits(&r.customer_phone) == phone_digits(&b.phone))
        .ok_or_else(|| AppError::not_found("예약 번호와 연락처를 다시 확인해 주세요"))?;
    Ok(Json(row))
}

#[derive(Debug, Deserialize)]
pub struct CancelInput {
    pub phone: Option<String>,
    pub reason: Option<String>,
}

/// 고객 취소: 방문 시간 전, 신청·확정 상태에서만
async fn cancel_booking(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Json(b): Json<CancelInput>,
) -> AppResult<StatusCode> {
    let row: Option<(Option<String>, String, String, String, NaiveDateTime, String)> = sqlx::query_as(
        "SELECT customer_user_id, customer_phone, workshop_id, customer_name, starts_at, status FROM bookings WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&s.db)
    .await?;
    let (owner, phone, workshop_id, name, starts_at, status) = row.ok_or_else(|| AppError::not_found("예약을 찾을 수 없습니다"))?;
    let owner_ok = matches!((&owner, &user), (Some(o), Some(u)) if o == &u.id);
    let phone_ok = b.phone.as_deref().map(phone_digits).is_some_and(|p| !p.is_empty() && p == phone_digits(&phone));
    if !owner_ok && !phone_ok {
        return Err(AppError::Forbidden("예약할 때 적은 연락처를 입력해 주세요".into()));
    }
    if !["requested", "confirmed"].contains(&status.as_str()) {
        return Err(AppError::Conflict("이미 처리가 끝난 예약입니다".into()));
    }
    if starts_at <= kst_now() {
        return Err(AppError::Conflict("방문 시간이 지난 예약은 취소할 수 없습니다. 업체에 연락해 주세요".into()));
    }
    let reason = opt_text(b.reason.as_deref(), "취소 사유", 300)?;
    sqlx::query("UPDATE bookings SET status = 'cancelled', decline_reason = ? WHERE id = ?")
        .bind(reason.map(|r| format!("고객 취소: {r}")).unwrap_or_else(|| "고객 취소".into()))
        .bind(&id)
        .execute(&s.db)
        .await?;
    crate::notify::booking_cancelled_by_customer(&s, &workshop_id, &name, &starts_at.format("%Y-%m-%d %H:%M").to_string()).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct BookingQuery {
    pub status: Option<String>,
    /// `upcoming`(오늘 이후) · `past`
    pub when: Option<String>,
    pub q: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn partner_bookings(
    State(s): State<AppState>,
    ApprovedPartner(p): ApprovedPartner,
    Query(q): Query<BookingQuery>,
) -> AppResult<Json<Vec<BookingRow>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(100, 300);
    let when = q.when.as_deref().unwrap_or("");
    Ok(Json(
        sqlx::query_as::<_, BookingRow>(&format!(
            "{BOOKING_SQL} WHERE b.workshop_id = ? AND (? IS NULL OR b.status = ?) \
             AND (? <> 'upcoming' OR b.starts_at >= CURDATE()) AND (? <> 'past' OR b.starts_at < CURDATE()) \
             ORDER BY {} LIMIT ? OFFSET ?",
            if when == "upcoming" { "b.starts_at ASC" } else { "b.starts_at DESC" }
        ))
        .bind(&p.workshop_id)
        .bind(&q.status)
        .bind(&q.status)
        .bind(when)
        .bind(when)
        .bind(limit)
        .bind(offset)
        .fetch_all(&s.db)
        .await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct BookingStatusInput {
    pub status: String,
    pub reason: Option<String>,
}

/// 파트너가 바꿀 수 있는 상태 흐름
pub fn partner_transition_ok(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("requested", "confirmed") | ("requested", "declined") | ("confirmed", "completed") | ("confirmed", "cancelled") | ("requested", "completed")
    )
}

async fn partner_update_booking(
    State(s): State<AppState>,
    ApprovedPartner(p): ApprovedPartner,
    Path(id): Path<String>,
    Json(b): Json<BookingStatusInput>,
) -> AppResult<StatusCode> {
    let current: Option<String> = sqlx::query_scalar("SELECT status FROM bookings WHERE id = ? AND workshop_id = ?")
        .bind(&id)
        .bind(&p.workshop_id)
        .fetch_optional(&s.db)
        .await?;
    let current = current.ok_or_else(|| AppError::not_found("예약을 찾을 수 없습니다"))?;
    if current == b.status {
        return Ok(StatusCode::NO_CONTENT);
    }
    if !partner_transition_ok(&current, &b.status) {
        return Err(AppError::bad(format!("'{}' 상태의 예약은 '{}'(으)로 바꿀 수 없습니다", label(&current), label(&b.status))));
    }
    let reason = opt_text(b.reason.as_deref(), "사유", 300)?;
    if ["declined", "cancelled"].contains(&b.status.as_str()) && reason.is_none() {
        return Err(AppError::bad("거절·취소 사유를 적어 주세요 (고객에게 안내됩니다)"));
    }
    sqlx::query("UPDATE bookings SET status = ?, decline_reason = COALESCE(?, decline_reason) WHERE id = ? AND workshop_id = ? AND status = ?")
        .bind(&b.status)
        .bind(&reason)
        .bind(&id)
        .bind(&p.workshop_id)
        .bind(&current)
        .execute(&s.db)
        .await?;
    crate::notify::booking_status_changed(&s, &id, &b.status, reason.as_deref()).await;
    Ok(StatusCode::NO_CONTENT)
}

pub fn label(status: &str) -> &'static str {
    match status {
        "requested" => "신청",
        "confirmed" => "확정",
        "completed" => "완료",
        "declined" => "거절",
        "cancelled" => "취소",
        _ => "알 수 없음",
    }
}

async fn admin_bookings(State(s): State<AppState>, _a: Admin, Query(q): Query<BookingQuery>) -> AppResult<Json<Vec<BookingRow>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 300);
    let term = like_term(q.q.as_deref());
    Ok(Json(
        sqlx::query_as::<_, BookingRow>(&format!(
            "{BOOKING_SQL} WHERE (? IS NULL OR b.status = ?) \
             AND (? IS NULL OR w.name LIKE CONCAT('%', ?, '%') OR b.customer_name LIKE CONCAT('%', ?, '%') OR b.customer_phone LIKE CONCAT('%', ?, '%')) \
             ORDER BY b.starts_at DESC LIMIT ? OFFSET ?"
        ))
        .bind(&q.status)
        .bind(&q.status)
        .bind(&term)
        .bind(&term)
        .bind(&term)
        .bind(&term)
        .bind(limit)
        .bind(offset)
        .fetch_all(&s.db)
        .await?,
    ))
}

async fn admin_update_booking(
    State(s): State<AppState>,
    admin: Admin,
    Path(id): Path<String>,
    Json(b): Json<BookingStatusInput>,
) -> AppResult<StatusCode> {
    if !["requested", "confirmed", "completed", "cancelled", "declined"].contains(&b.status.as_str()) {
        return Err(AppError::bad("예약 상태 값이 올바르지 않습니다"));
    }
    let reason = opt_text(b.reason.as_deref(), "사유", 300)?;
    let r = sqlx::query("UPDATE bookings SET status = ?, decline_reason = COALESCE(?, decline_reason) WHERE id = ?")
        .bind(&b.status)
        .bind(&reason)
        .bind(&id)
        .execute(&s.db)
        .await?;
    if r.rows_affected() == 0 {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bookings WHERE id = ?").bind(&id).fetch_one(&s.db).await?;
        if n == 0 {
            return Err(AppError::not_found("예약을 찾을 수 없습니다"));
        }
    }
    audit::log(&s, &admin, &format!("booking.{}", b.status), "booking", &id, reason).await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::partner_transition_ok as ok;

    #[test]
    fn transitions() {
        assert!(ok("requested", "confirmed"));
        assert!(ok("requested", "declined"));
        assert!(ok("confirmed", "completed"));
        assert!(ok("confirmed", "cancelled"));
        assert!(!ok("completed", "requested"));
        assert!(!ok("declined", "confirmed"));
        assert!(!ok("cancelled", "confirmed"));
        assert!(!ok("requested", "requested"));
    }
}
