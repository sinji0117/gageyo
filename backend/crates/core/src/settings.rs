//! 운영자가 화면에서 바꾸는 플랫폼 설정 (`platform_settings`).

use serde::{Deserialize, Serialize};

use crate::{util::req_line, AppError, AppResult, AppState};

/// 플랫폼 수수료율 (bp = 1/10000). 350 = 3.5%
pub const FEE_RATE_BP: &str = "fee_rate_bp";
/// 무통장 입금 계좌
pub const BANK_NAME: &str = "bank_name";
pub const BANK_ACCOUNT: &str = "bank_account";
pub const BANK_HOLDER: &str = "bank_holder";
/// 무통장 입금 기한 (시간)
pub const BANK_DUE_HOURS: &str = "bank_due_hours";
/// 고객센터 안내 (사이트 아래쪽)
pub const SUPPORT_PHONE: &str = "support_phone";
pub const SUPPORT_EMAIL: &str = "support_email";
pub const COMPANY_INFO: &str = "company_info";

pub async fn get(state: &AppState, key: &str) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar("SELECT v FROM platform_settings WHERE k = ?").bind(key).fetch_optional(&state.db).await?)
}

pub async fn set(state: &AppState, key: &str, value: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO platform_settings (k, v) VALUES (?, ?) ON DUPLICATE KEY UPDATE v = VALUES(v)")
        .bind(key)
        .bind(value)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn fee_rate_bp(state: &AppState) -> AppResult<u32> {
    Ok(get(state, FEE_RATE_BP).await?.and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(0).min(3000))
}

/// 무통장 입금 계좌 (세 칸이 모두 있어야 사용)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BankAccount {
    pub bank_name: String,
    pub account: String,
    pub holder: String,
    pub due_hours: u32,
}

pub async fn bank_account(state: &AppState) -> AppResult<Option<BankAccount>> {
    let (name, acct, holder) = (get(state, BANK_NAME).await?, get(state, BANK_ACCOUNT).await?, get(state, BANK_HOLDER).await?);
    let due = get(state, BANK_DUE_HOURS).await?.and_then(|v| v.parse().ok()).unwrap_or(48);
    match (name, acct, holder) {
        (Some(n), Some(a), Some(h)) if !n.is_empty() && !a.is_empty() && !h.is_empty() => {
            Ok(Some(BankAccount { bank_name: n, account: a, holder: h, due_hours: due }))
        }
        _ => Ok(None),
    }
}

#[derive(Debug, Deserialize)]
pub struct SettingsInput {
    pub fee_rate_bp: Option<u32>,
    pub bank_name: Option<String>,
    pub bank_account: Option<String>,
    pub bank_holder: Option<String>,
    pub bank_due_hours: Option<u32>,
    pub support_phone: Option<String>,
    pub support_email: Option<String>,
    pub company_info: Option<String>,
}

/// 검사하고 저장. 바뀐 항목 이름을 돌려줍니다 (작업 기록용)
pub async fn save(state: &AppState, b: &SettingsInput) -> AppResult<Vec<&'static str>> {
    let mut changed = Vec::new();
    if let Some(f) = b.fee_rate_bp {
        if f > 3000 {
            return Err(AppError::bad("수수료율은 0~30% 사이로 입력해 주세요"));
        }
        set(state, FEE_RATE_BP, &f.to_string()).await?;
        changed.push(FEE_RATE_BP);
    }
    if let Some(h) = b.bank_due_hours {
        if !(1..=168).contains(&h) {
            return Err(AppError::bad("입금 기한은 1~168시간입니다"));
        }
        set(state, BANK_DUE_HOURS, &h.to_string()).await?;
        changed.push(BANK_DUE_HOURS);
    }
    let text_fields: [(&'static str, &Option<String>, &str, usize); 6] = [
        (BANK_NAME, &b.bank_name, "은행 이름", 30),
        (BANK_ACCOUNT, &b.bank_account, "계좌번호", 40),
        (BANK_HOLDER, &b.bank_holder, "예금주", 40),
        (SUPPORT_PHONE, &b.support_phone, "고객센터 전화", 30),
        (SUPPORT_EMAIL, &b.support_email, "고객센터 이메일", 254),
        (COMPANY_INFO, &b.company_info, "사업자 정보", 500),
    ];
    for (key, value, label, max) in text_fields {
        if let Some(v) = value {
            let v = v.trim();
            if key == BANK_ACCOUNT && !v.is_empty() && !v.chars().all(|c| c.is_ascii_digit() || c == '-') {
                return Err(AppError::bad("계좌번호는 숫자와 - 만 입력해 주세요"));
            }
            if key == SUPPORT_EMAIL && !v.is_empty() && !crate::mail::looks_like_email(v) {
                return Err(AppError::bad("고객센터 이메일 형식을 확인해 주세요"));
            }
            let clean = if v.is_empty() {
                String::new()
            } else if key == COMPANY_INFO {
                crate::util::req_text(v, label, max)?
            } else {
                req_line(v, label, max)?
            };
            set(state, key, &clean).await?;
            changed.push(key);
        }
    }
    Ok(changed)
}

/// 화면에 보여 줄 전체 설정
pub async fn all(state: &AppState) -> AppResult<serde_json::Value> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT k, v FROM platform_settings").fetch_all(&state.db).await?;
    let mut m = serde_json::Map::new();
    for (k, v) in rows {
        m.insert(k, serde_json::Value::String(v));
    }
    m.entry(FEE_RATE_BP).or_insert_with(|| "0".into());
    m.entry(BANK_DUE_HOURS).or_insert_with(|| "48".into());
    Ok(serde_json::Value::Object(m))
}
