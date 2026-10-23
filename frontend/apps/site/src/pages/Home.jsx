import { useCallback, useEffect, useMemo, useState } from "react";
import { api, qs, Empty, ErrorBox, Loading, hoursText } from "@gageyo/ui";
import { useApp } from "../context.js";
import WorkshopCard from "../components/WorkshopCard.jsx";

const SORTS = [
  ["rating", "평점순"],
  ["reviews", "리뷰 많은 순"],
  ["distance", "가까운 순"],
  ["recent", "새로 입점"],
];

export default function Home() {
  const { categories, open, toast } = useApp();
  const [stats, setStats] = useState(null);
  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [district, setDistrict] = useState("");
  const [category, setCategory] = useState("");
  const [group, setGroup] = useState("");
  const [sort, setSort] = useState("rating");
  const [coords, setCoords] = useState(null);
  const [radius, setRadius] = useState(10);
  const [items, setItems] = useState([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    api("/stats").then(setStats).catch(() => setStats(null));
  }, []);

  const groups = useMemo(() => [...new Set(categories.map((c) => c.group_name))], [categories]);
  useEffect(() => {
    if (!group && groups.length) setGroup(groups[0]);
  }, [groups, group]);

  const params = useMemo(
    () => ({
      q: submitted,
      category,
      district,
      lat: coords?.lat,
      lng: coords?.lng,
      radius_km: coords ? radius : undefined,
      sort: sort === "distance" && !coords ? "rating" : sort,
    }),
    [submitted, category, district, coords, radius, sort],
  );

  const load = useCallback(
    async (nextPage) => {
      setLoading(true);
      setError("");
      try {
        const res = await api(`/workshops${qs({ ...params, page: nextPage, size: 24 })}`);
        setItems((prev) => (nextPage === 1 ? res.items : [...prev, ...res.items]));
        setHasMore(res.has_more);
        setPage(nextPage);
      } catch (e) {
        setError(e.message);
      } finally {
        setLoading(false);
      }
    },
    [params],
  );
  useEffect(() => {
    load(1);
  }, [load]);

  function locate() {
    if (!navigator.geolocation) {
      toast("이 브라우저에서는 위치를 쓸 수 없습니다.");
      return;
    }
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        setCoords({ lat: Number(pos.coords.latitude.toFixed(5)), lng: Number(pos.coords.longitude.toFixed(5)) });
        setSort("distance");
        toast("내 주변 업체를 찾습니다.");
      },
      () => toast("위치 권한이 없어 내 주변 검색을 할 수 없습니다. 지역을 입력해 주세요."),
      { enableHighAccuracy: false, timeout: 8000, maximumAge: 300000 },
    );
  }

  function search(e) {
    e.preventDefault();
    setSubmitted(query.trim());
    document.querySelector("#shops")?.scrollIntoView({ behavior: "smooth" });
  }

  return (
    <>
      <section className="hero">
        <div className="hero-photo" />
        <div className="hero-content">
          <span className="eyebrow">CAR CARE, CLOSER TO YOU</span>
          <h1>
            내 차 관리,
            <br />
            동네에서 더 똑똑하게.
          </h1>
          <p>믿을 수 있는 정비소를 찾고, 여러 곳의 견적을 한 번에 받아보세요. 자동차 용품 구매와 방문 장착 예약까지 한곳에서.</p>
          <form className="search" onSubmit={search} role="search">
            <span aria-hidden="true">⌕</span>
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="업체 이름, 서비스, 동네 검색" aria-label="검색어" />
            <button className="gy-button">찾기</button>
          </form>
          <div className="hero-actions">
            <button type="button" className="link-light" onClick={locate}>
              ◎ 내 주변 업체 보기
            </button>
            <button type="button" className="link-light" onClick={() => open({ type: "quote" })}>
              무료 견적 한 번에 받기 →
            </button>
          </div>
          {stats && (stats.workshops > 0 || stats.reviews > 0) && (
            <div className="stats">
              <span>
                <b>{stats.workshops.toLocaleString("ko-KR")}</b> 파트너 업체
              </span>
              {stats.average_rating && (
                <span>
                  <b>★ {stats.average_rating}</b> 평균 평점 ({stats.reviews.toLocaleString("ko-KR")}개 리뷰)
                </span>
              )}
              <span>
                <b>무료</b> 견적 요청
              </span>
            </div>
          )}
        </div>
      </section>

      <section className="main-wrap" id="shops">
        <div className="category-browser">
          <div className="category-groups" role="tablist">
            {groups.map((g) => (
              <button key={g} type="button" className={group === g ? "active" : ""} onClick={() => setGroup(g)}>
                {g}
              </button>
            ))}
          </div>
          <div className="category-content">
            <div className="category-heading">
              <h3>{group || "서비스"}</h3>
              <button type="button" onClick={() => setCategory("")}>
                {category ? "전체 서비스 보기" : "전체 서비스 선택됨"}
              </button>
            </div>
            <div className="category-links">
              {categories
                .filter((c) => c.group_name === group)
                .map((c) => (
                  <button key={c.id} type="button" className={category === c.name ? "chosen" : ""} onClick={() => setCategory(category === c.name ? "" : c.name)}>
                    {c.name}
                    <span aria-hidden="true">›</span>
                  </button>
                ))}
            </div>
          </div>
        </div>

        <div className="section-title">
          <div>
            <h2>{category ? `${category} 파트너` : "가게요 파트너"}</h2>
            <p>{submitted ? `'${submitted}' 검색 결과` : "지역과 전문 분야로 업체를 찾아보세요."}</p>
          </div>
          <div className="filters">
            <input aria-label="지역" value={district} onChange={(e) => setDistrict(e.target.value)} placeholder="지역 (예: 강남구, 역삼동)" />
            <select aria-label="정렬" value={sort} onChange={(e) => (e.target.value === "distance" && !coords ? locate() : setSort(e.target.value))}>
              {SORTS.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
            {coords && (
              <select aria-label="반경" value={radius} onChange={(e) => setRadius(Number(e.target.value))}>
                {[3, 5, 10, 20, 50].map((r) => (
                  <option key={r} value={r}>
                    반경 {r}km
                  </option>
                ))}
              </select>
            )}
            {coords && (
              <button type="button" className="gy-button ghost small" onClick={() => setCoords(null)}>
                위치 끄기
              </button>
            )}
          </div>
        </div>

        <ErrorBox error={error} onRetry={() => load(1)} />
        <div className="shop-grid">
          {items.map((w) => (
            <WorkshopCard key={w.id} workshop={w} subtitle={hoursText(w)} />
          ))}
        </div>
        {loading && <Loading />}
        {!loading && !error && items.length === 0 && (
          <Empty
            action={
              <button type="button" className="gy-button" onClick={() => open({ type: "quote", category })}>
                견적 요청 남기기
              </button>
            }
          >
            조건에 맞는 업체가 아직 없습니다. 견적 요청을 남기면 가까운 업체가 연락드려요.
          </Empty>
        )}
        {hasMore && !loading && (
          <div className="more">
            <button type="button" className="gy-button ghost" onClick={() => load(page + 1)}>
              더 보기
            </button>
          </div>
        )}

        <div className="quote-banner">
          <div>
            <b>여러 파트너의 견적을 한 번에 받아보세요</b>
            <p>차량과 원하는 작업을 적으면 분야·지역이 맞는 업체들이 견적을 보내드립니다. 받은 견적을 비교해 고르면 바로 예약할 수 있어요.</p>
          </div>
          <button type="button" className="gy-button dark" onClick={() => open({ type: "quote", category })}>
            무료 견적 요청하기
          </button>
        </div>
      </section>
    </>
  );
}
