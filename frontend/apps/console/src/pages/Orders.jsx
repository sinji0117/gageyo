import { useEffect, useState } from "react";
import { api, money, Modal, Field, Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateTimeText, statusLabel } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

const VIEWS = [
  ["todo", "처리할 주문"],
  ["shipped", "발송됨"],
  ["done", "완료"],
  ["cancelled", "취소"],
  ["all", "전체"],
];

/** 우리 업체 상품 줄만 보입니다. 예전에는 다른 업체 상품까지 합친 주문 전체 금액이 보였고 배송 처리를 할 수 없었습니다. */
export default function Orders() {
  const { toast, loadCounts } = usePartner();
  const [view, setView] = useState("todo");
  const { data, loading, error, reload } = useLoad(() => papi(`/partner/orders?view=${view}`), [view]);
  const [shipping, setShipping] = useState(null);
  const [carriers, setCarriers] = useState([]);
  useEffect(() => {
    api("/config").then((c) => setCarriers(c.carriers || [])).catch(() => {});
  }, []);

  async function setStatus(item, status, extra = {}) {
    try {
      await papi(`/partner/order-items/${item.item_id}`, { method: "PATCH", body: { status, ...extra } });
      toast(status === "shipped" ? "발송 처리했습니다. 고객에게 알림이 갑니다." : "상태를 바꿨습니다.");
      setShipping(null);
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
          <h2>주문·배송</h2>
          <p>결제가 끝난 주문만 보입니다. 상품을 준비해 택배사·운송장 번호를 넣고 발송 처리해 주세요.</p>
        </div>
        <button type="button" onClick={reload}>
          ↻ 새로고침
        </button>
      </div>
      <Tabs tabs={VIEWS} value={view} onChange={setView} />
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>주문이 없습니다.</Empty>}
      <ul className="order-list">
        {data?.map((it) => (
          <li key={it.item_id}>
            <div>
              <b>
                {it.product_name} × {it.quantity}
              </b>
              <small>
                #{it.order_id.slice(0, 8).toUpperCase()} · {dateTimeText(it.paid_at || it.created_at)} · {statusLabel("payment", it.payment_method)}
              </small>
            </div>
            <div>
              <b>{money(it.amount)}</b>
              {it.fee_amount > 0 && <small>수수료 {money(it.fee_amount)}</small>}
            </div>
            <div className="who">
              <b>
                {it.customer_name} · <a href={`tel:${it.customer_phone}`}>{it.customer_phone}</a>
              </b>
              <small>{it.fulfillment === "visit" ? `방문 장착 ${dateTimeText(it.visit_at)} · ${it.vehicle || ""}` : it.address}</small>
              {it.memo && <small>요청: {it.memo}</small>}
            </div>
            <div className="gy-actions">
              <Badge kind="item" value={it.item_status} />
              {it.tracking_url && (
                <a href={it.tracking_url} target="_blank" rel="noreferrer" className="small-link">
                  {it.carrier_name} {it.tracking_no}
                </a>
              )}
              {["pending", "ordered"].includes(it.item_status) && it.order_status !== "cancelled" && (
                <button type="button" className="gy-button ghost small" onClick={() => setStatus(it, "preparing")}>
                  준비 중으로
                </button>
              )}
              {["pending", "ordered", "preparing"].includes(it.item_status) && it.fulfillment === "delivery" && (
                <button type="button" className="gy-button small" onClick={() => setShipping(it)}>
                  발송 처리
                </button>
              )}
              {["pending", "ordered", "preparing"].includes(it.item_status) && it.fulfillment === "visit" && (
                <button type="button" className="gy-button small" onClick={() => setStatus(it, "delivered")}>
                  장착 완료
                </button>
              )}
              {it.item_status === "shipped" && (
                <button type="button" className="gy-button ghost small" onClick={() => setStatus(it, "delivered")}>
                  배송 완료
                </button>
              )}
            </div>
          </li>
        ))}
      </ul>
      {shipping && <ShipForm item={shipping} carriers={carriers} onClose={() => setShipping(null)} onSubmit={(extra) => setStatus(shipping, "shipped", extra)} />}
    </section>
  );
}

const LAST_CARRIER = "gageyo_last_carrier";

function ShipForm({ item, carriers, onClose, onSubmit }) {
  const [carrier, setCarrier] = useState(() => {
    try {
      return window.localStorage.getItem(LAST_CARRIER) || "cj";
    } catch {
      return "cj";
    }
  });
  const [trackingNo, setTrackingNo] = useState("");
  return (
    <Modal title="발송 처리" eyebrow={`${item.product_name} × ${item.quantity}`} onClose={onClose}>
      <form
        className="gy-form"
        onSubmit={(e) => {
          e.preventDefault();
          try {
            window.localStorage.setItem(LAST_CARRIER, carrier);
          } catch {
            /* 무시 */
          }
          onSubmit({ carrier, tracking_no: trackingNo });
        }}
      >
        <p className="gy-muted">{item.address}</p>
        <Field label="택배사">
          <select value={carrier} onChange={(e) => setCarrier(e.target.value)}>
            {carriers.map((c) => (
              <option key={c.code} value={c.code}>
                {c.name}
              </option>
            ))}
          </select>
        </Field>
        <Field label="운송장 번호" hint={carrier === "direct" ? "직접 배송은 비워도 됩니다" : "숫자·영문·- 만"}>
          <input value={trackingNo} required={carrier !== "direct"} onChange={(e) => setTrackingNo(e.target.value)} inputMode="numeric" />
        </Field>
        <button className="gy-button block">발송 처리</button>
      </form>
    </Modal>
  );
}
