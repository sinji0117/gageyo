import { useState } from "react";
import { Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateTimeText, qs } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

export default function Bookings() {
  const { toast } = useAdmin();
  const [status, setStatus] = useState("");
  const [q, setQ] = useState("");
  const [term, setTerm] = useState("");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/bookings${qs({ status, q: term, size: 200 })}`), [status, term]);
  async function change(b, next) {
    const reason = ["cancelled", "declined"].includes(next) ? window.prompt("사유", "") : null;
    if (["cancelled", "declined"].includes(next) && !reason) return;
    try {
      await aapi(`/admin/bookings/${b.id}`, { method: "PATCH", body: { status: next, reason } });
      toast("예약 상태를 바꿨습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>예약</h1>
          <p>예약 처리는 업체가 합니다. 분쟁·연락 두절 같은 경우에만 운영자가 상태를 바꾸세요 (작업 기록에 남습니다).</p>
        </div>
      </div>
      <Tabs
        tabs={[
          ["", "전체"],
          ["requested", "신청"],
          ["confirmed", "확정"],
          ["completed", "완료"],
          ["cancelled", "취소"],
          ["declined", "거절"],
        ]}
        value={status}
        onChange={setStatus}
      />
      <form className="search-row" onSubmit={(e) => { e.preventDefault(); setTerm(q); }}>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="업체·고객·연락처" />
        <button className="gy-button ghost small">검색</button>
      </form>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>예약이 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>방문 시간</th>
                <th>업체</th>
                <th>고객·차량</th>
                <th>연락처</th>
                <th>상태</th>
                <th>변경</th>
              </tr>
            </thead>
            <tbody>
              {data.map((b) => (
                <tr key={b.id}>
                  <td>{dateTimeText(b.starts_at)}</td>
                  <td>
                    {b.workshop_name}
                    {b.service_name && <small>{b.service_name}</small>}
                  </td>
                  <td>
                    {b.customer_name}
                    <small>{b.vehicle}</small>
                  </td>
                  <td>{b.customer_phone}</td>
                  <td>
                    <Badge kind="booking" value={b.status} />
                    {b.decline_reason && <small>{b.decline_reason}</small>}
                  </td>
                  <td>
                    <select value={b.status} onChange={(e) => change(b, e.target.value)} aria-label="상태 변경">
                      <option value="requested">신청</option>
                      <option value="confirmed">확정</option>
                      <option value="completed">완료</option>
                      <option value="cancelled">취소</option>
                      <option value="declined">거절</option>
                    </select>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
