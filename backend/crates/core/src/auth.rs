//! 인증 — 고객·파트너·운영자 세션.
//!
//! - 비밀번호: argon2id
//! - 세션: 불투명한 무작위 토큰을 HttpOnly 쿠키로 주고 DB(`auth_sessions`)에는 SHA-256 해시만 둡니다.
//! - 범위(scope)마다 쿠키 이름이 다릅니다: 고객 `gy_c`, 파트너 `gy_p`, 운영자 `gy_a`.
//!   세션에도 범위를 저장해서, 고객 세션 토큰으로 파트너·운영자 API 를 열 수 없습니다.
//!   (로컬 개발에서는 화면들이 같은 `localhost` 라 쿠키가 섞일 수 있어 이름을 나눴습니다.)
//! - 계정을 정지하면 그 계정의 세션을 모두 지워 바로 로그아웃됩니다.
//! - 로그인 5번 실패하면 15분 잠금(HTTP 423). 같은 IP 는 15분에 50번까지 시도할 수 있습니다.
//! - 예전 방식(`x-partner-key`, `x-admin-key` 헤더)도 계속 받습니다 — 자동화·비상용.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap},
};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{ratelimit, util::new_id, AppError, AppResult, AppState};

pub const MAX_LOGIN_FAILS: i64 = 5;
pub const LOCK_SECS: i64 = 15 * 60;
pub const MAX_LOGIN_PER_IP: i64 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Customer,
    Partner,
    Admin,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Customer => "customer",
            Scope::Partner => "partner",
            Scope::Admin => "admin",
        }
    }
    pub fn cookie_name(self) -> &'static str {
        match self {
            Scope::Customer => "gy_c",
            Scope::Partner => "gy_p",
            Scope::Admin => "gy_a",
        }
    }
    /// 세션 유효 기간 (초)
    pub fn ttl_secs(self) -> i64 {
        match self {
            Scope::Customer => 30 * 86_400,
            Scope::Partner => 14 * 86_400,
            Scope::Admin => 12 * 3_600,
        }
    }
    pub fn parse(v: &str) -> Option<Self> {
        match v {
            "customer" => Some(Scope::Customer),
            "partner" => Some(Scope::Partner),
            "admin" => Some(Scope::Admin),
            _ => None,
        }
    }
}

pub fn hash_password(plain: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("argon2: {e}")))
}

pub fn verify_password(plain: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(plain.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

/// 비밀번호 규칙: 10~128자, 한 글자 반복 금지
pub fn check_password(p: &str) -> AppResult<()> {
    let n = p.chars().count();
    if !(10..=128).contains(&n) {
        return Err(AppError::bad("비밀번호는 10~128자로 입력해 주세요"));
    }
    let first = p.chars().next().unwrap_or(' ');
    if p.chars().all(|c| c == first) {
        return Err(AppError::bad("같은 글자만 반복한 비밀번호는 쓸 수 없습니다"));
    }
    Ok(())
}

pub fn random_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// 같은 길이 문자열을 시간 차이 없이 비교
pub fn constant_eq(a: &str, b: &str) -> bool {
    crate::verify::same(a, b)
}

pub fn read_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).find_map(|raw| {
        raw.split(';')
            .map(str::trim)
            .find_map(|kv| kv.strip_prefix(name).and_then(|rest| rest.strip_prefix('=')))
            .filter(|v| !v.is_empty())
    })
}

pub fn session_cookie(state: &AppState, scope: Scope, token: &str) -> String {
    format!(
        "{}={token}; Path=/api; Max-Age={}; HttpOnly; SameSite=Lax{}",
        scope.cookie_name(),
        scope.ttl_secs(),
        if state.cfg.cookie_secure { "; Secure" } else { "" }
    )
}

pub fn clear_cookie(state: &AppState, scope: Scope) -> String {
    format!(
        "{}=; Path=/api; Max-Age=0; HttpOnly; SameSite=Lax{}",
        scope.cookie_name(),
        if state.cfg.cookie_secure { "; Secure" } else { "" }
    )
}

/// 방문자 IP: Rpanel Nginx 의 `X-Real-IP` > `X-Forwarded-For` 첫 값
pub fn client_ip(headers: &HeaderMap) -> String {
    let pick = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(|v| v.split(',').next().unwrap_or("").trim().to_string());
    pick("x-real-ip")
        .filter(|v| !v.is_empty())
        .or_else(|| pick("x-forwarded-for").filter(|v| !v.is_empty()))
        .map(|v| v.chars().take(64).collect())
        .unwrap_or_default()
}

fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|v| v.chars().take(255).collect())
}

/// 새 세션을 만들고 `Set-Cookie` 값을 돌려줍니다. 마지막 로그인 시각·IP 도 기록합니다.
pub async fn create_session(state: &AppState, user_id: &str, scope: Scope, headers: &HeaderMap) -> AppResult<String> {
    let token = random_token();
    let ip = client_ip(headers);
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, scope, token_hash, expires_at, ip, user_agent) \
         VALUES (?, ?, ?, ?, NOW() + INTERVAL ? SECOND, ?, ?)",
    )
    .bind(new_id())
    .bind(user_id)
    .bind(scope.as_str())
    .bind(token_hash(&token))
    .bind(scope.ttl_secs())
    .bind(&ip)
    .bind(user_agent(headers))
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE users SET last_login_at = NOW(), last_login_ip = ? WHERE id = ?")
        .bind(&ip)
        .bind(user_id)
        .execute(&state.db)
        .await?;
    // 만료된 세션 정리 (가끔)
    if rand::random::<u8>() < 8 {
        let _ = sqlx::query("DELETE FROM auth_sessions WHERE expires_at < NOW()").execute(&state.db).await;
    }
    Ok(session_cookie(state, scope, &token))
}

/// 로그아웃: 이 요청의 세션을 지우고 쿠키 만료 값을 돌려줍니다.
pub async fn end_session(state: &AppState, headers: &HeaderMap, scope: Scope) -> AppResult<String> {
    if let Some(token) = read_cookie(headers, scope.cookie_name()) {
        sqlx::query("DELETE FROM auth_sessions WHERE token_hash = ? AND scope = ?")
            .bind(token_hash(token))
            .bind(scope.as_str())
            .execute(&state.db)
            .await?;
    }
    Ok(clear_cookie(state, scope))
}

/// 계정의 모든 세션 삭제 (정지·비밀번호 변경·탈퇴)
pub async fn revoke_user_sessions(db: &sqlx::MySqlPool, user_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM auth_sessions WHERE user_id = ?").bind(user_id).execute(db).await?;
    Ok(())
}

/// 로그인 시도 전에 잠금 확인 + IP 당 시도 횟수 세기
pub async fn check_login_allowed(state: &AppState, scope: Scope, email: &str, ip: &str) -> AppResult<()> {
    if ratelimit::count(&state.db, &fail_key(scope, email), LOCK_SECS).await? >= MAX_LOGIN_FAILS {
        return Err(AppError::Locked);
    }
    if !ip.is_empty() && !ratelimit::hit(&state.db, &format!("login-ip:{ip}"), LOCK_SECS, MAX_LOGIN_PER_IP).await? {
        return Err(AppError::TooMany("로그인 시도가 너무 많습니다. 잠시 후 다시 시도해 주세요".into()));
    }
    Ok(())
}

pub async fn record_login_fail(state: &AppState, scope: Scope, email: &str) -> AppResult<()> {
    ratelimit::hit(&state.db, &fail_key(scope, email), LOCK_SECS, i64::MAX).await?;
    Ok(())
}

pub async fn clear_login_fails(state: &AppState, scope: Scope, email: &str) -> AppResult<()> {
    ratelimit::clear(&state.db, &fail_key(scope, email)).await
}

fn fail_key(scope: Scope, email: &str) -> String {
    format!("login-fail:{}:{email}", scope.as_str())
}

/// 세션 쿠키 주인
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SessionUser {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub phone: Option<String>,
    pub role: String,
    pub status: String,
}

pub async fn session_user(state: &AppState, headers: &HeaderMap, scope: Scope) -> AppResult<Option<SessionUser>> {
    let Some(token) = read_cookie(headers, scope.cookie_name()) else { return Ok(None) };
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(None);
    }
    let user = sqlx::query_as::<_, SessionUser>(
        "SELECT u.id, u.email, u.display_name, u.phone, u.role, u.status FROM auth_sessions a \
         JOIN users u ON u.id = a.user_id \
         WHERE a.token_hash = ? AND a.scope = ? AND a.expires_at > NOW() AND u.status = 'active'",
    )
    .bind(token_hash(token))
    .bind(scope.as_str())
    .fetch_optional(&state.db)
    .await?;
    Ok(user)
}

/// 로그인한 고객 (없으면 401)
#[derive(Debug, Clone)]
pub struct Customer(pub SessionUser);

impl FromRequestParts<AppState> for Customer {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        match session_user(state, &parts.headers, Scope::Customer).await? {
            Some(u) if u.role == "customer" => Ok(Customer(u)),
            _ => Err(AppError::login_required()),
        }
    }
}

/// 로그인했으면 고객, 아니면 None (비회원도 쓰는 API)
#[derive(Debug, Clone)]
pub struct MaybeCustomer(pub Option<SessionUser>);

impl FromRequestParts<AppState> for MaybeCustomer {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(MaybeCustomer(session_user(state, &parts.headers, Scope::Customer).await?.filter(|u| u.role == "customer")))
    }
}

/// 파트너(업체). 세션(파트너 계정) 또는 예전 `x-partner-key` 로 확인합니다.
#[derive(Debug, Clone)]
pub struct Partner {
    pub workshop_id: String,
    pub workshop_name: String,
    pub workshop_status: String,
    /// 계정으로 로그인했을 때만 있음 (파트너 키 접속은 None)
    pub user: Option<SessionUser>,
}

impl Partner {
    pub fn is_approved(&self) -> bool {
        self.workshop_status == "approved"
    }
    pub fn actor(&self) -> String {
        self.user.as_ref().map(|u| u.email.clone()).unwrap_or_else(|| "partner-key".into())
    }
}

pub async fn partner_from_headers(state: &AppState, headers: &HeaderMap) -> AppResult<Partner> {
    if let Some(user) = session_user(state, headers, Scope::Partner).await? {
        if user.role != "workshop" {
            return Err(AppError::forbidden());
        }
        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT id, name, status FROM workshops WHERE owner_user_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&user.id)
        .fetch_optional(&state.db)
        .await?;
        let (workshop_id, workshop_name, workshop_status) =
            row.ok_or_else(|| AppError::Forbidden("이 계정에 연결된 업체가 없습니다".into()))?;
        return Ok(Partner { workshop_id, workshop_name, workshop_status, user: Some(user) });
    }
    let key = headers.get("x-partner-key").and_then(|v| v.to_str().ok()).unwrap_or("").trim();
    if key.is_empty() {
        return Err(AppError::login_required());
    }
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT w.id, w.name, w.status FROM workshop_access_tokens t JOIN workshops w ON w.id = t.workshop_id WHERE t.token_hash = ?",
    )
    .bind(token_hash(key))
    .fetch_optional(&state.db)
    .await?;
    let (workshop_id, workshop_name, workshop_status) =
        row.ok_or_else(|| AppError::Unauthorized("파트너 키를 확인해 주세요".into()))?;
    Ok(Partner { workshop_id, workshop_name, workshop_status, user: None })
}

impl FromRequestParts<AppState> for Partner {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let p = partner_from_headers(state, &parts.headers).await?;
        if p.workshop_status == "suspended" {
            return Err(AppError::Forbidden("운영자가 이용을 중지한 업체입니다. 고객센터로 문의해 주세요".into()));
        }
        Ok(p)
    }
}

/// 승인된 파트너만 (견적 답변·예약·상품 판매)
#[derive(Debug, Clone)]
pub struct ApprovedPartner(pub Partner);

impl FromRequestParts<AppState> for ApprovedPartner {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let p = Partner::from_request_parts(parts, state).await?;
        if !p.is_approved() {
            return Err(AppError::Forbidden("운영자 승인 뒤에 이용할 수 있습니다".into()));
        }
        Ok(ApprovedPartner(p))
    }
}

/// 운영자. `ADMIN_EMAILS` 에 있는 계정의 운영자 세션 또는 `x-admin-key`.
#[derive(Debug, Clone)]
pub struct Admin {
    /// 작업 기록에 남는 이름 (이메일 또는 `api-key`)
    pub actor: String,
    pub user_id: Option<String>,
    pub ip: String,
}

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let ip = client_ip(&parts.headers);
        if let Some(key) = parts.headers.get("x-admin-key").and_then(|v| v.to_str().ok()) {
            return match state.cfg.admin_api_key.as_deref() {
                Some(expected) if constant_eq(key.trim(), expected) => Ok(Admin { actor: "api-key".into(), user_id: None, ip }),
                Some(_) => Err(AppError::Unauthorized("관리자 API 키를 확인해 주세요".into())),
                None => Err(AppError::Unavailable("서버에 ADMIN_API_KEY 가 설정되지 않았습니다".into())),
            };
        }
        if let Some(u) = session_user(state, &parts.headers, Scope::Admin).await? {
            if state.cfg.admin_emails.iter().any(|e| e == &u.email) {
                return Ok(Admin { actor: u.email, user_id: Some(u.id), ip });
            }
            return Err(AppError::forbidden());
        }
        Err(AppError::Unauthorized("운영자 로그인이 필요합니다".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn password_rules() {
        let h = hash_password("correct horse battery").unwrap();
        assert!(verify_password("correct horse battery", &h));
        assert!(!verify_password("wrong", &h));
        assert!(!verify_password("x", "not-a-hash"));
        assert!(check_password("short").is_err());
        assert!(check_password("aaaaaaaaaaaa").is_err());
        assert!(check_password("good-password-1").is_ok());
    }

    #[test]
    fn cookies_and_tokens() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("a=1; gy_c=abc123; gy_cx=no"));
        assert_eq!(read_cookie(&h, "gy_c"), Some("abc123"));
        assert_eq!(read_cookie(&h, "gy_p"), None);
        let t = random_token();
        assert_eq!(t.len(), 64);
        assert_eq!(token_hash(&t).len(), 64);
        assert_ne!(token_hash(&t), t);
        assert_eq!(Scope::parse("admin"), Some(Scope::Admin));
        assert_ne!(Scope::Customer.cookie_name(), Scope::Partner.cookie_name());
    }

    #[test]
    fn ip_headers() {
        let mut h = HeaderMap::new();
        assert_eq!(client_ip(&h), "");
        h.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4, 10.0.0.1"));
        assert_eq!(client_ip(&h), "1.2.3.4");
        h.insert("x-real-ip", HeaderValue::from_static("5.6.7.8"));
        assert_eq!(client_ip(&h), "5.6.7.8");
    }
}
