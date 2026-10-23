import { useEffect, useState } from "react";
import { api, money, Modal, Field, Empty, todayIso, toLocalIso, statusLabel } from "@gageyo/ui";
import { useApp } from "../context.js";
import { Cover } from "../components/WorkshopCard.jsx";
import { requestTossPayment, rememberOrderPhone } from "../toss.js";

/** 장바구니 → 주문서 → 결제 */
export default function CartDialog() {
  const { close, cart, user, open, toast } = useApp();
  const [step, setStep] = useState("cart");
  const [checkout, setCheckout] = useState(null);
  const [form, setForm] = useState({
    fulfillment: "delivery",
    customer_name: user?.display_name || "",
    customer_phone: user?.phone || "",
    email: user ? "" : "",
    address: "",
    visit_date: todayIso(1),
    visit_time: "10:00",
    vehicle: "",
    memo: "",
    payment_method: "",
    depositor_name: "",
    agree: false,
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    api("/checkout/config")
      .then((c) => {
        setCheckout(c);
        setForm((f) => ({ ...f, payment_method: c.toss ? "toss" : c.bank ? "bank" : "none" }));
      })
      .catch(() => setCheckout({ none_allowed: true }));
  }, []);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.type === "checkbox" ? e.target.checked : e.target.value });
  const singleWorkshop = cart.workshops.length === 1;

  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      const body = {
        customer_name: form.customer_name,
        customer_phone: form.customer_phone,
        email: form.email || undefined,
        fulfillment: form.fulfillment,
        address: form.address,
        vehicle: form.vehicle,
        visit_at: form.fulfillment === "visit" ? toLocalIso(form.visit_date, form.visit_time) : undefined,
        memo: form.memo,
        payment_method: form.payment_method,
        depositor_name: form.depositor_name || undefined,
        items: cart.items.map((i) => ({ product_id: i.id, quantity: i.quantity })),
      };
      const order = await api("/orders", { method: "POST", body });
      rememberOrderPhone(order.id, form.customer_phone);
      if (order.payment_method === "toss") {
        try {
          await requestTossPayment(order.toss);
          // 결제창이 다른 주소로 이동하므로 여기까지 오지 않는 것이 보통입니다.
        } catch (payErr) {
          toast(payErr?.message || "결제를 마치지 않았습니다. 주문 조회에서 다시 결제할 수 있습니다.");
          open({ type: "order", id: order.id, phone: form.customer_phone });
        }
        return;
      }
      cart.clear();
      toast(order.payment_method === "bank" ? "주문을 받았습니다. 안내된 계좌로 입금해 주세요." : "주문을 받았습니다.");
      open({ type: "order", id: order.id, phone: form.customer_phone, fresh: true });
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  if (!cart.items.length) {
    return (
      <Modal title="장바구니" onClose={close}>
        <Empty>장바구니가 비어 있습니다.</Empty>
      </Modal>
    );
  }

  if (step === "cart") {
    return (
      <Modal
        title="장바구니"
        eyebrow={`${cart.count}개`}
        onClose={close}
        footer={
          <>
            <span className="foot-total">합계 {money(cart.total)}</span>
            <button type="button" className="gy-button" onClick={() => setStep("checkout")}>
              주문서 작성
            </button>
          </>
        }
      >
        <ul className="cart-list">
          {cart.items.map((i) => (
            <li key={i.id}>
              <Cover url={i.image_url} name={i.name} className="mini-photo" />
              <div>
                <b>{i.name}</b>
                <small>{i.workshop_name}</small>
                <span>{money(i.price)}</span>
              </div>
              <div className="qty">
                <button type="button" aria-label="하나 빼기" onClick={() => cart.setQuantity(i.id, i.quantity - 1)} disabled={i.quantity <= 1}>
                  −
                </button>
                <span>{i.quantity}</span>
                <button type="button" aria-label="하나 더" onClick={() => cart.setQuantity(i.id, i.quantity + 1)}>
                  ＋
                </button>
              </div>
              <button type="button" className="link-button small" onClick={() => cart.remove(i.id)}>
                삭제
              </button>
            </li>
          ))}
        </ul>
        {cart.workshops.length > 1 && <p className="gy-muted small">여러 업체 상품은 업체마다 따로 보내 드립니다.</p>}
      </Modal>
    );
  }

  const methods = [
    checkout?.toss && ["toss", "카드·간편결제"],
    checkout?.bank && ["bank", "무통장입금"],
    checkout?.none_allowed && ["none", "업체와 직접 결제 (주문만 접수)"],
  ].filter(Boolean);

  return (
    <Modal title="주문서" eyebrow={`${cart.count}개 · ${money(cart.total)}`} onClose={close}>
      <form className="gy-form" onSubmit={submit}>
        <Field label="받는 방법">
          <div className="segmented">
            <button type="button" className={form.fulfillment === "delivery" ? "on" : ""} onClick={() => setForm({ ...form, fulfillment: "delivery" })}>
              택배로 받기
            </button>
            <button
              type="button"
              disabled={!singleWorkshop}
              title={singleWorkshop ? "" : "방문 장착은 한 업체 상품만 함께 주문할 수 있어요"}
              className={form.fulfillment === "visit" ? "on" : ""}
              onClick={() => setForm({ ...form, fulfillment: "visit" })}
            >
              업체 방문 장착
            </button>
          </div>
        </Field>
        {form.fulfillment === "delivery" ? (
          <Field label="배송 주소" required>
            <input required maxLength={500} value={form.address} onChange={set("address")} placeholder="도로명 주소와 상세 주소" autoComplete="street-address" />
          </Field>
        ) : (
          <>
            <p className="gy-note">{cart.items[0].workshop_name}에 방문 예약이 함께 신청됩니다. 업체가 시간을 확정하면 알려 드려요.</p>
            <div className="gy-row">
              <Field label="방문 날짜" required>
                <input type="date" required min={todayIso(0)} max={todayIso(89)} value={form.visit_date} onChange={set("visit_date")} />
              </Field>
              <Field label="시간" required>
                <input type="time" required value={form.visit_time} onChange={set("visit_time")} step="1800" />
              </Field>
            </div>
            <Field label="차량" required>
              <input required maxLength={160} value={form.vehicle} onChange={set("vehicle")} placeholder="예: 기아 K5 2020년식" />
            </Field>
          </>
        )}
        <div className="gy-row">
          <Field label="주문자" required>
            <input required maxLength={100} value={form.customer_name} onChange={set("customer_name")} autoComplete="name" />
          </Field>
          <Field label="연락처" required>
            <input required type="tel" value={form.customer_phone} onChange={set("customer_phone")} autoComplete="tel" />
          </Field>
        </div>
        {!user && (
          <Field label="이메일" hint="주문·발송 알림을 받을 주소 (선택)">
            <input type="email" value={form.email} onChange={set("email")} autoComplete="email" />
          </Field>
        )}
        <Field label="요청 사항">
          <input maxLength={500} value={form.memo} onChange={set("memo")} placeholder="배송·장착 요청 사항" />
        </Field>
        <Field label="결제 방법">
          <div className="radio-list">
            {methods.map(([v, l]) => (
              <label key={v} className={form.payment_method === v ? "on" : ""}>
                <input type="radio" name="pm" value={v} checked={form.payment_method === v} onChange={set("payment_method")} />
                {l}
              </label>
            ))}
          </div>
        </Field>
        {form.payment_method === "bank" && (
          <Field label="입금자명" hint="비우면 주문자 이름">
            <input maxLength={60} value={form.depositor_name} onChange={set("depositor_name")} />
          </Field>
        )}
        {form.payment_method === "none" && <p className="gy-note warn">지금은 온라인 결제가 설정되지 않아 주문만 접수됩니다. 업체가 연락해 결제를 안내합니다.</p>}
        <label className="gy-check">
          <input type="checkbox" required checked={form.agree} onChange={set("agree")} />
          주문 내용과 {statusLabel("payment", form.payment_method)} 결제에 동의합니다. 상품을 보내는 업체에 이름·연락처·주소가 전달됩니다.
        </label>
        {error && <div className="gy-error">{error}</div>}
        <div className="cta-row">
          <button type="button" className="gy-button ghost" onClick={() => setStep("cart")}>
            ← 장바구니
          </button>
          <button className="gy-button" disabled={busy}>
            {busy ? "처리 중…" : `${money(cart.total)} ${form.payment_method === "toss" ? "결제하기" : "주문하기"}`}
          </button>
        </div>
      </form>
    </Modal>
  );
}
