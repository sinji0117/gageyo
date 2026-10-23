//! 토스페이먼츠 결제 승인·취소 (bandiuri `pay-toss` 플러그인을 옮김).
//!
//! 흐름 (결제창 SDK v2):
//! 1. 고객 화면이 `https://js.tosspayments.com/v2/standard` 로 결제창을 띄움
//! 2. 결제 성공 시 `successUrl?paymentKey=&orderId=&amount=` 로 돌아옴
//! 3. 서버가 금액을 주문과 대조한 뒤 `POST /v1/payments/confirm` 호출 (Authorization: Basic base64("{secretKey}:"))
//! 4. 환불은 `POST /v1/payments/{paymentKey}/cancel` (Idempotency-Key 헤더로 중복 방지)
//! 5. 승인 결과를 모를 때는 `GET /v1/payments/orders/{orderId}` 로 주문번호의 결제를 찾습니다
//!
//! API 주소는 `TOSS_API_BASE` 로 바꿀 수 있습니다 (테스트에서는 로컬 모의 서버).

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

pub const DEFAULT_API_BASE: &str = "https://api.tosspayments.com";

pub struct TossClient {
    http: reqwest::Client,
    base: String,
}

#[derive(Debug)]
pub enum TossError {
    /// 결제사가 거절 (카드 거절·잔액 부족·키 오류 등) — 고객에게 보여 줄 문구
    Rejected(String),
    /// 통신 오류 — 결과를 모름 (운영자가 토스 상점관리자에서 확인해야 함)
    Transport(String),
}

#[derive(Debug, Clone)]
pub struct Confirmed {
    pub approved_amount: u64,
    pub method: String,
    pub easy_pay_provider: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TossPayment {
    status: String,
    #[serde(default)]
    payment_key: Option<String>,
    total_amount: u64,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    easy_pay: Option<TossEasyPay>,
}

#[derive(Deserialize)]
struct TossEasyPay {
    #[serde(default)]
    provider: Option<String>,
}

#[derive(Deserialize)]
struct TossStatus {
    status: String,
}

#[derive(Deserialize)]
struct TossErrorBody {
    code: Option<String>,
    message: Option<String>,
}

impl TossClient {
    pub fn new(base: &str) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client");
        Self { http, base: base.trim_end_matches('/').to_string() }
    }

    pub async fn confirm(&self, secret_key: &str, payment_key: &str, order_id: &str, amount: u64) -> Result<Confirmed, TossError> {
        if !safe_key(payment_key) {
            return Err(TossError::Rejected("결제 키가 올바르지 않습니다".into()));
        }
        let res = self
            .http
            .post(format!("{}/v1/payments/confirm", self.base))
            .basic_auth(secret_key, Some(""))
            .json(&json!({ "paymentKey": payment_key, "orderId": order_id, "amount": amount }))
            .send()
            .await
            .map_err(|e| TossError::Transport(e.to_string()))?;
        let status = res.status();
        let body = res.text().await.map_err(|e| TossError::Transport(e.to_string()))?;
        if !status.is_success() {
            let err: TossErrorBody = serde_json::from_str(&body).unwrap_or(TossErrorBody { code: None, message: None });
            // 이미 승인된 결제 (승인 요청을 두 번 보낸 경우) — 상태를 조회해 확인
            if err.code.as_deref() == Some("ALREADY_PROCESSED_PAYMENT") {
                return self.lookup(secret_key, payment_key).await;
            }
            tracing::warn!(order_id, http = %status, code = ?err.code, "toss confirm rejected");
            return Err(TossError::Rejected(err.message.unwrap_or_else(|| format!("결제 승인 실패 ({status})"))));
        }
        parse_done(&body)
    }

    async fn lookup(&self, secret_key: &str, payment_key: &str) -> Result<Confirmed, TossError> {
        let res = self
            .http
            .get(format!("{}/v1/payments/{payment_key}", self.base))
            .basic_auth(secret_key, Some(""))
            .send()
            .await
            .map_err(|e| TossError::Transport(e.to_string()))?;
        let status = res.status();
        let body = res.text().await.map_err(|e| TossError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(TossError::Transport(format!("결제 조회 실패 ({status})")));
        }
        parse_done(&body)
    }

    /// 주문번호로 결제를 찾습니다. 승인 완료(`DONE`)면 (결제 키, 승인 정보), 결제가 없거나 다른 상태면 `None`.
    /// 승인 요청이 중간에 끊겨 결과를 모를 때, 결제하지 않은 주문을 취소하기 전에 씁니다.
    pub async fn find_by_order(&self, secret_key: &str, order_id: &str) -> Result<Option<(String, Confirmed)>, TossError> {
        if !safe_key(order_id) {
            return Ok(None);
        }
        let res = self
            .http
            .get(format!("{}/v1/payments/orders/{order_id}", self.base))
            .basic_auth(secret_key, Some(""))
            .send()
            .await
            .map_err(|e| TossError::Transport(e.to_string()))?;
        let status = res.status();
        let body = res.text().await.map_err(|e| TossError::Transport(e.to_string()))?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(TossError::Transport(format!("결제 조회 실패 ({status})")));
        }
        let p: TossPayment = serde_json::from_str(&body).map_err(|e| TossError::Transport(format!("응답 해석 실패: {e}")))?;
        let Some(key) = p.payment_key.clone().filter(|k| !k.is_empty()) else {
            return Ok(None);
        };
        if p.status != "DONE" {
            return Ok(None);
        }
        Ok(Some((key, parse_done(&body)?)))
    }

    pub async fn cancel(&self, secret_key: &str, payment_key: &str, reason: &str, idempotency_key: &str) -> Result<String, TossError> {
        if !safe_key(payment_key) {
            return Err(TossError::Rejected("결제 키가 올바르지 않습니다".into()));
        }
        let res = self
            .http
            .post(format!("{}/v1/payments/{payment_key}/cancel", self.base))
            .basic_auth(secret_key, Some(""))
            .header("Idempotency-Key", idempotency_key)
            .json(&json!({ "cancelReason": reason }))
            .send()
            .await
            .map_err(|e| TossError::Transport(e.to_string()))?;
        let status = res.status();
        let body = res.text().await.map_err(|e| TossError::Transport(e.to_string()))?;
        if !status.is_success() {
            let err: TossErrorBody = serde_json::from_str(&body).unwrap_or(TossErrorBody { code: None, message: None });
            if err.code.as_deref() == Some("ALREADY_CANCELED_PAYMENT") {
                return Ok("CANCELED".into());
            }
            tracing::warn!(http = %status, code = ?err.code, "toss cancel rejected");
            return Err(TossError::Rejected(err.message.unwrap_or_else(|| format!("결제 취소 실패 ({status})"))));
        }
        let p: TossStatus = serde_json::from_str(&body).map_err(|e| TossError::Transport(format!("응답 해석 실패: {e}")))?;
        if p.status != "CANCELED" && p.status != "PARTIAL_CANCELED" {
            return Err(TossError::Rejected(format!("취소 상태가 아닙니다 ({})", p.status)));
        }
        Ok(p.status)
    }
}

fn parse_done(body: &str) -> Result<Confirmed, TossError> {
    let p: TossPayment = serde_json::from_str(body).map_err(|e| TossError::Transport(format!("응답 해석 실패: {e}")))?;
    if p.status != "DONE" {
        return Err(TossError::Rejected(format!("결제가 완료 상태가 아닙니다 ({})", p.status)));
    }
    Ok(Confirmed {
        approved_amount: p.total_amount,
        method: p.method.unwrap_or_default(),
        easy_pay_provider: p.easy_pay.and_then(|e| e.provider).filter(|s| !s.is_empty()),
    })
}

/// 결제 키는 경로에 넣으므로 안전한 글자만
fn safe_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 200 && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::Path,
        http::{HeaderMap, StatusCode},
        routing::post,
        Json, Router,
    };
    use serde_json::Value;

    /// 토스 API 를 흉내 내는 로컬 서버: 인증 헤더·요청 본문 검사
    async fn mock() -> String {
        async fn confirm(headers: HeaderMap, Json(b): Json<Value>) -> (StatusCode, Json<Value>) {
            // "test_sk_x:" 의 base64
            if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("Basic dGVzdF9za194Og==") {
                return (StatusCode::UNAUTHORIZED, Json(json!({"code":"UNAUTHORIZED_KEY","message":"인증되지 않은 시크릿 키"})));
            }
            if b["paymentKey"] == "reject-me" {
                return (StatusCode::BAD_REQUEST, Json(json!({"code":"REJECT_CARD_COMPANY","message":"카드사에서 거절"})));
            }
            (
                StatusCode::OK,
                Json(json!({"status":"DONE","totalAmount": b["amount"],"method":"간편결제","easyPay":{"provider":"토스페이"}})),
            )
        }
        async fn cancel(Path(key): Path<String>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
            if headers.get("idempotency-key").is_none() {
                return (StatusCode::BAD_REQUEST, Json(json!({"code":"NO_KEY","message":"멱등 키 없음"})));
            }
            if key == "done-before" {
                return (StatusCode::BAD_REQUEST, Json(json!({"code":"ALREADY_CANCELED_PAYMENT","message":"이미 취소"})));
            }
            (StatusCode::OK, Json(json!({"status":"CANCELED"})))
        }
        let app = Router::new()
            .route("/v1/payments/confirm", post(confirm))
            .route("/v1/payments/{key}/cancel", post(cancel));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn confirm_and_cancel_against_mock() {
        let c = TossClient::new(&mock().await);
        let ok = c.confirm("test_sk_x", "pay-1", "order-1", 15_000).await.unwrap();
        assert_eq!(ok.approved_amount, 15_000);
        assert_eq!(ok.easy_pay_provider.as_deref(), Some("토스페이"));
        assert!(matches!(c.confirm("test_sk_x", "reject-me", "o", 1).await, Err(TossError::Rejected(m)) if m.contains("거절")));
        assert!(matches!(c.confirm("wrong", "pay-1", "o", 1).await, Err(TossError::Rejected(_))));
        assert!(matches!(c.confirm("test_sk_x", "../evil", "o", 1).await, Err(TossError::Rejected(_))));
        assert_eq!(c.cancel("test_sk_x", "pay-1", "고객 요청", "idem-1").await.unwrap(), "CANCELED");
        assert_eq!(c.cancel("test_sk_x", "done-before", "x", "idem-2").await.unwrap(), "CANCELED");
    }

    #[tokio::test]
    async fn transport_error_when_unreachable() {
        let c = TossClient::new("http://127.0.0.1:9");
        assert!(matches!(c.confirm("test_sk_x", "p", "o", 1).await, Err(TossError::Transport(_))));
    }
}
