import { useState } from "react";
import { Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateText, qs } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

export default function Quotes() {
  const { toast } = useAdmin();
  const [status, setStatus] = useState("open");
  const [q, setQ] = useState("");
  const [term, setTerm] = useState("");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/quotes${qs({ status, q: term, size: 200 })}`), [status, term]);
  async function change(item, next) {
    try {
      await aapi(`/admin/quotes/${item.id}`, { method: "PATCH", body: { status: next } });
      toast("상태를 바꿨습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>견적 요청</h1>
          <p>요청은 14일 동안 받으며 지나면 마감으로 보입니다. 스팸·장난 요청은 마감 처리하세요.</p>
        </div>
      </div>
      <Tabs
        tabs={[
          ["open", "받는 중"],
          ["accepted", "업체 선택"],
          ["closed", "마감"],
          ["cancelled", "취소"],
          ["", "전체"],
        ]}
        value={status}
        onChange={setStatus}
      />
      <form className="search-row" onSubmit={(e) => { e.preventDefault(); setTerm(q); }}>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="차량·지역·연락처" />
        <button className="gy-button ghost small">검색</button>
      </form>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>견적 요청이 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>차량</th>
                <th>분야·지역</th>
                <th>내용</th>
                <th>연락처</th>
                <th>견적</th>
                <th>상태</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {data.map((x) => (
                <tr key={x.id}>
                  <td>
                    <b>{x.vehicle}</b>
                    <small>{dateText(x.created_at)}</small>
                  </td>
                  <td>
                    {x.service_category}
                    <small>{x.district}</small>
                  </td>
                  <td className="clip">{x.description}</td>
                  <td>{x.customer_phone}</td>
                  <td>{x.offer_count}건</td>
                  <td>
                    <Badge kind="quote" value={x.status} />
                  </td>
                  <td>
                    {x.status === "open" && (
                      <button type="button" className="gy-button ghost small" onClick={() => change(x, "closed")}>
                        마감
                      </button>
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
