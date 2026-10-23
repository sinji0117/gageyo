import { useState } from "react";
import { Field, Badge, Empty, Loading, ErrorBox, useLoad, dateText, dateTimeText } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

/** 가게요 공지와 운영자 1:1 문의 */
export default function Support() {
  const { toast } = usePartner();
  const notices = useLoad(() => papi("/partner/notices?limit=30"), []);
  const inquiries = useLoad(() => papi("/partner/inquiries"), []);
  const [openNotice, setOpenNotice] = useState(null);
  const [thread, setThread] = useState(null);
  const [form, setForm] = useState({ subject: "", body: "" });
  const [reply, setReply] = useState("");

  async function create(e) {
    e.preventDefault();
    try {
      const r = await papi("/partner/inquiries", { method: "POST", body: form });
      setForm({ subject: "", body: "" });
      toast("문의를 보냈습니다. 답변이 오면 메일로 알려 드립니다.");
      inquiries.reload();
      setThread(await papi(`/partner/inquiries/${r.id}`));
    } catch (err) {
      toast(err.message);
    }
  }
  async function send(e) {
    e.preventDefault();
    try {
      setThread(await papi(`/partner/inquiries/${thread.id}/messages`, { method: "POST", body: { body: reply } }));
      setReply("");
      inquiries.reload();
    } catch (err) {
      toast(err.message);
    }
  }

  return (
    <div className="grid-2">
      <section className="panel">
        <h2>공지</h2>
        <ErrorBox error={notices.error} onRetry={notices.reload} />
        {notices.loading && !notices.data && <Loading />}
        {notices.data?.length === 0 && <Empty>공지가 없습니다.</Empty>}
        <ul className="notice-list">
          {notices.data?.map((n) => (
            <li key={n.id}>
              <button type="button" onClick={() => setOpenNotice(openNotice === n.id ? null : n.id)}>
                {n.pinned ? "📌 " : ""}
                <b>{n.title}</b> <small>{dateText(n.created_at)}</small>
              </button>
              {openNotice === n.id && <p className="pre">{n.body}</p>}
            </li>
          ))}
        </ul>
      </section>
      <section className="panel">
        <h2>운영자에게 문의</h2>
        {thread ? (
          <div>
            <button type="button" className="link-button" onClick={() => setThread(null)}>
              ← 문의 목록
            </button>
            <h3>
              {thread.subject} <Badge kind="inquiry" value={thread.status} />
            </h3>
            <ul className="thread">
              {thread.messages.map((m) => (
                <li key={m.id} className={m.author === "admin" ? "from-admin" : ""}>
                  <b>{m.author === "admin" ? "가게요 운영팀" : "우리 업체"}</b> <small>{dateTimeText(m.created_at)}</small>
                  <p className="pre">{m.body}</p>
                </li>
              ))}
            </ul>
            {thread.status !== "closed" && (
              <form className="gy-form" onSubmit={send}>
                <Field label="이어서 쓰기">
                  <textarea required maxLength={5000} value={reply} onChange={(e) => setReply(e.target.value)} />
                </Field>
                <button className="gy-button">보내기</button>
              </form>
            )}
          </div>
        ) : (
          <>
            <form className="gy-form" onSubmit={create}>
              <Field label="제목" required>
                <input required maxLength={200} value={form.subject} onChange={(e) => setForm({ ...form, subject: e.target.value })} />
              </Field>
              <Field label="내용" required>
                <textarea required maxLength={5000} rows={4} value={form.body} onChange={(e) => setForm({ ...form, body: e.target.value })} />
              </Field>
              <button className="gy-button">문의 보내기</button>
            </form>
            <ul className="inquiry-list">
              {inquiries.data?.map((q) => (
                <li key={q.id}>
                  <button type="button" onClick={async () => setThread(await papi(`/partner/inquiries/${q.id}`))}>
                    <Badge kind="inquiry" value={q.status} /> <b>{q.subject}</b> <small>{dateText(q.last_message_at)}</small>
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  );
}
