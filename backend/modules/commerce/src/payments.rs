//! 결제 승인: 토스페이먼츠 카드·간편결제, 무통장입금 확인(운영자), 결제 취소(환불).

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use gageyo_core::{auth::MaybeCustomer, settings, toss::TossError, AppError, AppResult, AppState};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route("/orders/{id}/payments/toss", post(confirm_toss))
}

/// 결제 대기 중인 카드 주문을 토스에서 찾아, 이미 승인된 결제면 결제 완료로 맞춥니다.
/// (승인 요청이 통신 오류로 끊긴 경우, 고객이 결제창에서 돌아오지 못한 경우)
///
/// 돌려주는 값: 결제 완료로 바꿨으면 `true`. 토스 설정이 없거나 카드 결제 대기 주문이 아니면 `false`.
/// 토스와 통신하지 못하면 오류 — 결과를 모르는 채로 주문을 취소하면 안 되기 때문입니다.
pub async fn sync_toss(s: &AppState, o: &crate::orders::OrderRow) -> AppResult<bool> {
    if o.payment_method != "toss" || o.status != "payment_pending" {
        return Ok(false);
    }
    let Some(secret) = s.cfg.toss_secret_key.clone() else {
        return Ok(false);
    };
    let found = match s.toss.find_by_order(&secret, &o.id).await {
        Ok(f) => f,
        Err(TossError::Rejected(m)) | Err(TossError::Transport(m)) => {
            tracing::warn!(order_id = %o.id, error = %m, "toss lookup failed");
            return Err(AppError::Unavailable("결제사에 결제 상태를 확인하지 못했습니다. 잠시 후 다시 시도해 주세요".into()));
        }
    };
    let Some((key, c)) = found else { return Ok(false) };
    if c.approved_amount != o.total_amount {
        tracing::error!(order_id = %o.id, approved = c.approved_amount, "toss lookup amount mismatch — 토스 상점관리자에서 확인 필요");
        return Err(AppError::Conflict("토스에 승인된 금액이 주문 금액과 다릅니다. 토스 상점관리자에서 확인해 주세요".into()));
    }
    let detail = match &c.easy_pay_provider {
        Some(p) => format!("{} ({p})", c.method),
        None => c.method.clone(),
    };
    match mark_paid(s, &o.id, Some(&key), &detail).await {
        Ok(()) => {
            tracing::info!(order_id = %o.id, "toss payment found and marked paid");
            crate::notify::order_paid(s, &o.id).await;
            Ok(true)
        }
        Err(AppError::Conflict(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct TossConfirmInput {
    pub payment_key: String,
    pub amount: u64,
    /// 비회원 주문 확인용
    pub phone: Option<String>,
}

/// 결제창에서 돌아온 뒤 화면이 부릅니다. 서버가 주문 금액과 대조하고 토스에 승인을 요청합니다.
async fn confirm_toss(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Json(b): Json<TossConfirmInput>,
) -> AppResult<Json<serde_json::Value>> {
    let o = crate::orders::load_order(&s, &id).await?;
    // 결제창에서 돌아온 주문번호는 토스가 보증하므로, 회원·연락처 확인이 안 되더라도 금액이 맞으면 승인합니다.
    // 다만 응답에는 상세 정보를 회원·연락처가 맞을 때만 담습니다.
    let can_see = crate::orders::can_access(&o, user.as_ref(), b.phone.as_deref());
    if o.payment_method != "toss" {
        return Err(AppError::bad("카드 결제 주문이 아닙니다"));
    }
    if o.status != "payment_pending" {
        let same_key: Option<String> = sqlx::query_scalar("SELECT payment_key FROM orders WHERE id = ?").bind(&id).fetch_one(&s.db).await?;
        if same_key.as_deref() == Some(b.payment_key.as_str()) {
            // 같은 결제를 두 번 확인 (새로고침 등)
            return Ok(Json(serde_json::json!({ "id": id, "status": o.status, "already": true })));
        }
        return Err(AppError::Conflict("이미 처리된 주문입니다".into()));
    }
    if b.amount != o.total_amount {
        tracing::warn!(order_id = %id, expected = o.total_amount, got = b.amount, "toss amount mismatch");
        return Err(AppError::bad("결제 금액이 주문 금액과 다릅니다"));
    }
    let secret = s.cfg.toss_secret_key.clone().ok_or_else(|| AppError::Unavailable("카드 결제가 설정되지 않았습니다".into()))?;
    let confirmed = match s.toss.confirm(&secret, &b.payment_key, &id, o.total_amount).await {
        Ok(c) => c,
        Err(TossError::Rejected(m)) => return Err(AppError::PaymentRejected(m)),
        Err(TossError::Transport(m)) => {
            tracing::error!(order_id = %id, error = %m, "toss confirm transport error — 토스 상점관리자에서 결제 상태 확인 필요");
            return Err(AppError::Unavailable(
                "결제 결과를 확인하지 못했습니다. 카드사 승인 문자가 왔다면 고객센터로 주문번호를 알려 주세요".into(),
            ));
        }
    };
    if confirmed.approved_amount != o.total_amount {
        tracing::error!(order_id = %id, approved = confirmed.approved_amount, "toss approved amount mismatch");
        return Err(AppError::Internal("approved amount mismatch".into()));
    }
    let detail = match &confirmed.easy_pay_provider {
        Some(p) => format!("{} ({p})", confirmed.method),
        None => confirmed.method.clone(),
    };
    mark_paid(&s, &id, Some(&b.payment_key), &detail).await?;
    crate::notify::order_paid(&s, &id).await;
    let mut res = serde_json::json!({ "id": id, "status": "paid" });
    if can_see {
        let o = crate::orders::load_order(&s, &id).await?;
        res = crate::orders::order_json(&s, &o).await?;
    }
    Ok(Json(res))
}

/// 결제 완료 처리: 주문 `paid`, 줄 `ordered`, 줄마다 수수료 고정
pub async fn mark_paid(s: &AppState, order_id: &str, payment_key: Option<&str>, detail: &str) -> AppResult<()> {
    let rate = settings::fee_rate_bp(s).await?;
    let mut tx = s.db.begin().await?;
    let r = sqlx::query(
        "UPDATE orders SET status = 'paid', paid_at = NOW(), payment_key = COALESCE(?, payment_key), pay_detail = ? \
         WHERE id = ? AND status = 'payment_pending'",
    )
    .bind(payment_key)
    .bind(detail)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict("결제 대기 중인 주문이 아닙니다".into()));
    }
    sqlx::query(
        "UPDATE order_items SET status = 'ordered', fee_amount = FLOOR(unit_price * quantity * ? / 10000) WHERE order_id = ? AND status = 'pending'",
    )
    .bind(rate)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 결제 취소(환불). 카드 결제는 토스에 취소를 요청하고, 무통장은 운영자가 직접 돌려줍니다.
/// 돌려주는 값: 최종 주문 상태 (`refunded` 또는 `cancelled`)
pub async fn refund(s: &AppState, o: &crate::orders::OrderRow, reason: &str) -> AppResult<&'static str> {
    if o.status == "payment_pending" {
        return Ok("cancelled");
    }
    match o.payment_method.as_str() {
        "toss" => {
            let key: Option<String> = sqlx::query_scalar("SELECT payment_key FROM orders WHERE id = ?").bind(&o.id).fetch_one(&s.db).await?;
            let key = key.ok_or_else(|| AppError::Conflict("결제 키가 없어 자동 환불할 수 없습니다. 토스 상점관리자에서 취소해 주세요".into()))?;
            let secret = s.cfg.toss_secret_key.clone().ok_or_else(|| AppError::Unavailable("카드 결제 설정(TOSS_SECRET_KEY)이 없습니다".into()))?;
            match s.toss.cancel(&secret, &key, reason, &format!("gy-cancel-{}", o.id)).await {
                Ok(_) => Ok("refunded"),
                Err(TossError::Rejected(m)) => Err(AppError::PaymentRejected(format!("환불 실패: {m}"))),
                Err(TossError::Transport(m)) => {
                    tracing::error!(order_id = %o.id, error = %m, "toss cancel transport error");
                    Err(AppError::Unavailable("결제사와 통신하지 못했습니다. 잠시 후 다시 시도해 주세요".into()))
                }
            }
        }
        // 무통장: 운영자가 계좌로 돌려줌 (화면에 안내)
        "bank" => Ok("refunded"),
        _ => Ok("cancelled"),
    }
}
