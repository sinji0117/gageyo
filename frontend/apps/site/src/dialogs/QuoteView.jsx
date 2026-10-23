import { useCallback, useEffect, useState } from "react";
import { api, money, Modal, Field, Loading, Badge, dateText, dateTimeText } from "@gageyo/ui";
import { useApp } from "../context.js";
import { savedQuotes } from "./QuoteDialog.jsx";

/** 견적 요청과 받은 견적 비교·선택 */
export default function QuoteView({ id, phone: givenPhone = "", fresh = false }) {
  const { close, open, toast } = useApp();
  const stored = savedQuotes().find((q) => q.id === id)?.phone || "";
  const [phone, setPhone] = useState(givenPhone || stored);
  const [quote, setQuote] = useState(null);
  const [needPhone, setNeedPhone] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const load = useCallback(
    async (p = phone) => {
      setLoading(true);
      setError("");
      try {
        const q = await api(`/quotes/${encodeURIComponent(id)}${p ? `?phone=${encodeURIComponent(p)}` : ""}`);
        setQuote(q);
        setNeedPhone(false);
      } catch (e) {
        if (e.status === 403) setNeedPhone(true);
        else setError(e.message);
      } finally {
        setLoading(false);
      }
    },
    [id, phone],
  );
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  async function accept(offer) {
    if (!window.confirm(`${offer.workshop_name}의 ${money(offer.amount)} 견적을 고를까요? 다른 견적은 선택 안 됨으로 바뀝니다.`)) return;
    try {
      const r = await api(`/quotes/${id}/accept`, { method: "POST", body: { offer_id: offer.id, phone } });
      toast("견적을 골랐습니다. 방문 시간을 정해 예약해 주세요.");
      open({
        type: "booking",
        workshopId: r.workshop_id,
        workshopName: r.workshop_name,
        quoteOfferId: r.offer_id,
        vehicle: r.vehicle,
        name: r.customer_name || "",
        phone: r.customer_phone,
      });
    } catch (e) {
      toast(e.message);
    }
  }
  async function cancel() {
    if (!window.confirm("견적 요청을 취소할까요? 업체들이 더 이상 견적을 보낼 수 없습니다.")) return;
    try {
      await api(`/quotes/${id}/cancel`, { method: "POST", body: { phone } });
      toast("견적 요청을 취소했습니다.");
      load();
    } catch (e) {
      toast(e.message);
    }
  }

  return (
    <Modal title={fresh ? "견적 요청 완료" : "받은 견적"} eyebrow="견적 비교" onClose={close} wide>
      {loading && !quote && <Loading />}
      {error && <div className="gy-error">{error}</div>}
      {needPhone && (
        <form
          className="gy-form"
          onSubmit={(e) => {
            e.preventDefault();
            load(phone);
          }}
        >
          <p className="gy-muted">견적 요청할 때 적은 연락처를 넣어 주세요.</p>
          <Field label="연락처">
            <input required type="tel" value={phone} onChange={(e) => setPhone(e.target.value)} placeholder="010-0000-0000" />
          </Field>
          <button className="gy-button">확인</button>
        </form>
      )}
      {quote && (
        <div className="quote-view">
          {fresh && <p className="gy-note">요청을 보냈습니다. 업체 견적이 도착하면 {quote.customer_user_id ? "메일로 알려 드리고 마이페이지에 쌓입니다" : "이 화면이나 [주문·예약 조회 → 견적]에서 확인할 수 있습니다"}.</p>}
          <div className="quote-summary">
            <div>
              <Badge kind="quote" value={quote.status} /> <b>{quote.service_category}</b> · {quote.district}
              <p>
                {quote.vehicle} — {quote.description}
              </p>
              <small className="gy-muted">
                요청 {dateText(quote.created_at)} · 번호 <span className="mono">{quote.id}</span>
              </small>
            </div>
            <div className="gy-actions">
              <button type="button" className="gy-button ghost small" onClick={() => load()}>
                ↻ 새로고침
              </button>
              {quote.status === "open" && (
                <button type="button" className="gy-button danger small" onClick={cancel}>
                  요청 취소
                </button>
              )}
            </div>
          </div>
          <h3 className="sub">
            받은 견적 <small>{quote.offers.length}건</small>
          </h3>
          {quote.offers.length === 0 && <p className="gy-muted">아직 도착한 견적이 없습니다. 보통 하루 안에 업체들이 답합니다.</p>}
          <ul className="offer-list">
            {quote.offers.map((o) => (
              <li key={o.id} className={o.status === "accepted" ? "chosen" : ""}>
                <div>
                  <button type="button" className="link-button strong" onClick={() => open({ type: "workshop", id: o.workshop_id })}>
                    {o.workshop_name}
                  </button>
                  <small>
                    {o.workshop_district} · ★ {Number(o.workshop_rating).toFixed(1)} ({o.workshop_review_count})
                  </small>
                  <p>{o.description}</p>
                  {o.valid_until && <small className="gy-muted">유효 기간 {dateTimeText(o.valid_until)}</small>}
                </div>
                <div className="offer-side">
                  <b>{money(o.amount)}</b>
                  {quote.status === "open" && o.status === "sent" ? (
                    <button type="button" className="gy-button small" onClick={() => accept(o)}>
                      이 견적 선택
                    </button>
                  ) : (
                    <Badge kind="offer" value={o.status} />
                  )}
                  {o.status === "accepted" && (
                    <button
                      type="button"
                      className="gy-button ghost small"
                      onClick={() =>
                        open({ type: "booking", workshopId: o.workshop_id, workshopName: o.workshop_name, quoteOfferId: o.id, vehicle: quote.vehicle, name: quote.customer_name || "", phone: quote.customer_phone })
                      }
                    >
                      예약하기
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Modal>
  );
}
