import { useState } from "react";
import { Empty, Loading, ErrorBox, useLoad, dateTimeText, qs } from "@gageyo/ui";
import { aapi } from "../aapi.js";

const TYPES = [
  ["", "전체"],
  ["workshop", "업체"],
  ["user", "회원"],
  ["order", "주문"],
  ["review", "리뷰"],
  ["product", "상품"],
  ["booking", "예약"],
  ["quote", "견적"],
  ["inquiry", "문의"],
  ["category", "카테고리"],
  ["notice", "공지"],
  ["settings", "설정"],
];

export default function Audit() {
  const [type, setType] = useState("");
  const [page, setPage] = useState(1);
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/audit${qs({ target_type: type, page, size: 100 })}`), [type, page]);
  return (
    <>
      <div className="page-title">
        <div>
          <h1>작업 기록</h1>
          <p>누가 언제 무엇을 승인·정지·변경했는지와 사유, IP 입니다.</p>
        </div>
        <select value={type} onChange={(e) => { setType(e.target.value); setPage(1); }} aria-label="대상">
          {TYPES.map(([v, l]) => (
            <option key={v} value={v}>
              {l}
            </option>
          ))}
        </select>
      </div>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>기록이 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>시각</th>
                <th>작업</th>
                <th>대상</th>
                <th>내용</th>
                <th>누가</th>
                <th>IP</th>
              </tr>
            </thead>
            <tbody>
              {data.map((a) => (
                <tr key={a.id}>
                  <td>{dateTimeText(a.created_at)}</td>
                  <td>
                    <b>{a.action}</b>
                  </td>
                  <td>
                    {a.target_type}
                    <small className="mono">{a.target_id}</small>
                  </td>
                  <td>{a.detail || "-"}</td>
                  <td>{a.actor}</td>
                  <td>{a.ip || "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="pager">
        <button type="button" disabled={page <= 1} onClick={() => setPage(page - 1)}>
          ← 이전
        </button>
        <span>{page}쪽</span>
        <button type="button" disabled={(data?.length || 0) < 100} onClick={() => setPage(page + 1)}>
          다음 →
        </button>
      </div>
    </>
  );
}
