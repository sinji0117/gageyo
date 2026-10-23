//! 입력 검사와 작은 도움 함수들.

use chrono::{Duration, NaiveDateTime, Utc};
use serde::Deserialize;

use crate::{AppError, AppResult};

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 한국 시간(KST) 지금. DB 연결도 `time_zone = '+09:00'` 으로 맞춰 `NOW()` 와 같습니다.
pub fn kst_now() -> NaiveDateTime {
    (Utc::now() + Duration::hours(9)).naive_utc()
}

/// 필수 글자 입력: 앞뒤 공백을 지우고, 비었거나 `max` 자를 넘으면 오류
pub fn req_text(v: &str, label: &str, max: usize) -> AppResult<String> {
    let t = v.trim();
    if t.is_empty() {
        return Err(AppError::bad(format!("{label}을(를) 입력해 주세요")));
    }
    if t.chars().count() > max {
        return Err(AppError::bad(format!("{label}은(는) {max}자 이내로 입력해 주세요")));
    }
    if t.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
        return Err(AppError::bad(format!("{label}에 쓸 수 없는 글자가 있습니다")));
    }
    Ok(t.to_string())
}

/// 선택 글자 입력: 비우면 None
pub fn opt_text(v: Option<&str>, label: &str, max: usize) -> AppResult<Option<String>> {
    match v.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => req_text(t, label, max).map(Some),
        None => Ok(None),
    }
}

/// 한 줄 입력 (줄바꿈 금지)
pub fn req_line(v: &str, label: &str, max: usize) -> AppResult<String> {
    let t = req_text(v, label, max)?;
    if t.contains(['\n', '\r']) {
        return Err(AppError::bad(format!("{label}은(는) 한 줄로 입력해 주세요")));
    }
    Ok(t)
}

/// 이메일: 소문자·공백 제거 후 형식 검사
pub fn normalize_email(v: &str) -> AppResult<String> {
    let e = v.trim().to_ascii_lowercase();
    if !crate::mail::looks_like_email(&e) {
        return Err(AppError::bad("이메일 형식을 확인해 주세요"));
    }
    Ok(e)
}

/// 휴대폰·일반 전화: 숫자 9~11자리, 0으로 시작 → `010-1234-5678` 형태
pub fn normalize_phone(v: &str) -> AppResult<String> {
    let raw = v.trim();
    if raw.chars().any(|c| !(c.is_ascii_digit() || " -+().".contains(c))) {
        return Err(AppError::bad("연락처는 숫자로 입력해 주세요"));
    }
    let mut d: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if let Some(rest) = d.strip_prefix("82") {
        if raw.starts_with('+') {
            d = format!("0{rest}");
        }
    }
    if !(9..=11).contains(&d.len()) || !d.starts_with('0') {
        return Err(AppError::bad("연락처를 확인해 주세요 (예: 010-1234-5678)"));
    }
    Ok(format_phone(&d))
}

fn format_phone(d: &str) -> String {
    if d.starts_with("02") {
        match d.len() {
            9 => format!("{}-{}-{}", &d[..2], &d[2..5], &d[5..]),
            _ => format!("{}-{}-{}", &d[..2], &d[2..d.len() - 4], &d[d.len() - 4..]),
        }
    } else if d.len() == 8 {
        format!("{}-{}", &d[..4], &d[4..])
    } else {
        format!("{}-{}-{}", &d[..3], &d[3..d.len() - 4], &d[d.len() - 4..])
    }
}

/// 전화번호 숫자만 (비교용)
pub fn phone_digits(v: &str) -> String {
    v.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// 운영자 목록용: 가운데를 가림 `010-****-5678`
pub fn mask_phone(v: &str) -> String {
    let parts: Vec<&str> = v.split('-').collect();
    if parts.len() == 3 {
        return format!("{}-{}-{}", parts[0], "*".repeat(parts[1].len()), parts[2]);
    }
    let n = v.chars().count();
    if n <= 4 {
        return "*".repeat(n);
    }
    let keep: String = v.chars().skip(n - 4).collect();
    format!("{}{}", "*".repeat(n - 4), keep)
}

/// 사업자등록번호: 숫자 10자리 + 국세청 검증 숫자 → `000-00-00000`
pub fn normalize_business_number(v: &str) -> AppResult<String> {
    let d: Vec<u32> = v.chars().filter(|c| c.is_ascii_digit()).filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 10 || v.chars().any(|c| !(c.is_ascii_digit() || c == '-' || c == ' ')) {
        return Err(AppError::bad("사업자등록번호는 숫자 10자리입니다"));
    }
    let w = [1, 3, 7, 1, 3, 7, 1, 3, 5];
    let mut sum: u32 = d.iter().zip(w.iter()).map(|(a, b)| a * b).sum();
    sum += (d[8] * 5) / 10;
    let check = (10 - sum % 10) % 10;
    if check != d[9] {
        return Err(AppError::bad("사업자등록번호를 다시 확인해 주세요"));
    }
    let s: String = d.iter().map(|n| char::from_digit(*n, 10).unwrap()).collect();
    Ok(format!("{}-{}-{}", &s[..3], &s[3..5], &s[5..]))
}

/// 위도·경도 범위 (대한민국 주변)
pub fn check_coords(lat: Option<f64>, lng: Option<f64>) -> AppResult<(Option<f64>, Option<f64>)> {
    match (lat, lng) {
        (None, None) => Ok((None, None)),
        (Some(a), Some(o)) if (32.0..=39.5).contains(&a) && (123.0..=132.5).contains(&o) => Ok((Some(a), Some(o))),
        _ => Err(AppError::bad("위치(위도·경도)를 확인해 주세요")),
    }
}

/// 두 좌표 사이 거리 (km, 하버사인). SQL 의 거리 계산과 같은 식
pub fn distance_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let r = 6371.0_f64;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lng2 - lng1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().asin()
}

/// 목록 쪽수 (`?page=1&size=20`)
#[derive(Debug, Deserialize, Default, Clone, Copy)]
pub struct Page {
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl Page {
    /// (limit, offset). 한 번에 최대 `max` 개
    pub fn limit_offset(&self, default: u32, max: u32) -> (i64, i64) {
        let size = self.size.unwrap_or(default).clamp(1, max);
        let page = self.page.unwrap_or(1).clamp(1, 10_000);
        (i64::from(size), i64::from((page - 1) * size))
    }
}

/// LIKE 검색어: `%`·`_`·`\` 를 이스케이프하고 길이 제한
pub fn like_term(q: Option<&str>) -> Option<String> {
    let t = q.map(str::trim).filter(|t| !t.is_empty())?;
    let t: String = t.chars().take(60).collect();
    Some(t.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"))
}

/// 택배사 (코드, 이름, 조회 주소). bandiuri 와 같은 목록
pub const CARRIERS: &[(&str, &str, &str)] = &[
    ("cj", "CJ대한통운", "https://trace.cjlogistics.com/next/tracking.html?wblNo={no}"),
    ("epost", "우체국택배", "https://service.epost.go.kr/trace.RetrieveDomRigiTraceList.comm?sid1={no}"),
    ("hanjin", "한진택배", "https://www.hanjin.com/kor/CMS/DeliveryMgr/WaybillResult.do?mCode=MN038&schLang=KR&wblnumText2={no}"),
    ("lotte", "롯데택배", "https://www.lotteglogis.com/home/reservation/tracking/linkView?InvNo={no}"),
    ("logen", "로젠택배", "https://www.ilogen.com/web/personal/trace/{no}"),
    ("kdexp", "경동택배", "https://kdexp.com/service/delivery/etc/delivery.do?barcode={no}"),
    ("daesin", "대신택배", "https://www.ds3211.co.kr/freight/internalFreightSearch.ht?billno={no}"),
    ("ilyang", "일양로지스", "https://www.ilyanglogis.com/functionality/card_form_waybill.asp?hawb_no={no}"),
    ("hapdong", "합동택배", "https://hdexp.co.kr/basic_delivery.hd?barcode={no}"),
    ("cvsnet", "GS Postbox 택배", "https://www.cvsnet.co.kr/invoice/tracking.do?invoice_no={no}"),
    ("cupost", "CU 편의점택배", "https://www.cupost.co.kr/postbox/delivery/localResult.cupost?invoice_no={no}"),
    ("direct", "직접 배송", ""),
];

pub fn carrier(code: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    CARRIERS.iter().find(|c| c.0 == code)
}

/// 운송장 번호: 숫자·영문·하이픈만 (조회 주소에 그대로 넣으므로)
pub fn clean_tracking_no(v: &str) -> Option<String> {
    let t: String = v.trim().chars().filter(|c| *c != ' ').collect();
    if t.chars().count() > 40 || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(t)
}

pub fn tracking_url(carrier_code: &str, tracking_no: &str) -> Option<String> {
    let (_, _, url) = carrier(carrier_code)?;
    let no = clean_tracking_no(tracking_no).filter(|n| !n.is_empty())?;
    if url.is_empty() {
        return None;
    }
    Some(url.replace("{no}", &no.replace('-', "")))
}

pub fn carriers_json() -> serde_json::Value {
    serde_json::json!(CARRIERS
        .iter()
        .map(|(c, n, u)| serde_json::json!({ "code": c, "name": n, "trackable": !u.is_empty() }))
        .collect::<Vec<_>>())
}

/// 금액 × 수수료율(bp, 1/10000) — 원 단위 내림
pub fn fee_of(amount: u64, rate_bp: u32) -> u64 {
    amount.saturating_mul(u64::from(rate_bp)) / 10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_rules() {
        assert_eq!(req_text("  안녕  ", "이름", 5).unwrap(), "안녕");
        assert!(req_text("   ", "이름", 5).is_err());
        assert!(req_text("가나다라마바", "이름", 5).is_err());
        assert!(req_text("a\u{0007}b", "이름", 5).is_err());
        assert!(req_line("a\nb", "이름", 5).is_err());
        assert_eq!(opt_text(Some("  "), "메모", 5).unwrap(), None);
        assert_eq!(opt_text(None, "메모", 5).unwrap(), None);
    }

    #[test]
    fn phones() {
        assert_eq!(normalize_phone("01012345678").unwrap(), "010-1234-5678");
        assert_eq!(normalize_phone("010-123-4567").unwrap(), "010-123-4567");
        assert_eq!(normalize_phone("02-123-4567").unwrap(), "02-123-4567");
        assert_eq!(normalize_phone("0212345678").unwrap(), "02-1234-5678");
        assert_eq!(normalize_phone("+82 10-1234-5678").unwrap(), "010-1234-5678");
        assert_eq!(normalize_phone("031 123 4567").unwrap(), "031-123-4567");
        assert!(normalize_phone("12345").is_err());
        assert!(normalize_phone("010-1234-567a").is_err());
        assert_eq!(mask_phone("010-1234-5678"), "010-****-5678");
        assert_eq!(phone_digits("010-1234-5678"), "01012345678");
    }

    #[test]
    fn business_numbers() {
        // 국세청 검증식을 통과하는 예시 번호
        assert_eq!(normalize_business_number("1208147521").unwrap(), "120-81-47521");
        assert_eq!(normalize_business_number("220-81-62517").unwrap(), "220-81-62517");
        assert!(normalize_business_number("1208147522").is_err());
        assert!(normalize_business_number("12345").is_err());
        assert!(normalize_business_number("120a8147521").is_err());
    }

    #[test]
    fn geo_and_paging() {
        let d = distance_km(37.4979, 127.0276, 37.5665, 126.9780); // 강남역 → 서울시청
        assert!((8.0..10.0).contains(&d), "{d}");
        assert!(check_coords(Some(37.5), Some(127.0)).is_ok());
        assert!(check_coords(Some(37.5), None).is_err());
        assert!(check_coords(Some(10.0), Some(127.0)).is_err());
        assert_eq!(Page { page: Some(3), size: Some(20) }.limit_offset(20, 100), (20, 40));
        assert_eq!(Page { page: Some(0), size: Some(1000) }.limit_offset(20, 100), (100, 0));
        assert_eq!(like_term(Some(" 50%_off ")).as_deref(), Some("50\\%\\_off"));
        assert_eq!(like_term(Some("  ")), None);
    }

    #[test]
    fn carriers_and_fee() {
        assert_eq!(
            tracking_url("cj", "1234-5678-9012").as_deref(),
            Some("https://trace.cjlogistics.com/next/tracking.html?wblNo=123456789012")
        );
        assert_eq!(tracking_url("direct", "1234"), None);
        assert_eq!(tracking_url("nope", "1234"), None);
        assert!(clean_tracking_no("12<34").is_none());
        assert_eq!(fee_of(10_000, 350), 350);
        assert_eq!(fee_of(9_999, 350), 349);
        assert_eq!(fee_of(10_000, 0), 0);
    }
}
