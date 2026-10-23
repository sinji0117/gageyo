//! 운영자: 업체 목록·상세·심사(승인·반려·중지)·수정·직접 등록, 정보 변경 요청 승인, 파트너 키(예전 방식), 계정 비밀번호 설정 링크.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::{self, Admin, Scope},
    mail::Mail,
    schedule,
    util::{check_coords, like_term, new_id, normalize_email, normalize_phone, opt_text, req_line, req_text, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

use crate::partner::{apply_core, change_request_json, clean_core, new_slug, profile_json, replace_categories, ChangeRequest, WorkshopCore};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/workshops", get(list).post(create))
        .route("/admin/workshops/{id}", get(detail).patch(set_status).put(edit))
        .route("/admin/workshops/{id}/partner-key", post(create_partner_key))
        .route("/admin/workshops/{id}/owner-link", post(owner_link))
        .route("/admin/change-requests", get(list_change_requests))
        .route("/admin/change-requests/{id}", patch(decide_change_request))
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub q: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminWorkshop {
    pub id: String,
    pub name: String,
    pub district: String,
    pub category: String,
    pub status: String,
    pub rating: f32,
    pub review_count: i32,
    pub owner_email: Option<String>,
    pub business_number: Option<String>,
    pub created_at: NaiveDateTime,
    pub approved_at: Option<NaiveDateTime>,
    pub pending_changes: i64,
}

async fn list(State(s): State<AppState>, _a: Admin, Query(q): Query<ListQuery>) -> AppResult<Json<Vec<AdminWorkshop>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 500);
    let term = like_term(q.q.as_deref());
    Ok(Json(
        sqlx::query_as::<_, AdminWorkshop>(
            "SELECT w.id, w.name, w.district, w.category, w.status, w.rating, w.review_count, u.email AS owner_email, w.business_number, \
               w.created_at, w.approved_at, \
               (SELECT COUNT(*) FROM workshop_change_requests c WHERE c.workshop_id = w.id AND c.status = 'pending') AS pending_changes \
             FROM workshops w LEFT JOIN users u ON u.id = w.owner_user_id \
             WHERE (? IS NULL OR w.status = ?) \
               AND (? IS NULL OR w.name LIKE CONCAT('%', ?, '%') OR w.district LIKE CONCAT('%', ?, '%') OR u.email LIKE CONCAT('%', ?, '%') \
                    OR w.business_number LIKE CONCAT('%', ?, '%')) \
             ORDER BY (w.status = 'pending') DESC, w.created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(&q.status)
        .bind(&q.status)
        .bind(&term)
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

async fn detail(State(s): State<AppState>, _a: Admin, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let mut v = profile_json(&s, &id).await?;
    let owner: Option<(String, String, String, Option<String>, String, Option<NaiveDateTime>)> = sqlx::query_as(
        "SELECT u.id, u.email, u.display_name, u.phone, u.status, u.last_login_at FROM workshops w JOIN users u ON u.id = w.owner_user_id WHERE w.id = ?",
    )
    .bind(&id)
    .fetch_optional(&s.db)
    .await?;
    v["owner"] = match owner {
        Some((uid, email, name, phone, status, last)) => {
            serde_json::json!({"id": uid, "email": email, "display_name": name, "phone": phone, "status": status, "last_login_at": last})
        }
        None => serde_json::Value::Null,
    };
    let (services, products, bookings, completed, offers): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM workshop_services WHERE workshop_id = ? AND is_active = 1), \
                (SELECT COUNT(*) FROM products WHERE workshop_id = ?), \
                (SELECT COUNT(*) FROM bookings WHERE workshop_id = ?), \
                (SELECT COUNT(*) FROM bookings WHERE workshop_id = ? AND status = 'completed'), \
                (SELECT COUNT(*) FROM quote_offers WHERE workshop_id = ?)",
    )
    .bind(&id)
    .bind(&id)
    .bind(&id)
    .bind(&id)
    .bind(&id)
    .fetch_one(&s.db)
    .await?;
    v["counts"] = serde_json::json!({"services": services, "products": products, "bookings": bookings, "completed_bookings": completed, "offers": offers});
    let photos: Vec<(String, String)> = sqlx::query_as("SELECT id, url FROM workshop_photos WHERE workshop_id = ? ORDER BY sort_order")
        .bind(&id)
        .fetch_all(&s.db)
        .await?;
    v["photos"] = serde_json::json!(photos.into_iter().map(|(id, url)| serde_json::json!({"id": id, "url": url})).collect::<Vec<_>>());
    let changes = sqlx::query_as::<_, ChangeRequest>(
        "SELECT id, workshop_id, payload, status, reason, decided_at, created_at FROM workshop_change_requests WHERE workshop_id = ? ORDER BY created_at DESC LIMIT 20",
    )
    .bind(&id)
    .fetch_all(&s.db)
    .await?;
    v["change_requests"] = serde_json::json!(changes.into_iter().map(change_request_json).collect::<Vec<_>>());
    let has_key: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshop_access_tokens WHERE workshop_id = ?").bind(&id).fetch_one(&s.db).await?;
    v["has_partner_key"] = serde_json::json!(has_key > 0);
    Ok(Json(v))
}

#[derive(Debug, Deserialize)]
pub struct StatusInput {
    pub status: String,
    pub reason: Option<String>,
}

/// 승인·반려·중지·다시 대기. 반려·중지는 사유를 받아 업체에 메일로 알립니다.
async fn set_status(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<StatusInput>) -> AppResult<StatusCode> {
    if !["approved", "rejected", "suspended", "pending"].contains(&b.status.as_str()) {
        return Err(AppError::bad("업체 상태 값이 올바르지 않습니다"));
    }
    let reason = opt_text(b.reason.as_deref(), "사유", 500)?;
    if ["rejected", "suspended"].contains(&b.status.as_str()) && reason.is_none() {
        return Err(AppError::bad("반려·중지 사유를 적어 주세요 (업체에 안내됩니다)"));
    }
    let r = sqlx::query(
        "UPDATE workshops SET status = ?, reject_reason = ?, approved_at = IF(? = 'approved', COALESCE(approved_at, NOW()), approved_at) WHERE id = ?",
    )
    .bind(&b.status)
    .bind(&reason)
    .bind(&b.status)
    .bind(&id)
    .execute(&s.db)
    .await?;
    if r.rows_affected() == 0 {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshops WHERE id = ?").bind(&id).fetch_one(&s.db).await?;
        if n == 0 {
            return Err(AppError::not_found("업체를 찾을 수 없습니다"));
        }
    }
    audit::log(&s, &admin, &format!("workshop.{}", b.status), "workshop", &id, reason.clone()).await;
    crate::notify::workshop_decision(&s, &id, &b.status, reason.as_deref()).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct EditInput {
    #[serde(flatten)]
    pub core: WorkshopCore,
    pub phone: String,
    pub description: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub open_time: Option<String>,
    pub close_time: Option<String>,
    pub closed_days: Option<String>,
}

struct CleanEdit {
    phone: String,
    description: String,
    lat: Option<f64>,
    lng: Option<f64>,
    open: Option<chrono::NaiveTime>,
    close: Option<chrono::NaiveTime>,
    closed_days: String,
}

fn clean_edit(b: &EditInput) -> AppResult<CleanEdit> {
    let open = schedule::parse_time(b.open_time.as_deref())?;
    let close = schedule::parse_time(b.close_time.as_deref())?;
    schedule::check_hours(open, close)?;
    let (lat, lng) = check_coords(b.latitude, b.longitude)?;
    Ok(CleanEdit {
        phone: normalize_phone(&b.phone)?,
        description: req_text(&b.description, "업체 소개", 3000)?,
        lat,
        lng,
        open,
        close,
        closed_days: schedule::normalize_closed_days(b.closed_days.as_deref().unwrap_or(""))?,
    })
}

/// 운영자가 업체 정보를 직접 고침 (변경 요청 없이 바로 반영)
async fn edit(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<EditInput>) -> AppResult<Json<serde_json::Value>> {
    let core = clean_core(&s, &b.core).await?;
    let e = clean_edit(&b)?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshops WHERE id = ?").bind(&id).fetch_one(&s.db).await?;
    if n == 0 {
        return Err(AppError::not_found("업체를 찾을 수 없습니다"));
    }
    apply_core(&s, &id, &core).await?;
    sqlx::query(
        "UPDATE workshops SET phone = ?, description = ?, latitude = ?, longitude = ?, open_time = ?, close_time = ?, closed_days = ? WHERE id = ?",
    )
    .bind(&e.phone)
    .bind(&e.description)
    .bind(e.lat)
    .bind(e.lng)
    .bind(e.open)
    .bind(e.close)
    .bind(&e.closed_days)
    .bind(&id)
    .execute(&s.db)
    .await?;
    audit::log(&s, &admin, "workshop.edit", "workshop", &id, Some(core.name.clone())).await;
    Ok(Json(profile_json(&s, &id).await?))
}

#[derive(Debug, Deserialize)]
pub struct CreateInput {
    pub owner_email: String,
    pub owner_name: String,
    pub owner_phone: Option<String>,
    #[serde(flatten)]
    pub edit: EditInput,
    /// 바로 승인할지 (기본 승인)
    pub approve: Option<bool>,
}

/// 운영자가 오프라인으로 받은 업체를 직접 등록.
/// 주인 이메일로 파트너 계정이 없으면 만들고, 비밀번호 설정 링크를 메일로 보냅니다(메일 설정이 없으면 화면에 링크를 보여 줌).
async fn create(State(s): State<AppState>, admin: Admin, Json(b): Json<CreateInput>) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let email = normalize_email(&b.owner_email)?;
    let owner_name = req_line(&b.owner_name, "담당자 이름", 100)?;
    let owner_phone = match b.owner_phone.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(p) => Some(normalize_phone(p)?),
        None => None,
    };
    let core = clean_core(&s, &b.edit.core).await?;
    let e = clean_edit(&b.edit)?;
    let existing: Option<(String, String)> = sqlx::query_as("SELECT id, role FROM users WHERE email = ?").bind(&email).fetch_optional(&s.db).await?;
    let (user_id, new_user) = match existing {
        Some((id, role)) if role == "workshop" => (id, false),
        Some(_) => return Err(AppError::Conflict("이 이메일은 고객 계정으로 가입되어 있습니다. 다른 이메일을 써 주세요".into())),
        None => (new_id(), true),
    };
    let workshop_id = new_id();
    let status = if b.approve.unwrap_or(true) { "approved" } else { "pending" };
    let mut tx = s.db.begin().await?;
    if new_user {
        sqlx::query("INSERT INTO users (id, email, display_name, phone, role) VALUES (?, ?, ?, ?, 'workshop')")
            .bind(&user_id)
            .bind(&email)
            .bind(&owner_name)
            .bind(&owner_phone)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO workshops (id, owner_user_id, name, slug, category, description, phone, address, district, latitude, longitude, \
         business_number, representative, open_time, close_time, closed_days, status, approved_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, IF(? = 'approved', NOW(), NULL))",
    )
    .bind(&workshop_id)
    .bind(&user_id)
    .bind(&core.name)
    .bind(new_slug())
    .bind(&core.categories[0])
    .bind(&e.description)
    .bind(&e.phone)
    .bind(&core.address)
    .bind(&core.district)
    .bind(e.lat)
    .bind(e.lng)
    .bind(&core.business_number)
    .bind(&core.representative)
    .bind(e.open)
    .bind(e.close)
    .bind(&e.closed_days)
    .bind(status)
    .bind(status)
    .execute(&mut *tx)
    .await?;
    replace_categories(&mut tx, &workshop_id, &core.categories).await?;
    tx.commit().await?;
    audit::log(&s, &admin, "workshop.create", "workshop", &workshop_id, Some(format!("{} / {email}", core.name))).await;
    let link = if new_user { Some(send_owner_link(&s, &user_id, &email, &core.name).await?) } else { None };
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": workshop_id, "status": status, "owner_created": new_user,
            "owner_link": link.filter(|_| !s.mailer.enabled()) })),
    ))
}

async fn send_owner_link(s: &AppState, user_id: &str, email: &str, workshop_name: &str) -> AppResult<String> {
    let link = auth::create_password_reset(s, user_id, Scope::Partner).await?;
    s.mailer.send(Mail::new(
        email,
        "[가게요] 파트너 센터 비밀번호를 설정해 주세요",
        format!(
            "{workshop_name} 파트너 계정이 준비되었습니다.\n아래 링크에서 비밀번호를 정하면 파트너 센터에 로그인할 수 있습니다. (30분 동안 유효)\n\n{link}"
        ),
    ));
    Ok(link)
}

/// 업체 주인에게 비밀번호 설정 링크 보내기 (예전 파트너 키 업체를 계정으로 옮길 때)
async fn owner_link(State(s): State<AppState>, admin: Admin, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT u.id, u.email, u.role, w.name FROM workshops w JOIN users u ON u.id = w.owner_user_id WHERE w.id = ?",
    )
    .bind(&id)
    .fetch_optional(&s.db)
    .await?;
    let (user_id, email, role, name) = row.ok_or_else(|| AppError::not_found("업체 주인 계정을 찾을 수 없습니다"))?;
    if role != "workshop" {
        sqlx::query("UPDATE users SET role = 'workshop' WHERE id = ? AND role = 'customer'").bind(&user_id).execute(&s.db).await?;
    }
    let link = send_owner_link(&s, &user_id, &email, &name).await?;
    audit::log(&s, &admin, "workshop.owner_link", "workshop", &id, Some(email.clone())).await;
    // 메일이 꺼져 있으면 운영자가 직접 전달할 수 있게 링크를 돌려줍니다.
    Ok(Json(serde_json::json!({ "email": email, "mailed": s.mailer.enabled(), "link": (!s.mailer.enabled()).then_some(link) })))
}

/// 예전 방식 파트너 키 발급 (원문은 한 번만 보여 주고 DB 에는 해시만 저장, 재발급하면 이전 키는 바로 무효)
async fn create_partner_key(State(s): State<AppState>, admin: Admin, Path(id): Path<String>) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshops WHERE id = ?").bind(&id).fetch_one(&s.db).await?;
    if n == 0 {
        return Err(AppError::not_found("업체를 찾을 수 없습니다"));
    }
    let token = auth::random_token();
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM workshop_access_tokens WHERE workshop_id = ?").bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO workshop_access_tokens (token_hash, workshop_id) VALUES (?, ?)")
        .bind(auth::token_hash(&token))
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::log(&s, &admin, "workshop.partner_key", "workshop", &id, None).await;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "partner_key": token }))))
}

#[derive(Debug, Deserialize)]
pub struct ChangeQuery {
    pub status: Option<String>,
}

async fn list_change_requests(State(s): State<AppState>, _a: Admin, Query(q): Query<ChangeQuery>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let status = q.status.unwrap_or_else(|| "pending".into());
    let rows: Vec<(String, String, String, String, String, Option<String>, Option<NaiveDateTime>, NaiveDateTime, String, String, String)> = sqlx::query_as(
        "SELECT c.id, c.workshop_id, c.payload, c.status, w.name, c.reason, c.decided_at, c.created_at, w.district, w.address, \
           COALESCE((SELECT GROUP_CONCAT(category ORDER BY category SEPARATOR ',') FROM workshop_categories wc WHERE wc.workshop_id = w.id), '') \
         FROM workshop_change_requests c JOIN workshops w ON w.id = c.workshop_id \
         WHERE (? = 'all' OR c.status = ?) ORDER BY c.created_at DESC LIMIT 200",
    )
    .bind(&status)
    .bind(&status)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, workshop_id, payload, status, name, reason, decided_at, created_at, district, address, cats)| {
                serde_json::json!({
                    "id": id, "workshop_id": workshop_id, "status": status, "reason": reason, "decided_at": decided_at, "created_at": created_at,
                    "changes": serde_json::from_str::<serde_json::Value>(&payload).unwrap_or_default(),
                    "current": {"name": name, "district": district, "address": address, "categories": cats.split(',').filter(|c| !c.is_empty()).collect::<Vec<_>>()},
                })
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct DecideInput {
    pub approve: bool,
    pub reason: Option<String>,
}

async fn decide_change_request(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<DecideInput>) -> AppResult<StatusCode> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT workshop_id, payload FROM workshop_change_requests WHERE id = ? AND status = 'pending'")
            .bind(&id)
            .fetch_optional(&s.db)
            .await?;
    let (workshop_id, payload) = row.ok_or_else(|| AppError::not_found("심사 중인 변경 요청이 아닙니다"))?;
    let reason = opt_text(b.reason.as_deref(), "사유", 500)?;
    if b.approve {
        let core: WorkshopCore = serde_json::from_str(&payload)?;
        let clean = clean_core(&s, &core).await?;
        apply_core(&s, &workshop_id, &clean).await?;
    } else if reason.is_none() {
        return Err(AppError::bad("반려 사유를 적어 주세요"));
    }
    sqlx::query("UPDATE workshop_change_requests SET status = ?, reason = ?, decided_by = ?, decided_at = NOW() WHERE id = ?")
        .bind(if b.approve { "approved" } else { "rejected" })
        .bind(&reason)
        .bind(&admin.actor)
        .bind(&id)
        .execute(&s.db)
        .await?;
    audit::log(&s, &admin, if b.approve { "workshop.change_approved" } else { "workshop.change_rejected" }, "workshop", &workshop_id, reason.clone()).await;
    crate::notify::change_request_decided(&s, &workshop_id, b.approve, reason.as_deref()).await;
    Ok(StatusCode::NO_CONTENT)
}
