import { useState } from "react";
import { money, Empty, Loading, ErrorBox, useLoad, dateText } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

function months() {
  const out = [];
  const d = new Date();
  for (let i = 0; i < 12; i += 1) {
    const m = new Date(d.getFullYear(), d.getMonth() - i, 1);
    out.push(`${m.getFullYear()}-${String(m.getMonth() + 1).padStart(2, "0")}`);
  }
  return out;
}

/** 업체별 월 정산: 매출 − 수수료 = 지급액, 송금 뒤 '지급 완료' 기록 */
export default function Settlements() {
  const { toast } = useAdmin();
  const list = months();
  const [period, setPeriod] = useState(list[1]);
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/settlements?period=${period}`), [period]);

  async function markPaid(row) {
    const memo = window.prompt(`${row.workshop_name} ${period} 정산 ${money(row.net)} 송금을 마쳤나요?\n메모(송금일·이체 번호 등, 선택):`, "");
    if (memo === null) return;
    try {
      await aapi("/admin/settlements", { method: "POST", body: { workshop_id: row.workshop_id, period, memo } });
      toast("지급 완료로 기록했습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  async function undo(row) {
    if (!window.confirm("지급 완료 기록을 지울까요?")) return;
    try {
      await aapi(`/admin/settlements?workshop_id=${row.workshop_id}&period=${period}`, { method: "DELETE" });
      toast("기록을 지웠습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  function downloadCsv() {
    const rows = [["업체", "사업자번호", "주문수", "매출", "수수료", "지급액", "지급"]].concat(
      data.items.map((r) => [r.workshop_name, r.business_number || "", r.orders, r.gross, r.fee, r.net, r.paid ? `지급 ${dateText(r.paid.paid_at)}` : "대기"]),
    );
    const csv = "﻿" + rows.map((r) => r.map((v) => `"${String(v).replaceAll('"', '""')}"`).join(",")).join("\n");
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([csv], { type: "text/csv;charset=utf-8" }));
    a.download = `gageyo-settlement-${period}.csv`;
    a.click();
    URL.revokeObjectURL(a.href);
  }

  return (
    <>
      <div className="page-title">
        <div>
          <h1>정산</h1>
          <p>플랫폼이 받은 결제(카드·무통장)를 결제 완료 월 기준으로 업체별로 합칩니다. 수수료는 결제 때 고정됩니다. 실제 송금·세금계산서는 따로 처리합니다.</p>
        </div>
        <div className="gy-actions">
          <select value={period} onChange={(e) => setPeriod(e.target.value)} aria-label="정산 월">
            {list.map((m) => (
              <option key={m}>{m}</option>
            ))}
          </select>
          {data?.items.length > 0 && (
            <button type="button" onClick={downloadCsv}>
              CSV
            </button>
          )}
        </div>
      </div>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data && (
        <div className="metric-grid small">
          <div>
            <span>매출</span>
            <b>{money(data.totals.gross)}</b>
          </div>
          <div>
            <span>플랫폼 수수료</span>
            <b>{money(data.totals.fee)}</b>
          </div>
          <div>
            <span>업체 지급액</span>
            <b>{money(data.totals.net)}</b>
          </div>
          <div>
            <span>상태</span>
            <b>{data.closed ? "마감된 달" : "진행 중인 달"}</b>
          </div>
        </div>
      )}
      {data?.items.length === 0 && <Empty>{period} 정산할 매출이 없습니다.</Empty>}
      {data?.items.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>업체</th>
                <th>주문</th>
                <th>매출</th>
                <th>수수료</th>
                <th>지급액</th>
                <th>지급</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((r) => (
                <tr key={r.workshop_id}>
                  <td>
                    <b>{r.workshop_name}</b>
                    <small>{r.business_number || "사업자번호 없음"}</small>
                  </td>
                  <td>{r.orders}건</td>
                  <td>{money(r.gross)}</td>
                  <td>{money(r.fee)}</td>
                  <td>
                    <b>{money(r.net)}</b>
                  </td>
                  <td>
                    {r.paid ? (
                      <>
                        {dateText(r.paid.paid_at)} 지급 {money(r.paid.net)}
                        <small>
                          {r.paid.paid_by}
                          {r.paid.memo && ` · ${r.paid.memo}`}
                        </small>
                        {r.changed_after_paid && <small className="warn-text">지급 뒤 금액 변동 (환불 등)</small>}
                        <button type="button" className="link-button" onClick={() => undo(r)}>
                          기록 지우기
                        </button>
                      </>
                    ) : data.closed ? (
                      <button type="button" className="gy-button small" onClick={() => markPaid(r)}>
                        지급 완료
                      </button>
                    ) : (
                      <span className="gy-muted">달이 끝난 뒤</span>
                    )}
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
