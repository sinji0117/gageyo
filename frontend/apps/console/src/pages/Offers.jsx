import { money, Badge, Empty, Loading, ErrorBox, useLoad, dateText, statusLabel } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

export default function Offers() {
  const { toast } = usePartner();
  const { data, loading, error, reload } = useLoad(() => papi("/partner/offers"), []);
  async function withdraw(o) {
    if (!window.confirm("이 견적을 철회할까요? 고객 화면에서 사라집니다.")) return;
    try {
      await papi(`/partner/offers/${o.id}`, { method: "DELETE" });
      toast("견적을 철회했습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <section className="panel">
      <div className="panel-title">
        <div>
          <h2>보낸 견적</h2>
          <p>고객이 우리 견적을 고르면 연락처가 보이고, 고객이 예약을 신청하면 [예약 일정]에 들어옵니다.</p>
        </div>
        <button type="button" onClick={reload}>
          ↻ 새로고침
        </button>
      </div>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>보낸 견적이 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>요청</th>
                <th>금액</th>
                <th>고객</th>
                <th>상태</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {data.map((o) => (
                <tr key={o.id}>
                  <td>
                    <b>{o.vehicle}</b>
                    <small>
                      {o.service_category} · {o.district} · {dateText(o.created_at)}
                    </small>
                  </td>
                  <td>{money(o.amount)}</td>
                  <td>
                    {o.customer_name || "-"}
                    <small>{o.customer_phone}</small>
                  </td>
                  <td>
                    <Badge kind="offer" value={o.status} />
                    <small>요청: {statusLabel("quote", o.quote_status)}</small>
                  </td>
                  <td>
                    {o.status === "sent" && (
                      <button type="button" className="gy-button ghost small" onClick={() => withdraw(o)}>
                        철회
                      </button>
                    )}
                    {o.status === "accepted" && (
                      <a className="gy-button small" href={`tel:${o.customer_phone}`}>
                        전화
                      </a>
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
