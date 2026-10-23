// 금액·날짜·상태 표시

export const money = (value) => new Intl.NumberFormat("ko-KR").format(Number(value || 0)) + "원";

const pad = (n) => String(n).padStart(2, "0");

/** 서버의 "2026-10-24T14:30:00" (한국 시간, 시간대 없음) → Date */
export function parseDate(value) {
  if (!value) return null;
  if (value instanceof Date) return value;
  const d = new Date(String(value).replace(" ", "T"));
  return Number.isNaN(d.getTime()) ? null : d;
}

export function dateText(value) {
  const d = parseDate(value);
  return d ? `${d.getFullYear()}.${pad(d.getMonth() + 1)}.${pad(d.getDate())}` : "-";
}

export function dateTimeText(value) {
  const d = parseDate(value);
  if (!d) return "-";
  const days = ["일", "월", "화", "수", "목", "금", "토"];
  return `${d.getMonth() + 1}월 ${d.getDate()}일 (${days[d.getDay()]}) ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function timeText(value) {
  if (!value) return "";
  return String(value).slice(0, 5);
}

/** 서버로 보낼 "YYYY-MM-DDTHH:MM:00" */
export function toLocalIso(date, time) {
  return `${date}T${time.length === 5 ? `${time}:00` : time}`;
}

export function todayIso(offsetDays = 0) {
  const d = new Date(Date.now() + offsetDays * 86400000);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function relativeText(value) {
  const d = parseDate(value);
  if (!d) return "";
  const diff = (Date.now() - d.getTime()) / 1000;
  if (diff < 60) return "방금";
  if (diff < 3600) return `${Math.floor(diff / 60)}분 전`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}시간 전`;
  if (diff < 86400 * 7) return `${Math.floor(diff / 86400)}일 전`;
  return dateText(d);
}

export const DAY_NAMES = ["일", "월", "화", "수", "목", "금", "토"];

export function hoursText(w) {
  if (!w) return "";
  const hours = w.open_time && w.close_time ? `${timeText(w.open_time)}~${timeText(w.close_time)}` : "영업시간 문의";
  const closed = (w.closed_days || "")
    .split(",")
    .filter((d) => d !== "")
    .map((d) => DAY_NAMES[Number(d)])
    .join("·");
  return closed ? `${hours} · ${closed} 휴무` : hours;
}

const LABELS = {
  booking: { requested: "신청", confirmed: "확정", completed: "완료", declined: "거절", cancelled: "취소" },
  quote: { open: "받는 중", accepted: "업체 선택", closed: "마감", cancelled: "취소", matched: "매칭" },
  offer: { sent: "보냄", accepted: "고객 선택", declined: "선택 안 됨", withdrawn: "철회" },
  order: { payment_pending: "결제 대기", paid: "결제 완료", preparing: "준비 중", shipped: "발송", delivered: "배송 완료", cancelled: "취소", refunded: "환불" },
  item: { pending: "결제 대기", ordered: "새 주문", preparing: "준비 중", shipped: "발송", delivered: "완료", cancelled: "취소" },
  workshop: { pending: "승인 대기", approved: "승인", rejected: "반려", suspended: "이용 중지" },
  product: { draft: "준비 중", active: "판매 중", sold_out: "품절", archived: "숨김", blocked: "판매 중지" },
  inquiry: { open: "답변 대기", answered: "답변 완료", closed: "종료" },
  review: { visible: "공개", hidden: "숨김" },
  user: { active: "정상", suspended: "정지", withdrawn: "탈퇴" },
  payment: { toss: "카드·간편결제", bank: "무통장입금", none: "업체와 직접 결제" },
};

export function statusLabel(kind, value) {
  return LABELS[kind]?.[value] || value || "-";
}

/** 상태 → 색 이름 (CSS .badge.{tone}) */
export function statusTone(value) {
  if (["approved", "confirmed", "completed", "paid", "delivered", "active", "answered", "accepted", "visible"].includes(value)) return "good";
  if (["pending", "requested", "open", "payment_pending", "ordered", "preparing", "sent", "draft"].includes(value)) return "wait";
  if (["rejected", "declined", "cancelled", "suspended", "refunded", "blocked", "hidden", "withdrawn", "sold_out"].includes(value)) return "bad";
  return "plain";
}

export function maskName(name) {
  if (!name) return "";
  return name[0] + "*".repeat(Math.min(Math.max(name.length - 1, 1), 4));
}
