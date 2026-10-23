//! 정산: 업체별 월 매출 − 플랫폼 수수료 = 지급액. 운영자가 송금한 뒤 '지급 완료'로 기록합니다.
//!
//! - 매출은 플랫폼이 받은 결제(카드·무통장)의 결제 완료 시각(`paid_at`) 기준 월로 셉니다.
//!   취소·환불된 주문과 취소된 줄은 빼고, 결제 수단 없이 업체와 직접 결제한 주문(`none`)은 정산 대상이 아닙니다.
//! - 수수료는 결제 완료 때 줄마다 고정된 값(`order_items.fee_amount`)입니다.
//! - 지급 완료 기록에는 그때 금액이 남습니다. 지급 뒤에 환불이 생기면 화면에 '금액 변동'으로 표시됩니다.
//! - 실제 송금(펌뱅킹)·세금계산서 발행은 운영자가 따로 합니다.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, NaiveDateTime};
use gageyo_core::{
    audit,
    auth::{Admin, ApprovedPartner},
    util::{kst_now, new_id, opt_text},
    AppError, AppResult, AppState,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/partner/settlements", get(partner_settlements))
        .route("/admin/settlements", get(admin_settlements).post(mark_paid).delete(undo_paid))
}

const SALES_WHERE: &str = "oi.status <> 'cancelled' AND o.status NOT IN ('payment_pending','cancelled','refunded') \
    AND o.payment_method IN ('toss','bank') AND o.paid_at IS NOT NULL";

/// `2026-10` 형식 검사
pub fn check_period(p: &str) -> AppResult<String> {
    let ok = p.len() == 7
        && p.as_bytes()[4] == b'-'
        && p[..4].bytes().all(|b| b.is_ascii_digit())
        && p[5..].bytes().all(|b| b.is_ascii_digit())
        && (1..=12).contains(&p[5..].parse::<u32>().unwrap_or(0));
    if !ok {
        return Err(AppError::bad("정산 월은 2026-10 형식으로 입력해 주세요"));
    }
    Ok(p.to_string())
}

fn current_period() -> String {
    let n = kst_now();
    format!("{:04}-{:02}", n.year(), n.month())
}

#[derive(Debug, Deserialize)]
pub struct YearQuery {
    pub year: Option<i32>,
}

async fn partner_settlements(State(s): State<AppState>, ApprovedPartner(p): ApprovedPartner, Query(q): Query<YearQuery>) -> AppResult<Json<serde_json::Value>> {
    let year = q.year.unwrap_or_else(|| kst_now().year());
    let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(&format!(
        "SELECT DATE_FORMAT(o.paid_at, '%Y-%m') AS period, CAST(SUM(oi.unit_price * oi.quantity) AS SIGNED), \
           CAST(SUM(oi.fee_amount) AS SIGNED), COUNT(DISTINCT o.id) \
         FROM order_items oi JOIN orders o ON o.id = oi.order_id \
         WHERE oi.workshop_id = ? AND {SALES_WHERE} AND YEAR(o.paid_at) = ? GROUP BY period ORDER BY period DESC"
    ))
    .bind(&p.workshop_id)
    .bind(year)
    .fetch_all(&s.db)
    .await?;
    let paid: Vec<(String, u64, u64, u64, NaiveDateTime, Option<String>)> =
        sqlx::query_as("SELECT period, gross, fee, net, paid_at, memo FROM settlements WHERE workshop_id = ? AND period LIKE ?")
            .bind(&p.workshop_id)
            .bind(format!("{year}-%"))
            .fetch_all(&s.db)
            .await?;
    let months: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(period, gross, fee, orders)| {
            let rec = paid.iter().find(|r| r.0 == period);
            serde_json::json!({
                "period": period, "gross": gross, "fee": fee, "net": gross - fee, "orders": orders,
                "paid": rec.map(|r| serde_json::json!({"gross": r.1, "fee": r.2, "net": r.3, "paid_at": r.4, "memo": r.5})),
                "changed_after_paid": rec.is_some_and(|r| r.1 as i64 != gross || r.2 as i64 != fee),
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "year": year, "fee_rate_bp": gageyo_core::settings::fee_rate_bp(&s).await?, "months": months })))
}

#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    pub period: Option<String>,
}

async fn admin_settlements(State(s): State<AppState>, _a: Admin, Query(q): Query<PeriodQuery>) -> AppResult<Json<serde_json::Value>> {
    let period = match q.period.as_deref() {
        Some(p) => check_period(p)?,
        None => current_period(),
    };
    let rows: Vec<(String, String, Option<String>, i64, i64, i64)> = sqlx::query_as(&format!(
        "SELECT w.id, w.name, w.business_number, CAST(SUM(oi.unit_price * oi.quantity) AS SIGNED), CAST(SUM(oi.fee_amount) AS SIGNED), \
           COUNT(DISTINCT o.id) \
         FROM order_items oi JOIN orders o ON o.id = oi.order_id JOIN workshops w ON w.id = oi.workshop_id \
         WHERE {SALES_WHERE} AND DATE_FORMAT(o.paid_at, '%Y-%m') = ? GROUP BY w.id, w.name, w.business_number ORDER BY w.name"
    ))
    .bind(&period)
    .fetch_all(&s.db)
    .await?;
    let paid: Vec<(String, u64, u64, u64, NaiveDateTime, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT workshop_id, gross, fee, net, paid_at, paid_by, memo FROM settlements WHERE period = ?")
            .bind(&period)
            .fetch_all(&s.db)
            .await?;
    let mut totals = (0i64, 0i64);
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(wid, name, biz, gross, fee, orders)| {
            totals.0 += gross;
            totals.1 += fee;
            let rec = paid.iter().find(|r| r.0 == wid);
            serde_json::json!({
                "workshop_id": wid, "workshop_name": name, "business_number": biz, "gross": gross, "fee": fee, "net": gross - fee, "orders": orders,
                "paid": rec.map(|r| serde_json::json!({"gross": r.1, "fee": r.2, "net": r.3, "paid_at": r.4, "paid_by": r.5, "memo": r.6})),
                "changed_after_paid": rec.is_some_and(|r| r.1 as i64 != gross || r.2 as i64 != fee),
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "period": period, "closed": period < current_period(), "items": items,
        "totals": {"gross": totals.0, "fee": totals.1, "net": totals.0 - totals.1},
    })))
}

#[derive(Debug, Deserialize)]
pub struct MarkInput {
    pub workshop_id: String,
    pub period: String,
    pub memo: Option<String>,
}

/// 지급 완료 기록 (끝난 달만). 그때 금액을 그대로 남깁니다.
async fn mark_paid(State(s): State<AppState>, admin: Admin, Json(b): Json<MarkInput>) -> AppResult<StatusCode> {
    let period = check_period(&b.period)?;
    if period >= current_period() {
        return Err(AppError::bad("이번 달 정산은 달이 끝난 뒤에 지급 처리할 수 있습니다"));
    }
    let memo = opt_text(b.memo.as_deref(), "메모", 300)?;
    let (gross, fee): (i64, i64) = sqlx::query_as(&format!(
        "SELECT CAST(COALESCE(SUM(oi.unit_price * oi.quantity), 0) AS SIGNED), CAST(COALESCE(SUM(oi.fee_amount), 0) AS SIGNED) \
         FROM order_items oi JOIN orders o ON o.id = oi.order_id WHERE oi.workshop_id = ? AND {SALES_WHERE} AND DATE_FORMAT(o.paid_at, '%Y-%m') = ?"
    ))
    .bind(&b.workshop_id)
    .bind(&period)
    .fetch_one(&s.db)
    .await?;
    if gross <= 0 {
        return Err(AppError::bad("이 달에 정산할 매출이 없습니다"));
    }
    let r = sqlx::query("INSERT INTO settlements (id, workshop_id, period, gross, fee, net, memo, paid_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(new_id())
        .bind(&b.workshop_id)
        .bind(&period)
        .bind(gross as u64)
        .bind(fee as u64)
        .bind((gross - fee).max(0) as u64)
        .bind(&memo)
        .bind(&admin.actor)
        .execute(&s.db)
        .await;
    if let Err(e) = r {
        if gageyo_core::error::is_duplicate(&e) {
            return Err(AppError::Conflict("이미 지급 완료로 기록된 정산입니다".into()));
        }
        return Err(e.into());
    }
    audit::log(&s, &admin, "settlement.paid", "workshop", &b.workshop_id, Some(format!("{period} {}원", gross - fee))).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct UndoQuery {
    pub workshop_id: String,
    pub period: String,
}

/// 잘못 누른 지급 완료 기록 지우기
async fn undo_paid(State(s): State<AppState>, admin: Admin, Query(q): Query<UndoQuery>) -> AppResult<StatusCode> {
    let period = check_period(&q.period)?;
    let r = sqlx::query("DELETE FROM settlements WHERE workshop_id = ? AND period = ?").bind(&q.workshop_id).bind(&period).execute(&s.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::not_found("지급 기록이 없습니다"));
    }
    audit::log(&s, &admin, "settlement.undo", "workshop", &q.workshop_id, Some(period)).await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::check_period;

    #[test]
    fn periods() {
        assert!(check_period("2026-10").is_ok());
        assert!(check_period("2026-13").is_err());
        assert!(check_period("2026-1").is_err());
        assert!(check_period("2026/10").is_err());
        assert!(check_period("abcd-10").is_err());
        assert!("2026-09".to_string() < "2026-10".to_string());
    }
}
