import { money, Loading, ErrorBox, useLoad, relativeText } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

function MonthlyChart({ rows }) {
  if (!rows.length) return <p className="gy-muted">최근 6개월 결제 기록이 없습니다.</p>;
  const max = Math.max(...rows.map((r) => r.gmv), 1);
  return (
    <div className="bar-chart" role="img" aria-label="월별 거래액">
      {rows.map((r) => (
        <div key={r.month} className="bar-col" title={`${r.month} 거래액 ${money(r.gmv)} · 수수료 ${money(r.fee)} · ${r.orders}건`}>
          <span className="bar-value">{money(r.gmv)}</span>
          <div className="bar" style={{ height: `${Math.max(4, (r.gmv / max) * 140)}px` }}>
            <i style={{ height: `${r.gmv ? Math.max(2, (r.fee / r.gmv) * 100) : 0}%` }} />
          </div>
          <small>{r.month.slice(2).replace("-", ".")}</small>
        </div>
      ))}
    </div>
  );
}

export default function Dashboard() {
  const { go } = useAdmin();
  const { data: d, loading, error, reload } = useLoad(() => aapi("/admin/dashboard"), []);
  if (loading && !d) return <Loading />;
  if (!d) return <ErrorBox error={error} onRetry={reload} />;
  const open = d.open_inquiries || {};
  const cards = [
    ["승인 대기 업체", d.workshops.pending, "심사가 필요합니다", "workshops", { status: "pending" }],
    ["정보 변경 요청", d.workshops.change_requests, "업체 기본 정보", "changes"],
    ["답변 대기 문의", (open.customer || 0) + (open.partner || 0), `신고 ${open.report || 0}건`, "inquiries"],
    ["입금 확인 대기", d.commerce.awaiting_deposit, "무통장 주문", "orders", { status: "payment_pending", method: "bank" }],
    ["30일 거래액", money(d.commerce.gmv_30d), `결제 ${d.commerce.orders_30d}건`, "orders"],
    ["보낼 상품", d.commerce.items_to_ship, "업체 발송 대기", "orders", { status: "paid" }],
    ["승인 업체", d.workshops.approved, `중지 ${d.workshops.suspended}곳`, "workshops", { status: "approved" }],
    ["고객 회원", d.users.customers, `7일 신규 ${d.users.new_customers_7d}명`, "members"],
    ["받는 중인 견적", d.activity.open_quotes, `30일 요청 ${d.activity.quotes_30d}건`, "quotes"],
    ["이번 달 예약", d.activity.bookings_this_month, `30일 완료 ${d.activity.completed_bookings_30d}건`, "bookings"],
    ["30일 리뷰", d.activity.reviews_30d, "리뷰 관리", "reviews"],
    ["파트너 계정", d.users.partners, "업체 담당자", "members", { role: "workshop" }],
  ];
  return (
    <>
      <div className="page-title">
        <div>
          <h1>플랫폼 현황</h1>
          <p>숫자를 누르면 해당 화면으로 이동합니다.</p>
        </div>
        <button type="button" onClick={reload}>
          ↻ 새로고침
        </button>
      </div>
      <div className="metric-grid">
        {cards.map(([label, value, note, tab, params]) => (
          <button type="button" key={label} onClick={() => go(tab, params || {})} className={typeof value === "number" && value > 0 && ["승인 대기 업체", "정보 변경 요청", "답변 대기 문의", "입금 확인 대기"].includes(label) ? "alert" : ""}>
            <span>{label}</span>
            <b>{value}</b>
            <small>{note}</small>
          </button>
        ))}
      </div>
      <div className="grid-2">
        <article className="box">
          <div className="box-title">
            <div>
              <h2>월별 거래액</h2>
              <p>결제 완료 기준, 취소·환불 제외. 진한 부분은 플랫폼 수수료.</p>
            </div>
          </div>
          <MonthlyChart rows={d.monthly} />
        </article>
        <article className="box">
          <div className="box-title">
            <div>
              <h2>최근 작업 기록</h2>
            </div>
            <button type="button" onClick={() => go("audit")}>
              전체 →
            </button>
          </div>
          <ul className="audit-mini">
            {d.recent_audit.map((a) => (
              <li key={a.id}>
                <b>{a.action}</b> <span>{a.actor}</span>
                <small>{relativeText(a.created_at)}</small>
              </li>
            ))}
            {!d.recent_audit.length && <li className="gy-muted">기록이 없습니다.</li>}
          </ul>
        </article>
      </div>
    </>
  );
}
