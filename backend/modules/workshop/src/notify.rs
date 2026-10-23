//! 메일 알림 (견적·예약·리뷰·입점 심사). 보내기는 백그라운드이고 실패해도 요청은 성공합니다.
//! 메일 설정(`SMTP_URL`)이 없으면 로그에만 남습니다.

use gageyo_core::{mail::Mail, AppState};

/// 업체 주인(파트너 계정) 이메일
pub async fn owner_email(s: &AppState, workshop_id: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT u.email FROM workshops w JOIN users u ON u.id = w.owner_user_id WHERE w.id = ? AND u.status = 'active'",
    )
    .bind(workshop_id)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten()
}

async fn customer_email(s: &AppState, user_id: Option<&str>) -> Option<String> {
    let id = user_id?;
    sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = ? AND status = 'active'")
        .bind(id)
        .fetch_optional(&s.db)
        .await
        .ok()
        .flatten()
}

/// 새 견적 요청 → 분야·지역이 맞는 승인 업체들 (최대 30곳)
pub async fn quote_created(s: &AppState, category: &str, district: &str, vehicle: &str) {
    let emails: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT u.email FROM workshops w JOIN users u ON u.id = w.owner_user_id \
         WHERE w.status = 'approved' AND u.status = 'active' AND ? LIKE CONCAT('%', w.district, '%') \
           AND (? = '기타' OR EXISTS (SELECT 1 FROM workshop_categories c WHERE c.workshop_id = w.id AND c.category = ?)) \
         LIMIT 30",
    )
    .bind(district)
    .bind(category)
    .bind(category)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();
    let link = format!("{}/?tab=quotes", s.cfg.partner_url);
    for e in emails {
        s.mailer.send(Mail::new(
            e,
            format!("[가게요] 새 견적 요청 · {category}"),
            format!("{district}에서 새 견적 요청이 들어왔습니다.\n\n차량: {vehicle}\n분야: {category}\n\n파트너 센터에서 견적을 보내 주세요.\n{link}"),
        ));
    }
}

pub async fn offer_received(s: &AppState, quote_id: &str, workshop_name: &str, amount: u64) {
    let user: Option<String> = sqlx::query_scalar("SELECT customer_user_id FROM quote_requests WHERE id = ?")
        .bind(quote_id)
        .fetch_optional(&s.db)
        .await
        .ok()
        .flatten()
        .flatten();
    if let Some(to) = customer_email(s, user.as_deref()).await {
        s.mailer.send(Mail::new(
            to,
            format!("[가게요] {workshop_name}에서 견적이 도착했습니다"),
            format!(
                "요청하신 견적에 {workshop_name}이(가) {}원으로 답했습니다.\n\n받은 견적을 비교하고 마음에 드는 곳을 골라 예약하세요.\n{}/?quote={quote_id}",
                won(amount),
                s.cfg.app_url
            ),
        ));
    }
}

pub async fn offer_accepted(s: &AppState, workshop_id: &str, vehicle: &str, customer_phone: &str) {
    if let Some(to) = owner_email(s, workshop_id).await {
        s.mailer.send(Mail::new(
            to,
            "[가게요] 고객이 견적을 선택했습니다",
            format!(
                "고객이 보내신 견적을 선택했습니다.\n\n차량: {vehicle}\n연락처: {customer_phone}\n\n고객이 방문 예약을 신청하면 예약 일정에 표시됩니다.\n{}/?tab=offers",
                s.cfg.partner_url
            ),
        ));
    }
}

pub async fn booking_created(s: &AppState, workshop_id: &str, customer_name: &str, vehicle: &str, starts_at: &str) {
    if let Some(to) = owner_email(s, workshop_id).await {
        s.mailer.send(Mail::new(
            to,
            format!("[가게요] 새 예약 요청 · {starts_at}"),
            format!(
                "새 예약 요청이 들어왔습니다.\n\n고객: {customer_name}\n차량: {vehicle}\n방문 시간: {starts_at}\n\n파트너 센터에서 확정하거나 거절해 주세요.\n{}/?tab=bookings",
                s.cfg.partner_url
            ),
        ));
    }
}

pub async fn booking_cancelled_by_customer(s: &AppState, workshop_id: &str, customer_name: &str, starts_at: &str) {
    if let Some(to) = owner_email(s, workshop_id).await {
        s.mailer.send(Mail::new(
            to,
            "[가게요] 고객이 예약을 취소했습니다",
            format!("{customer_name} 고객이 {starts_at} 예약을 취소했습니다."),
        ));
    }
}

/// 파트너가 예약 상태를 바꾸면 고객에게 (회원만 — 비회원은 연락처로 업체가 직접 연락)
pub async fn booking_status_changed(s: &AppState, booking_id: &str, status: &str, reason: Option<&str>) {
    let row: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT b.customer_user_id, w.name, DATE_FORMAT(b.starts_at, '%Y-%m-%d %H:%i') FROM bookings b \
         JOIN workshops w ON w.id = b.workshop_id WHERE b.id = ?",
    )
    .bind(booking_id)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten();
    let Some((user, workshop, at)) = row else { return };
    let Some(to) = customer_email(s, user.as_deref()).await else { return };
    let (title, body) = match status {
        "confirmed" => ("예약이 확정되었습니다", format!("{workshop} {at} 예약이 확정되었습니다. 시간에 맞춰 방문해 주세요.")),
        "declined" => (
            "예약이 거절되었습니다",
            format!("{workshop}에서 {at} 예약을 받을 수 없다고 알려 왔습니다.{}", reason.map(|r| format!("\n사유: {r}")).unwrap_or_default()),
        ),
        "cancelled" => (
            "예약이 취소되었습니다",
            format!("{workshop} {at} 예약이 업체 사정으로 취소되었습니다.{}", reason.map(|r| format!("\n사유: {r}")).unwrap_or_default()),
        ),
        "completed" => ("작업이 완료되었습니다", format!("{workshop}에서 작업을 마쳤습니다. 이용 후기를 남겨 주시면 다른 운전자에게 큰 도움이 됩니다.")),
        _ => return,
    };
    s.mailer.send(Mail::new(to, format!("[가게요] {title}"), format!("{body}\n\n{}/?my=bookings", s.cfg.app_url)));
}

pub async fn review_posted(s: &AppState, workshop_id: &str, rating: i8) {
    if let Some(to) = owner_email(s, workshop_id).await {
        s.mailer.send(Mail::new(
            to,
            format!("[가게요] 새 후기 ★{rating}"),
            format!("고객이 새 후기를 남겼습니다. 파트너 센터에서 확인하고 답글을 달아 보세요.\n{}/?tab=reviews", s.cfg.partner_url),
        ));
    }
}

pub async fn workshop_decision(s: &AppState, workshop_id: &str, status: &str, reason: Option<&str>) {
    let Some(to) = owner_email(s, workshop_id).await else { return };
    let (title, body) = match status {
        "approved" => (
            "입점이 승인되었습니다",
            "가게요 입점 심사가 승인되었습니다. 이제 고객 화면에 업체가 보이고, 견적 요청과 예약을 받을 수 있습니다.".to_string(),
        ),
        "rejected" => (
            "입점 신청이 반려되었습니다",
            format!("입점 신청이 반려되었습니다.{}\n파트너 센터에서 정보를 고쳐 다시 신청할 수 있습니다.", reason.map(|r| format!("\n사유: {r}")).unwrap_or_default()),
        ),
        "suspended" => (
            "업체 이용이 중지되었습니다",
            format!("운영 정책에 따라 업체 이용이 중지되었습니다.{}\n자세한 내용은 고객센터로 문의해 주세요.", reason.map(|r| format!("\n사유: {r}")).unwrap_or_default()),
        ),
        _ => return,
    };
    s.mailer.send(Mail::new(to, format!("[가게요] {title}"), format!("{body}\n\n{}", s.cfg.partner_url)));
}

pub async fn change_request_decided(s: &AppState, workshop_id: &str, approved: bool, reason: Option<&str>) {
    let Some(to) = owner_email(s, workshop_id).await else { return };
    let body = if approved {
        "요청하신 업체 정보 변경이 승인되어 반영되었습니다.".to_string()
    } else {
        format!("요청하신 업체 정보 변경이 반려되었습니다.{}", reason.map(|r| format!("\n사유: {r}")).unwrap_or_default())
    };
    s.mailer.send(Mail::new(to, "[가게요] 업체 정보 변경 요청 결과", format!("{body}\n\n{}/?tab=profile", s.cfg.partner_url)));
}

/// 운영 알림 (새 입점 신청·정보 변경 요청)
pub fn ops(s: &AppState, subject: &str, body: &str) {
    s.mailer.send_all(&s.cfg.ops_recipients(), &format!("[가게요 운영] {subject}"), &format!("{body}\n\n{}", s.cfg.admin_url));
}

/// 12345 → "12,345"
pub fn won(n: u64) -> String {
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

#[cfg(test)]
mod tests {
    #[test]
    fn won_format() {
        assert_eq!(super::won(0), "0");
        assert_eq!(super::won(999), "999");
        assert_eq!(super::won(1000), "1,000");
        assert_eq!(super::won(1234567), "1,234,567");
    }
}
