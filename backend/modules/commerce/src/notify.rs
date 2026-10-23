//! 주문 메일 (고객·업체). 메일 설정이 없으면 로그에만 남습니다.

use gageyo_core::{mail::Mail, settings::BankAccount, AppState};

fn won(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 주문 받는 사람 이메일 (주문서에 적은 이메일 > 회원 이메일)
async fn order_email(s: &AppState, order_id: &str) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT COALESCE(o.email, u.email) FROM orders o LEFT JOIN users u ON u.id = o.customer_user_id WHERE o.id = ?",
    )
    .bind(order_id)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten()
    .flatten()
}

fn short(id: &str) -> String {
    id.chars().take(8).collect::<String>().to_uppercase()
}

pub async fn order_received(s: &AppState, order_id: &str, total: u64, bank: Option<&BankAccount>, depositor: Option<&str>) {
    let Some(to) = order_email(s, order_id).await else { return };
    let pay = match bank {
        Some(b) => format!(
            "\n아래 계좌로 {}원을 입금해 주세요 ({}시간 안에).\n{} {} (예금주 {})\n입금자명: {}\n입금이 확인되면 결제 완료로 바뀝니다.\n",
            won(total),
            b.due_hours,
            b.bank_name,
            b.account,
            b.holder,
            depositor.unwrap_or("주문자 이름")
        ),
        None => String::new(),
    };
    s.mailer.send(Mail::new(
        to,
        format!("[가게요] 주문이 접수되었습니다 (#{})", short(order_id)),
        format!(
            "주문번호: {order_id}\n결제 금액: {}원\n{pay}\n주문 조회: {}/?order={order_id}",
            won(total),
            s.cfg.app_url
        ),
    ));
}

pub async fn order_paid(s: &AppState, order_id: &str) {
    let total: Option<u64> = sqlx::query_scalar("SELECT total_amount FROM orders WHERE id = ?").bind(order_id).fetch_optional(&s.db).await.ok().flatten();
    if let Some(to) = order_email(s, order_id).await {
        s.mailer.send(Mail::new(
            to,
            format!("[가게요] 결제가 완료되었습니다 (#{})", short(order_id)),
            format!(
                "결제 금액 {}원이 확인되었습니다. 업체가 상품을 준비해 보내 드립니다.\n\n주문 조회: {}/?order={order_id}",
                won(total.unwrap_or(0)),
                s.cfg.app_url
            ),
        ));
    }
    partners_new_order(s, order_id).await;
}

/// 주문에 상품이 들어간 업체들에게 새 주문 알림
pub async fn partners_new_order(s: &AppState, order_id: &str) {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT u.email, w.name, COUNT(*) FROM order_items oi JOIN workshops w ON w.id = oi.workshop_id \
         JOIN users u ON u.id = w.owner_user_id WHERE oi.order_id = ? AND u.status = 'active' GROUP BY u.email, w.name",
    )
    .bind(order_id)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();
    for (email, name, n) in rows {
        s.mailer.send(Mail::new(
            email,
            format!("[가게요] 새 주문 {n}건 · {name}"),
            format!("새 주문이 들어왔습니다 (#{}).\n파트너 센터 주문 관리에서 상품을 준비하고 발송 처리해 주세요.\n{}/?tab=orders", short(order_id), s.cfg.partner_url),
        ));
    }
}

pub async fn item_shipped(s: &AppState, order_id: &str, product: &str, carrier_name: &str, tracking_url: Option<&str>) {
    let Some(to) = order_email(s, order_id).await else { return };
    s.mailer.send(Mail::new(
        to,
        format!("[가게요] 상품이 발송되었습니다 (#{})", short(order_id)),
        format!(
            "{product} 상품이 {carrier_name}(으)로 발송되었습니다.\n{}\n주문 조회: {}/?order={order_id}",
            tracking_url.map(|u| format!("배송 조회: {u}")).unwrap_or_default(),
            s.cfg.app_url
        ),
    ));
}

pub async fn order_cancelled(s: &AppState, order_id: &str, refunded: bool, reason: Option<&str>) {
    if let Some(to) = order_email(s, order_id).await {
        let what = if refunded { "취소되고 결제가 환불 처리되었습니다" } else { "취소되었습니다" };
        s.mailer.send(Mail::new(
            to,
            format!("[가게요] 주문이 {what} (#{})", short(order_id)),
            format!(
                "주문(#{})이 {what}.{}\n카드 결제는 카드사 사정에 따라 3~5영업일 안에 취소됩니다.",
                short(order_id),
                reason.map(|r| format!("\n사유: {r}")).unwrap_or_default()
            ),
        ));
    }
    let partners: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT u.email FROM order_items oi JOIN workshops w ON w.id = oi.workshop_id JOIN users u ON u.id = w.owner_user_id \
         WHERE oi.order_id = ? AND u.status = 'active'",
    )
    .bind(order_id)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();
    for e in partners {
        s.mailer.send(Mail::new(e, format!("[가게요] 주문 취소 (#{})", short(order_id)), "주문이 취소되었습니다. 준비 중이던 상품은 보내지 마세요.".to_string()));
    }
}

/// 결제 기한이 지나 자동 취소 (결제 전이라 업체에는 알리지 않음)
pub async fn order_expired(s: &AppState, order_id: &str, reason: &str) {
    if let Some(to) = order_email(s, order_id).await {
        s.mailer.send(Mail::new(
            to,
            format!("[가게요] 주문이 취소되었습니다 (#{})", short(order_id)),
            format!("주문(#{})이 {reason}되었습니다. 필요하면 다시 주문해 주세요.\n{}/?request=cart", short(order_id), s.cfg.app_url),
        ));
    }
}
