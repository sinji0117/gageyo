//! 운영자 로그인과 2단계 인증(OTP) — bandiuri 운영자 화면 방식을 옮김.
//!
//! - 따로 가입하지 않습니다. 서버 설정 `ADMIN_EMAILS` 에 적힌 이메일만 로그인할 수 있습니다.
//!   처음에는 운영자 화면의 **비밀번호 설정 메일 받기**로 비밀번호를 정합니다 (메일 설정이 없으면 서버 로그에 링크).
//! - OTP 를 켜면 로그인할 때 6자리 코드를 함께 넣고, 같은 코드는 한 번만 쓸 수 있습니다.
//!   비밀값은 DB 에 암호화해서 둡니다. OTP 앱을 잃어버리면 서버 DB 에서 `users.totp_secret` 을 비워 끕니다.

use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use gageyo_core::{
    audit,
    auth::{self, client_ip, Admin, Scope},
    totp, AppError, AppResult, AppState,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/auth/login", post(login))
        .route("/admin/auth/logout", post(logout))
        .route("/admin/auth/me", get(me))
        .route("/admin/auth/otp/setup", post(otp_setup))
        .route("/admin/auth/otp/enable", post(otp_enable))
        .route("/admin/auth/otp/disable", post(otp_disable))
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
    pub otp: Option<String>,
}

async fn login(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<LoginInput>) -> AppResult<Response> {
    if s.cfg.admin_emails.is_empty() {
        return Err(AppError::Unavailable("서버에 ADMIN_EMAILS 가 설정되지 않았습니다".into()));
    }
    let email = b.email.trim().to_ascii_lowercase();
    auth::check_login_allowed(&s, Scope::Admin, &email, &client_ip(&headers)).await?;
    let row: Option<(String, Option<String>, String, Option<String>, Option<i64>)> =
        sqlx::query_as("SELECT id, password_hash, status, totp_secret, totp_last_step FROM users WHERE email = ?")
            .bind(&email)
            .fetch_optional(&s.db)
            .await?;
    let allowed = s.cfg.admin_emails.contains(&email);
    let ok = allowed && row.as_ref().is_some_and(|(_, h, st, _, _)| st == "active" && h.as_deref().is_some_and(|h| auth::verify_password(&b.password, h)));
    let Some((user_id, _, _, secret, last_step)) = row.filter(|_| ok) else {
        auth::record_login_fail(&s, Scope::Admin, &email).await?;
        return Err(AppError::Unauthorized("이메일 또는 비밀번호를 확인해 주세요".into()));
    };
    if let Some(enc) = secret.filter(|v| !v.is_empty()) {
        let code = b.otp.as_deref().map(str::trim).filter(|c| !c.is_empty()).ok_or(AppError::OtpRequired)?;
        let secret = s.secrets.decrypt(&enc).map_err(AppError::Internal)?;
        let step = totp::verify(&secret, code, totp::now_step());
        match step {
            Some(st) if last_step.is_none_or(|l| (st as i64) > l) => {
                sqlx::query("UPDATE users SET totp_last_step = ? WHERE id = ?").bind(st as i64).bind(&user_id).execute(&s.db).await?;
            }
            _ => {
                auth::record_login_fail(&s, Scope::Admin, &email).await?;
                return Err(AppError::Unauthorized("OTP 코드가 맞지 않거나 이미 사용한 코드입니다".into()));
            }
        }
    }
    auth::clear_login_fails(&s, Scope::Admin, &email).await?;
    let cookie = auth::create_session(&s, &user_id, Scope::Admin, &headers).await?;
    let admin = Admin { actor: email.clone(), user_id: Some(user_id), ip: client_ip(&headers) };
    audit::log(&s, &admin, "admin.login", "user", admin.user_id.as_deref().unwrap_or(""), None).await;
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "email": email }))).into_response())
}

async fn logout(State(s): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let cookie = auth::end_session(&s, &headers, Scope::Admin).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ok": true }))).into_response())
}

async fn me(State(s): State<AppState>, admin: Admin) -> AppResult<Json<serde_json::Value>> {
    let otp_enabled = match &admin.user_id {
        Some(id) => {
            let v: Option<String> = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = ?").bind(id).fetch_one(&s.db).await?;
            v.is_some_and(|v| !v.is_empty())
        }
        None => false,
    };
    Ok(Json(serde_json::json!({
        "email": admin.actor, "via_key": admin.user_id.is_none(), "otp_enabled": otp_enabled,
        "mail_enabled": s.mailer.enabled(), "toss_enabled": s.cfg.toss_enabled(),
    })))
}

fn need_account(admin: &Admin) -> AppResult<&str> {
    admin.user_id.as_deref().ok_or_else(|| AppError::bad("API 키 접속에서는 OTP 를 설정할 수 없습니다. 운영자 계정으로 로그인해 주세요"))
}

/// OTP 등록 시작: 새 비밀값을 만들어 (아직 켜지 않고) 돌려줍니다. 화면이 QR 코드로 보여 줍니다.
async fn otp_setup(admin: Admin) -> AppResult<Json<serde_json::Value>> {
    need_account(&admin)?;
    let secret = totp::new_secret();
    let url = totp::otpauth_url("가게요", &admin.actor, &secret);
    // 아직 켜지 않음 — 확인 코드를 넣어야 저장
    Ok(Json(serde_json::json!({ "secret": secret, "otpauth_url": url })))
}

#[derive(Debug, Deserialize)]
pub struct EnableInput {
    pub secret: String,
    pub code: String,
}

async fn otp_enable(State(s): State<AppState>, admin: Admin, Json(b): Json<EnableInput>) -> AppResult<Json<serde_json::Value>> {
    let id = need_account(&admin)?.to_string();
    let secret = b.secret.trim().to_ascii_uppercase();
    if totp::base32_decode(&secret).is_none_or(|k| k.len() < 10) {
        return Err(AppError::bad("OTP 비밀값이 올바르지 않습니다. 다시 시작해 주세요"));
    }
    let step = totp::verify(&secret, &b.code, totp::now_step()).ok_or_else(|| AppError::bad("코드가 맞지 않습니다. OTP 앱의 지금 코드를 넣어 주세요"))?;
    sqlx::query("UPDATE users SET totp_secret = ?, totp_last_step = ? WHERE id = ?")
        .bind(s.secrets.encrypt(&secret))
        .bind(step as i64)
        .bind(&id)
        .execute(&s.db)
        .await?;
    audit::log(&s, &admin, "admin.otp_enabled", "user", &id, None).await;
    Ok(Json(serde_json::json!({ "otp_enabled": true })))
}

#[derive(Debug, Deserialize)]
pub struct DisableInput {
    pub code: String,
}

async fn otp_disable(State(s): State<AppState>, admin: Admin, Json(b): Json<DisableInput>) -> AppResult<Json<serde_json::Value>> {
    let id = need_account(&admin)?.to_string();
    let enc: Option<String> = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = ?").bind(&id).fetch_one(&s.db).await?;
    let enc = enc.filter(|v| !v.is_empty()).ok_or_else(|| AppError::bad("OTP 가 켜져 있지 않습니다"))?;
    let secret = s.secrets.decrypt(&enc).map_err(AppError::Internal)?;
    totp::verify(&secret, &b.code, totp::now_step()).ok_or_else(|| AppError::bad("코드가 맞지 않습니다"))?;
    sqlx::query("UPDATE users SET totp_secret = NULL, totp_last_step = NULL WHERE id = ?").bind(&id).execute(&s.db).await?;
    audit::log(&s, &admin, "admin.otp_disabled", "user", &id, None).await;
    Ok(Json(serde_json::json!({ "otp_enabled": false })))
}
