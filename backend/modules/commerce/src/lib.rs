//! 상품·주문·결제·배송·정산 도메인.
//!
//! 주문 흐름 (bandiuri 쇼핑 흐름을 여러 업체 장터에 맞게 바꿈):
//!
//! ```text
//! 결제 대기(payment_pending) ─카드 승인·입금 확인→ 결제 완료(paid) → 준비 중(preparing) → 발송(shipped) → 배송 완료(delivered)
//!        │                                         │
//!        └─ 고객 취소(cancelled, 재고 복원)          └─ 운영자 취소(카드는 자동 환불 → refunded, 재고 복원)
//! ```
//!
//! - 주문 하나에 여러 업체 상품이 섞일 수 있어 **배송 상태는 상품 줄(order_items)마다** 업체가 처리하고,
//!   주문 전체 상태는 줄 상태로 다시 계산합니다.
//! - 금액은 항상 서버가 상품 가격으로 다시 계산합니다. 브라우저가 보낸 금액은 쓰지 않습니다.
//! - 수수료는 결제 완료 시점의 플랫폼 수수료율로 줄마다 고정합니다 (정산에 씀).

use axum::Router;
use gageyo_core::AppState;

pub mod admin;
pub mod fulfillment;
pub mod notify;
pub mod orders;
pub mod payments;
pub mod products;
pub mod settlement;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(products::routes())
        .merge(orders::routes())
        .merge(payments::routes())
        .merge(fulfillment::routes())
        .merge(settlement::routes())
        .merge(admin::routes())
}

/// 주문 줄 상태들로 주문 전체 상태 계산 (결제 전·취소·환불 주문은 그대로)
pub fn derive_order_status(current: &str, item_statuses: &[&str]) -> String {
    if ["payment_pending", "cancelled", "refunded"].contains(&current) {
        return current.to_string();
    }
    let active: Vec<&str> = item_statuses.iter().copied().filter(|s| *s != "cancelled").collect();
    if active.is_empty() {
        return current.to_string();
    }
    if active.iter().all(|s| *s == "delivered") {
        "delivered".into()
    } else if active.iter().all(|s| *s == "shipped" || *s == "delivered") {
        "shipped".into()
    } else if active.iter().any(|s| ["preparing", "shipped", "delivered"].contains(s)) {
        "preparing".into()
    } else {
        "paid".into()
    }
}

pub fn order_status_label(s: &str) -> &'static str {
    match s {
        "payment_pending" => "결제 대기",
        "paid" => "결제 완료",
        "preparing" => "준비 중",
        "shipped" => "발송",
        "delivered" => "배송 완료",
        "cancelled" => "취소",
        "refunded" => "환불",
        _ => "알 수 없음",
    }
}

#[cfg(test)]
mod tests {
    use super::derive_order_status as d;

    #[test]
    fn order_status_from_items() {
        assert_eq!(d("payment_pending", &["pending"]), "payment_pending");
        assert_eq!(d("cancelled", &["cancelled"]), "cancelled");
        assert_eq!(d("paid", &["ordered", "ordered"]), "paid");
        assert_eq!(d("paid", &["ordered", "preparing"]), "preparing");
        assert_eq!(d("preparing", &["shipped", "preparing"]), "preparing");
        assert_eq!(d("preparing", &["shipped", "delivered"]), "shipped");
        assert_eq!(d("shipped", &["delivered", "delivered", "cancelled"]), "delivered");
        assert_eq!(d("paid", &["cancelled"]), "paid");
    }
}
