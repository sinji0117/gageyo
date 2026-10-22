import { useEffect, useState } from "react";
import { api, money } from "@gageyo/ui";

const demoShops = [
  { id: "demo-1", name: "오토랩 강남점", district: "역삼동", address: "서울 강남구 테헤란로", category: "자동차 정비", rating: 4.9, review_count: 218, distance: "0.8km", service: "엔진오일 · 종합 점검", price: 55000, photo: "photo-1486262715619-67b85e0b08d3" },
  { id: "demo-2", name: "카인드 디테일링", district: "논현동", address: "서울 강남구 논현로", category: "디테일링세차/광택", rating: 5.0, review_count: 146, distance: "1.2km", service: "프리미엄 세차 · 광택", price: 45000, photo: "photo-1607860108855-64acf2078ed9" },
  { id: "demo-3", name: "타이어프로 대치점", district: "대치동", address: "서울 강남구 삼성로", category: "자동차 정비", rating: 4.8, review_count: 302, distance: "1.7km", service: "타이어 교체 · 얼라인먼트", price: 0, photo: "photo-1632823471565-1ecdf5c7f6a1" },
  { id: "demo-4", name: "EV 케어 스테이션", district: "삼성동", address: "서울 강남구 영동대로", category: "전기차충전기 설치 및 수리", rating: 4.9, review_count: 87, distance: "2.1km", service: "전기차 정비 · 배터리 점검", price: 39000, photo: "photo-1593941707882-a5bba14938c7" },
];
const fallbackCategories = [
  ...["디테일링세차/광택", "스팀 세차", "오토바이 튜닝", "자동차 썬팅", "자동차 정비", "전기차충전기 설치 및 수리", "블랙박스/내비게이션 설치 및 수리", "오토바이 정비 및 수리", "자동차 랩핑", "자동차 외부수리", "자동차 튜닝", "출장 세차"].map((name, i) => ({ id: `care-${i}`, group_name: "차량 관리", name, sort_order: i * 10 })),
  ...["신차 구매", "자동차 구매동행", "자동차 리스/렌트", "중고차 구매", "중고차 판매", "캠핑카 제작"].map((name, i) => ({ id: `sale-${i}`, group_name: "자동차 매매", name, sort_order: i * 10 })),
];
const demoProducts = [
  { id: "product-1", name: "프리미엄 카샴푸 세트", category: "세차용품", price: 29000, photo: "photo-1607860108855-64acf2078ed9" },
  { id: "product-2", name: "차량용 무선 청소기", category: "차량용품", price: 48900, photo: "photo-1601362840469-51e4d8d58785" },
  { id: "product-3", name: "올시즌 타이어 케어", category: "안전·정비", price: 12500, photo: "photo-1558618666-fcd25c85cd64" },
  { id: "product-4", name: "대시보드 보호 코팅제", category: "차량용품", price: 18000, photo: "photo-1619642751034-765dfdf7c58e" },
];
const photo = (id) => `https://images.unsplash.com/${id}?auto=format&fit=crop&w=800&q=82`;

export default function App() {
  const [shops, setShops] = useState(demoShops);
  const [products, setProducts] = useState(demoProducts);
  const [categories, setCategories] = useState(fallbackCategories);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("전체");
  const [activeCategoryGroup, setActiveCategoryGroup] = useState("차량 관리");
  const [tab, setTab] = useState("shops");
  const [dialog, setDialog] = useState(null);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    api("/workshops").then(setShops).catch(() => {});
    api("/products").then(setProducts).catch(() => {});
    api("/categories").then((items) => { if (items.length) setCategories(items); }).catch(() => {});
  }, []);
  const toast = (message) => { setNotice(message); window.setTimeout(() => setNotice(""), 2500); };
  const visibleShops = shops.filter((shop) =>
    (category === "전체" || shop.category === category) &&
    `${shop.name} ${shop.district} ${shop.category}`.toLowerCase().includes(query.toLowerCase()),
  );
  async function submitLead(event) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    try {
      const endpoint = dialog.type === "quote" ? "/quotes" : "/bookings";
      const payload = dialog.type === "quote"
        ? { vehicle: form.get("vehicle"), service_category: form.get("service_category"), description: form.get("description"), district: form.get("district"), customer_phone: form.get("phone") }
        : { workshop_id: dialog.shop.id, vehicle: form.get("vehicle"), starts_at: `${form.get("date")}T${form.get("time")}:00`, customer_name: form.get("name"), customer_phone: form.get("phone"), note: form.get("note") };
      await api(endpoint, { method: "POST", body: JSON.stringify(payload) });
      toast(dialog.type === "quote" ? "견적 요청을 등록했어요." : "예약 요청을 등록했어요.");
    } catch { toast("API 연결 전 미리보기입니다. 요청 내용은 저장되지 않았어요."); }
    setDialog(null);
  }

  return <>
    <header className="topbar"><a className="brand" href="#top"><span className="brand-mark">✳</span>carcare</a><nav><button onClick={() => { setTab("shops"); document.querySelector("#shops")?.scrollIntoView({ behavior: "smooth" }); }}>정비소 찾기</button><button onClick={() => { setTab("products"); document.querySelector("#products")?.scrollIntoView({ behavior: "smooth" }); }}>자동차 용품</button><button onClick={() => setDialog({ type: "quote" })}>견적 요청</button></nav><div className="top-actions"><button className="text-button" onClick={() => toast("회원 기능을 연결하는 중입니다.")}>로그인</button><a className="dark-button" href="http://localhost:5174">업체 등록</a></div></header>
    <main id="top"><section className="hero"><div className="hero-photo"/><div className="hero-content"><span className="eyebrow">CAR CARE, CLOSER TO YOU</span><h1>내 차 관리,<br/>동네에서 더 똑똑하게.</h1><p>믿을 수 있는 정비소를 찾고, 여러 곳의 견적을 한 번에 받아보세요.<br/>자동차에 필요한 상품과 예약까지 한곳에서.</p><form className="search" onSubmit={(e) => { e.preventDefault(); document.querySelector("#shops")?.scrollIntoView({ behavior: "smooth" }); }}><span>⌕</span><input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="정비소, 서비스 또는 동네 검색"/><button>가까운 업체 찾기</button></form><div className="stats"><span><b>1,240+</b> 등록 업체</span><span><b>4.9</b> 평균 만족도</span><span><b>무료</b> 견적 요청</span></div></div></section>
      <section className="main-wrap"><div className="tabs"><button className={tab === "shops" ? "active" : ""} onClick={() => setTab("shops")}>동네 정비소</button><button className={tab === "products" ? "active" : ""} onClick={() => setTab("products")}>자동차 용품</button><button onClick={() => setDialog({ type: "quote" })}>견적 한 번에 받기 ↗</button></div>
        {tab === "shops" ? <section id="shops"><div className="category-browser"><div className="category-groups">{[...new Set(categories.map((item) => item.group_name))].map((group) => <button key={group} className={activeCategoryGroup === group ? "active" : ""} onClick={() => setActiveCategoryGroup(group)}>{group}</button>)}</div><div className="category-content"><div className="category-heading"><h3>{activeCategoryGroup}</h3><button onClick={() => setCategory("전체")}>{category === "전체" ? "전체 서비스 선택됨" : "전체 서비스 보기"}</button></div><div className="category-links">{categories.filter((item) => item.group_name === activeCategoryGroup).map((item) => <button key={item.id} className={category === item.name ? "chosen" : ""} onClick={() => setCategory(category === item.name ? "전체" : item.name)}>{item.name}<span>›</span></button>)}</div></div></div><div className="section-title"><div><h2>{category === "전체" ? "내 주변 인기 정비소" : `${category} 업체`}</h2><p>서울 강남구 기준 · 후기와 거리로 골라보세요</p></div><span className="location">⌖ 서울 강남구</span></div><div className="shop-grid">{visibleShops.map((shop) => <article className="shop-card" key={shop.id}><div className="shop-photo" style={{ backgroundImage: `url(${photo(shop.photo || "photo-1486262715619-67b85e0b08d3")})` }}><span>{shop.category.includes("전기차") ? "전기차 전문" : "오늘 예약 가능"}</span><button aria-label="관심 업체" onClick={(e) => { e.currentTarget.classList.toggle("saved"); toast("관심 업체를 저장했어요."); }}>♡</button></div><div className="shop-info"><div className="shop-heading"><div><h3>{shop.name}</h3><p>{shop.district} · {shop.distance || "주변 업체"} · {shop.category}</p></div><b className="rating">★ {Number(shop.rating).toFixed(1)} <small>({shop.review_count})</small></b></div><div className="service-row"><span>{shop.service || shop.address}</span><button onClick={() => setDialog({ type: "booking", shop })}>예약하기 →</button></div></div></article>)}</div>{visibleShops.length === 0 && <div className="empty-shops">선택한 카테고리에 등록된 업체가 아직 없어요.</div>}<div className="quote-banner"><div><b>여러 업체 견적을 한 번에 받아보세요</b><p>정비 내용을 남기면 주변 업체가 맞춤 견적을 보내드려요.</p></div><button onClick={() => setDialog({ type: "quote" })}>무료 견적 요청하기</button></div></section>
        : <section id="products"><div className="section-title"><div><h2>차량 관리 베스트 상품</h2><p>동네 업체가 직접 추천하는 차량 관리 상품</p></div><span className="location">상품 판매 업체 {products.length}곳</span></div><div className="product-grid">{products.map((product) => <article className="product-card" key={product.id}><div className="product-photo" style={{ backgroundImage: `url(${photo(product.photo || "photo-1601362840469-51e4d8d58785")})` }}/><small>{product.category}</small><h3>{product.name}</h3><div className="product-price">{money(product.price)}<button onClick={() => toast("장바구니 기능은 주문 화면과 연결하는 중입니다.")}>＋</button></div></article>)}</div></section>}
      </section>
    </main><footer>carcare · 우리 동네 자동차 관리 플랫폼 <span>정비소 찾기 · 맞춤 견적 · 상품 구매 · 간편 예약</span></footer>
    {dialog && <div className="overlay" onMouseDown={(e) => { if (e.target === e.currentTarget) setDialog(null); }}><section className="dialog"><div className="dialog-title"><div><span className="eyebrow">CARCARE REQUEST</span><h2>{dialog.type === "quote" ? "주변 업체에 견적 요청" : `${dialog.shop.name} 예약 요청`}</h2></div><button className="close" onClick={() => setDialog(null)}>×</button></div><p>차량과 요청 내용을 알려주시면 업체가 확인 후 연락드려요.</p><form onSubmit={submitLead}>{dialog.type === "quote" && <><label>서비스 종류<select name="service_category">{categories.map((item) => <option key={item.id}>{item.name}</option>)}</select></label><label>지역<input name="district" required placeholder="예: 서울 강남구 역삼동"/></label><label>요청 내용<textarea name="description" required placeholder="차량 증상이나 원하는 작업을 적어주세요."/></label></>}<label>차량 정보<input name="vehicle" required placeholder="예: 현대 아반떼 2021년식"/></label>{dialog.type === "booking" && <div className="date-row"><label>방문 날짜<input name="date" type="date" required/></label><label>희망 시간<input name="time" type="time" required/></label></div>}<label>이름<input name="name" required placeholder="성함"/></label><label>연락처<input name="phone" required type="tel" placeholder="010-0000-0000"/></label>{dialog.type === "booking" && <label>추가 메모<textarea name="note" placeholder="방문 전 전달할 내용"/></label>}<button className="submit-button">{dialog.type === "quote" ? "무료 견적 요청하기" : "예약 신청하기"}</button></form></section></div>}
    {notice && <div className="toast">{notice}</div>}
  </>;
}

