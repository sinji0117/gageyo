use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub type AppResult<T> = Result<T, AppError>;

/// API 공통 오류.
///
/// 응답은 항상 `{"error": "사람이 읽는 안내", "code": "bad_request"}` 입니다.
/// 화면(`@gageyo/ui` 의 `api()`)은 `error` 문자열을 그대로 보여 줍니다.
///
/// 예전 코드는 오류를 `(StatusCode, String)` 글자 응답으로 보내 화면이 안내 문구를 읽지 못하고
/// 항상 "요청을 처리하지 못했습니다" 만 보였습니다. 이제 모든 오류가 JSON 입니다.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    TooMany(String),
    /// 로그인 잠금 (HTTP 423)
    #[error("로그인 시도가 너무 많습니다. 15분 뒤에 다시 시도해 주세요")]
    Locked,
    /// 운영자 2단계 인증 코드 필요 (HTTP 401, code `otp_required`)
    #[error("OTP 앱의 6자리 코드를 입력해 주세요")]
    OtpRequired,
    #[error("{0}")]
    Unavailable(String),
    /// 외부 결제사 등에서 거절 (HTTP 402)
    #[error("{0}")]
    PaymentRejected(String),
    #[error("요청을 처리하지 못했습니다. 잠시 후 다시 시도해 주세요")]
    Internal(String),
}

impl AppError {
    pub fn bad(msg: impl Into<String>) -> Self {
        AppError::BadRequest(msg.into())
    }
    /// 예: `AppError::not_found("업체를 찾을 수 없습니다")`
    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound(msg.into())
    }
    pub fn login_required() -> Self {
        AppError::Unauthorized("로그인이 필요합니다".into())
    }
    pub fn forbidden() -> Self {
        AppError::Forbidden("권한이 없습니다".into())
    }

    fn parts(&self) -> (StatusCode, &'static str) {
        match self {
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            AppError::TooMany(_) => (StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
            AppError::Locked => (StatusCode::LOCKED, "locked"),
            AppError::OtpRequired => (StatusCode::UNAUTHORIZED, "otp_required"),
            AppError::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            AppError::PaymentRejected(_) => (StatusCode::PAYMENT_REQUIRED, "payment_rejected"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }

    pub fn status(&self) -> StatusCode {
        self.parts().0
    }
}

pub fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({ "error": message, "code": code }))).into_response()
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();
        if let AppError::Internal(detail) = &self {
            // 내부 사유는 로그에만 남기고 응답에는 노출하지 않습니다.
            tracing::error!(%detail, "internal error");
        }
        json_error(status, code, &self.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("찾을 수 없습니다".into()),
            other => AppError::Internal(format!("db: {other}")),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(format!("json: {e}"))
    }
}

/// MariaDB 중복 키(1062) 오류인지
pub fn is_duplicate(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23000") || db.code().as_deref() == Some("1062"))
        || e.to_string().contains("Duplicate entry")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes() {
        assert_eq!(AppError::bad("x").status(), StatusCode::BAD_REQUEST);
        assert_eq!(AppError::login_required().status(), StatusCode::UNAUTHORIZED);
        assert_eq!(AppError::Locked.status(), StatusCode::LOCKED);
        assert_eq!(AppError::OtpRequired.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(AppError::not_found("업체를 찾을 수 없습니다").status(), StatusCode::NOT_FOUND);
        // 내부 사유는 응답 문구에 섞이지 않음
        assert!(!AppError::Internal("db: secret".into()).to_string().contains("secret"));
    }
}
