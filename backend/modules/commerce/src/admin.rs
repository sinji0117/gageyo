//! 운영자 주문 관리: 목록·상세, 무통장 입금 확인, 카드 결제 확인(토스 조회), 취소·환불, (결제 수단 없는 주문의) 상태 직접 변경.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::Admin,
    util::{like_term, opt_text, Page},
    AppError, AppResult, AppState,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/orders", get(list_orders))
        .route("/admin/orders/{id}", get(order_detail).patch(set_status))
        .route("/admin/orders/{id}/confirm-deposit", post(confirm_deposit))
        .route("/admin/orders/{id}/cancel", post(cancel_order))
        .route("/admin/orders/{id}/toss-sync", post(toss_sync))
}

#[derive(Debug, Deserialize)]
pub struct OrderQuery {
    pub status: Option<String>,
    pub method: Option<String>,
    pub q: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn list_orders(State(s): State<AppState>, _a: Admin, Query(q): Query<OrderQuery>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 500);
    let term = like_term(q.q.as_deref());
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, String, String, String, String, u64, String, String, Option<String>, NaiveDateTime, Option<NaiveDateTime>, i64, String)> =
        sqlx::query_as(
            "SELECT o.id, o.customer_name, o.customer_phone, o.address, o.status, o.total_amount, o.payment_method, o.fulfillment, \
               o.depositor_name, o.created_at, o.paid_at, (SELECT COUNT(*) FROM order_items oi WHERE oi.order_id = o.id), \
               COALESCE((SELECT GROUP_CONCAT(DISTINCT w.name SEPARATOR ', ') FROM order_items oi JOIN workshops w ON w.id = oi.workshop_id WHERE oi.order_id = o.id), '') \
             FROM orders o WHERE (? IS NULL OR o.status = ?) AND (? IS NULL OR o.payment_method = ?) \
               AND (? IS NULL OR o.id LIKE CONCAT(?, '%') OR o.customer_name LIKE CONCAT('%', ?, '%') OR o.customer_phone LIKE CONCAT('%', ?, '%') \
                    OR o.depositor_name LIKE CONCAT('%', ?, '%')) \
             ORDER BY o.created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(&q.status)
        .bind(&q.status)
        .bind(&q.method)
        .bind(&q.method)
        .bind(&term)
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
            .map(|(id, name, phone, address, status, total, method, fulfillment, depositor, created, paid, items, workshops)| {
                serde_json::json!({"id": id, "customer_name": name, "customer_phone": phone, "address": address, "status": status,
                    "status_label": crate::order_status_label(&status), "total_amount": total, "payment_method": method,
                    "fulfillment": fulfillment, "depositor_name": depositor, "created_at": created, "paid_at": paid,
                    "item_count": items, "workshops": workshops})
            })
            .collect(),
    ))
}

async fn order_detail(State(s): State<AppState>, _a: Admin, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let o = crate::orders::load_order(&s, &id).await?;
    let mut v = crate::orders::order_json(&s, &o).await?;
    v["cancellable"] = serde_json::json!(!["cancelled", "refunded", "delivered"].contains(&o.status.as_str()));
    Ok(Json(v))
}

/// 무통장 입금 확인 → 결제 완료
async fn confirm_deposit(State(s): State<AppState>, admin: Admin, Path(id): Path<String>) -> AppResult<StatusCode> {
    let o = crate::orders::load_order(&s, &id).await?;
    if o.payment_method != "bank" {
        return Err(AppError::bad("무통장입금 주문이 아닙니다"));
    }
    crate::payments::mark_paid(&s, &id, None, "무통장입금").await?;
    audit::log(&s, &admin, "order.deposit_confirmed", "order", &id, Some(format!("{}원 {}", o.total_amount, o.depositor_name.unwrap_or_default()))).await;
    crate::notify::order_paid(&s, &id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// 카드 결제 확인: 결제 대기 중인 카드 주문을 토스에서 주문번호로 찾아, 승인돼 있으면 결제 완료로 맞춥니다.
async fn toss_sync(State(s): State<AppState>, admin: Admin, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let o = crate::orders::load_order(&s, &id).await?;
    if o.payment_method != "toss" || o.status != "payment_pending" {
        return Err(AppError::bad("결제 대기 중인 카드 결제 주문이 아닙니다"));
    }
    if s.cfg.toss_secret_key.is_none() {
        return Err(AppError::Unavailable("카드 결제 설정(TOSS_SECRET_KEY)이 없습니다".into()));
    }
    let paid = crate::payments::sync_toss(&s, &o).await?;
    if paid {
        audit::log(&s, &admin, "order.toss_synced", "order", &id, Some(format!("{}원", o.total_amount))).await;
    }
    Ok(Json(serde_json::json!({ "paid": paid })))
}

#[derive(Debug, Deserialize)]
pub struct CancelInput {
    pub reason: Option<String>,
}

/// 운영자 취소: 발송된 상품이 있으면 막습니다. 카드 결제는 토스에서 자동 환불합니다.
async fn cancel_order(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<CancelInput>) -> AppResult<Json<serde_json::Value>> {
    let reason = opt_text(b.reason.as_deref(), "취소 사유", 300)?.ok_or_else(|| AppError::bad("취소 사유를 적어 주세요 (고객에게 안내됩니다)"))?;
    let o = crate::orders::load_order(&s, &id).await?;
    if ["cancelled", "refunded"].contains(&o.status.as_str()) {
        return Err(AppError::Conflict("이미 취소된 주문입니다".into()));
    }
    let shipped: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM order_items WHERE order_id = ? AND status IN ('shipped','delivered')")
        .bind(&id)
        .fetch_one(&s.db)
        .await?;
    if shipped > 0 {
        return Err(AppError::Conflict("이미 발송된 상품이 있어 주문 전체를 취소할 수 없습니다. 반품은 업체와 협의해 처리해 주세요".into()));
    }
    // 결제 대기로 보이는 카드 주문도 토스에는 승인됐을 수 있으므로 먼저 확인 (그렇다면 환불까지 진행)
    let o = if crate::payments::sync_toss(&s, &o).await? { crate::orders::load_order(&s, &id).await? } else { o };
    let final_status = crate::payments::refund(&s, &o, &reason).await?;
    crate::orders::cancel_in_db(&s, &id, final_status, &reason).await?;
    audit::log(&s, &admin, &format!("order.{final_status}"), "order", &id, Some(reason.clone())).await;
    crate::notify::order_cancelled(&s, &id, final_status == "refunded", Some(&reason)).await;
    let manual_refund = final_status == "refunded" && o.payment_method == "bank";
    Ok(Json(serde_json::json!({ "status": final_status, "manual_refund_needed": manual_refund })))
}

#[derive(Debug, Deserialize)]
pub struct StatusInput {
    pub status: String,
}

/// 상태 직접 변경. 취소·환불은 위의 취소 기능으로만 합니다(재고·결제 처리 때문).
/// 결제 수단 없이 접수한 주문은 여기서 '결제 완료'로 바꿀 수 있습니다.
async fn set_status(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<StatusInput>) -> AppResult<StatusCode> {
    let o = crate::orders::load_order(&s, &id).await?;
    match b.status.as_str() {
        "cancelled" | "refunded" => return Err(AppError::bad("취소·환불은 '주문 취소' 버튼으로 해 주세요")),
        "paid" if o.status == "payment_pending" => {
            if o.payment_method == "toss" {
                return Err(AppError::bad("카드 결제 주문은 결제창 승인으로만 결제 완료가 됩니다"));
            }
            crate::payments::mark_paid(&s, &id, None, if o.payment_method == "bank" { "무통장입금" } else { "직접 결제" }).await?;
        }
        "paid" | "preparing" | "shipped" | "delivered" => {
            if ["payment_pending", "cancelled", "refunded"].contains(&o.status.as_str()) {
                return Err(AppError::bad("결제 완료 전이거나 취소된 주문입니다"));
            }
            sqlx::query("UPDATE orders SET status = ? WHERE id = ?").bind(&b.status).bind(&id).execute(&s.db).await?;
        }
        _ => return Err(AppError::bad("주문 상태 값이 올바르지 않습니다")),
    }
    audit::log(&s, &admin, &format!("order.status.{}", b.status), "order", &id, Some(format!("{} → {}", o.status, b.status))).await;
    Ok(StatusCode::NO_CONTENT)
}
