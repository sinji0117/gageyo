import { useState } from "react";
import { api, Modal, Field } from "@gageyo/ui";
import { useApp } from "../context.js";

const REASONS = {
  workshop: ["허위 정보·사진", "불친절·부당한 요금", "영업하지 않는 업체", "기타"],
  review: ["광고·홍보", "욕설·비방", "거짓 후기", "개인정보 노출", "기타"],
  product: ["허위·과장 정보", "판매 금지 상품", "가격 문제", "기타"],
};

export default function ReportDialog({ targetType, targetId, back = null }) {
  const { close, open, user, toast } = useApp();
  const [reason, setReason] = useState(REASONS[targetType]?.[0] || "기타");
  const [detail, setDetail] = useState("");
  const [email, setEmail] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const done = () => (back ? open(back) : close());

  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      await api("/reports", {
        method: "POST",
        body: { target_type: targetType, target_id: targetId, reason: `[${reason}] ${detail}`.trim(), email: email || undefined },
      });
      toast("신고를 받았습니다. 운영자가 확인 후 조치합니다.");
      done();
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title="신고하기" eyebrow={{ workshop: "업체", review: "리뷰", product: "상품" }[targetType]} onClose={done}>
      <form className="gy-form" onSubmit={submit}>
        <Field label="신고 사유">
          <div className="radio-list">
            {(REASONS[targetType] || ["기타"]).map((r) => (
              <label key={r} className={reason === r ? "on" : ""}>
                <input type="radio" name="reason" checked={reason === r} onChange={() => setReason(r)} />
                {r}
              </label>
            ))}
          </div>
        </Field>
        <Field label="자세한 내용">
          <textarea maxLength={1800} value={detail} onChange={(e) => setDetail(e.target.value)} placeholder="확인에 도움이 되는 내용을 적어 주세요." />
        </Field>
        {!user && (
          <Field label="결과 받을 이메일" hint="선택">
            <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} />
          </Field>
        )}
        {error && <div className="gy-error">{error}</div>}
        <button className="gy-button block danger" disabled={busy}>
          신고하기
        </button>
      </form>
    </Modal>
  );
}
