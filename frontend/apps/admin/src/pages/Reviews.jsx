import { useState } from "react";
import { Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateText, qs } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

export default function Reviews() {
  const { toast } = useAdmin();
  const [status, setStatus] = useState("");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/reviews${qs({ status, size: 200 })}`), [status]);
  async function toggle(r) {
    const next = r.status === "visible" ? "hidden" : "visible";
    const reason = next === "hidden" ? window.prompt("숨기는 사유 (작업 기록에 남습니다)", "") : null;
    if (next === "hidden" && !reason) return;
    try {
      await aapi(`/admin/reviews/${r.id}`, { method: "PATCH", body: { status: next, reason } });
      toast(next === "hidden" ? "리뷰를 숨겼습니다. 업체 평점에서 빠집니다." : "리뷰를 다시 공개했습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>리뷰</h1>
          <p>리뷰는 작업이 완료된 예약에만 쓸 수 있습니다. 신고된 리뷰는 [문의·신고 → 신고]에서 확인하세요.</p>
        </div>
      </div>
      <Tabs
        tabs={[
          ["", "전체"],
          ["visible", "공개"],
          ["hidden", "숨김"],
        ]}
        value={status}
        onChange={setStatus}
      />
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>리뷰가 없습니다.</Empty>}
      <div className="review-grid">
        {data?.map((r) => (
          <article key={r.id} className="box">
            <div className="box-title">
              <div>
                <h2>
                  <span className="gy-stars">{"★".repeat(r.rating)}</span> {r.workshop_name}
                </h2>
                <p>
                  {r.author} · {dateText(r.created_at)} · <Badge kind="review" value={r.status} />
                </p>
              </div>
              <button type="button" className={`gy-button small ${r.status === "visible" ? "danger" : ""}`} onClick={() => toggle(r)}>
                {r.status === "visible" ? "숨기기" : "공개"}
              </button>
            </div>
            <p className="pre">{r.content}</p>
            {r.reply && <p className="reply-inline">업체 답글: {r.reply}</p>}
          </article>
        ))}
      </div>
    </>
  );
}
