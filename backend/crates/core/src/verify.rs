//! 회원가입 이메일 코드 인증 (bandiuri `verify.rs` 를 DB 저장 방식으로 옮김).
//!
//! 1. 가입 화면에서 이메일을 넣고 **인증 코드 받기** → 6자리 숫자 코드를 메일로 보냅니다 (10분 유효).
//! 2. 가입할 때 `email_code` 로 그 코드를 함께 보냅니다. 맞으면 가입되고 코드는 지워집니다.
//!
//! - 고객 가입(`customer`)과 파트너 입점 신청(`partner`)은 코드가 따로입니다.
//! - DB 에는 코드 원문이 아니라 서버 비밀키로 만든 해시만 둡니다. 5번 틀리면 코드가 지워집니다.
//! - 다시 받기는 같은 이메일당 60초에 한 번, 1시간에 5번, IP 당 1시간에 30번까지입니다.
//! - `SIGNUP_EMAIL_VERIFY`: `auto`(기본, 메일 설정 `SMTP_URL` 이 있을 때만 켬) · `on` · `off`.

use rand::Rng;

use crate::{
    mail::{mask_email, Mail},
    ratelimit, AppError, AppResult, AppState,
};

pub const CODE_TTL_MINUTES: i64 = 10;
pub const RESEND_SECS: i64 = 60;
pub const MAX_SENDS_PER_HOUR: i64 = 5;
pub const MAX_SENDS_PER_IP_HOUR: i64 = 30;
pub const MAX_TRIES: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Auto,
    On,
    Off,
}

impl Mode {
    pub fn parse(v: Option<&str>) -> Self {
        match v.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("on" | "true" | "1" | "yes") => Mode::On,
            Some("off" | "false" | "0" | "no") => Mode::Off,
            _ => Mode::Auto,
        }
    }
}

/// 지금 가입에 이메일 인증이 필요한지
pub fn required(state: &AppState) -> bool {
    match state.cfg.signup_email_verify {
        Mode::On => true,
        Mode::Off => false,
        Mode::Auto => state.mailer.enabled(),
    }
}

pub fn new_code() -> String {
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000u32))
}

/// 숫자만 남겨 6자리인지
pub fn clean_code(code: &str) -> Option<String> {
    let c: String = code.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    (c.len() == 6 && c.bytes().all(|b| b.is_ascii_digit())).then_some(c)
}

/// 같은 길이 문자열을 시간 차이 없이 비교
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn hash(state: &AppState, scope: &str, email: &str, code: &str) -> String {
    state.secrets.keyed_hash(&["signup-code", scope, email, code])
}

/// 코드를 만들어 메일로 보냅니다.
pub async fn send_code(state: &AppState, scope: &str, email: &str, ip: &str) -> AppResult<()> {
    let since: Option<i64> =
        sqlx::query_scalar("SELECT TIMESTAMPDIFF(SECOND, sent_at, NOW()) FROM email_codes WHERE scope = ? AND email = ?")
            .bind(scope)
            .bind(email)
            .fetch_optional(&state.db)
            .await?;
    if let Some(s) = since {
        if s < RESEND_SECS {
            return Err(AppError::TooMany(format!("{}초 뒤에 다시 받을 수 있습니다", (RESEND_SECS - s).max(1))));
        }
    }
    if !ratelimit::hit(&state.db, &format!("verify:n:{scope}:{email}"), 3600, MAX_SENDS_PER_HOUR).await? {
        return Err(AppError::TooMany("인증 코드를 너무 많이 요청했습니다. 1시간 뒤에 다시 해 주세요".into()));
    }
    if !ip.is_empty() && !ratelimit::hit(&state.db, &format!("verify:ip:{ip}"), 3600, MAX_SENDS_PER_IP_HOUR).await? {
        return Err(AppError::TooMany("잠시 후 다시 시도해 주세요".into()));
    }
    let code = new_code();
    sqlx::query(
        "REPLACE INTO email_codes (scope, email, code_hash, tries, expires_at, sent_at) \
         VALUES (?, ?, ?, 0, NOW() + INTERVAL ? MINUTE, NOW())",
    )
    .bind(scope)
    .bind(email)
    .bind(hash(state, scope, email, &code))
    .bind(CODE_TTL_MINUTES)
    .execute(&state.db)
    .await?;
    if !state.mailer.enabled() {
        // 메일 설정 없이 SIGNUP_EMAIL_VERIFY=on 으로 띄운 개발 서버 — 서버 로그로 확인
        tracing::warn!(to = %mask_email(email), %code, "메일 설정이 없어 인증 코드를 로그에만 남깁니다 (개발용)");
    }
    let what = if scope == "partner" { "파트너 입점 신청" } else { "회원가입" };
    state.mailer.send(Mail::new(
        email,
        format!("[가게요] 이메일 인증 코드 {code}"),
        format!(
            "가게요 {what} 인증 코드입니다.\n\n    {code}\n\n화면에 이 6자리 숫자를 넣어 주세요. {CODE_TTL_MINUTES}분 동안 쓸 수 있습니다.\n\
             본인이 요청하지 않았다면 이 메일은 무시하셔도 됩니다."
        ),
    ));
    Ok(())
}

/// 가입 직전에 코드 확인. 맞으면 코드를 지웁니다. 인증이 꺼져 있으면 그냥 통과.
pub async fn consume(state: &AppState, scope: &str, email: &str, code: Option<&str>) -> AppResult<()> {
    if !required(state) {
        return Ok(());
    }
    let code = code
        .filter(|c| !c.trim().is_empty())
        .ok_or_else(|| AppError::bad("이메일로 받은 인증 코드를 입력해 주세요"))?;
    let code = clean_code(code).ok_or_else(|| AppError::bad("인증 코드는 6자리 숫자입니다"))?;
    let row: Option<(String, i32)> = sqlx::query_as(
        "SELECT code_hash, tries FROM email_codes WHERE scope = ? AND email = ? AND expires_at > NOW()",
    )
    .bind(scope)
    .bind(email)
    .fetch_optional(&state.db)
    .await?;
    let Some((stored, tries)) = row else {
        return Err(AppError::bad("인증 코드가 없거나 만료되었습니다. 코드를 다시 받아 주세요"));
    };
    let remove = || sqlx::query("DELETE FROM email_codes WHERE scope = ? AND email = ?").bind(scope).bind(email);
    if tries >= MAX_TRIES {
        remove().execute(&state.db).await?;
        return Err(AppError::bad("인증 코드를 여러 번 틀렸습니다. 코드를 다시 받아 주세요"));
    }
    if !same(&stored, &hash(state, scope, email, &code)) {
        sqlx::query("UPDATE email_codes SET tries = tries + 1 WHERE scope = ? AND email = ?")
            .bind(scope)
            .bind(email)
            .execute(&state.db)
            .await?;
        let left = MAX_TRIES - tries - 1;
        if left <= 0 {
            remove().execute(&state.db).await?;
            return Err(AppError::bad("인증 코드를 여러 번 틀렸습니다. 코드를 다시 받아 주세요"));
        }
        return Err(AppError::bad(format!("인증 코드가 맞지 않습니다 (남은 횟수 {left}번)")));
    }
    remove().execute(&state.db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes() {
        assert_eq!(Mode::parse(None), Mode::Auto);
        assert_eq!(Mode::parse(Some(" ON ")), Mode::On);
        assert_eq!(Mode::parse(Some("off")), Mode::Off);
        assert_eq!(Mode::parse(Some("maybe")), Mode::Auto);
    }

    #[test]
    fn codes() {
        for _ in 0..200 {
            let c = new_code();
            assert_eq!(c.len(), 6);
            assert!(c.bytes().all(|b| b.is_ascii_digit()));
        }
        assert_eq!(clean_code(" 012 345 ").as_deref(), Some("012345"));
        assert_eq!(clean_code("012-345").as_deref(), Some("012345"));
        assert_eq!(clean_code("12345"), None);
        assert_eq!(clean_code("12345a"), None);
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
    }
}
