//! 영업시간·방문 시간 규칙 (예약·방문 장착 주문이 함께 씀).

use chrono::{Datelike, Duration, NaiveDateTime, NaiveTime};

use crate::{AppError, AppResult};

/// 지금부터 최소 몇 분 뒤부터 예약할 수 있는지
pub const MIN_LEAD_MINUTES: i64 = 30;
/// 몇 일 뒤까지 예약할 수 있는지
pub const MAX_AHEAD_DAYS: i64 = 90;

const DAY_NAMES: [&str; 7] = ["일", "월", "화", "수", "목", "금", "토"];

/// 휴무 요일 입력 `"6, 0"` → `"0,6"` (0=일 … 6=토)
pub fn normalize_closed_days(v: &str) -> AppResult<String> {
    let mut days: Vec<u32> = Vec::new();
    for part in v.split([',', ' ']).filter(|p| !p.trim().is_empty()) {
        let d: u32 = part.trim().parse().map_err(|_| AppError::bad("휴무 요일을 확인해 주세요"))?;
        if d > 6 {
            return Err(AppError::bad("휴무 요일을 확인해 주세요"));
        }
        if !days.contains(&d) {
            days.push(d);
        }
    }
    if days.len() == 7 {
        return Err(AppError::bad("모든 요일을 휴무로 정할 수는 없습니다"));
    }
    days.sort_unstable();
    Ok(days.iter().map(u32::to_string).collect::<Vec<_>>().join(","))
}

/// 영업시간 입력 검사 (둘 다 있거나 둘 다 없음, 여는 시간 < 닫는 시간)
pub fn check_hours(open: Option<NaiveTime>, close: Option<NaiveTime>) -> AppResult<()> {
    match (open, close) {
        (None, None) => Ok(()),
        (Some(o), Some(c)) if o < c => Ok(()),
        (Some(_), Some(_)) => Err(AppError::bad("영업 시작 시간이 종료 시간보다 빨라야 합니다")),
        _ => Err(AppError::bad("영업 시작·종료 시간을 함께 입력해 주세요")),
    }
}

/// 방문(예약) 시각이 받을 수 있는 시간인지
pub fn check_visit_time(
    at: NaiveDateTime,
    now: NaiveDateTime,
    open: Option<NaiveTime>,
    close: Option<NaiveTime>,
    closed_days: &str,
) -> AppResult<()> {
    if at < now + Duration::minutes(MIN_LEAD_MINUTES) {
        return Err(AppError::bad(format!("방문 시간은 지금부터 {MIN_LEAD_MINUTES}분 이후로 골라 주세요")));
    }
    if at > now + Duration::days(MAX_AHEAD_DAYS) {
        return Err(AppError::bad(format!("{MAX_AHEAD_DAYS}일 이내의 날짜만 예약할 수 있습니다")));
    }
    let wd = at.weekday().num_days_from_sunday();
    if closed_days.split(',').filter_map(|d| d.trim().parse::<u32>().ok()).any(|d| d == wd) {
        return Err(AppError::bad(format!("{}요일은 휴무입니다. 다른 날을 골라 주세요", DAY_NAMES[wd as usize])));
    }
    if let (Some(o), Some(c)) = (open, close) {
        let t = at.time();
        if t < o || t >= c {
            return Err(AppError::bad(format!(
                "영업시간({}~{}) 안에서 골라 주세요",
                o.format("%H:%M"),
                c.format("%H:%M")
            )));
        }
    }
    Ok(())
}

/// `"09:00"` 또는 `"09:00:00"` → 시각. 비우면 None
pub fn parse_time(v: Option<&str>) -> AppResult<Option<NaiveTime>> {
    let Some(s) = v.map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None) };
    NaiveTime::parse_from_str(s, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M:%S"))
        .map(Some)
        .map_err(|_| AppError::bad("시간은 09:00 형식으로 입력해 주세요"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn dt(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    #[test]
    fn closed_days_normalized() {
        assert_eq!(normalize_closed_days("6, 0").unwrap(), "0,6");
        assert_eq!(normalize_closed_days("").unwrap(), "");
        assert_eq!(normalize_closed_days("1,1").unwrap(), "1");
        assert!(normalize_closed_days("7").is_err());
        assert!(normalize_closed_days("x").is_err());
        assert!(normalize_closed_days("0,1,2,3,4,5,6").is_err());
    }

    #[test]
    fn visit_time_rules() {
        let now = dt(2026, 10, 23, 10, 0); // 금요일
        let (o, c) = (parse_time(Some("09:00")).unwrap(), parse_time(Some("18:00")).unwrap());
        assert!(check_visit_time(dt(2026, 10, 23, 14, 0), now, o, c, "").is_ok());
        assert!(check_visit_time(dt(2026, 10, 23, 10, 10), now, o, c, "").is_err(), "too soon");
        assert!(check_visit_time(dt(2027, 3, 1, 10, 0), now, o, c, "").is_err(), "too far");
        assert!(check_visit_time(dt(2026, 10, 25, 10, 0), now, o, c, "0").is_err(), "sunday closed");
        assert!(check_visit_time(dt(2026, 10, 24, 18, 0), now, o, c, "0").is_err(), "after close");
        assert!(check_visit_time(dt(2026, 10, 24, 8, 59), now, o, c, "").is_err(), "before open");
        assert!(check_visit_time(dt(2026, 10, 24, 22, 0), now, None, None, "").is_ok(), "no hours set");
    }

    #[test]
    fn hours_and_parse() {
        assert!(check_hours(None, None).is_ok());
        assert!(check_hours(parse_time(Some("18:00")).unwrap(), parse_time(Some("09:00")).unwrap()).is_err());
        assert!(check_hours(parse_time(Some("09:00")).unwrap(), None).is_err());
        assert_eq!(parse_time(Some("09:30:00")).unwrap(), NaiveTime::from_hms_opt(9, 30, 0));
        assert!(parse_time(Some("9시")).is_err());
        assert_eq!(parse_time(Some(" ")).unwrap(), None);
    }
}
