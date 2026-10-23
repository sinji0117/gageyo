import { useState } from "react";
import { api, money, Modal, Loading, ErrorBox, useLoad, hoursText, dateText } from "@gageyo/ui";
import { useApp } from "../context.js";
import { Cover, Stars } from "../components/WorkshopCard.jsx";

/** 업체 상세: 사진·소개·영업시간·위치·서비스·상품·리뷰 */
export default function WorkshopDialog({ id }) {
  const { close, open, user, toast, cart } = useApp();
  const { data: w, loading, error, reload, setData } = useLoad(() => api(`/workshops/${id}`), [id]);
  const [photo, setPhoto] = useState(0);
  const [reviews, setReviews] = useState(null);

  async function toggleFavorite() {
    if (!user) {
      open({ type: "auth" });
      return;
    }
    try {
      await api(`/me/favorites/${id}`, { method: w.is_favorite ? "DELETE" : "PUT" });
      setData({ ...w, is_favorite: !w.is_favorite });
      toast(w.is_favorite ? "관심 업체에서 뺐습니다." : "관심 업체에 저장했습니다.");
    } catch (e) {
      toast(e.message);
    }
  }
  async function moreReviews() {
    // 처음에는 상세에 딸려 온 5개 대신 10개씩 새로 받습니다
    const current = reviews || [];
    const next = await api(`/workshops/${id}/reviews?page=${Math.floor(current.length / 10) + 1}&size=10`).catch(() => []);
    setReviews([...current, ...next]);
  }
  async function share() {
    const url = `${window.location.origin}/?shop=${id}`;
    try {
      if (navigator.share) await navigator.share({ title: w.name, url });
      else {
        await navigator.clipboard.writeText(url);
        toast("업체 주소를 복사했습니다.");
      }
    } catch {
      /* 취소 */
    }
  }

  const photos = w ? [...(w.cover_image_url ? [w.cover_image_url] : []), ...w.photos.map((p) => p.url).filter((u) => u !== w.cover_image_url)] : [];
  const mapQuery = w ? encodeURIComponent(`${w.name} ${w.address}`) : "";
  const shownReviews = reviews || w?.reviews || [];

  return (
    <Modal title={w?.name || "업체 정보"} eyebrow={w?.categories?.join(" · ")} onClose={close} wide>
      {loading && !w && <Loading />}
      <ErrorBox error={error} onRetry={reload} />
      {w && (
        <div className="workshop-detail">
          <div className="gallery">
            <Cover url={photos[photo]} name={w.name} className="gallery-main" />
            {photos.length > 1 && (
              <div className="thumbs">
                {photos.map((u, i) => (
                  <button key={u} type="button" className={i === photo ? "on" : ""} style={{ backgroundImage: `url(${u})` }} onClick={() => setPhoto(i)} aria-label={`사진 ${i + 1}`} />
                ))}
              </div>
            )}
          </div>
          <div className="detail-head">
            <div>
              <p className="rating-line">
                <Stars value={w.rating} /> <b>{Number(w.rating).toFixed(1)}</b> <span className="gy-muted">리뷰 {w.review_count}개</span>
              </p>
              <p className="gy-muted">{hoursText(w)}</p>
            </div>
            <div className="gy-actions">
              <button type="button" className="gy-button ghost small" onClick={toggleFavorite} aria-pressed={w.is_favorite}>
                {w.is_favorite ? "♥ 저장됨" : "♡ 관심 업체"}
              </button>
              <button type="button" className="gy-button ghost small" onClick={share}>
                공유
              </button>
            </div>
          </div>
          <p className="description">{w.description}</p>
          <dl className="info-list">
            <dt>주소</dt>
            <dd>
              {w.address}
              <span className="map-links">
                <a href={`https://map.kakao.com/link/search/${mapQuery}`} target="_blank" rel="noreferrer">
                  카카오맵
                </a>
                <a href={`https://map.naver.com/p/search/${mapQuery}`} target="_blank" rel="noreferrer">
                  네이버 지도
                </a>
                {w.latitude && (
                  <a href={`https://map.kakao.com/link/to/${encodeURIComponent(w.name)},${w.latitude},${w.longitude}`} target="_blank" rel="noreferrer">
                    길찾기
                  </a>
                )}
              </span>
            </dd>
            <dt>전화</dt>
            <dd>
              <a href={`tel:${w.phone}`}>{w.phone}</a>
            </dd>
            {w.business_number && (
              <>
                <dt>사업자</dt>
                <dd>
                  {w.representative ? `${w.representative} · ` : ""}
                  {w.business_number}
                </dd>
              </>
            )}
          </dl>

          <div className="cta-row">
            <button type="button" className="gy-button" onClick={() => open({ type: "booking", workshopId: w.id, workshopName: w.name })}>
              방문 예약하기
            </button>
            <button type="button" className="gy-button dark" onClick={() => open({ type: "quote", category: w.categories?.[0] })}>
              견적 요청하기
            </button>
          </div>

          <h3 className="sub">서비스와 가격</h3>
          {w.services.length ? (
            <ul className="service-list">
              {w.services.map((s) => (
                <li key={s.id}>
                  <div>
                    <b>{s.name}</b>
                    {s.description && <small>{s.description}</small>}
                  </div>
                  <span>
                    {money(s.price_from)}~ · {s.duration_minutes}분
                  </span>
                  <button type="button" className="gy-button ghost small" onClick={() => open({ type: "booking", workshopId: w.id, workshopName: w.name, serviceId: s.id })}>
                    예약
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="gy-muted">등록된 서비스가 없습니다. 예약 메모에 원하는 작업을 적어 주세요.</p>
          )}

          {w.products.length > 0 && (
            <>
              <h3 className="sub">이 업체의 상품</h3>
              <div className="mini-products">
                {w.products.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => {
                      cart.add({ ...p, workshop_id: w.id, workshop_name: w.name });
                      toast("장바구니에 담았어요.");
                    }}
                  >
                    <Cover url={p.image_url} name={p.name} className="mini-photo" />
                    <span>{p.name}</span>
                    <b>{money(p.price)} ＋</b>
                  </button>
                ))}
              </div>
            </>
          )}

          <h3 className="sub">
            리뷰 <small>{w.review_count}</small>
          </h3>
          {w.rating_counts.length > 0 && (
            <div className="rating-bars">
              {[5, 4, 3, 2, 1].map((r) => {
                const n = w.rating_counts.find((c) => c.rating === r)?.count || 0;
                return (
                  <div key={r}>
                    <span>{r}점</span>
                    <i style={{ width: `${w.review_count ? (n / w.review_count) * 100 : 0}%` }} />
                    <small>{n}</small>
                  </div>
                );
              })}
            </div>
          )}
          {shownReviews.length === 0 && <p className="gy-muted">아직 리뷰가 없습니다. 예약 후 작업이 끝나면 리뷰를 남길 수 있어요.</p>}
          <ul className="review-list">
            {shownReviews.map((r) => (
              <li key={r.id}>
                <div className="review-head">
                  <Stars value={r.rating} /> <b>{r.author}</b>
                  {r.service_name && <span className="chip">{r.service_name}</span>}
                  <small>{dateText(r.created_at)}</small>
                  <button type="button" className="link-button small" onClick={() => open({ type: "report", targetType: "review", targetId: r.id, back: { type: "workshop", id } })}>
                    신고
                  </button>
                </div>
                <p>{r.content}</p>
                {r.reply && (
                  <div className="reply">
                    <b>사장님 답글</b>
                    <p>{r.reply}</p>
                  </div>
                )}
              </li>
            ))}
          </ul>
          {w.review_count > shownReviews.length && (
            <button type="button" className="gy-button ghost block" onClick={moreReviews}>
              리뷰 더 보기
            </button>
          )}
          <p className="report-line">
            <button type="button" className="link-button small" onClick={() => open({ type: "report", targetType: "workshop", targetId: w.id, back: { type: "workshop", id } })}>
              이 업체 신고하기
            </button>
          </p>
        </div>
      )}
    </Modal>
  );
}
