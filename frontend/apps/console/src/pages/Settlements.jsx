import { useState } from "react";
import { money, Empty, Loading, ErrorBox, useLoad, dateText } from "@gageyo/ui";
import { papi } from "../papi.js";

export default function Settlements() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);
  const { data, loading, error, reload } = useLoad(() => papi(`/partner/settlements?year=${year}`), [year]);
  return (
    <section className="panel">
      <div className="panel-title">
        <div>
          <h2>정산</h2>
          <p>
            가게요가 받은 결제(카드·무통장)의 월별 매출에서 플랫폼 수수료를 뺀 금액을 지급합니다. 업체와 직접 결제한 주문은 포함되지 않습니다.
            {data && ` 지금 수수료율 ${(data.fee_rate_bp / 100).toFixed(2)}% (결제 때 고정)`}
          </p>
        </div>
        <select value={year} onChange={(e) => setYear(Number(e.target.value))} aria-label="연도">
          {[thisYear, thisYear - 1, thisYear - 2].map((y) => (
            <option key={y} value={y}>
              {y}년
            </option>
          ))}
        </select>
      </div>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.months.length === 0 && <Empty>{year}년 정산할 매출이 없습니다.</Empty>}
      {data?.months.length > 0 && (
        <div className="gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>월</th>
                <th>주문</th>
                <th>매출</th>
                <th>수수료</th>
                <th>지급액</th>
                <th>지급</th>
              </tr>
            </thead>
            <tbody>
              {data.months.map((m) => (
                <tr key={m.period}>
                  <td>
                    <b>{m.period}</b>
                  </td>
                  <td>{m.orders}건</td>
                  <td>{money(m.gross)}</td>
                  <td>{money(m.fee)}</td>
                  <td>
                    <b>{money(m.net)}</b>
                  </td>
                  <td>
                    {m.paid ? (
                      <>
                        지급 완료 {dateText(m.paid.paid_at)} · {money(m.paid.net)}
                        {m.changed_after_paid && <small className="warn-text">지급 뒤 금액이 바뀌었습니다(환불 등). 운영자와 확인해 주세요</small>}
                      </>
                    ) : (
                      <span className="gy-muted">지급 전</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
