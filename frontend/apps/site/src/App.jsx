import { useEffect, useState } from "react";
import { api, money } from "@gageyo/ui";

const fallbackCategories = [
  ...["디테일링세차/광택", "스팀 세차", "오토바이 튜닝", "자동차 썬팅", "자동차 정비", "전기차충전기 설치 및 수리", "블랙박스/내비게이션 설치 및 수리", "오토바이 정비 및 수리", "자동차 랩핑", "자동차 외부수리", "자동차 튜닝", "출장 세차"].map((name, i) => ({ id: `care-${i}`, group_name: "차량 관리", name, sort_order: i * 10 })),
  ...["신차 구매", "자동차 구매동행", "자동차 리스/렌트", "중고차 구매", "중고차 판매", "캠핑카 제작"].map((name, i) => ({ id: `sale-${i}`, group_name: "자동차 매매", name, sort_order: i * 10 })),
];
const photo = (id) => `https://images.unsplash.com/${id}?auto=format&fit=crop&w=800&q=82`;

export default function App() {
  const [shops, setShops] = useState([]);
  const [products, setProducts] = useState([]);
  const [categories, setCategories] = useState(fallbackCategories);
  const [query, setQuery] = useState("");
  const [district, setDistrict] = useState("");
  const [cart, setCart] = useState([]);
  const [quoteOffers, setQuoteOffers] = useState([]);
  const [category, setCategory] = useState("전체");
  const [activeCategoryGroup, setActiveCategoryGroup] = useState("차량 관리");
  const [tab, setTab] = useState("shops");
  const [dialog, setDialog] = useState(null);
  const [notice, setNotice] = useState("");
  const [apiError, setApiError] = useState("");

  useEffect(() => {
    api("/workshops").then((rows) => setShops(rows)).catch(() => setApiError("파트너 목록을 불러오지 못했습니다. API 연결 상태를 확인해 주세요."));
    api("/products").then(setProducts).catch(() => setApiError("상품 목록을 불러오지 못했습니다. API 연결 상태를 확인해 주세요."));
    api("/categories").then((items) => { if (items.length) setCategories(items); }).catch(() => {});
  }, []);
  useEffect(() => {
    if (dialog?.type !== "quoteResult") return;
    setQuoteOffers([]);
    api(`/quotes/${encodeURIComponent(dialog.quoteId)}/offers`).then(setQuoteOffers).catch(() => {});
  }, [dialog]);
  const toast = (message) => { setNotice(message); window.setTimeout(() => setNotice(""), 2500); };
  const visibleShops = shops.filter((shop) =>
    (category === "전체" || shop.category === category) &&
    (!district || `${shop.district} ${shop.address}`.toLowerCase().includes(district.toLowerCase())) &&
    `${shop.name} ${shop.district} ${shop.category}`.toLowerCase().includes(query.toLowerCase()),
  );
  async function submitLead(event) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    try {
      const endpoint = dialog.type === "quote" ? "/quotes" : dialog.type === "order" ? "/orders" : "/bookings";
      const payload = dialog.type === "quote"
        ? { vehicle: form.get("vehicle"), service_category: form.get("service_category"), description: form.get("description"), district: form.get("district"), customer_phone: form.get("phone") }
        : dialog.type === "order"
          ? { customer_name: form.get("name"), customer_phone: form.get("phone"), address: form.get("address"), items: cart.map(({ id, quantity }) => ({ product_id: id, quantity })) }
          : { workshop_id: dialog.shop.id, vehicle: form.get("vehicle"), starts_at: `${form.get("date")}T${form.get("time")}:00`, customer_name: form.get("name"), customer_phone: form.get("phone"), note: form.get("note") };
      const result = await api(endpoint, { method: "POST", body: JSON.stringify(payload) });
      if (dialog.type === "order") { setCart([]); toast(`주문이 접수됐어요. 주문번호 ${result.id}`); setDialog(null); }
      else if (dialog.type === "quote") {
        const saved = JSON.parse(localStorage.getItem("gageyo_quote_ids") || "[]");
        localStorage.setItem("gageyo_quote_ids", JSON.stringify([result.id, ...saved.filter((id) => id !== result.id)].slice(0, 20)));
        toast("견적 요청을 등록했어요."); setDialog({ type: "quoteResult", quoteId: result.id });
      } else { toast("예약 요청을 등록했어요."); setDialog(null); }
    } catch (error) { toast(error.message || "요청을 저장하지 못했어요. 잠시 후 다시 시도해 주세요."); }
    if (dialog.type !== "quote") setDialog(null);
  }
  function addToCart(product) {
    setCart((items) => items.some((item) => item.id === product.id)
      ? items.map((item) => item.id === product.id ? { ...item, quantity: item.quantity + 1 } : item)
      : [...items, { ...product, quantity: 1 }]);
    toast("장바구니에 담았어요.");
  }

  return <>
    <header className="topbar"><a className="brand" href="#top"><span className="brand-mark">✳</span>가게요</a><nav><button onClick={() => { setTab("shops"); document.querySelector("#shops")?.scrollIntoView({ behavior: "smooth" }); }}>파트너 찾기</button><button onClick={() => { setTab("products"); document.querySelector("#products")?.scrollIntoView({ behavior: "smooth" }); }}>자동차 용품</button><button onClick={() => setDialog({ type: "quote" })}>견적 요청</button></nav><div className="top-actions"><button className="text-button" onClick={() => setDialog({ type: "quoteHistory" })}>내 견적</button><button className="text-button" onClick={() => setDialog({ type: "orderLookup" })}>주문 조회</button><button className="text-button" onClick={() => toast(`장바구니 ${cart.reduce((sum, item) => sum + item.quantity, 0)}개`)}>장바구니 ({cart.reduce((sum, item) => sum + item.quantity, 0)})</button><a className="dark-button" href="https://partner.gageyo.com/">파트너 센터</a></div></header>
    <main id="top"><section className="hero"><div className="hero-photo"/><div className="hero-content"><span className="eyebrow">CAR CARE, CLOSER TO YOU</span><h1>내 차 관리,<br/>동네에서 더 똑똑하게.</h1><p>믿을 수 있는 정비소를 찾고, 여러 곳의 견적을 한 번에 받아보세요.<br/>자동차에 필요한 상품과 예약까지 한곳에서.</p><form className="search" onSubmit={(e) => { e.preventDefault(); document.querySelector("#shops")?.scrollIntoView({ behavior: "smooth" }); }}><span>⌕</span><input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="정비소, 서비스 또는 동네 검색"/><button>가까운 업체 찾기</button></form><div className="stats"><span><b>1,240+</b> 등록 업체</span><span><b>4.9</b> 평균 만족도</span><span><b>무료</b> 견적 요청</span></div></div></section>
      <section className="main-wrap">{apiError && <div className="empty-shops" role="status">{apiError}</div>}<div className="tabs"><button className={tab === "shops" ? "active" : ""} onClick={() => setTab("shops")}>파트너 찾기</button><button className={tab === "products" ? "active" : ""} onClick={() => setTab("products")}>자동차 용품</button><button onClick={() => setDialog({ type: "quote" })}>견적 한 번에 받기 ↗</button></div>
        {tab === "shops" ? <section id="shops"><div className="category-browser"><div className="category-groups">{[...new Set(categories.map((item) => item.group_name))].map((group) => <button key={group} className={activeCategoryGroup === group ? "active" : ""} onClick={() => setActiveCategoryGroup(group)}>{group}</button>)}</div><div className="category-content"><div className="category-heading"><h3>{activeCategoryGroup}</h3><button onClick={() => setCategory("전체")}>{category === "전체" ? "전체 서비스 선택됨" : "전체 서비스 보기"}</button></div><div className="category-links">{categories.filter((item) => item.group_name === activeCategoryGroup).map((item) => <button key={item.id} className={category === item.name ? "chosen" : ""} onClick={() => setCategory(category === item.name ? "전체" : item.name)}>{item.name}<span>›</span></button>)}</div></div></div><div className="section-title"><div><h2>{category === "전체" ? "가게요 파트너" : `${category} 파트너`}</h2><p>지역과 전문 분야로 업체를 찾아보세요.</p><input aria-label="지역 검색" value={district} onChange={(e) => setDistrict(e.target.value)} placeholder="지역 입력 (예: 강남구, 역삼동)"/></div><span className="location">파트너 {visibleShops.length}곳</span></div><div className="shop-grid">{visibleShops.map((shop) => <article className="shop-card" key={shop.id}><div className="shop-photo" style={{ backgroundImage: `url(${photo(shop.photo || "photo-1486262715619-67b85e0b08d3")})` }}><span>{shop.category}</span><button aria-label="관심 업체" onClick={(e) => { e.currentTarget.classList.toggle("saved"); toast("관심 업체는 현재 브라우저에 표시됩니다."); }}>♡</button></div><div className="shop-info"><div className="shop-heading"><div><h3>{shop.name}</h3><p>{shop.district} · {shop.address}</p></div><b className="rating">★ {Number(shop.rating).toFixed(1)} <small>({shop.review_count})</small></b></div><div className="service-row"><span>{shop.category}</span><button onClick={() => setDialog({ type: "booking", shop })}>예약하기 →</button></div></div></article>)}</div>{visibleShops.length === 0 && <div className="empty-shops">현재 등록된 파트너가 없거나 검색 결과가 없습니다.</div>}<div className="quote-banner"><div><b>여러 파트너의 견적을 한 번에 받아보세요</b><p>차량과 요청 내용을 적으면 업체가 견적을 보내드립니다.</p></div><button onClick={() => setDialog({ type: "quote" })}>무료 견적 요청하기</button></div></section>
        : <section id="products"><div className="section-title"><div><h2>파트너 판매 상품</h2><p>가게요 파트너가 등록한 차량 상품을 확인하세요.</p></div><span className="location">상품 {products.length}개</span></div><div className="product-grid">{products.map((product) => <article className="product-card" key={product.id}><div className="product-photo" style={{ backgroundImage: `url(${photo(product.photo || "photo-1601362840469-51e4d8d58785")})` }}/><small>{product.category}</small><h3>{product.name}</h3><div className="product-price">{money(product.price)}<button onClick={() => addToCart(product)}>＋</button></div></article>)}</div>{products.length > 0 && <div className="quote-banner"><div><b>장바구니 {cart.reduce((sum, item) => sum + item.quantity, 0)}개 · 합계 {money(cart.reduce((sum, item) => sum + item.price * item.quantity, 0))}</b><p>주문을 접수하면 업체가 확인 후 연락드립니다.</p></div><button disabled={!cart.length} onClick={() => setDialog({ type: "order" })}>주문하기</button></div>}{!products.length && <div className="empty-shops">판매 중인 상품이 아직 없습니다.</div>}</section>}
      </section>
    </main><footer>가게요 · 우리 동네 자동차 관리 플랫폼 <span>정비소 찾기 · 맞춤 견적 · 상품 구매 · 간편 예약</span></footer>
    {dialog && <div className="overlay" onMouseDown={(e) => { if (e.target === e.currentTarget) setDialog(null); }}><section className="dialog"><div className="dialog-title"><div><span className="eyebrow">GAGEYO</span><h2>{dialog.type === "quote" ? "파트너에게 견적 요청" : dialog.type === "booking" ? `${dialog.shop.name} 예약 요청` : dialog.type === "order" ? "주문 정보 입력" : dialog.type === "quoteResult" ? "견적 요청 접수 완료" : dialog.type === "quoteHistory" ? "내 견적 요청" : "주문 조회"}</h2></div><button className="close" onClick={() => setDialog(null)}>×</button></div>{dialog.type === "quoteHistory" ? <div><p>이 브라우저에 저장된 최근 견적 요청 번호입니다.</p>{(JSON.parse(localStorage.getItem("gageyo_quote_ids") || "[]")).map((id) => <button className="history-row" key={id} onClick={() => setDialog({ type: "quoteResult", quoteId: id })}>{id} · 견적 확인 →</button>)}{!JSON.parse(localStorage.getItem("gageyo_quote_ids") || "[]").length && <p>저장된 견적 요청이 없습니다.</p>}</div> : dialog.type === "quoteResult" ? <div><p>요청 번호: <b>{dialog.quoteId}</b></p><p>파트너 견적 {quoteOffers.length}건</p>{quoteOffers.map((offer) => <article className="history-row" key={offer.id}><b>{offer.workshop_name} · {money(offer.amount)}</b><span>{offer.description}</span></article>)}{!quoteOffers.length && <p>아직 도착한 견적이 없습니다. 나중에 내 견적에서 다시 확인할 수 있어요.</p>}</div> : dialog.type === "orderLookup" ? <form onSubmit={async (event) => { event.preventDefault(); const data = new FormData(event.currentTarget); try { const order = await api(`/orders/${encodeURIComponent(data.get("orderId"))}`); toast(`주문 상태: ${order.status} · ${money(order.total_amount)}`); setDialog(null); } catch (error) { toast(error.message || "주문을 찾지 못했습니다."); } }}><label>주문 번호<input name="orderId" required placeholder="주문 접수 후 받은 번호"/></label><button className="submit-button">주문 상태 확인</button></form> : <><p>{dialog.type === "order" ? "주문 접수 후 파트너가 확인합니다. 결제는 아직 연결되지 않았습니다." : "차량과 요청 내용을 알려주시면 파트너가 확인 후 연락드립니다."}</p><form onSubmit={submitLead}>{dialog.type === "quote" && <><label>서비스 종류<select name="service_category">{categories.map((item) => <option key={item.id}>{item.name}</option>)}</select></label><label>지역<input name="district" required placeholder="예: 서울 강남구 역삼동"/></label><label>요청 내용<textarea name="description" required placeholder="차량 증상이나 원하는 작업을 적어주세요."/></label></>}{dialog.type === "order" && <label>배송 주소<input name="address" required placeholder="도로명 주소와 상세 주소"/></label>}{dialog.type !== "order" && <label>차량 정보<input name="vehicle" required placeholder="예: 현대 아반떼 2021년식"/></label>}{dialog.type === "booking" && <div className="date-row"><label>방문 날짜<input name="date" type="date" required min={new Date().toISOString().slice(0, 10)}/></label><label>희망 시간<input name="time" type="time" required/></label></div>}<label>이름<input name="name" required placeholder="성함"/></label><label>연락처<input name="phone" required type="tel" placeholder="010-0000-0000"/></label>{dialog.type === "booking" && <label>추가 메모<textarea name="note" placeholder="방문 전 전달할 내용"/></label>}<button className="submit-button">{dialog.type === "quote" ? "무료 견적 요청하기" : dialog.type === "order" ? "주문 접수" : "예약 신청하기"}</button></form></>}</section></div>}
    {notice && <div className="toast">{notice}</div>}
  </>;
}
