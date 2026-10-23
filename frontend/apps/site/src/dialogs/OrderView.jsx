import { useCallback, useEffect, useState } from "react";
import { api, money, Modal, Field, Loading, Badge, dateTimeText, statusLabel } from "@gageyo/ui";
import { useApp } from "../context.js";
import { requestTossPayment, rememberOrderPhone } from "../toss.js";

/** 주문 상세 (회원은 바로, 비회원은 연락처 확인) */
export default function OrderView({ id, phone: givenPhone = "", fresh = false }) {
  const { close, user, toast, open } = useApp();
  const [phone, setPhone] = useState(givenPhone);
  const [order, setOrder] = useState(null);
  const [needPhone, setNeedPhone] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const load = useCallback(
    async (p = phone) => {
      setLoading(true);
      setError("");
      try {
        let o = null;
        if (user) o = await api(`/me/orders/${encodeURIComponent(id)}`).catch(() => null);
        if (!o && p) o = await api("/orders/lookup", { method: "POST", body: { order_id: id, phone: p } });
        if (!o) {
          setNeedPhone(true);
          return;
        }
        setOrder(o);
        setNeedPhone(false);
      } catch (e) {
        setError(e.message);
        setNeedPhone(true);
      } finally {
        setLoading(false);
      }
    },
    [id, phone, user],
  );
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  async function cancel() {
    if (!window.confirm("주문을 취소할까요?")) return;
    try {
      await api(`/orders/${id}/cancel`, { method: "POST", body: { phone } });
      toast("주문을 취소했습니다.");
      load();
    } catch (e) {
      toast(e.message);
    }
  }
  async function payAgain() {
    try {
      rememberOrderPhone(id, phone || order.customer_phone);
      await requestTossPayment({ ...order.toss, customer_key: user?.id });
    } catch (e) {
      toast(e?.message || "결제를 마치지 않았습니다.");
    }
  }
  async function copy(text) {
    try {
      await navigator.clipboard.writeText(text);
      toast("복사했습니다.");
    } catch {
      /* 무시 */
    }
  }

  return (
    <Modal title={fresh ? "주문 완료" : "주문 상세"} eyebrow={`주문번호 ${id.slice(0, 8).toUpperCase()}`} onClose={close}>
      {loading && !order && <Loading />}
      {needPhone && !order && (
        <form
          className="gy-form"
          onSubmit={(e) => {
            e.preventDefault();
            load(phone);
          }}
        >
          <p className="gy-muted">주문할 때 적은 연락처를 넣어 주세요.</p>
          <Field label="연락처">
            <input required type="tel" value={phone} onChange={(e) => setPhone(e.target.value)} />
          </Field>
          {error && <div className="gy-error">{error}</div>}
          <button className="gy-button">주문 보기</button>
        </form>
      )}
      {order && (
        <div className="order-view">
          <div className="order-status">
            <Badge kind="order" value={order.status} />
            <span>{dateTimeText(order.created_at)} 주문</span>
            <b>{money(order.total_amount)}</b>
          </div>
          {order.status === "payment_pending" && order.payment_method === "bank" && order.bank && (
            <div className="gy-note">
              <b>입금 계좌</b>
              <p>
                {order.bank.bank_name} {order.bank.account} (예금주 {order.bank.holder}){" "}
                <button type="button" className="link-button small" onClick={() => copy(order.bank.account)}>
                  복사
                </button>
                <br />
                입금자명 <b>{order.depositor_name}</b> · {money(order.total_amount)} · {order.bank.due_hours}시간 안에 입금해 주세요.
              </p>
            </div>
          )}
          {order.status === "payment_pending" && order.payment_method === "toss" && order.toss && (
            <div className="gy-note warn">
              아직 결제가 끝나지 않았습니다.{" "}
              <button type="button" className="gy-button small" onClick={payAgain}>
                결제하기
              </button>
            </div>
          )}
          <ul className="order-items">
            {order.items.map((it) => (
              <li key={it.id}>
                <div>
                  <b>{it.product_name}</b>
                  <small>
                    {it.workshop_name} · {it.quantity}개
                  </small>
                </div>
                <span>{money(it.amount)}</span>
                <div className="item-status">
                  <Badge kind="item" value={it.status} />
                  {it.tracking_url && (
                    <a href={it.tracking_url} target="_blank" rel="noreferrer">
                      {it.carrier_name} 배송 조회
                    </a>
                  )}
                  {!it.tracking_url && it.carrier_name && <small>{it.carrier_name}</small>}
                </div>
              </li>
            ))}
          </ul>
          <dl className="info-list">
            <dt>받는 방법</dt>
            <dd>{order.fulfillment === "visit" ? `업체 방문 장착 · ${dateTimeText(order.visit_at)}` : order.address}</dd>
            {order.booking && (
              <>
                <dt>방문 예약</dt>
                <dd>
                  <Badge kind="booking" value={order.booking.status} />
                </dd>
              </>
            )}
            <dt>결제</dt>
            <dd>
              {statusLabel("payment", order.payment_method)}
              {order.pay_detail ? ` · ${order.pay_detail}` : ""}
            </dd>
            {order.cancel_reason && (
              <>
                <dt>취소 사유</dt>
                <dd>{order.cancel_reason}</dd>
              </>
            )}
          </dl>
          {order.status === "refunded" && order.payment_method === "toss" && <p className="gy-muted small">카드 결제 취소는 카드사에 따라 3~5영업일 걸릴 수 있습니다.</p>}
          <div className="cta-row">
            {order.cancellable && (
              <button type="button" className="gy-button danger" onClick={cancel}>
                주문 취소
              </button>
            )}
            {!order.cancellable && !["cancelled", "refunded"].includes(order.status) && (
              <button type="button" className="gy-button ghost" onClick={() => open({ type: "support", subject: `주문 문의 (${id.slice(0, 8).toUpperCase()})` })}>
                취소·교환 문의
              </button>
            )}
            <button type="button" className="gy-button ghost" onClick={close}>
              닫기
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}
