import { useState } from "react";
import { Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateTimeText, qs } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

const VIEWS = [
  ["requested", "확정 대기"],
  ["upcoming", "다가오는 예약"],
  ["past", "지난 예약"],
  ["all", "전체"],
];

export default function Bookings() {
  const { toast, loadCounts } = usePartner();
  const [view, setView] = useState("requested");
  const query = view === "requested" ? { status: "requested" } : view === "all" ? {} : { when: view };
  const { data, loading, error, reload } = useLoad(() => papi(`/partner/bookings${qs(query)}`), [view]);

  async function change(b, status) {
    let reason = null;
    if (["declined", "cancelled"].includes(status)) {
      reason = window.prompt(status === "declined" ? "거절 사유 (고객에게 안내됩니다)" : "취소 사유 (고객에게 안내됩니다)", "");
      if (!reason) return;
    }
    try {
      await papi(`/partner/bookings/${b.id}`, { method: "PATCH", body: { status, reason } });
      toast("예약 상태를 바꿨습니다.");
      reload();
      loadCounts();
    } catch (e) {
      toast(e.message);
    }
  }

  return (
    <section className="panel">
      <div className="panel-title">
        <div>
          <h2>예약 일정</h2>
          <p>신청을 확정하거나 거절하고, 작업이 끝나면 완료로 바꿔 주세요. 완료된 예약에 고객이 리뷰를 남길 수 있습니다.</p>
        </div>
        <button type="button" onClick={reload}>
          ↻ 새로고침
        </button>
      </div>
      <Tabs tabs={VIEWS} value={view} onChange={setView} />
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>예약이 없습니다.</Empty>}
      <ul className="booking-list">
        {data?.map((b) => (
          <li key={b.id}>
            <div className="when">
              <b>{dateTimeText(b.starts_at)}</b>
              <Badge kind="booking" value={b.status} />
            </div>
            <div className="who">
              <b>
                {b.customer_name} · {b.vehicle}
              </b>
              <span>
                <a href={`tel:${b.customer_phone}`}>{b.customer_phone}</a>
                {b.service_name ? ` · ${b.service_name}` : ""}
                {b.order_id ? " · 상품 방문 장착" : ""}
              </span>
              {b.note && <small className="pre">{b.note}</small>}
              {b.decline_reason && <small>사유: {b.decline_reason}</small>}
            </div>
            <div className="gy-actions">
              {b.status === "requested" && (
                <>
                  <button type="button" className="gy-button small" onClick={() => change(b, "confirmed")}>
                    확정
                  </button>
                  <button type="button" className="gy-button ghost small" onClick={() => change(b, "declined")}>
                    거절
                  </button>
                </>
              )}
              {b.status === "confirmed" && (
                <>
                  <button type="button" className="gy-button small" onClick={() => change(b, "completed")}>
                    작업 완료
                  </button>
                  <button type="button" className="gy-button ghost small" onClick={() => change(b, "cancelled")}>
                    취소
                  </button>
                </>
              )}
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
