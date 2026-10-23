import { useEffect, useState } from "react";
import { api } from "@gageyo/ui";
import { useApp } from "../context.js";

export function Stars({ value }) {
  const v = Math.round(Number(value || 0));
  return (
    <span className="gy-stars" aria-label={`별점 ${Number(value || 0).toFixed(1)}`}>
      {"★".repeat(v)}
      <span className="dim">{"★".repeat(5 - v)}</span>
    </span>
  );
}

/** 사진이 없으면 실제 업체처럼 보이는 사진 대신 이름 첫 글자로 된 그림을 씁니다. */
export function Cover({ url, name, className = "shop-photo", children }) {
  // 지워졌거나 열리지 않는 이미지는 빈 칸 대신 이름 그림으로
  const [broken, setBroken] = useState(false);
  useEffect(() => {
    setBroken(false);
    if (!url) return undefined;
    let alive = true;
    const img = new Image();
    img.onerror = () => alive && setBroken(true);
    img.src = url;
    return () => {
      alive = false;
    };
  }, [url]);
  if (url && !broken) {
    return (
      <div className={className} style={{ backgroundImage: `url(${url})` }}>
        {children}
      </div>
    );
  }
  return (
    <div className={`${className} placeholder`}>
      <span className="initial" aria-hidden="true">
        {(name || "가")[0]}
      </span>
      {children}
    </div>
  );
}

export default function WorkshopCard({ workshop: w, subtitle }) {
  const { open, user, toast } = useApp();
  const [saved, setSaved] = useState(false);
  async function toggleFavorite(e) {
    e.stopPropagation();
    if (!user) {
      open({ type: "auth" });
      toast("로그인하면 관심 업체를 저장할 수 있습니다.");
      return;
    }
    try {
      await api(`/me/favorites/${w.id}`, { method: saved ? "DELETE" : "PUT" });
      setSaved(!saved);
      toast(saved ? "관심 업체에서 뺐습니다." : "관심 업체에 저장했습니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  return (
    <article className="shop-card">
      <button type="button" className="card-hit" onClick={() => open({ type: "workshop", id: w.id })} aria-label={`${w.name} 자세히 보기`} />
      <Cover url={w.cover_image_url} name={w.name}>
        <span className="tag">{w.category}</span>
        <button type="button" className={`heart ${saved ? "saved" : ""}`} aria-label="관심 업체" aria-pressed={saved} onClick={toggleFavorite}>
          {saved ? "♥" : "♡"}
        </button>
      </Cover>
      <div className="shop-info">
        <div className="shop-heading">
          <div>
            <h3>{w.name}</h3>
            <p>
              {w.district} · {w.address}
            </p>
          </div>
          <b className="rating">
            ★ {Number(w.rating || 0).toFixed(1)} <small>({w.review_count})</small>
          </b>
        </div>
        {(w.categories?.length > 1 || w.distance_km != null) && (
          <div className="chips">
            {w.distance_km != null && <span className="chip strong">{w.distance_km < 1 ? `${Math.round(w.distance_km * 1000)}m` : `${w.distance_km.toFixed(1)}km`}</span>}
            {(w.categories || []).slice(0, 3).map((c) => (
              <span key={c} className="chip">
                {c}
              </span>
            ))}
          </div>
        )}
        <div className="service-row">
          <span>{subtitle}</span>
          <button type="button" onClick={() => open({ type: "booking", workshopId: w.id, workshopName: w.name })}>
            예약하기 →
          </button>
        </div>
      </div>
    </article>
  );
}
