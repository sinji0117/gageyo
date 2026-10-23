import { useState } from "react";
import { Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateTimeText, relativeText, qs } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

const KINDS = [
  ["customer", "고객 문의"],
  ["partner", "파트너 문의"],
  ["report", "신고"],
];

/** 문의·신고 (왼쪽 목록 · 오른쪽 대화). 예전 운영 화면의 '신고·문의'는 준비 중이었습니다. */
export default function Inquiries() {
  const { toast, refreshCounts, go } = useAdmin();
  const [kind, setKind] = useState("customer");
  const [status, setStatus] = useState("open");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/inquiries${qs({ kind, status })}`), [kind, status]);
  const [thread, setThread] = useState(null);
  const [reply, setReply] = useState("");

  async function openThread(id) {
    try {
      setThread(await aapi(`/admin/inquiries/${id}`));
      setReply("");
    } catch (e) {
      toast(e.message);
    }
  }
  async function send(e) {
    e.preventDefault();
    try {
      setThread(await aapi(`/admin/inquiries/${thread.id}/reply`, { method: "POST", body: { body: reply } }));
      setReply("");
      toast(thread.email ? "답변을 저장하고 메일로 알렸습니다." : "답변을 저장했습니다.");
      reload();
      refreshCounts();
    } catch (err) {
      toast(err.message);
    }
  }
  async function setThreadStatus(next) {
    try {
      await aapi(`/admin/inquiries/${thread.id}`, { method: "PATCH", body: { status: next } });
      setThread({ ...thread, status: next });
      reload();
      refreshCounts();
    } catch (err) {
      toast(err.message);
    }
  }
  const counts = data?.open_counts || {};

  return (
    <>
      <div className="page-title">
        <div>
          <h1>문의·신고</h1>
          <p>답하면 문의한 사람에게 메일이 가고, 그쪽이 다시 쓰면 '답변 대기'로 돌아옵니다.</p>
        </div>
      </div>
      <Tabs tabs={KINDS.map(([k, l]) => [k, l, counts[k]])} value={kind} onChange={(k) => { setKind(k); setThread(null); }} />
      <div className="inbox">
        <div className="inbox-list box">
          <select value={status} onChange={(e) => setStatus(e.target.value)} aria-label="상태">
            <option value="open">답변 대기</option>
            <option value="answered">답변 완료</option>
            <option value="closed">종료</option>
            <option value="">전체</option>
          </select>
          <ErrorBox error={error} onRetry={reload} />
          {loading && !data && <Loading />}
          {data?.items.length === 0 && <Empty>문의가 없습니다.</Empty>}
          <ul>
            {data?.items.map((q) => (
              <li key={q.id}>
                <button type="button" className={thread?.id === q.id ? "on" : ""} onClick={() => openThread(q.id)}>
                  <b>{q.subject}</b>
                  <span>
                    {q.name} · {relativeText(q.last_message_at)}
                  </span>
                  <Badge kind="inquiry" value={q.status} />
                </button>
              </li>
            ))}
          </ul>
        </div>
        <div className="inbox-thread box">
          {!thread && <Empty>왼쪽에서 문의를 고르세요.</Empty>}
          {thread && (
            <>
              <div className="box-title">
                <div>
                  <h2>{thread.subject}</h2>
                  <p>
                    {thread.name} {thread.email && `· ${thread.email}`} {thread.user_id ? "(회원)" : "(비회원)"}
                  </p>
                </div>
                <div className="gy-actions">
                  <Badge kind="inquiry" value={thread.status} />
                  {thread.status !== "closed" ? (
                    <button type="button" className="gy-button ghost small" onClick={() => setThreadStatus("closed")}>
                      종료
                    </button>
                  ) : (
                    <button type="button" className="gy-button ghost small" onClick={() => setThreadStatus("open")}>
                      다시 열기
                    </button>
                  )}
                </div>
              </div>
              {thread.kind === "report" && thread.target_type && (
                <p className="gy-note">
                  신고 대상: {{ workshop: "업체", review: "리뷰", product: "상품" }[thread.target_type]}{" "}
                  <button
                    type="button"
                    className="link-button"
                    onClick={() => go({ workshop: "workshops", review: "reviews", product: "products" }[thread.target_type], {})}
                  >
                    관리 화면으로 →
                  </button>
                  <small className="mono"> {thread.target_id}</small>
                </p>
              )}
              <ul className="thread">
                {thread.messages.map((m) => (
                  <li key={m.id} className={m.author === "admin" ? "from-admin" : ""}>
                    <b>{m.author === "admin" ? "운영자" : thread.name}</b> <small>{dateTimeText(m.created_at)}</small>
                    <p className="pre">{m.body}</p>
                  </li>
                ))}
              </ul>
              <form className="gy-form" onSubmit={send}>
                <textarea className="gy-input" required rows={5} maxLength={5000} value={reply} onChange={(e) => setReply(e.target.value)} placeholder="답변 쓰기" />
                <button className="gy-button">답변 보내기</button>
              </form>
            </>
          )}
        </div>
      </div>
    </>
  );
}
