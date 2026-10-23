import { useState } from "react";
import { Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateText } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

const FIELDS = [
  ["name", "상호"],
  ["categories", "분야"],
  ["district", "지역"],
  ["address", "주소"],
  ["business_number", "사업자번호"],
  ["representative", "대표자"],
];
const show = (v) => (Array.isArray(v) ? v.join(", ") : v || "-");

/** 승인된 업체의 상호·분야·주소·사업자 정보 변경 요청 심사 */
export default function ChangeRequests() {
  const { toast, refreshCounts } = useAdmin();
  const [status, setStatus] = useState("pending");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/change-requests?status=${status}`), [status]);

  async function decide(c, approve) {
    let reason = null;
    if (!approve) {
      reason = window.prompt("반려 사유 (업체에 안내됩니다)", "");
      if (!reason) return;
    } else if (!window.confirm("변경을 승인하고 바로 반영할까요?")) return;
    try {
      await aapi(`/admin/change-requests/${c.id}`, { method: "PATCH", body: { approve, reason } });
      toast(approve ? "반영했습니다." : "반려했습니다.");
      reload();
      refreshCounts();
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>정보 변경 요청</h1>
          <p>승인된 업체가 상호·분야·주소·사업자 정보를 바꾸려면 여기서 승인해야 반영됩니다.</p>
        </div>
      </div>
      <Tabs
        tabs={[
          ["pending", "심사 중"],
          ["approved", "승인"],
          ["rejected", "반려"],
          ["all", "전체"],
        ]}
        value={status}
        onChange={setStatus}
      />
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>변경 요청이 없습니다.</Empty>}
      {data?.map((c) => (
        <article key={c.id} className="box">
          <div className="box-title">
            <div>
              <h2>{c.current.name}</h2>
              <p>
                {dateText(c.created_at)} 요청 · <Badge value={c.status === "pending" ? "pending" : c.status}>{{ pending: "심사 중", approved: "승인", rejected: "반려" }[c.status]}</Badge>
                {c.reason && ` · ${c.reason}`}
              </p>
            </div>
            {c.status === "pending" && (
              <div className="gy-actions">
                <button type="button" className="gy-button small" onClick={() => decide(c, true)}>
                  승인
                </button>
                <button type="button" className="gy-button ghost small" onClick={() => decide(c, false)}>
                  반려
                </button>
              </div>
            )}
          </div>
          <table className="gy-table diff">
            <thead>
              <tr>
                <th>항목</th>
                <th>지금</th>
                <th>변경 후</th>
              </tr>
            </thead>
            <tbody>
              {FIELDS.map(([k, label]) => {
                const before = show(c.current[k]);
                const after = show(c.changes?.[k]);
                if (!(k in (c.changes || {})) || (k in c.current && before === after)) return null;
                return (
                  <tr key={k}>
                    <td>{label}</td>
                    <td>{k in c.current ? before : "-"}</td>
                    <td className="changed">{after}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </article>
      ))}
    </>
  );
}
