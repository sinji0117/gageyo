import { useState } from "react";
import { api, Modal, Field, Loading, ErrorBox, Badge, useLoad, dateTimeText } from "@gageyo/ui";
import { useApp } from "../context.js";

/** 문의 대화 보기·이어서 묻기 (비회원은 비밀 링크의 token) */
export default function InquiryView({ id, token = "", fresh = false }) {
  const { close, toast } = useApp();
  const { data, loading, error, reload, setData } = useLoad(() => api(`/support/inquiries/${id}${token ? `?token=${encodeURIComponent(token)}` : ""}`), [id, token]);
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const link = token ? `${window.location.origin}/?inquiry=${id}&token=${token}` : "";

  async function send(e) {
    e.preventDefault();
    setBusy(true);
    try {
      setData(await api(`/support/inquiries/${id}/messages`, { method: "POST", body: { body, token } }));
      setBody("");
    } catch (err) {
      toast(err.message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title={data?.subject || "문의"} eyebrow="고객센터" onClose={close}>
      {loading && !data && <Loading />}
      <ErrorBox error={error} onRetry={reload} />
      {fresh && (
        <div className="gy-note">
          문의를 받았습니다.
          {link && (
            <>
              {" "}
              아래 비밀 링크로 답변을 확인할 수 있습니다 (메일로도 보냈어요).
              <input className="gy-input mono" readOnly value={link} onFocus={(e) => e.target.select()} />
            </>
          )}
        </div>
      )}
      {data && (
        <>
          <p>
            <Badge kind="inquiry" value={data.status} />
          </p>
          <ul className="thread">
            {data.messages.map((m) => (
              <li key={m.id} className={m.author === "admin" ? "from-admin" : ""}>
                <b>{m.author === "admin" ? "가게요 고객센터" : "나"}</b>
                <small>{dateTimeText(m.created_at)}</small>
                <p>{m.body}</p>
              </li>
            ))}
          </ul>
          {data.status !== "closed" && (
            <form className="gy-form" onSubmit={send}>
              <Field label="이어서 묻기">
                <textarea required maxLength={5000} value={body} onChange={(e) => setBody(e.target.value)} />
              </Field>
              <button className="gy-button" disabled={busy}>
                보내기
              </button>
            </form>
          )}
        </>
      )}
    </Modal>
  );
}
