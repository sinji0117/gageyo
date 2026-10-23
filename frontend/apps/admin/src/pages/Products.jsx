import { useState } from "react";
import { money, Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateText, qs } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

export default function Products() {
  const { toast } = useAdmin();
  const [status, setStatus] = useState("");
  const [q, setQ] = useState("");
  const [term, setTerm] = useState("");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/products${qs({ status, q: term, size: 300 })}`), [status, term]);
  async function change(p, next) {
    const reason = next === "blocked" ? window.prompt("판매 중지 사유 (작업 기록에 남습니다)", "") : null;
    if (next === "blocked" && !reason) return;
    try {
      await aapi(`/admin/products/${p.id}`, { method: "PATCH", body: { status: next, reason } });
      toast("상품 상태를 바꿨습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>상품</h1>
          <p>판매 중지(blocked)한 상품은 업체가 다시 열 수 없습니다. 재고가 0이면 자동으로 품절입니다.</p>
        </div>
      </div>
      <Tabs
        tabs={[
          ["", "전체"],
          ["active", "판매 중"],
          ["sold_out", "품절"],
          ["draft", "준비 중"],
          ["archived", "숨김"],
          ["blocked", "판매 중지"],
        ]}
        value={status}
        onChange={setStatus}
      />
      <form className="search-row" onSubmit={(e) => { e.preventDefault(); setTerm(q); }}>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="상품명·업체명" />
        <button className="gy-button ghost small">검색</button>
      </form>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>상품이 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th />
                <th>상품</th>
                <th>업체</th>
                <th>가격</th>
                <th>재고·판매</th>
                <th>상태</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {data.map((p) => (
                <tr key={p.id}>
                  <td>
                    <div className="thumb" style={p.image_url ? { backgroundImage: `url(${p.image_url})` } : undefined} />
                  </td>
                  <td>
                    <b>{p.name}</b>
                    <small>
                      {p.category} · {dateText(p.created_at)}
                    </small>
                  </td>
                  <td>{p.workshop_name}</td>
                  <td>{money(p.price)}</td>
                  <td>
                    {p.stock} · {p.sold}
                  </td>
                  <td>
                    <Badge kind="product" value={p.status} />
                  </td>
                  <td>
                    {p.status === "blocked" ? (
                      <button type="button" className="gy-button ghost small" onClick={() => change(p, "active")}>
                        판매 재개
                      </button>
                    ) : (
                      <button type="button" className="gy-button danger small" onClick={() => change(p, "blocked")}>
                        판매 중지
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
