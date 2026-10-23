import { useState } from "react";
import { api, Modal, Field } from "@gageyo/ui";
import { useApp } from "../context.js";

/** 고객센터 1:1 문의 (비회원은 이메일로 답변 링크를 받음) */
export default function SupportDialog({ subject: initialSubject = "" }) {
  const { close, user, open, config } = useApp();
  const [form, setForm] = useState({ name: "", email: "", subject: initialSubject, body: "", website: "" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });
  const support = config?.support || {};

  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      const r = await api("/support/inquiries", { method: "POST", body: form });
      open({ type: "inquiry", id: r.id, token: r.token || "", fresh: true });
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title="고객센터 문의" eyebrow="1:1 문의" onClose={close}>
      <p className="gy-muted">
        주문·예약·결제·업체 이용 중 불편한 점을 남겨 주세요. 답변이 등록되면 메일로 알려 드립니다.
        {support.phone && ` 전화 상담: ${support.phone}`}
      </p>
      <form className="gy-form" onSubmit={submit}>
        {!user && (
          <div className="gy-row">
            <Field label="이름" required>
              <input required maxLength={100} value={form.name} onChange={set("name")} autoComplete="name" />
            </Field>
            <Field label="답변 받을 이메일" required>
              <input required type="email" value={form.email} onChange={set("email")} autoComplete="email" />
            </Field>
          </div>
        )}
        <Field label="제목" required>
          <input required maxLength={200} value={form.subject} onChange={set("subject")} />
        </Field>
        <Field label="내용" required hint="주문·예약 번호가 있으면 함께 적어 주세요">
          <textarea required maxLength={5000} rows={6} value={form.body} onChange={set("body")} />
        </Field>
        {/* 사람에게는 보이지 않는 칸 (자동 등록 막기) */}
        <input className="gy-sr" tabIndex={-1} autoComplete="off" aria-hidden="true" value={form.website} onChange={set("website")} />
        {error && <div className="gy-error">{error}</div>}
        <button className="gy-button block" disabled={busy}>
          {busy ? "보내는 중…" : "문의 남기기"}
        </button>
      </form>
    </Modal>
  );
}
