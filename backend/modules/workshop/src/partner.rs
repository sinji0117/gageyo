//! 파트너(업체) 계정과 업체 정보.
//!
//! - 입점 신청: 계정(이메일·비밀번호) + 업체 정보 → 업체는 `pending`(승인 대기). 운영자가 승인하면 고객 화면에 보입니다.
//! - 로그인: `partner.gageyo.com` 에서 이메일·비밀번호 (예전 파트너 키도 계속 받음)
//! - 업체 정보: 소개·전화·영업시간·휴무일·대표 사진·위치는 바로 바뀝니다.
//!   상호·분야·지역·주소·사업자 정보는 승인된 업체라면 **변경 요청**으로 운영자 승인 뒤 바뀝니다
//!   (승인 전·반려된 업체는 바로 고치고 다시 심사를 받습니다).

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use chrono::{NaiveDateTime, NaiveTime};
use gageyo_core::{
    auth::{self, client_ip, Partner, Scope},
    files::check_image_url,
    schedule,
    util::{check_coords, new_id, normalize_business_number, normalize_email, normalize_phone, opt_text, req_line, req_text},
    verify, AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/partner/auth/send-code", post(send_code))
        .route("/partner/apply", post(apply))
        .route("/partner/auth/login", post(login))
        .route("/partner/auth/logout", post(logout))
        .route("/partner/me", get(me))
        .route("/partner/dashboard", get(dashboard))
        .route("/partner/profile", put(update_profile))
        .route("/partner/change-requests", get(list_change_requests).post(request_change))
        .route("/partner/photos", get(list_photos).post(add_photo))
        .route("/partner/photos/{id}", delete(delete_photo))
        .route("/partner/services", get(partner_services).post(create_service))
        .route("/partner/services/{id}", patch(update_service).delete(delete_service))
}

#[derive(Debug, Deserialize)]
pub struct EmailInput {
    pub email: String,
}

async fn send_code(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<EmailInput>) -> AppResult<Json<serde_json::Value>> {
    let email = normalize_email(&b.email)?;
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?").bind(&email).fetch_one(&s.db).await?;
    if exists > 0 {
        return Err(AppError::Conflict("이미 가입된 이메일입니다. 로그인해 주세요".into()));
    }
    verify::send_code(&s, "partner", &email, &client_ip(&headers)).await?;
    Ok(Json(serde_json::json!({ "sent": true, "ttl_minutes": verify::CODE_TTL_MINUTES })))
}

/// 업체 기본 정보 (입점 신청·변경 요청·운영자 수정이 함께 씀)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct WorkshopCore {
    pub name: String,
    pub categories: Vec<String>,
    pub district: String,
    pub address: String,
    pub business_number: Option<String>,
    pub representative: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CleanCore {
    pub name: String,
    pub categories: Vec<String>,
    pub district: String,
    pub address: String,
    pub business_number: Option<String>,
    pub representative: Option<String>,
}

pub async fn clean_core(s: &AppState, c: &WorkshopCore) -> AppResult<CleanCore> {
    let name = req_line(&c.name, "상호", 160)?;
    let district = req_line(&c.district, "지역(구·군)", 100)?;
    let address = req_line(&c.address, "주소", 255)?;
    let mut categories: Vec<String> = Vec::new();
    for cat in &c.categories {
        let cat = cat.trim().to_string();
        if !cat.is_empty() && !categories.contains(&cat) {
            if !crate::categories::is_active_category(s, &cat).await? {
                return Err(AppError::bad(format!("'{cat}' 분야를 찾을 수 없습니다")));
            }
            categories.push(cat);
        }
    }
    if categories.is_empty() {
        return Err(AppError::bad("서비스 분야를 하나 이상 골라 주세요"));
    }
    if categories.len() > 8 {
        return Err(AppError::bad("서비스 분야는 8개까지 고를 수 있습니다"));
    }
    let business_number = match c.business_number.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Some(normalize_business_number(v)?),
        None => None,
    };
    let representative = opt_text(c.representative.as_deref(), "대표자", 100)?;
    Ok(CleanCore { name, categories, district, address, business_number, representative })
}

/// 업체 분야 다시 쓰기 (첫 번째가 대표 분야)
pub async fn replace_categories(conn: &mut sqlx::MySqlConnection, workshop_id: &str, categories: &[String]) -> AppResult<()> {
    sqlx::query("DELETE FROM workshop_categories WHERE workshop_id = ?").bind(workshop_id).execute(&mut *conn).await?;
    for c in categories {
        sqlx::query("INSERT IGNORE INTO workshop_categories (workshop_id, category) VALUES (?, ?)")
            .bind(workshop_id)
            .bind(c)
            .execute(&mut *conn)
            .await?;
    }
    if let Some(first) = categories.first() {
        sqlx::query("UPDATE workshops SET category = ? WHERE id = ?").bind(first).bind(workshop_id).execute(&mut *conn).await?;
    }
    Ok(())
}

pub fn new_slug() -> String {
    format!("w-{}", &auth::random_token()[..12])
}

#[derive(Debug, Deserialize)]
pub struct ApplyInput {
    pub email: String,
    pub password: String,
    /// 담당자 이름
    pub display_name: String,
    pub phone: String,
    pub email_code: Option<String>,
    pub agree_terms: bool,
    pub workshop: ApplyWorkshop,
}

#[derive(Debug, Deserialize)]
pub struct ApplyWorkshop {
    #[serde(flatten)]
    pub core: WorkshopCore,
    pub phone: String,
    pub description: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// 입점 신청 = 파트너 계정 만들기 + 업체 등록(승인 대기). 바로 로그인 상태가 됩니다.
async fn apply(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<ApplyInput>) -> AppResult<Response> {
    if !b.agree_terms {
        return Err(AppError::bad("파트너 이용약관과 개인정보 처리에 동의해 주세요"));
    }
    let email = normalize_email(&b.email)?;
    auth::check_password(&b.password)?;
    let display_name = req_line(&b.display_name, "담당자 이름", 100)?;
    let contact_phone = normalize_phone(&b.phone)?;
    let core = clean_core(&s, &b.workshop.core).await?;
    if core.business_number.is_none() {
        return Err(AppError::bad("사업자등록번호를 입력해 주세요"));
    }
    let shop_phone = normalize_phone(&b.workshop.phone)?;
    let description = req_text(&b.workshop.description, "업체 소개", 3000)?;
    let (lat, lng) = check_coords(b.workshop.latitude, b.workshop.longitude)?;
    let ip = client_ip(&headers);
    if !ip.is_empty() && !gageyo_core::ratelimit::hit(&s.db, &format!("apply:{ip}"), 3600, 10).await? {
        return Err(AppError::TooMany("잠시 후 다시 시도해 주세요".into()));
    }
    verify::consume(&s, "partner", &email, b.email_code.as_deref()).await?;
    let hash = auth::hash_password(&b.password)?;
    let user_id = new_id();
    let workshop_id = new_id();
    let mut tx = s.db.begin().await?;
    let r = sqlx::query(
        "INSERT INTO users (id, email, display_name, phone, role, password_hash, email_verified_at) VALUES (?, ?, ?, ?, 'workshop', ?, IF(?, NOW(), NULL))",
    )
    .bind(&user_id)
    .bind(&email)
    .bind(&display_name)
    .bind(&contact_phone)
    .bind(&hash)
    .bind(verify::required(&s))
    .execute(&mut *tx)
    .await;
    if let Err(e) = r {
        if gageyo_core::error::is_duplicate(&e) {
            return Err(AppError::Conflict("이미 가입된 이메일입니다. 로그인해 주세요".into()));
        }
        return Err(e.into());
    }
    sqlx::query(
        "INSERT INTO workshops (id, owner_user_id, name, slug, category, description, phone, address, district, latitude, longitude, \
         business_number, representative, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')",
    )
    .bind(&workshop_id)
    .bind(&user_id)
    .bind(&core.name)
    .bind(new_slug())
    .bind(&core.categories[0])
    .bind(&description)
    .bind(&shop_phone)
    .bind(&core.address)
    .bind(&core.district)
    .bind(lat)
    .bind(lng)
    .bind(&core.business_number)
    .bind(&core.representative)
    .execute(&mut *tx)
    .await?;
    replace_categories(&mut tx, &workshop_id, &core.categories).await?;
    tx.commit().await?;
    crate::notify::ops(
        &s,
        &format!("새 입점 신청 · {}", core.name),
        &format!("{} ({} · {})\n담당자: {display_name} / {email}\n운영자 화면의 업체 관리에서 심사해 주세요.", core.name, core.district, core.categories.join(", ")),
    );
    let cookie = auth::create_session(&s, &user_id, Scope::Partner, &headers).await?;
    Ok((
        StatusCode::CREATED,
        [(header::SET_COOKIE, cookie)],
        Json(serde_json::json!({ "workshop_id": workshop_id, "status": "pending" })),
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

async fn login(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<LoginInput>) -> AppResult<Response> {
    let email = b.email.trim().to_ascii_lowercase();
    auth::check_login_allowed(&s, Scope::Partner, &email, &client_ip(&headers)).await?;
    let row: Option<(String, Option<String>, String, String)> =
        sqlx::query_as("SELECT id, password_hash, role, status FROM users WHERE email = ?").bind(&email).fetch_optional(&s.db).await?;
    let ok = row.as_ref().is_some_and(|(_, hash, role, _)| role == "workshop" && hash.as_deref().is_some_and(|h| auth::verify_password(&b.password, h)));
    let Some((user_id, _, _, status)) = row.filter(|_| ok) else {
        auth::record_login_fail(&s, Scope::Partner, &email).await?;
        return Err(AppError::Unauthorized("이메일 또는 비밀번호를 확인해 주세요".into()));
    };
    if status != "active" {
        return Err(AppError::Forbidden("이용이 중지된 계정입니다. 고객센터로 문의해 주세요".into()));
    }
    auth::clear_login_fails(&s, Scope::Partner, &email).await?;
    let cookie = auth::create_session(&s, &user_id, Scope::Partner, &headers).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ok": true }))).into_response())
}

async fn logout(State(s): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let cookie = auth::end_session(&s, &headers, Scope::Partner).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ok": true }))).into_response())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PartnerProfile {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub category: String,
    pub description: String,
    pub phone: String,
    pub address: String,
    pub district: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub status: String,
    pub rating: f32,
    pub review_count: i32,
    pub business_number: Option<String>,
    pub representative: Option<String>,
    pub open_time: Option<NaiveTime>,
    pub close_time: Option<NaiveTime>,
    pub closed_days: String,
    pub cover_image_url: Option<String>,
    pub reject_reason: Option<String>,
    pub approved_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

pub const PROFILE_COLS: &str = "id, name, slug, category, description, phone, address, district, latitude, longitude, status, rating, \
    review_count, business_number, representative, open_time, close_time, closed_days, cover_image_url, reject_reason, approved_at, created_at";

pub async fn profile_json(s: &AppState, workshop_id: &str) -> AppResult<serde_json::Value> {
    let w = sqlx::query_as::<_, PartnerProfile>(&format!("SELECT {PROFILE_COLS} FROM workshops WHERE id = ?"))
        .bind(workshop_id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| AppError::not_found("업체를 찾을 수 없습니다"))?;
    let categories: Vec<String> = crate::workshops::categories_for(s, &[workshop_id.to_string()]).await?.into_iter().map(|(_, c)| c).collect();
    let mut v = serde_json::to_value(&w)?;
    v["categories"] = serde_json::json!(categories);
    Ok(v)
}

/// 파트너 화면 첫 정보: 업체·계정·대기 중 변경 요청
async fn me(State(s): State<AppState>, p: Partner) -> AppResult<Json<serde_json::Value>> {
    let mut v = profile_json(&s, &p.workshop_id).await?;
    v["account"] = match &p.user {
        Some(u) => serde_json::json!({"email": u.email, "display_name": u.display_name, "phone": u.phone}),
        None => serde_json::json!({"via_key": true}),
    };
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshop_change_requests WHERE workshop_id = ? AND status = 'pending'")
        .bind(&p.workshop_id)
        .fetch_one(&s.db)
        .await?;
    v["pending_change_requests"] = serde_json::json!(pending);
    Ok(Json(v))
}

/// 대시보드 숫자
async fn dashboard(State(s): State<AppState>, p: Partner) -> AppResult<Json<serde_json::Value>> {
    let (today, upcoming, requested, to_ship, month_sales, unanswered_reviews, offers_sent, offers_accepted): (i64, i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
               (SELECT COUNT(*) FROM bookings WHERE workshop_id = ? AND DATE(starts_at) = CURDATE() AND status IN ('requested','confirmed')), \
               (SELECT COUNT(*) FROM bookings WHERE workshop_id = ? AND starts_at >= NOW() AND status = 'confirmed'), \
               (SELECT COUNT(*) FROM bookings WHERE workshop_id = ? AND status = 'requested'), \
               (SELECT COUNT(*) FROM order_items oi JOIN orders o ON o.id = oi.order_id WHERE oi.workshop_id = ? \
                  AND oi.status IN ('ordered','preparing') AND o.status NOT IN ('payment_pending','cancelled','refunded')), \
               (SELECT CAST(COALESCE(SUM(oi.unit_price * oi.quantity), 0) AS SIGNED) FROM order_items oi JOIN orders o ON o.id = oi.order_id \
                  WHERE oi.workshop_id = ? AND oi.status <> 'cancelled' AND o.status NOT IN ('payment_pending','cancelled','refunded') \
                  AND o.paid_at >= DATE_FORMAT(NOW(), '%Y-%m-01')), \
               (SELECT COUNT(*) FROM reviews WHERE workshop_id = ? AND reply IS NULL AND status = 'visible'), \
               (SELECT COUNT(*) FROM quote_offers WHERE workshop_id = ? AND created_at >= NOW() - INTERVAL 30 DAY), \
               (SELECT COUNT(*) FROM quote_offers WHERE workshop_id = ? AND status = 'accepted' AND created_at >= NOW() - INTERVAL 30 DAY)",
        )
        .bind(&p.workshop_id)
        .bind(&p.workshop_id)
        .bind(&p.workshop_id)
        .bind(&p.workshop_id)
        .bind(&p.workshop_id)
        .bind(&p.workshop_id)
        .bind(&p.workshop_id)
        .bind(&p.workshop_id)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(serde_json::json!({
        "bookings_today": today, "bookings_upcoming": upcoming, "bookings_requested": requested,
        "orders_to_ship": to_ship, "month_sales": month_sales, "reviews_unanswered": unanswered_reviews,
        "offers_sent_30d": offers_sent, "offers_accepted_30d": offers_accepted,
    })))
}

/// 바로 바뀌는 정보
#[derive(Debug, Deserialize)]
pub struct ProfileInput {
    pub description: String,
    pub phone: String,
    pub open_time: Option<String>,
    pub close_time: Option<String>,
    pub closed_days: Option<String>,
    pub cover_image_url: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

async fn update_profile(State(s): State<AppState>, p: Partner, Json(b): Json<ProfileInput>) -> AppResult<Json<serde_json::Value>> {
    let description = req_text(&b.description, "업체 소개", 3000)?;
    let phone = normalize_phone(&b.phone)?;
    let open = schedule::parse_time(b.open_time.as_deref())?;
    let close = schedule::parse_time(b.close_time.as_deref())?;
    schedule::check_hours(open, close)?;
    let closed_days = schedule::normalize_closed_days(b.closed_days.as_deref().unwrap_or(""))?;
    let cover = check_image_url(b.cover_image_url.as_deref())?;
    let (lat, lng) = check_coords(b.latitude, b.longitude)?;
    sqlx::query(
        "UPDATE workshops SET description = ?, phone = ?, open_time = ?, close_time = ?, closed_days = ?, cover_image_url = ?, \
         latitude = ?, longitude = ? WHERE id = ?",
    )
    .bind(&description)
    .bind(&phone)
    .bind(open)
    .bind(close)
    .bind(&closed_days)
    .bind(&cover)
    .bind(lat)
    .bind(lng)
    .bind(&p.workshop_id)
    .execute(&s.db)
    .await?;
    Ok(Json(profile_json(&s, &p.workshop_id).await?))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ChangeRequest {
    pub id: String,
    pub workshop_id: String,
    pub payload: String,
    pub status: String,
    pub reason: Option<String>,
    pub decided_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

pub fn change_request_json(r: ChangeRequest) -> serde_json::Value {
    let payload: serde_json::Value = serde_json::from_str(&r.payload).unwrap_or_default();
    serde_json::json!({"id": r.id, "workshop_id": r.workshop_id, "changes": payload, "status": r.status, "reason": r.reason,
        "decided_at": r.decided_at, "created_at": r.created_at})
}

async fn list_change_requests(State(s): State<AppState>, p: Partner) -> AppResult<Json<Vec<serde_json::Value>>> {
    let rows = sqlx::query_as::<_, ChangeRequest>(
        "SELECT id, workshop_id, payload, status, reason, decided_at, created_at FROM workshop_change_requests \
         WHERE workshop_id = ? ORDER BY created_at DESC LIMIT 50",
    )
    .bind(&p.workshop_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows.into_iter().map(change_request_json).collect()))
}

/// 상호·분야·지역·주소·사업자 정보 변경.
/// 승인 전·반려된 업체는 바로 반영하고 다시 심사(`pending`), 승인된 업체는 변경 요청을 남깁니다.
async fn request_change(State(s): State<AppState>, p: Partner, Json(b): Json<WorkshopCore>) -> AppResult<Json<serde_json::Value>> {
    let core = clean_core(&s, &b).await?;
    if p.workshop_status != "approved" {
        apply_core(&s, &p.workshop_id, &core).await?;
        sqlx::query("UPDATE workshops SET status = 'pending', reject_reason = NULL WHERE id = ?").bind(&p.workshop_id).execute(&s.db).await?;
        crate::notify::ops(&s, &format!("입점 재신청 · {}", core.name), "반려·대기 중이던 업체가 정보를 고쳐 다시 심사를 요청했습니다.");
        return Ok(Json(serde_json::json!({ "applied": true, "status": "pending" })));
    }
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshop_change_requests WHERE workshop_id = ? AND status = 'pending'")
        .bind(&p.workshop_id)
        .fetch_one(&s.db)
        .await?;
    if pending > 0 {
        return Err(AppError::Conflict("심사 중인 변경 요청이 있습니다. 결과가 나온 뒤 다시 요청해 주세요".into()));
    }
    let payload = serde_json::json!({
        "name": core.name, "categories": core.categories, "district": core.district, "address": core.address,
        "business_number": core.business_number, "representative": core.representative,
    });
    let id = new_id();
    sqlx::query("INSERT INTO workshop_change_requests (id, workshop_id, requested_by, payload) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(&p.workshop_id)
        .bind(p.user.as_ref().map(|u| u.id.clone()))
        .bind(payload.to_string())
        .execute(&s.db)
        .await?;
    crate::notify::ops(&s, &format!("업체 정보 변경 요청 · {}", p.workshop_name), "운영자 화면의 업체 관리 → 변경 요청에서 확인해 주세요.");
    Ok(Json(serde_json::json!({ "applied": false, "request_id": id })))
}

/// 기본 정보 반영 (변경 요청 승인·운영자 수정)
pub async fn apply_core(s: &AppState, workshop_id: &str, c: &CleanCore) -> AppResult<()> {
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE workshops SET name = ?, district = ?, address = ?, business_number = ?, representative = ? WHERE id = ?")
        .bind(&c.name)
        .bind(&c.district)
        .bind(&c.address)
        .bind(&c.business_number)
        .bind(&c.representative)
        .bind(workshop_id)
        .execute(&mut *tx)
        .await?;
    replace_categories(&mut tx, workshop_id, &c.categories).await?;
    tx.commit().await?;
    Ok(())
}

async fn list_photos(State(s): State<AppState>, p: Partner) -> AppResult<Json<Vec<serde_json::Value>>> {
    let rows: Vec<(String, String, i32)> =
        sqlx::query_as("SELECT id, url, sort_order FROM workshop_photos WHERE workshop_id = ? ORDER BY sort_order, created_at")
            .bind(&p.workshop_id)
            .fetch_all(&s.db)
            .await?;
    Ok(Json(rows.into_iter().map(|(id, url, sort)| serde_json::json!({"id": id, "url": url, "sort_order": sort})).collect()))
}

#[derive(Debug, Deserialize)]
pub struct PhotoInput {
    pub url: String,
}

pub const MAX_PHOTOS: i64 = 12;

async fn add_photo(State(s): State<AppState>, p: Partner, Json(b): Json<PhotoInput>) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let url = check_image_url(Some(&b.url))?.ok_or_else(|| AppError::bad("사진을 올려 주세요"))?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshop_photos WHERE workshop_id = ?").bind(&p.workshop_id).fetch_one(&s.db).await?;
    if n >= MAX_PHOTOS {
        return Err(AppError::bad(format!("업체 사진은 {MAX_PHOTOS}장까지 올릴 수 있습니다")));
    }
    let id = new_id();
    sqlx::query("INSERT INTO workshop_photos (id, workshop_id, url, sort_order) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(&p.workshop_id)
        .bind(&url)
        .bind(n as i32)
        .execute(&s.db)
        .await?;
    // 대표 사진이 없으면 첫 사진을 대표로
    sqlx::query("UPDATE workshops SET cover_image_url = ? WHERE id = ? AND cover_image_url IS NULL")
        .bind(&url)
        .bind(&p.workshop_id)
        .execute(&s.db)
        .await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id, "url": url }))))
}

async fn delete_photo(State(s): State<AppState>, p: Partner, Path(id): Path<String>) -> AppResult<StatusCode> {
    let url: Option<String> = sqlx::query_scalar("SELECT url FROM workshop_photos WHERE id = ? AND workshop_id = ?")
        .bind(&id)
        .bind(&p.workshop_id)
        .fetch_optional(&s.db)
        .await?;
    let url = url.ok_or_else(|| AppError::not_found("사진을 찾을 수 없습니다"))?;
    sqlx::query("DELETE FROM workshop_photos WHERE id = ?").bind(&id).execute(&s.db).await?;
    sqlx::query("UPDATE workshops SET cover_image_url = NULL WHERE id = ? AND cover_image_url = ?")
        .bind(&p.workshop_id)
        .bind(&url)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ServiceRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price_from: u64,
    pub duration_minutes: u16,
    pub sort_order: i32,
    pub is_active: bool,
}

/// 예전에는 지운(숨긴) 서비스도 구분 없이 목록에 보였습니다. 이제 `is_active` 를 함께 내려줍니다.
async fn partner_services(State(s): State<AppState>, p: Partner) -> AppResult<Json<Vec<ServiceRow>>> {
    Ok(Json(
        sqlx::query_as::<_, ServiceRow>(
            "SELECT id, name, description, price_from, duration_minutes, sort_order, is_active FROM workshop_services \
             WHERE workshop_id = ? ORDER BY is_active DESC, sort_order, name",
        )
        .bind(&p.workshop_id)
        .fetch_all(&s.db)
        .await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct ServiceInput {
    pub name: String,
    pub description: Option<String>,
    pub price_from: u64,
    pub duration_minutes: u16,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

fn check_service(b: &ServiceInput) -> AppResult<(String, String)> {
    let name = req_line(&b.name, "서비스명", 160)?;
    let description = b.description.as_deref().map(str::trim).unwrap_or("").to_string();
    if description.chars().count() > 2000 {
        return Err(AppError::bad("서비스 설명은 2000자 이내로 입력해 주세요"));
    }
    if b.price_from == 0 || b.price_from > 100_000_000 {
        return Err(AppError::bad("시작 가격을 확인해 주세요"));
    }
    if b.duration_minutes == 0 || b.duration_minutes > 24 * 60 * 7 {
        return Err(AppError::bad("소요 시간을 확인해 주세요"));
    }
    Ok((name, description))
}

async fn service_by_id(s: &AppState, workshop_id: &str, id: &str) -> AppResult<ServiceRow> {
    sqlx::query_as::<_, ServiceRow>(
        "SELECT id, name, description, price_from, duration_minutes, sort_order, is_active FROM workshop_services WHERE id = ? AND workshop_id = ?",
    )
    .bind(id)
    .bind(workshop_id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| AppError::not_found("서비스를 찾을 수 없습니다"))
}

async fn create_service(State(s): State<AppState>, p: Partner, Json(b): Json<ServiceInput>) -> AppResult<(StatusCode, Json<ServiceRow>)> {
    let (name, description) = check_service(&b)?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshop_services WHERE workshop_id = ?").bind(&p.workshop_id).fetch_one(&s.db).await?;
    if n >= 100 {
        return Err(AppError::bad("서비스는 100개까지 등록할 수 있습니다"));
    }
    let id = new_id();
    sqlx::query(
        "INSERT INTO workshop_services (id, workshop_id, name, description, price_from, duration_minutes, is_active, sort_order) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&p.workshop_id)
    .bind(&name)
    .bind(&description)
    .bind(b.price_from)
    .bind(b.duration_minutes)
    .bind(b.is_active.unwrap_or(true))
    .bind(b.sort_order.unwrap_or(0))
    .execute(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(service_by_id(&s, &p.workshop_id, &id).await?)))
}

async fn update_service(State(s): State<AppState>, p: Partner, Path(id): Path<String>, Json(b): Json<ServiceInput>) -> AppResult<Json<ServiceRow>> {
    let (name, description) = check_service(&b)?;
    let current = service_by_id(&s, &p.workshop_id, &id).await?;
    sqlx::query("UPDATE workshop_services SET name = ?, description = ?, price_from = ?, duration_minutes = ?, sort_order = ?, is_active = ? WHERE id = ? AND workshop_id = ?")
        .bind(&name)
        .bind(&description)
        .bind(b.price_from)
        .bind(b.duration_minutes)
        .bind(b.sort_order.unwrap_or(current.sort_order))
        .bind(b.is_active.unwrap_or(current.is_active))
        .bind(&id)
        .bind(&p.workshop_id)
        .execute(&s.db)
        .await?;
    Ok(Json(service_by_id(&s, &p.workshop_id, &id).await?))
}

/// 예약에 쓰인 서비스는 기록을 위해 숨김 처리, 아니면 지웁니다.
async fn delete_service(State(s): State<AppState>, p: Partner, Path(id): Path<String>) -> AppResult<StatusCode> {
    service_by_id(&s, &p.workshop_id, &id).await?;
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bookings WHERE service_id = ?").bind(&id).fetch_one(&s.db).await?;
    if used > 0 {
        sqlx::query("UPDATE workshop_services SET is_active = 0 WHERE id = ? AND workshop_id = ?").bind(&id).bind(&p.workshop_id).execute(&s.db).await?;
    } else {
        sqlx::query("DELETE FROM workshop_services WHERE id = ? AND workshop_id = ?").bind(&id).bind(&p.workshop_id).execute(&s.db).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
