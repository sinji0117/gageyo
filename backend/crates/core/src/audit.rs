//! 운영자 작업 기록 (`admin_audit`). 누가 언제 무엇을 승인·정지·변경했는지와 IP 를 남깁니다.

use serde::Serialize;

use crate::{auth::Admin, AppResult, AppState};

pub async fn log(state: &AppState, admin: &Admin, action: &str, target_type: &str, target_id: &str, detail: Option<String>) {
    let r = sqlx::query("INSERT INTO admin_audit (actor, action, target_type, target_id, detail, ip) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&admin.actor)
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(detail.map(|d| d.chars().take(2000).collect::<String>()))
        .bind(&admin.ip)
        .execute(&state.db)
        .await;
    if let Err(e) = r {
        // 기록 실패로 작업 자체를 되돌리지는 않습니다.
        tracing::error!(error = %e, action, target_type, target_id, "admin audit write failed");
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditRow {
    pub id: u64,
    pub actor: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub detail: Option<String>,
    pub ip: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn list(state: &AppState, target_type: Option<&str>, limit: i64, offset: i64) -> AppResult<Vec<AuditRow>> {
    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT id, actor, action, target_type, target_id, detail, ip, created_at FROM admin_audit \
         WHERE (? IS NULL OR target_type = ?) ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(target_type)
    .bind(target_type)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}
