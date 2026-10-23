import { useState } from "react";
import { api, Modal, Field, Tabs, Badge, dateTimeText, dateText } from "@gageyo/ui";
import { useApp } from "../context.js";
import { savedQuotes } from "./QuoteDialog.jsx";

/** 비회원 조회: 주문·예약·견적 (번호 + 연락처) */
export default function LookupDialog({ tab: initialTab = "order" }) {
  const { close, open, toast, user, go } = useApp();
  const [tab, setTab] = useState(initialTab);
  const [form, setForm] = useState({ id: "", phone: "" });
  const [booking, setBooking] = useState(null);
  const [error, setError] = useState("");
  const quotes = savedQuotes();

  async function submit(e) {
    e.preventDefault();
    setError("");
    const id = form.id.trim();
    if (tab === "order") return open({ type: "order", id, phone: form.phone });
    if (tab === "quote") return open({ type: "quoteView", id, phone: form.phone });
    try {
      setBooking(await api("/bookings/lookup", { method: "POST", body: { booking_id: id, phone: form.phone } }));
    } catch (err) {
      setError(err.message);
    }
    return undefined;
  }
  async function cancelBooking() {
    if (!window.confirm("예약을 취소할까요?")) return;
    try {
      await api(`/bookings/${booking.id}/cancel`, { method: "POST", body: { phone: form.phone } });
      toast("예약을 취소했습니다.");
      setBooking({ ...booking, status: "cancelled" });
    } catch (err) {
      toast(err.message);
    }
  }

  return (
    <Modal title="주문·예약 조회" eyebrow="비회원 조회" onClose={close}>
      {user && (
        <p className="gy-note">
          로그인한 회원은{" "}
          <button type="button" className="link-button" onClick={() => { close(); go("my"); }}>
            마이페이지
          </button>
          에서 모두 볼 수 있습니다.
        </p>
      )}
      <Tabs
        tabs={[
          ["order", "주문"],
          ["booking", "예약"],
          ["quote", "견적"],
        ]}
        value={tab}
        onChange={(t) => {
          setTab(t);
          setBooking(null);
          setError("");
        }}
      />
      <form className="gy-form" onSubmit={submit}>
        <Field label={{ order: "주문 번호", booking: "예약 번호", quote: "견적 요청 번호" }[tab]} required>
          <input required value={form.id} onChange={(e) => setForm({ ...form, id: e.target.value })} placeholder="메일·완료 화면에서 받은 번호" />
        </Field>
        <Field label="연락처" required>
          <input required type="tel" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} placeholder="010-0000-0000" />
        </Field>
        {error && <div className="gy-error">{error}</div>}
        <button className="gy-button block">조회하기</button>
      </form>
      {tab === "booking" && booking && (
        <div className="lookup-result">
          <Badge kind="booking" value={booking.status} /> <b>{booking.workshop_name}</b>
          <p>
            {dateTimeText(booking.starts_at)} · {booking.vehicle}
            {booking.service_name ? ` · ${booking.service_name}` : ""}
          </p>
          <p className="gy-muted">
            {booking.workshop_address} · <a href={`tel:${booking.workshop_phone}`}>{booking.workshop_phone}</a>
          </p>
          {booking.decline_reason && <p className="gy-muted">사유: {booking.decline_reason}</p>}
          {["requested", "confirmed"].includes(booking.status) && (
            <button type="button" className="gy-button danger small" onClick={cancelBooking}>
              예약 취소
            </button>
          )}
        </div>
      )}
      {tab === "quote" && quotes.length > 0 && (
        <div className="lookup-result">
          <b>이 브라우저에서 요청한 견적</b>
          <ul className="plain-list">
            {quotes.map((q) => (
              <li key={q.id}>
                <button type="button" className="link-button" onClick={() => open({ type: "quoteView", id: q.id, phone: q.phone })}>
                  {dateText(q.at)} 요청 · {q.id.slice(0, 8)} →
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Modal>
  );
}
