//! 고객센터: 문의(고객·파트너), 신고, 공지 (bandiuri 고객 문의·사장님 문의·공지를 옮김).
//!
//! - 고객 문의: 로그인 없이도 남길 수 있고, 비밀 링크(48자)로 답변을 확인·이어서 묻습니다. 회원은 마이페이지에 쌓입니다.
//! - 파트너 문의: 파트너 센터에서 남기고 답변을 봅니다.
//! - 신고: 업체·리뷰·상품을 신고하면 운영자 문의함(신고 탭)에 쌓입니다.
//! - 운영자가 답하면 메일로 알리고, 문의한 사람이 다시 쓰면 '답변 대기'로 돌아갑니다.
//! - 공지: 운영자가 고객·파트너·전체 대상으로 올립니다 (맨 위 고정 가능).

use axum::Router;
use gageyo_core::AppState;

pub mod inquiries;
pub mod notices;

pub fn routes() -> Router<AppState> {
    Router::new().merge(inquiries::routes()).merge(notices::routes())
}
