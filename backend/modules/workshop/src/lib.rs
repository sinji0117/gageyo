//! 업체·서비스·견적·예약·리뷰 도메인.
//!
//! - 공개: 카테고리, 업체 검색(지역·분야·반경), 업체 상세·서비스·리뷰, 통계
//! - 고객: 견적 요청·수락·취소, 예약·취소, 리뷰, 관심 업체
//! - 파트너: 입점 신청·로그인, 업체 정보·사진·영업시간, 서비스, 견적 답변, 예약 처리, 리뷰 답글
//! - 운영자: 업체 승인·반려·중지·수정, 정보 변경 승인, 카테고리, 견적·예약·리뷰 관리

use axum::Router;
use gageyo_core::AppState;

pub mod admin;
pub mod bookings;
pub mod categories;
pub mod notify;
pub mod partner;
pub mod quotes;
pub mod reviews;
pub mod workshops;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(categories::routes())
        .merge(workshops::routes())
        .merge(quotes::routes())
        .merge(bookings::routes())
        .merge(reviews::routes())
        .merge(partner::routes())
        .merge(admin::routes())
}

/// 리뷰 작성자 등 공개 화면용 이름 가리기: `김가게` → `김**`
pub fn mask_name(name: &str) -> String {
    let mut chars = name.trim().chars();
    match chars.next() {
        Some(first) => {
            let rest = chars.count();
            format!("{first}{}", "*".repeat(rest.clamp(1, 4)))
        }
        None => "고객".into(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mask_name() {
        assert_eq!(super::mask_name("김가게"), "김**");
        assert_eq!(super::mask_name("A"), "A*");
        assert_eq!(super::mask_name(""), "고객");
        assert_eq!(super::mask_name("가나다라마바사"), "가****");
    }
}
