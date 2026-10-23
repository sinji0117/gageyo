import { useState } from "react";
import { money, Modal, Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateTimeText, qs, statusLabel } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

export default function Orders() {
  const { tabParams, toast, refreshCounts } = useAdmin();
  const [status, setStatus] = useState(tabParams.status || "");
  const [method, setMethod] = useState(tabParams.method || "");
  const [q, setQ] = useState("");
  const [term, setTerm] = useState("");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/orders${qs({ status, method, q: term, size: 200 })}`), [status, method, term]);
  const [detail, setDetail] = useState(null);

  const after = () => {
    reload();
    refreshCounts();
  };
  return (
    <>
      <div className="page-title">
        <div>
          <h1>주문</h1>
          <p>무통장입금은 통장에서 입금을 확인한 뒤 [입금 확인]을 누르세요. 카드 결제 취소는 토스에서 자동 환불됩니다.</p>
        </div>
      </div>
      <Tabs
        tabs={[
          ["", "전체"],
          ["payment_pending", "결제 대기"],
          ["paid", "결제 완료"],
          ["preparing", "준비 중"],
          ["shipped", "발송"],
          ["delivered", "배송 완료"],
          ["cancelled", "취소"],
          ["refunded", "환불"],
        ]}
        value={status}
        onChange={setStatus}
      />
      <form className="search-row" onSubmit={(e) => { e.preventDefault(); setTerm(q); }}>
        <select value={method} onChange={(e) => setMethod(e.target.value)} aria-label="결제 수단">
          <option value="">모든 결제 수단</option>
          <option value="toss">카드·간편결제</option>
          <option value="bank">무통장입금</option>
          <option value="none">직접 결제</option>
        </select>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="주문번호 앞자리·주문자·연락처·입금자명" />
        <button className="gy-button ghost small">검색</button>
      </form>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>주문이 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>주문</th>
                <th>주문자</th>
                <th>업체</th>
                <th>금액</th>
                <th>결제</th>
                <th>상태</th>
              </tr>
            </thead>
            <tbody>
              {data.map((o) => (
                <tr key={o.id} className="clickable" onClick={() => setDetail(o.id)}>
                  <td>
                    <b>#{o.id.slice(0, 8).toUpperCase()}</b>
                    <small>{dateTimeText(o.created_at)}</small>
                  </td>
                  <td>
                    {o.customer_name}
                    <small>{o.customer_phone}</small>
                  </td>
                  <td>
                    {o.workshops}
                    <small>
                      {o.item_count}개 상품 · {o.fulfillment === "visit" ? "방문 장착" : "택배"}
                    </small>
                  </td>
                  <td>{money(o.total_amount)}</td>
                  <td>
                    {statusLabel("payment", o.payment_method)}
                    {o.depositor_name && <small>입금자 {o.depositor_name}</small>}
                  </td>
                  <td>
                    <Badge kind="order" value={o.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {detail && <OrderDetail id={detail} onClose={() => setDetail(null)} onChanged={after} toast={toast} />}
    </>
  );
}

function OrderDetail({ id, onClose, onChanged, toast }) {
  const { data: o, loading, error, reload } = useLoad(() => aapi(`/admin/orders/${id}`), [id]);
  async function act(fn, done) {
    try {
      const r = await fn();
      toast(done(r));
      reload();
      onChanged();
    } catch (e) {
      toast(e.message);
    }
  }
  const confirmDeposit = () => {
    if (!window.confirm(`입금자 ${o.depositor_name} · ${money(o.total_amount)} 입금을 확인했나요?`)) return;
    act(() => aapi(`/admin/orders/${id}/confirm-deposit`, { method: "POST" }), () => "결제 완료로 바꿨습니다. 업체에 새 주문 알림이 갑니다.");
  };
  const cancel = () => {
    const reason = window.prompt(o.payment_method === "toss" && o.status !== "payment_pending" ? "취소 사유 (카드 결제는 바로 환불됩니다)" : "취소 사유 (고객에게 안내됩니다)", "");
    if (!reason) return;
    act(
      () => aapi(`/admin/orders/${id}/cancel`, { method: "POST", body: { reason } }),
      (r) => (r.manual_refund_needed ? "취소했습니다. 무통장 입금액은 고객 계좌로 직접 돌려주세요." : r.status === "refunded" ? "취소하고 환불했습니다." : "취소했습니다."),
    );
  };
  const markPaid = () => {
    if (!window.confirm("업체와 직접 결제한 주문을 결제 완료로 바꿀까요?")) return;
    act(() => aapi(`/admin/orders/${id}`, { method: "PATCH", body: { status: "paid" } }), () => "결제 완료로 바꿨습니다.");
  };
  return (
    <Modal title={`주문 #${id.slice(0, 8).toUpperCase()}`} eyebrow="주문 상세" onClose={onClose} wide>
      {loading && !o && <Loading />}
      <ErrorBox error={error} onRetry={reload} />
      {o && (
        <div className="detail">
          <div className="detail-actions">
            <Badge kind="order" value={o.status} />
            <b>{money(o.total_amount)}</b>
            {o.status === "payment_pending" && o.payment_method === "bank" && (
              <button type="button" className="gy-button small" onClick={confirmDeposit}>
                입금 확인
              </button>
            )}
            {o.status === "payment_pending" && o.payment_method === "none" && (
              <button type="button" className="gy-button small" onClick={markPaid}>
                결제 완료로
              </button>
            )}
            {o.cancellable && (
              <button type="button" className="gy-button danger small" onClick={cancel}>
                주문 취소{o.status !== "payment_pending" && o.payment_method === "toss" ? "·환불" : ""}
              </button>
            )}
          </div>
          <dl className="kv">
            <dt>주문자</dt>
            <dd>
              {o.customer_name} · {o.customer_phone} {o.email && `· ${o.email}`} {o.customer_user_id ? "(회원)" : "(비회원)"}
            </dd>
            <dt>받는 방법</dt>
            <dd>{o.fulfillment === "visit" ? `방문 장착 ${dateTimeText(o.visit_at)} · ${o.vehicle || ""}` : o.address}</dd>
            {o.memo && (
              <>
                <dt>요청</dt>
                <dd>{o.memo}</dd>
              </>
            )}
            <dt>결제</dt>
            <dd>
              {statusLabel("payment", o.payment_method)}
              {o.pay_detail && ` · ${o.pay_detail}`}
              {o.depositor_name && ` · 입금자 ${o.depositor_name}`}
              {o.paid_at && ` · ${dateTimeText(o.paid_at)} 결제`}
            </dd>
            {o.cancel_reason && (
              <>
                <dt>취소</dt>
                <dd>
                  {o.cancel_reason} · {dateTimeText(o.cancelled_at)}
                </dd>
              </>
            )}
            {o.booking && (
              <>
                <dt>방문 예약</dt>
                <dd>
                  <Badge kind="booking" value={o.booking.status} />
                </dd>
              </>
            )}
          </dl>
          <table className="gy-table">
            <thead>
              <tr>
                <th>상품</th>
                <th>업체</th>
                <th>수량</th>
                <th>금액</th>
                <th>상태</th>
              </tr>
            </thead>
            <tbody>
              {o.items.map((it) => (
                <tr key={it.id}>
                  <td>{it.product_name}</td>
                  <td>{it.workshop_name}</td>
                  <td>{it.quantity}</td>
                  <td>{money(it.amount)}</td>
                  <td>
                    <Badge kind="item" value={it.status} />
                    {it.tracking_url ? (
                      <a className="small-link" href={it.tracking_url} target="_blank" rel="noreferrer">
                        {" "}
                        {it.carrier_name} {it.tracking_no}
                      </a>
                    ) : (
                      it.carrier_name && <small>{it.carrier_name}</small>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Modal>
  );
}
