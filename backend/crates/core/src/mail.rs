//! 메일 보내기 (SMTP). bandiuri `mail.rs` 를 옮김.
//!
//! - `SMTP_URL` 이 없으면 보내지 않고 로그에만 남깁니다 (개발·메일 미설정 서버).
//!   예: `smtps://아이디:앱비밀번호@smtp.gmail.com:465`, `smtp://아이디:비번@smtp.example.com:587?tls=required`
//! - `MAIL_FROM` 은 보내는 사람 (예: `가게요 <no-reply@gageyo.com>`).
//! - 보내기는 요청과 따로(백그라운드) 처리합니다. 메일 서버가 느리거나 실패해도 예약·주문은 영향을 받지 않습니다.
//! - 본문은 글자(text/plain)만 씁니다 — 고객이 입력한 값이 들어가도 HTML 로 해석되지 않습니다.

use lettre::{
    message::{header::ContentType, Mailbox},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Option<Mailbox>,
}

#[derive(Debug, Clone)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

impl Mail {
    pub fn new(to: impl Into<String>, subject: impl Into<String>, body: impl Into<String>) -> Self {
        Self { to: to.into(), subject: subject.into(), body: body.into() }
    }
}

impl Mailer {
    pub fn disabled() -> Self {
        Self { transport: None, from: None }
    }

    /// 환경변수 값으로 만듭니다. 잘못된 설정은 경고만 남기고 끈 상태로 둡니다.
    pub fn from_config(smtp_url: Option<&str>, from: Option<&str>) -> Self {
        let Some(url) = smtp_url.map(str::trim).filter(|u| !u.is_empty()) else {
            tracing::info!("메일: SMTP_URL 이 없어 메일을 보내지 않습니다 (로그에만 기록)");
            return Self::disabled();
        };
        let from = match from.map(str::trim).filter(|f| !f.is_empty()).map(str::parse::<Mailbox>) {
            Some(Ok(m)) => m,
            Some(Err(e)) => {
                tracing::error!("메일: MAIL_FROM 형식이 올바르지 않습니다 ({e}) — 메일을 끕니다");
                return Self::disabled();
            }
            None => {
                tracing::error!("메일: SMTP_URL 은 있는데 MAIL_FROM 이 없습니다 — 메일을 끕니다");
                return Self::disabled();
            }
        };
        match AsyncSmtpTransport::<Tokio1Executor>::from_url(url) {
            Ok(b) => {
                tracing::info!("메일: SMTP 사용 ({})", redact(url));
                Self { transport: Some(b.build()), from: Some(from) }
            }
            Err(e) => {
                tracing::error!("메일: SMTP_URL 을 해석할 수 없습니다 ({e}) — 메일을 끕니다");
                Self::disabled()
            }
        }
    }

    pub fn enabled(&self) -> bool {
        self.transport.is_some()
    }

    /// 백그라운드로 보냅니다. 실패는 로그로만 남깁니다.
    pub fn send(&self, mail: Mail) {
        if !looks_like_email(&mail.to) {
            tracing::debug!("메일: 받는 주소가 없거나 올바르지 않아 건너뜀");
            return;
        }
        let (Some(t), Some(from)) = (self.transport.clone(), self.from.clone()) else {
            tracing::info!(to = %mask_email(&mail.to), subject = %mail.subject, "메일(미설정, 보내지 않음)");
            return;
        };
        tokio::spawn(async move {
            let Ok(to) = mail.to.parse::<Mailbox>() else { return };
            let msg = Message::builder()
                .from(from)
                .to(to)
                .subject(mail.subject.clone())
                .header(ContentType::TEXT_PLAIN)
                .body(format!("{}\n\n-- \n가게요 · 우리 동네 자동차 관리 플랫폼\n이 메일은 발신 전용입니다.\n", mail.body));
            let msg = match msg {
                Ok(m) => m,
                Err(e) => return tracing::warn!("메일 만들기 실패: {e}"),
            };
            match t.send(msg).await {
                Ok(_) => tracing::info!(to = %mask_email(&mail.to), subject = %mail.subject, "메일 보냄"),
                Err(e) => tracing::warn!(to = %mask_email(&mail.to), error = %e, "메일 보내기 실패"),
            }
        });
    }

    pub fn send_all(&self, to: &[String], subject: &str, body: &str) {
        for t in to {
            self.send(Mail::new(t.clone(), subject, body));
        }
    }
}

/// 로그에 비밀번호가 남지 않게: `smtps://user:pw@host` → `smtps://user:***@host`
fn redact(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(s), Some(at)) if at > s => {
            let cred = &url[s + 3..at];
            let user = cred.split(':').next().unwrap_or("");
            format!("{}{user}:***{}", &url[..s + 3], &url[at..])
        }
        _ => url.to_string(),
    }
}

/// 로그용: `kim@example.com` → `k***@example.com`
pub fn mask_email(e: &str) -> String {
    match e.split_once('@') {
        Some((l, d)) => format!("{}***@{d}", l.chars().next().unwrap_or('*')),
        None => "***".into(),
    }
}

/// 주소처럼 생겼는지 (헤더 주입 방지 포함)
pub fn looks_like_email(e: &str) -> bool {
    let e = e.trim();
    !e.is_empty()
        && e.len() <= 254
        && !e.contains(['<', '>', '\r', '\n', ' ', ',', ';'])
        && e.split_once('@').is_some_and(|(l, d)| !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'))
        && e.parse::<Mailbox>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_password() {
        assert_eq!(redact("smtps://me:secret@smtp.x.com:465"), "smtps://me:***@smtp.x.com:465");
        assert_eq!(redact("smtp://127.0.0.1:2525"), "smtp://127.0.0.1:2525");
    }

    #[test]
    fn masks_and_validates() {
        assert_eq!(mask_email("kim@example.com"), "k***@example.com");
        assert!(looks_like_email("kim@example.com"));
        assert!(!looks_like_email("kim@example.com\r\nBcc: x@y.z"));
        assert!(!looks_like_email("not-an-email"));
        assert!(!looks_like_email("a@b"));
        assert!(!looks_like_email(""));
    }

    #[test]
    fn bad_config_disables() {
        assert!(!Mailer::from_config(None, None).enabled());
        assert!(!Mailer::from_config(Some("smtp://127.0.0.1:2525"), None).enabled());
        assert!(!Mailer::from_config(Some("smtp://127.0.0.1:2525"), Some("<<bad")).enabled());
    }
}
