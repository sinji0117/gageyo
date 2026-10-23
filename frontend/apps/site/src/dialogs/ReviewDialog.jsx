import { useState } from "react";
import { api, Modal, Field } from "@gageyo/ui";
import { useApp } from "../context.js";

/** 완료된 예약에 리뷰 쓰기·고치기 */
export default function ReviewDialog({ bookingId, workshopName, review = null, onDone }) {
  const { close, toast } = useApp();
  const [rating, setRating] = useState(review?.rating || 5);
  const [content, setContent] = useState(review?.content || "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      if (review) await api(`/me/reviews/${review.id}`, { method: "PUT", body: { rating, content } });
      else await api("/me/reviews", { method: "POST", body: { booking_id: bookingId, rating, content } });
      toast("후기를 남겼습니다. 감사합니다!");
      onDone?.();
      close();
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title={review ? "후기 고치기" : "이용 후기"} eyebrow={workshopName} onClose={close}>
      <form className="gy-form" onSubmit={submit}>
        <Field label="별점">
          <div className="star-input" role="radiogroup" aria-label="별점">
            {[1, 2, 3, 4, 5].map((n) => (
              <button key={n} type="button" role="radio" aria-checked={rating === n} className={n <= rating ? "on" : ""} onClick={() => setRating(n)} aria-label={`${n}점`}>
                ★
              </button>
            ))}
          </div>
        </Field>
        <Field label="후기" required hint="5자 이상 · 작업 내용, 친절도, 가격 등을 알려 주세요">
          <textarea required minLength={5} maxLength={1000} rows={5} value={content} onChange={(e) => setContent(e.target.value)} />
        </Field>
        {error && <div className="gy-error">{error}</div>}
        <button className="gy-button block" disabled={busy}>
          {review ? "고치기" : "후기 남기기"}
        </button>
      </form>
    </Modal>
  );
}
