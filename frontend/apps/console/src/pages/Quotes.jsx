import { useState } from "react";
import { Modal, Field, Empty, Loading, ErrorBox, useLoad, relativeText, todayIso, money } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

/** 견적 요청 (예전에는 브라우저 prompt 창 두 개로 금액·내용을 받았음 → 정식 입력 창으로 바꿈) */
export default function Quotes() {
  const { toast, loadCounts } = usePartner();
  const { data, loading, error, reload } = useLoad(() => papi("/partner/quotes"), []);
  const [answering, setAnswering] = useState(null);

  return (
    <section className="panel">
      <div className="panel-title">
        <div>
          <h2>견적 요청</h2>
          <p>업체 분야와 지역이 맞고 아직 답하지 않은 요청입니다. 연락처는 고객이 우리 견적을 고르면 보입니다.</p>
        </div>
        <button type="button" onClick={reload}>
          ↻ 새로고침
        </button>
      </div>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>대기 중인 견적 요청이 없습니다. 분야를 넓히려면 [업체 정보]에서 변경을 요청하세요.</Empty>}
      <div className="quote-cards">
        {data?.map((q) => (
          <article key={q.id} className="quote-card">
            <header>
              <b>{q.vehicle}</b>
              <small>{relativeText(q.created_at)}</small>
            </header>
            <p className="chips">
              <span className="chip">{q.service_category}</span>
              <span className="chip">{q.district}</span>
              {q.offer_count > 0 && <span className="chip warn">다른 업체 견적 {q.offer_count}건</span>}
            </p>
            <p className="pre">{q.description}</p>
            <footer>
              <small className="gy-muted">{q.customer_phone}</small>
              <button type="button" className="gy-button small" onClick={() => setAnswering(q)}>
                견적 보내기
              </button>
            </footer>
          </article>
        ))}
      </div>
      {answering && (
        <OfferForm
          quote={answering}
          onClose={() => setAnswering(null)}
          onSent={() => {
            setAnswering(null);
            toast("견적을 고객에게 보냈습니다.");
            reload();
            loadCounts();
          }}
        />
      )}
    </section>
  );
}

function OfferForm({ quote, onClose, onSent }) {
  const [form, setForm] = useState({ amount: "", description: "", valid_until: todayIso(7) });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      await papi(`/partner/quotes/${quote.id}/offers`, {
        method: "POST",
        body: {
          amount: Number(String(form.amount).replaceAll(",", "")),
          description: form.description,
          valid_until: form.valid_until ? `${form.valid_until}T23:59:00` : null,
        },
      });
      onSent();
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }
  const amount = Number(String(form.amount).replaceAll(",", "")) || 0;
  return (
    <Modal title="견적 보내기" eyebrow={`${quote.vehicle} · ${quote.service_category}`} onClose={onClose}>
      <p className="pre gy-muted">{quote.description}</p>
      <form className="gy-form" onSubmit={submit}>
        <Field label="견적 금액 (원)" required hint={amount ? money(amount) : "부품·공임을 포함한 예상 금액"}>
          <input required inputMode="numeric" value={form.amount} onChange={(e) => setForm({ ...form, amount: e.target.value })} />
        </Field>
        <Field label="작업 내용·안내" required>
          <textarea required maxLength={2000} rows={5} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="작업 범위, 사용 부품, 소요 시간, 추가 비용이 생길 수 있는 경우 등" />
        </Field>
        <Field label="견적 유효 기간">
          <input type="date" min={todayIso(0)} value={form.valid_until} onChange={(e) => setForm({ ...form, valid_until: e.target.value })} />
        </Field>
        {error && <div className="gy-error">{error}</div>}
        <button className="gy-button block" disabled={busy}>
          {busy ? "보내는 중…" : "견적 보내기"}
        </button>
      </form>
    </Modal>
  );
}
