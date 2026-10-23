//! 운영자: 현황 대시보드, 회원 관리(검색·정지), 작업 기록, 플랫폼 설정.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::{self, Admin},
    settings,
    util::{like_term, mask_phone, opt_text, Page},
    AppError, AppResult, AppState,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/dashboard", get(dashboard))
        .route("/admin/users", get(list_users))
        .route("/admin/users/{id}", get(user_detail).patch(set_user_status))
        .route("/admin/audit", get(audit_log))
        .route("/admin/settings", get(get_settings).put(put_settings))
}

const PAID: &str = "o.status NOT IN ('payment_pending','cancelled','refunded') AND o.paid_at IS NOT NULL";

/// 예전 운영 화면은 '새로고침'을 눌러도 숫자를 다시 받지 않았고, 회원·신고 메뉴는 준비 중이었습니다.
async fn dashboard(State(s): State<AppState>, _a: Admin) -> AppResult<Json<serde_json::Value>> {
    let (customers, partners, new_customers, approved, pending, suspended, changes): (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM users WHERE role = 'customer' AND status = 'active'), \
                (SELECT COUNT(*) FROM users WHERE role = 'workshop' AND status = 'active'), \
                (SELECT COUNT(*) FROM users WHERE role = 'customer' AND created_at >= NOW() - INTERVAL 7 DAY), \
                (SELECT COUNT(*) FROM workshops WHERE status = 'approved'), \
                (SELECT COUNT(*) FROM workshops WHERE status = 'pending'), \
                (SELECT COUNT(*) FROM workshops WHERE status = 'suspended'), \
                (SELECT COUNT(*) FROM workshop_change_requests WHERE status = 'pending')",
    )
    .fetch_one(&s.db)
    .await?;
    let (open_quotes, quotes_30d, bookings_month, completed_30d, reviews_30d): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM quote_requests WHERE status = 'open' AND created_at >= NOW() - INTERVAL 14 DAY), \
                (SELECT COUNT(*) FROM quote_requests WHERE created_at >= NOW() - INTERVAL 30 DAY), \
                (SELECT COUNT(*) FROM bookings WHERE starts_at >= DATE_FORMAT(NOW(), '%Y-%m-01')), \
                (SELECT COUNT(*) FROM bookings WHERE status = 'completed' AND starts_at >= NOW() - INTERVAL 30 DAY), \
                (SELECT COUNT(*) FROM reviews WHERE created_at >= NOW() - INTERVAL 30 DAY)",
    )
    .fetch_one(&s.db)
    .await?;
    let (gmv_30d, orders_30d, awaiting_deposit, to_ship): (i64, i64, i64, i64) = sqlx::query_as(&format!(
        "SELECT CAST(COALESCE((SELECT SUM(o.total_amount) FROM orders o WHERE {PAID} AND o.paid_at >= NOW() - INTERVAL 30 DAY), 0) AS SIGNED), \
                (SELECT COUNT(*) FROM orders o WHERE {PAID} AND o.paid_at >= NOW() - INTERVAL 30 DAY), \
                (SELECT COUNT(*) FROM orders WHERE status = 'payment_pending' AND payment_method = 'bank'), \
                (SELECT COUNT(*) FROM order_items oi JOIN orders o ON o.id = oi.order_id WHERE oi.status IN ('ordered','preparing') AND {PAID})"
    ))
    .fetch_one(&s.db)
    .await?;
    let inquiries: Vec<(String, i64)> = sqlx::query_as("SELECT kind, COUNT(*) FROM inquiries WHERE status = 'open' GROUP BY kind").fetch_all(&s.db).await?;
    let since = "o.paid_at >= DATE_FORMAT(NOW() - INTERVAL 5 MONTH, '%Y-%m-01')";
    let monthly_gmv: Vec<(String, i64, i64)> = sqlx::query_as(&format!(
        "SELECT DATE_FORMAT(o.paid_at, '%Y-%m') AS m, CAST(SUM(o.total_amount) AS SIGNED), COUNT(*) \
         FROM orders o WHERE {PAID} AND {since} GROUP BY m ORDER BY m"
    ))
    .fetch_all(&s.db)
    .await?;
    let monthly_fee: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT DATE_FORMAT(o.paid_at, '%Y-%m') AS m, CAST(SUM(oi.fee_amount) AS SIGNED) \
         FROM order_items oi JOIN orders o ON o.id = oi.order_id WHERE {PAID} AND {since} AND oi.status <> 'cancelled' GROUP BY m"
    ))
    .fetch_all(&s.db)
    .await?;
    let monthly: Vec<(String, i64, i64, i64)> = monthly_gmv
        .into_iter()
        .map(|(m, gmv, n)| {
            let fee = monthly_fee.iter().find(|(fm, _)| fm == &m).map(|(_, f)| *f).unwrap_or(0);
            (m, gmv, fee, n)
        })
        .collect();
    let recent = audit::list(&s, None, 8, 0).await?;
    Ok(Json(serde_json::json!({
        "users": {"customers": customers, "partners": partners, "new_customers_7d": new_customers},
        "workshops": {"approved": approved, "pending": pending, "suspended": suspended, "change_requests": changes},
        "activity": {"open_quotes": open_quotes, "quotes_30d": quotes_30d, "bookings_this_month": bookings_month,
                     "completed_bookings_30d": completed_30d, "reviews_30d": reviews_30d},
        "commerce": {"gmv_30d": gmv_30d, "orders_30d": orders_30d, "awaiting_deposit": awaiting_deposit, "items_to_ship": to_ship},
        "open_inquiries": inquiries.into_iter().map(|(k, n)| (k, serde_json::json!(n))).collect::<serde_json::Map<_, _>>(),
        "monthly": monthly.into_iter().map(|(m, gmv, fee, n)| serde_json::json!({"month": m, "gmv": gmv, "fee": fee, "orders": n})).collect::<Vec<_>>(),
        "recent_audit": recent,
    })))
}

#[derive(Debug, Deserialize)]
pub struct UserQuery {
    pub q: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn list_users(State(s): State<AppState>, _a: Admin, Query(q): Query<UserQuery>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 300);
    let term = like_term(q.q.as_deref());
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, String, String, Option<String>, String, String, NaiveDateTime, Option<NaiveDateTime>, i64, i64)> = sqlx::query_as(
        "SELECT u.id, u.email, u.display_name, u.phone, u.role, u.status, u.created_at, u.last_login_at, \
           (SELECT COUNT(*) FROM orders o WHERE o.customer_user_id = u.id), (SELECT COUNT(*) FROM bookings b WHERE b.customer_user_id = u.id) \
         FROM users u WHERE (? IS NULL OR u.role = ?) AND (? IS NULL OR u.status = ?) \
           AND (? IS NULL OR u.email LIKE CONCAT('%', ?, '%') OR u.display_name LIKE CONCAT('%', ?, '%') OR u.phone LIKE CONCAT('%', ?, '%')) \
         ORDER BY u.created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&q.role)
    .bind(&q.role)
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
            .map(|(id, email, name, phone, role, status, created, last, orders, bookings)| {
                serde_json::json!({"id": id, "email": email, "display_name": name, "phone": phone.as_deref().map(mask_phone),
                    "role": role, "status": status, "created_at": created, "last_login_at": last, "orders": orders, "bookings": bookings,
                    "is_admin": s.cfg.admin_emails.contains(&email)})
            })
            .collect(),
    ))
}

async fn user_detail(State(s): State<AppState>, _a: Admin, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    #[allow(clippy::type_complexity)]
    let row: Option<(String, String, String, Option<String>, String, String, NaiveDateTime, Option<NaiveDateTime>, Option<String>, Option<NaiveDateTime>)> =
        sqlx::query_as(
            "SELECT id, email, display_name, phone, role, status, created_at, last_login_at, last_login_ip, email_verified_at FROM users WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&s.db)
        .await?;
    let (id, email, name, phone, role, status, created, last, ip, verified) = row.ok_or_else(|| AppError::not_found("회원을 찾을 수 없습니다"))?;
    let workshops: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, name, status FROM workshops WHERE owner_user_id = ?").bind(&id).fetch_all(&s.db).await?;
    let (orders, bookings, quotes, reviews): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM orders WHERE customer_user_id = ?), (SELECT COUNT(*) FROM bookings WHERE customer_user_id = ?), \
                (SELECT COUNT(*) FROM quote_requests WHERE customer_user_id = ?), (SELECT COUNT(*) FROM reviews WHERE user_id = ?)",
    )
    .bind(&id)
    .bind(&id)
    .bind(&id)
    .bind(&id)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(serde_json::json!({
        "id": id, "email": email, "display_name": name, "phone": phone, "role": role, "status": status, "created_at": created,
        "last_login_at": last, "last_login_ip": ip, "email_verified_at": verified, "is_admin": s.cfg.admin_emails.contains(&email),
        "workshops": workshops.into_iter().map(|(id, n, st)| serde_json::json!({"id": id, "name": n, "status": st})).collect::<Vec<_>>(),
        "counts": {"orders": orders, "bookings": bookings, "quotes": quotes, "reviews": reviews},
    })))
}

#[derive(Debug, Deserialize)]
pub struct UserStatusInput {
    pub status: String,
    pub reason: Option<String>,
}

/// 정지하면 그 계정은 바로 로그아웃되고 다시 로그인할 수 없습니다. 운영자 계정은 정지할 수 없습니다.
async fn set_user_status(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<UserStatusInput>) -> AppResult<StatusCode> {
    if !["active", "suspended"].contains(&b.status.as_str()) {
        return Err(AppError::bad("회원 상태 값이 올바르지 않습니다"));
    }
    let row: Option<(String, String)> = sqlx::query_as("SELECT email, status FROM users WHERE id = ?").bind(&id).fetch_optional(&s.db).await?;
    let (email, current) = row.ok_or_else(|| AppError::not_found("회원을 찾을 수 없습니다"))?;
    if current == "withdrawn" {
        return Err(AppError::Conflict("탈퇴한 회원입니다".into()));
    }
    if b.status == "suspended" && s.cfg.admin_emails.contains(&email) {
        return Err(AppError::bad("운영자 계정은 정지할 수 없습니다"));
    }
    let reason = opt_text(b.reason.as_deref(), "사유", 500)?;
    if b.status == "suspended" && reason.is_none() {
        return Err(AppError::bad("정지 사유를 적어 주세요 (작업 기록에 남습니다)"));
    }
    sqlx::query("UPDATE users SET status = ? WHERE id = ?").bind(&b.status).bind(&id).execute(&s.db).await?;
    if b.status == "suspended" {
        auth::revoke_user_sessions(&s.db, &id).await?;
    }
    audit::log(&s, &admin, &format!("user.{}", b.status), "user", &id, reason.map(|r| format!("{email}: {r}")).or(Some(email))).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub target_type: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn audit_log(State(s): State<AppState>, _a: Admin, Query(q): Query<AuditQuery>) -> AppResult<Json<Vec<audit::AuditRow>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 300);
    Ok(Json(audit::list(&s, q.target_type.as_deref().filter(|t| !t.is_empty()), limit, offset).await?))
}

async fn get_settings(State(s): State<AppState>, _a: Admin) -> AppResult<Json<serde_json::Value>> {
    let mut v = settings::all(&s).await?;
    v["toss_enabled"] = serde_json::json!(s.cfg.toss_enabled());
    v["toss_test_mode"] = serde_json::json!(s.cfg.toss_client_key.as_deref().is_some_and(|k| k.starts_with("test_")));
    v["mail_enabled"] = serde_json::json!(s.mailer.enabled());
    v["signup_email_verify"] = serde_json::json!(gageyo_core::verify::required(&s));
    Ok(Json(v))
}

async fn put_settings(State(s): State<AppState>, admin: Admin, Json(b): Json<settings::SettingsInput>) -> AppResult<Json<serde_json::Value>> {
    let changed = settings::save(&s, &b).await?;
    if !changed.is_empty() {
        audit::log(&s, &admin, "settings.update", "settings", "platform", Some(changed.join(", "))).await;
    }
    get_settings(State(s), admin).await
}
