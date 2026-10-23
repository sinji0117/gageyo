import { useCallback, useEffect, useState } from "react";
import { api, qs, money, Empty, ErrorBox, Loading } from "@gageyo/ui";
import { useApp } from "../context.js";
import { Cover } from "../components/WorkshopCard.jsx";

export function ProductCard({ product: p }) {
  const { cart, open, toast } = useApp();
  const soldOut = p.status === "sold_out" || p.stock === 0;
  return (
    <article className="product-card">
      <button type="button" className="card-hit" onClick={() => open({ type: "product", id: p.id })} aria-label={`${p.name} 자세히 보기`} />
      <Cover url={p.image_url} name={p.name} className="product-photo">
        {soldOut && <span className="tag dark">품절</span>}
      </Cover>
      <small>
        {p.category} · {p.workshop_name}
      </small>
      <h3>{p.name}</h3>
      <div className="product-price">
        <span>
          {p.list_price > p.price && <s>{money(p.list_price)}</s>}
          {money(p.price)}
        </span>
        <button
          type="button"
          disabled={soldOut}
          aria-label="장바구니에 담기"
          onClick={() => {
            cart.add(p);
            toast("장바구니에 담았어요.");
          }}
        >
          ＋
        </button>
      </div>
    </article>
  );
}

export default function Products() {
  const { open, cart } = useApp();
  const [cats, setCats] = useState([]);
  const [category, setCategory] = useState("");
  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [sort, setSort] = useState("recent");
  const [items, setItems] = useState([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    api("/products/categories").then(setCats).catch(() => setCats([]));
  }, []);
  const load = useCallback(
    async (n) => {
      setLoading(true);
      setError("");
      try {
        const res = await api(`/products${qs({ q: submitted, category, sort, page: n, size: 24 })}`);
        setItems((prev) => (n === 1 ? res.items : [...prev, ...res.items]));
        setHasMore(res.has_more);
        setPage(n);
      } catch (e) {
        setError(e.message);
      } finally {
        setLoading(false);
      }
    },
    [submitted, category, sort],
  );
  useEffect(() => {
    load(1);
  }, [load]);

  return (
    <section className="main-wrap products-page">
      <div className="section-title">
        <div>
          <h2>자동차 용품</h2>
          <p>가게요 파트너가 파는 상품입니다. 택배로 받거나 그 업체에 방문해 장착할 수 있어요.</p>
        </div>
        <form
          className="filters"
          onSubmit={(e) => {
            e.preventDefault();
            setSubmitted(query.trim());
          }}
        >
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="상품 검색" aria-label="상품 검색" />
          <select value={sort} onChange={(e) => setSort(e.target.value)} aria-label="정렬">
            <option value="recent">최신순</option>
            <option value="price_asc">낮은 가격순</option>
            <option value="price_desc">높은 가격순</option>
          </select>
        </form>
      </div>
      <div className="chip-row">
        <button type="button" className={!category ? "on" : ""} onClick={() => setCategory("")}>
          전체
        </button>
        {cats.map((c) => (
          <button type="button" key={c.name} className={category === c.name ? "on" : ""} onClick={() => setCategory(c.name)}>
            {c.name} <small>{c.count}</small>
          </button>
        ))}
      </div>
      <ErrorBox error={error} onRetry={() => load(1)} />
      <div className="product-grid">
        {items.map((p) => (
          <ProductCard key={p.id} product={p} />
        ))}
      </div>
      {loading && <Loading />}
      {!loading && !error && items.length === 0 && <Empty>판매 중인 상품이 아직 없습니다.</Empty>}
      {hasMore && !loading && (
        <div className="more">
          <button type="button" className="gy-button ghost" onClick={() => load(page + 1)}>
            더 보기
          </button>
        </div>
      )}
      {cart.count > 0 && (
        <div className="quote-banner sticky-cart">
          <div>
            <b>
              장바구니 {cart.count}개 · {money(cart.total)}
            </b>
            <p>주문할 때 가격과 재고를 다시 확인합니다.</p>
          </div>
          <button type="button" className="gy-button dark" onClick={() => open({ type: "cart" })}>
            주문하기
          </button>
        </div>
      )}
    </section>
  );
}
