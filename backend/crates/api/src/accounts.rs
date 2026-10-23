//! 고객 계정: 가입(이메일 코드 인증)·로그인·로그아웃, 내 정보·비밀번호·탈퇴, 내 차량, 비밀번호 재설정(고객·파트너·운영자 공통).

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    auth::{self, client_ip, Customer, Scope},
    mail::Mail,
    ratelimit,
    util::{new_id, normalize_email, normalize_phone, opt_text, req_line},
    verify, AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/send-code", post(send_code))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/password/forgot", post(forgot_password))
        .route("/auth/password/reset", post(reset_password))
        .route("/me/profile", put(update_profile))
        .route("/me/password", put(change_password))
        .route("/me", delete(withdraw))
        .route("/me/vehicles", get(list_vehicles).post(create_vehicle))
        .route("/me/vehicles/{id}", put(update_vehicle).delete(delete_vehicle))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Profile {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub phone: Option<String>,
    pub role: String,
    pub email_verified_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

async fn profile(s: &AppState, id: &str) -> AppResult<Profile> {
    sqlx::query_as::<_, Profile>("SELECT id, email, display_name, phone, role, email_verified_at, created_at FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(AppError::login_required)
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
    verify::send_code(&s, "customer", &email, &client_ip(&headers)).await?;
    Ok(Json(serde_json::json!({ "sent": true, "ttl_minutes": verify::CODE_TTL_MINUTES })))
}

#[derive(Debug, Deserialize)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub phone: Option<String>,
    pub email_code: Option<String>,
    pub agree_terms: Option<bool>,
}

async fn register(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<RegisterInput>) -> AppResult<Response> {
    if b.agree_terms == Some(false) {
        return Err(AppError::bad("이용약관과 개인정보 처리에 동의해 주세요"));
    }
    let email = normalize_email(&b.email)?;
    let name = req_line(&b.display_name, "이름", 100)?;
    auth::check_password(&b.password)?;
    let phone = match b.phone.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(normalize_phone(p)?),
        None => None,
    };
    let ip = client_ip(&headers);
    if !ip.is_empty() && !ratelimit::hit(&s.db, &format!("register:{ip}"), 3600, 20).await? {
        return Err(AppError::TooMany("잠시 후 다시 시도해 주세요".into()));
    }
    verify::consume(&s, "customer", &email, b.email_code.as_deref()).await?;
    let hash = auth::hash_password(&b.password)?;
    let id = new_id();
    let r = sqlx::query(
        "INSERT INTO users (id, email, display_name, phone, role, password_hash, email_verified_at) VALUES (?, ?, ?, ?, 'customer', ?, IF(?, NOW(), NULL))",
    )
    .bind(&id)
    .bind(&email)
    .bind(&name)
    .bind(&phone)
    .bind(&hash)
    .bind(verify::required(&s))
    .execute(&s.db)
    .await;
    if let Err(e) = r {
        if gageyo_core::error::is_duplicate(&e) {
            return Err(AppError::Conflict("이미 가입된 이메일입니다".into()));
        }
        return Err(e.into());
    }
    let cookie = auth::create_session(&s, &id, Scope::Customer, &headers).await?;
    Ok((StatusCode::CREATED, [(header::SET_COOKIE, cookie)], Json(profile(&s, &id).await?)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

async fn login(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<LoginInput>) -> AppResult<Response> {
    let email = b.email.trim().to_ascii_lowercase();
    auth::check_login_allowed(&s, Scope::Customer, &email, &client_ip(&headers)).await?;
    let row: Option<(String, Option<String>, String, String)> =
        sqlx::query_as("SELECT id, password_hash, role, status FROM users WHERE email = ?").bind(&email).fetch_optional(&s.db).await?;
    let ok = row
        .as_ref()
        .is_some_and(|(_, hash, role, _)| role == "customer" && hash.as_deref().is_some_and(|h| auth::verify_password(&b.password, h)));
    let Some((id, _, _, status)) = row.filter(|_| ok) else {
        auth::record_login_fail(&s, Scope::Customer, &email).await?;
        return Err(AppError::Unauthorized("이메일 또는 비밀번호를 확인해 주세요".into()));
    };
    if status != "active" {
        return Err(AppError::Forbidden("이용이 중지된 계정입니다. 고객센터로 문의해 주세요".into()));
    }
    auth::clear_login_fails(&s, Scope::Customer, &email).await?;
    let cookie = auth::create_session(&s, &id, Scope::Customer, &headers).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(profile(&s, &id).await?)).into_response())
}

/// 예전 코드는 로그아웃 응답 타입 오류로 빌드가 되지 않았습니다(크기 없는 배열). 이제 쿠키 만료 헤더를 정상으로 돌려줍니다.
async fn logout(State(s): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let cookie = auth::end_session(&s, &headers, Scope::Customer).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ok": true }))).into_response())
}

async fn me(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<serde_json::Value>> {
    let p = profile(&s, &u.id).await?;
    let (quotes, bookings, orders, favorites): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM quote_requests WHERE customer_user_id = ? AND status = 'open'), \
                (SELECT COUNT(*) FROM bookings WHERE customer_user_id = ? AND status IN ('requested','confirmed') AND starts_at >= NOW()), \
                (SELECT COUNT(*) FROM orders WHERE customer_user_id = ? AND status IN ('payment_pending','paid','preparing','shipped')), \
                (SELECT COUNT(*) FROM favorites WHERE user_id = ?)",
    )
    .bind(&u.id)
    .bind(&u.id)
    .bind(&u.id)
    .bind(&u.id)
    .fetch_one(&s.db)
    .await?;
    let mut v = serde_json::to_value(p)?;
    v["counts"] = serde_json::json!({"open_quotes": quotes, "upcoming_bookings": bookings, "active_orders": orders, "favorites": favorites});
    Ok(Json(v))
}

#[derive(Debug, Deserialize)]
pub struct ProfileInput {
    pub display_name: String,
    pub phone: Option<String>,
}

async fn update_profile(State(s): State<AppState>, Customer(u): Customer, Json(b): Json<ProfileInput>) -> AppResult<Json<Profile>> {
    let name = req_line(&b.display_name, "이름", 100)?;
    let phone = match b.phone.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(normalize_phone(p)?),
        None => None,
    };
    sqlx::query("UPDATE users SET display_name = ?, phone = ? WHERE id = ?").bind(&name).bind(&phone).bind(&u.id).execute(&s.db).await?;
    Ok(Json(profile(&s, &u.id).await?))
}

#[derive(Debug, Deserialize)]
pub struct PasswordInput {
    pub current_password: String,
    pub new_password: String,
}

/// 비밀번호를 바꾸면 다른 기기의 로그인도 모두 끊고 이 기기만 새로 로그인합니다.
async fn change_password(State(s): State<AppState>, Customer(u): Customer, headers: HeaderMap, Json(b): Json<PasswordInput>) -> AppResult<Response> {
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?").bind(&u.id).fetch_one(&s.db).await?;
    if !hash.as_deref().is_some_and(|h| auth::verify_password(&b.current_password, h)) {
        return Err(AppError::bad("지금 비밀번호가 맞지 않습니다"));
    }
    auth::check_password(&b.new_password)?;
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?").bind(auth::hash_password(&b.new_password)?).bind(&u.id).execute(&s.db).await?;
    auth::revoke_user_sessions(&s.db, &u.id).await?;
    let cookie = auth::create_session(&s, &u.id, Scope::Customer, &headers).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ok": true }))).into_response())
}

#[derive(Debug, Deserialize)]
pub struct WithdrawInput {
    pub password: String,
}

/// 탈퇴: 진행 중인 주문·예약이 있으면 막습니다. 주문·예약 기록은 남기고 이메일·이름·연락처는 지워 같은 이메일로 다시 가입할 수 있습니다.
async fn withdraw(State(s): State<AppState>, Customer(u): Customer, Json(b): Json<WithdrawInput>) -> AppResult<Response> {
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?").bind(&u.id).fetch_one(&s.db).await?;
    if !hash.as_deref().is_some_and(|h| auth::verify_password(&b.password, h)) {
        return Err(AppError::bad("비밀번호가 맞지 않습니다"));
    }
    let (orders, bookings): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM orders WHERE customer_user_id = ? AND status IN ('paid','preparing','shipped')), \
                (SELECT COUNT(*) FROM bookings WHERE customer_user_id = ? AND status IN ('requested','confirmed') AND starts_at >= NOW())",
    )
    .bind(&u.id)
    .bind(&u.id)
    .fetch_one(&s.db)
    .await?;
    if orders > 0 {
        return Err(AppError::Conflict("배송이 끝나지 않은 주문이 있어 탈퇴할 수 없습니다".into()));
    }
    if bookings > 0 {
        return Err(AppError::Conflict("다가오는 예약이 있습니다. 예약을 취소한 뒤 탈퇴해 주세요".into()));
    }
    sqlx::query(
        "UPDATE users SET status = 'withdrawn', withdrawn_at = NOW(), email = CONCAT('withdrawn+', id, '@invalid.gageyo'), \
         display_name = '탈퇴 회원', phone = NULL, password_hash = NULL, totp_secret = NULL WHERE id = ?",
    )
    .bind(&u.id)
    .execute(&s.db)
    .await?;
    sqlx::query("DELETE FROM vehicles WHERE user_id = ?").bind(&u.id).execute(&s.db).await?;
    sqlx::query("DELETE FROM favorites WHERE user_id = ?").bind(&u.id).execute(&s.db).await?;
    auth::revoke_user_sessions(&s.db, &u.id).await?;
    let clear = auth::clear_cookie(&s, Scope::Customer);
    Ok(([(header::SET_COOKIE, clear)], Json(serde_json::json!({ "ok": true }))).into_response())
}

#[derive(Debug, Deserialize)]
pub struct ForgotInput {
    pub email: String,
    /// customer(기본) · partner · admin
    pub scope: Option<String>,
}

/// 비밀번호 재설정 메일. 가입 여부를 알 수 없게 항상 같은 응답을 줍니다.
/// 운영자: `ADMIN_EMAILS` 에 있는데 계정이 없으면 운영자 계정을 만들고 설정 링크를 보냅니다 (첫 설정).
async fn forgot_password(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<ForgotInput>) -> AppResult<Json<serde_json::Value>> {
    let scope = Scope::parse(b.scope.as_deref().unwrap_or("customer")).ok_or_else(|| AppError::bad("범위가 올바르지 않습니다"))?;
    let email = normalize_email(&b.email)?;
    let ip = client_ip(&headers);
    if !ratelimit::hit(&s.db, &format!("forgot:{ip}"), 3600, 10).await? || !ratelimit::hit(&s.db, &format!("forgot:{email}"), 3600, 3).await? {
        return Err(AppError::TooMany("잠시 후 다시 시도해 주세요".into()));
    }
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT id, role, status FROM users WHERE email = ?").bind(&email).fetch_optional(&s.db).await?;
    let user_id = match (scope, row) {
        (Scope::Customer, Some((id, role, status))) if role == "customer" && status == "active" => Some(id),
        (Scope::Partner, Some((id, role, status))) if role == "workshop" && status == "active" => Some(id),
        (Scope::Admin, Some((id, _, status))) if status == "active" && s.cfg.admin_emails.contains(&email) => Some(id),
        (Scope::Admin, None) if s.cfg.admin_emails.contains(&email) => {
            let id = new_id();
            sqlx::query("INSERT INTO users (id, email, display_name, role) VALUES (?, ?, '운영자', 'admin')")
                .bind(&id)
                .bind(&email)
                .execute(&s.db)
                .await?;
            Some(id)
        }
        _ => None,
    };
    if let Some(id) = user_id {
        let link = auth::create_password_reset(&s, &id, scope).await?;
        if !s.mailer.enabled() {
            // 메일 설정이 없는 서버: 운영자가 서버 로그에서 확인 (첫 운영자 설정용)
            tracing::warn!(%link, "메일 설정이 없어 비밀번호 설정 링크를 로그에만 남깁니다");
        }
        s.mailer.send(Mail::new(
            email.clone(),
            "[가게요] 비밀번호 설정 안내",
            format!("아래 링크에서 새 비밀번호를 정해 주세요. 링크는 30분 동안 한 번만 쓸 수 있습니다.\n\n{link}\n\n본인이 요청하지 않았다면 이 메일은 무시하셔도 됩니다."),
        ));
    }
    Ok(Json(serde_json::json!({ "sent": true })))
}

#[derive(Debug, Deserialize)]
pub struct ResetInput {
    pub token: String,
    pub password: String,
}

async fn reset_password(State(s): State<AppState>, Json(b): Json<ResetInput>) -> AppResult<Json<serde_json::Value>> {
    auth::check_password(&b.password)?;
    let token = b.token.trim();
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT user_id, scope FROM password_resets WHERE token_hash = ? AND used_at IS NULL AND expires_at > NOW()",
    )
    .bind(auth::token_hash(token))
    .fetch_optional(&s.db)
    .await?;
    let (user_id, scope) = row.ok_or_else(|| AppError::bad("링크가 만료되었거나 이미 사용되었습니다. 다시 요청해 주세요"))?;
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE password_resets SET used_at = NOW() WHERE token_hash = ?").bind(auth::token_hash(token)).execute(&mut *tx).await?;
    sqlx::query("UPDATE users SET password_hash = ?, email_verified_at = COALESCE(email_verified_at, NOW()) WHERE id = ?")
        .bind(auth::hash_password(&b.password)?)
        .bind(&user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM auth_sessions WHERE user_id = ?").bind(&user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({ "ok": true, "scope": scope })))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Vehicle {
    pub id: String,
    pub label: String,
    pub model: String,
    pub model_year: Option<i16>,
    pub plate: Option<String>,
    pub mileage: Option<i32>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct VehicleInput {
    pub label: Option<String>,
    pub model: String,
    pub model_year: Option<i16>,
    pub plate: Option<String>,
    pub mileage: Option<i32>,
}

fn check_vehicle(b: &VehicleInput) -> AppResult<(String, String, Option<String>)> {
    let model = req_line(&b.model, "차종", 120)?;
    let label = opt_text(b.label.as_deref(), "별명", 60)?.unwrap_or_else(|| model.chars().take(60).collect());
    if let Some(y) = b.model_year {
        if !(1950..=2100).contains(&y) {
            return Err(AppError::bad("연식을 확인해 주세요"));
        }
    }
    if let Some(m) = b.mileage {
        if !(0..=3_000_000).contains(&m) {
            return Err(AppError::bad("주행거리를 확인해 주세요"));
        }
    }
    let plate = opt_text(b.plate.as_deref(), "차량 번호", 20)?;
    Ok((label, model, plate))
}

async fn list_vehicles(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<Vec<Vehicle>>> {
    Ok(Json(
        sqlx::query_as::<_, Vehicle>("SELECT id, label, model, model_year, plate, mileage, created_at FROM vehicles WHERE user_id = ? ORDER BY created_at")
            .bind(&u.id)
            .fetch_all(&s.db)
            .await?,
    ))
}

async fn create_vehicle(State(s): State<AppState>, Customer(u): Customer, Json(b): Json<VehicleInput>) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let (label, model, plate) = check_vehicle(&b)?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vehicles WHERE user_id = ?").bind(&u.id).fetch_one(&s.db).await?;
    if n >= 10 {
        return Err(AppError::bad("차량은 10대까지 등록할 수 있습니다"));
    }
    let id = new_id();
    sqlx::query("INSERT INTO vehicles (id, user_id, label, model, model_year, plate, mileage) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&u.id)
        .bind(&label)
        .bind(&model)
        .bind(b.model_year)
        .bind(&plate)
        .bind(b.mileage)
        .execute(&s.db)
        .await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn update_vehicle(State(s): State<AppState>, Customer(u): Customer, Path(id): Path<String>, Json(b): Json<VehicleInput>) -> AppResult<StatusCode> {
    let (label, model, plate) = check_vehicle(&b)?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vehicles WHERE id = ? AND user_id = ?").bind(&id).bind(&u.id).fetch_one(&s.db).await?;
    if n == 0 {
        return Err(AppError::not_found("차량을 찾을 수 없습니다"));
    }
    sqlx::query("UPDATE vehicles SET label = ?, model = ?, model_year = ?, plate = ?, mileage = ? WHERE id = ? AND user_id = ?")
        .bind(&label)
        .bind(&model)
        .bind(b.model_year)
        .bind(&plate)
        .bind(b.mileage)
        .bind(&id)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_vehicle(State(s): State<AppState>, Customer(u): Customer, Path(id): Path<String>) -> AppResult<StatusCode> {
    let r = sqlx::query("DELETE FROM vehicles WHERE id = ? AND user_id = ?").bind(&id).bind(&u.id).execute(&s.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::not_found("차량을 찾을 수 없습니다"));
    }
    Ok(StatusCode::NO_CONTENT)
}
