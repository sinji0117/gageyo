//! 파트너 주문 처리: 우리 업체 상품 줄만 보고, 준비 → 발송(택배사·운송장) → 배송 완료로 바꿉니다.
//!
//! 예전에는 파트너 주문 목록에 주문 전체 금액(다른 업체 상품 포함)이 보였고, 파트너가 배송 상태를 바꿀 수 없었습니다.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    auth::ApprovedPartner,
    util::{carrier, clean_tracking_no, tracking_url, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/partner/orders", get(partner_orders))
        .route("/partner/order-items/{id}", patch(update_item))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PartnerOrderItem {
    pub item_id: String,
    pub order_id: String,
    pub product_name: String,
    pub unit_price: u64,
    pub quantity: u32,
    pub fee_amount: u64,
    pub item_status: String,
    pub carrier: Option<String>,
    pub tracking_no: Option<String>,
    pub order_status: String,
    pub payment_method: String,
    pub customer_name: String,
    pub customer_phone: String,
    pub address: String,
    pub fulfillment: String,
    pub visit_at: Option<NaiveDateTime>,
    pub vehicle: Option<String>,
    pub memo: Option<String>,
    pub paid_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct OrdersQuery {
    /// todo(처리할 것) · shipped · done · cancelled · all
    pub view: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

/// 결제가 끝난 주문(또는 결제 수단이 없는 서버의 접수 주문)만 보입니다.
async fn partner_orders(State(s): State<AppState>, ApprovedPartner(p): ApprovedPartner, Query(q): Query<OrdersQuery>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(100, 300);
    let filter = match q.view.as_deref().unwrap_or("all") {
        "todo" => "AND oi.status IN ('pending','ordered','preparing')",
        "shipped" => "AND oi.status = 'shipped'",
        "done" => "AND oi.status = 'delivered'",
        "cancelled" => "AND oi.status = 'cancelled'",
        _ => "",
    };
    let rows = sqlx::query_as::<_, PartnerOrderItem>(&format!(
        "SELECT oi.id AS item_id, o.id AS order_id, oi.product_name, oi.unit_price, oi.quantity, oi.fee_amount, oi.status AS item_status, \
         oi.carrier, oi.tracking_no, o.status AS order_status, o.payment_method, o.customer_name, o.customer_phone, o.address, o.fulfillment, \
         o.visit_at, o.vehicle, o.memo, o.paid_at, o.created_at \
         FROM order_items oi JOIN orders o ON o.id = oi.order_id \
         WHERE oi.workshop_id = ? AND (o.status <> 'payment_pending' OR o.payment_method = 'none') {filter} \
         ORDER BY o.created_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(&p.workshop_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| {
                let mut v = serde_json::to_value(&r).unwrap_or_default();
                let code = r.carrier.as_deref().unwrap_or("");
                v["amount"] = serde_json::json!(r.unit_price * u64::from(r.quantity));
                v["carrier_name"] = serde_json::json!(carrier(code).map(|c| c.1).unwrap_or(""));
                v["tracking_url"] = serde_json::json!(tracking_url(code, r.tracking_no.as_deref().unwrap_or("")));
                v
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ItemStatusInput {
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_no: Option<String>,
}

/// 줄 상태 흐름. 방문 장착 주문은 발송 없이 바로 완료할 수 있습니다.
pub fn item_transition_ok(from: &str, to: &str, visit: bool) -> bool {
    match (from, to) {
        ("pending" | "ordered", "preparing") => true,
        ("pending" | "ordered" | "preparing", "shipped") => !visit,
        ("shipped", "delivered") => true,
        ("pending" | "ordered" | "preparing", "delivered") => visit,
        _ => false,
    }
}

async fn update_item(
    State(s): State<AppState>,
    ApprovedPartner(p): ApprovedPartner,
    Path(id): Path<String>,
    Json(b): Json<ItemStatusInput>,
) -> AppResult<StatusCode> {
    let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT oi.order_id, oi.status, oi.product_name, o.status, o.payment_method, o.fulfillment FROM order_items oi \
         JOIN orders o ON o.id = oi.order_id WHERE oi.id = ? AND oi.workshop_id = ?",
    )
    .bind(&id)
    .bind(&p.workshop_id)
    .fetch_optional(&s.db)
    .await?;
    let (order_id, current, product_name, order_status, method, fulfillment) = row.ok_or_else(|| AppError::not_found("주문 상품을 찾을 수 없습니다"))?;
    if ["cancelled", "refunded"].contains(&order_status.as_str()) {
        return Err(AppError::Conflict("취소된 주문입니다".into()));
    }
    if order_status == "payment_pending" && method != "none" {
        return Err(AppError::Conflict("결제가 끝나지 않은 주문입니다".into()));
    }
    if !item_transition_ok(&current, &b.status, fulfillment == "visit") {
        return Err(AppError::bad("지금 상태에서 바꿀 수 없는 단계입니다"));
    }
    let mut carrier_code: Option<String> = None;
    let mut tracking: Option<String> = None;
    if b.status == "shipped" {
        let code = b.carrier.as_deref().map(str::trim).unwrap_or("");
        if carrier(code).is_none() {
            return Err(AppError::bad("택배사를 골라 주세요"));
        }
        let no = clean_tracking_no(b.tracking_no.as_deref().unwrap_or("")).ok_or_else(|| AppError::bad("운송장 번호는 숫자·영문·- 만 입력해 주세요"))?;
        if code != "direct" && no.is_empty() {
            return Err(AppError::bad("운송장 번호를 입력해 주세요"));
        }
        carrier_code = Some(code.to_string());
        tracking = Some(no).filter(|n| !n.is_empty());
    }
    let mut tx = s.db.begin().await?;
    sqlx::query(
        "UPDATE order_items SET status = ?, carrier = COALESCE(?, carrier), tracking_no = COALESCE(?, tracking_no), \
         shipped_at = IF(? = 'shipped', NOW(), shipped_at), delivered_at = IF(? = 'delivered', NOW(), delivered_at) \
         WHERE id = ? AND workshop_id = ? AND status = ?",
    )
    .bind(&b.status)
    .bind(&carrier_code)
    .bind(&tracking)
    .bind(&b.status)
    .bind(&b.status)
    .bind(&id)
    .bind(&p.workshop_id)
    .bind(&current)
    .execute(&mut *tx)
    .await?;
    let statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM order_items WHERE order_id = ?").bind(&order_id).fetch_all(&mut *tx).await?;
    let refs: Vec<&str> = statuses.iter().map(String::as_str).collect();
    // 결제 수단 없는 접수 주문을 업체가 처리하기 시작하면 결제 완료로 봅니다 (업체와 직접 결제)
    let base = if order_status == "payment_pending" { "paid" } else { order_status.as_str() };
    let next = crate::derive_order_status(base, &refs);
    sqlx::query("UPDATE orders SET status = ? WHERE id = ?").bind(&next).bind(&order_id).execute(&mut *tx).await?;
    if b.status == "delivered" && fulfillment == "visit" {
        sqlx::query("UPDATE bookings SET status = 'completed' WHERE order_id = ? AND status IN ('requested','confirmed')")
            .bind(&order_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    if b.status == "shipped" {
        let code = carrier_code.as_deref().unwrap_or("");
        let url = tracking_url(code, tracking.as_deref().unwrap_or(""));
        crate::notify::item_shipped(&s, &order_id, &product_name, carrier(code).map(|c| c.1).unwrap_or(""), url.as_deref()).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::item_transition_ok as ok;

    #[test]
    fn item_transitions() {
        assert!(ok("ordered", "preparing", false));
        assert!(ok("ordered", "shipped", false));
        assert!(ok("preparing", "shipped", false));
        assert!(ok("shipped", "delivered", false));
        assert!(!ok("ordered", "delivered", false), "배송 주문은 발송을 거쳐야 함");
        assert!(ok("ordered", "delivered", true), "방문 장착은 바로 완료");
        assert!(!ok("preparing", "shipped", true));
        assert!(!ok("delivered", "shipped", false));
        assert!(!ok("cancelled", "preparing", false));
    }
}
