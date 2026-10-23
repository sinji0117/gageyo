import { useState } from "react";
import { Badge, Empty, Loading, ErrorBox, useLoad, dateText } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

export default function Reviews() {
  const { toast, loadCounts } = usePartner();
  const { data, loading, error, reload } = useLoad(() => papi("/partner/reviews"), []);
  const [drafts, setDrafts] = useState({});

  async function reply(r) {
    try {
      await papi(`/partner/reviews/${r.id}/reply`, { method: "PUT", body: { reply: drafts[r.id] ?? r.reply ?? "" } });
      toast("답글을 저장했습니다.");
      reload();
      loadCounts();
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <section className="panel">
      <div className="panel-title">
        <div>
          <h2>리뷰</h2>
          <p>답글은 고객 화면 리뷰 아래에 보입니다. 부당한 리뷰는 [공지·문의]로 운영자에게 알려 주세요.</p>
        </div>
      </div>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>아직 리뷰가 없습니다.</Empty>}
      <ul className="review-admin">
        {data?.map((r) => (
          <li key={r.id}>
            <div className="review-top">
              <span className="gy-stars">{"★".repeat(r.rating)}</span> <b>{r.author}</b> <small>{dateText(r.created_at)}</small>
              {r.status === "hidden" && <Badge kind="review" value="hidden" />}
            </div>
            <p className="pre">{r.content}</p>
            <div className="reply-box">
              <textarea
                maxLength={1000}
                placeholder="답글 달기 (비우고 저장하면 지워집니다)"
                value={drafts[r.id] ?? r.reply ?? ""}
                onChange={(e) => setDrafts({ ...drafts, [r.id]: e.target.value })}
              />
              <button type="button" className="gy-button small" onClick={() => reply(r)}>
                {r.reply ? "답글 고치기" : "답글 달기"}
              </button>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
