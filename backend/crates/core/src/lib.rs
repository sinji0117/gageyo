//! gageyo 공통 코어.
//!
//! - [`state::AppState`]: DB 풀, 설정, 메일, 비밀값 암호화, 외부 HTTP 클라이언트를 담아 모든 모듈이 같이 씁니다.
//! - [`error::AppError`]: 모든 API 오류는 `{"error": "안내 문구", "code": "..."}` JSON 으로 응답합니다.
//! - [`auth`]: 고객·파트너·운영자 세션(쿠키)과 요청 추출기.
//!
//! bandiuri 코어(설정·오류·인증·메일·가입 코드·OTP·비밀값·이미지·토스 결제)를 가게요에 맞게 옮겼습니다.
//! bandiuri 와 달리 Redis 없이 MariaDB 만으로 세션·인증 코드·요청 제한을 처리합니다.

pub mod audit;
pub mod auth;
pub mod config;
pub mod error;
pub mod files;
pub mod mail;
pub mod ratelimit;
pub mod schedule;
pub mod secret;
pub mod settings;
pub mod state;
pub mod toss;
pub mod totp;
pub mod util;
pub mod verify;

pub use error::{AppError, AppResult};
pub use state::AppState;

/// 하위 호환: 예전 `gageyo_core::response::error` 사용처
pub mod response {
    use axum::{http::StatusCode, response::Response};

    pub fn error(status: StatusCode, message: impl Into<String>) -> Response {
        crate::error::json_error(status, "error", &message.into())
    }
}
