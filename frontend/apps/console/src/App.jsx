import { useEffect, useState } from "react";
import { api, money } from "@gageyo/ui";

const emptyProduct = { name: "", category: "차량용품", description: "", price: "", stock: "", image_url: "" };
const emptyService = { name: "", description: "", price_from: "", duration_minutes: "60", sort_order: "0" };
const nav = [["overview", "▦", "대시보드"], ["quotes", "☷", "견적 요청"], ["bookings", "▣", "예약 일정"], ["services", "◷", "서비스 관리"], ["products", "◇", "상품 관리"], ["orders", "▤", "주문 관리"], ["profile", "⚙", "업체 정보"]];

export default function App() {
  const [tab, setTab] = useState("overview");
  const [partnerKey, setPartnerKey] = useState(() => sessionStorage.getItem("gageyo_partner_key") || "");
  const [keyDraft, setKeyDraft] = useState(() => sessionStorage.getItem("gageyo_partner_key") || "");
  const [workshop, setWorkshop] = useState(null);
  const [quotes, setQuotes] = useState([]);
  const [bookings, setBookings] = useState([]);
  const [products, setProducts] = useState([]);
  const [services, setServices] = useState([]);
  const [orders, setOrders] = useState([]);
  const [productForm, setProductForm] = useState(emptyProduct);
  const [serviceForm, setServiceForm] = useState(emptyService);
  const [editingProduct, setEditingProduct] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [connected, setConnected] = useState(false);

  function flash(message) { setNotice(message); window.setTimeout(() => setNotice(""), 2800); }
  const partnerHeaders = { "x-partner-key": partnerKey };

  async function connect(key = keyDraft) {
    setLoading(true); setError("");
    try {
      const me = await api("/partner/me", { headers: { "x-partner-key": key } });
      setWorkshop(me); setPartnerKey(key); setKeyDraft(key); setConnected(true);
      sessionStorage.setItem("gageyo_partner_key", key);
    } catch (e) { setConnected(false); setError(e.message || "파트너 키를 확인해 주세요."); }
    finally { setLoading(false); }
  }

  async function loadData() {
    if (!connected || !partnerKey) return;
    setLoading(true); setError("");
    try {
      const headers = partnerHeaders;
      const [me, quoteRows, bookingRows, serviceRows, productRows, orderRows] = await Promise.all([
        api("/partner/me", { headers }), api("/partner/quotes", { headers }), api("/partner/bookings", { headers }),
        api("/partner/services", { headers }), api("/partner/products", { headers }), api("/partner/orders", { headers }),
      ]);
      setWorkshop(me); setQuotes(quoteRows); setBookings(bookingRows); setServices(serviceRows); setProducts(productRows); setOrders(orderRows);
    } catch (e) { setError(e.message || "파트너 정보를 불러오지 못했습니다."); if (e.message?.includes("파트너 키")) { setConnected(false); sessionStorage.removeItem("gageyo_partner_key"); } }
    finally { setLoading(false); }
  }

  useEffect(() => { if (partnerKey) connect(partnerKey); }, []);
  useEffect(() => { if (connected) loadData(); }, [connected, partnerKey]);

  async function answerQuote(quote) {
    const amount = window.prompt(`${quote.vehicle} · ${quote.service_category}\n견적 금액을 입력하세요.`);
    if (amount === null) return;
    const description = window.prompt("작업 범위와 안내 사항을 입력하세요.");
    if (description === null) return;
    try {
      await api(`/partner/quotes/${quote.id}/offers`, { method: "POST", headers: partnerHeaders, body: JSON.stringify({ workshop_id: workshop.id, amount: Number(amount.replaceAll(",", "")), description }) });
      flash("견적을 고객에게 전달했습니다."); await loadData();
    } catch (e) { flash(e.message || "견적을 보내지 못했습니다."); }
  }

  async function updateBooking(id, status) {
    try { await api(`/partner/bookings/${id}`, { method: "PATCH", headers: partnerHeaders, body: JSON.stringify({ status }) }); flash("예약 상태를 변경했습니다."); await loadData(); }
    catch (e) { flash(e.message || "예약을 변경하지 못했습니다."); }
  }

  async function saveProduct(event) {
    event.preventDefault();
    const payload = { ...productForm, price: Number(productForm.price), stock: Number(productForm.stock), image_url: productForm.image_url || null };
    try {
      await api(editingProduct ? `/partner/products/${editingProduct}` : "/partner/products", { method: editingProduct ? "PATCH" : "POST", headers: partnerHeaders, body: JSON.stringify(payload) });
      setProductForm(emptyProduct); setEditingProduct(null); flash("상품을 저장했습니다."); await loadData();
    } catch (e) { flash(e.message || "상품을 저장하지 못했습니다."); }
  }
  async function saveService(event) {
    event.preventDefault();
    const payload = { ...serviceForm, price_from: Number(serviceForm.price_from), duration_minutes: Number(serviceForm.duration_minutes), sort_order: Number(serviceForm.sort_order) };
    try {
      await api("/partner/services", { method: "POST", headers: partnerHeaders, body: JSON.stringify(payload) });
      setServiceForm(emptyService); flash("서비스를 등록했습니다."); await loadData();
    } catch (e) { flash(e.message || "서비스를 등록하지 못했습니다."); }
  }

  if (!connected) return <main className="partner-connect"><section className="panel"><a className="logo" href="https://app.gageyo.com/">✳ 가게요</a><h1>파트너 센터 로그인</h1><p>운영자가 발급한 파트너 키를 입력해 주세요. 키는 이 브라우저 세션에만 보관됩니다.</p><form onSubmit={(e) => { e.preventDefault(); connect(); }}><label>파트너 키<input type="password" value={keyDraft} onChange={(e) => setKeyDraft(e.target.value)} required autoComplete="current-password"/></label><button className="primary" disabled={loading}>{loading ? "확인 중…" : "파트너 센터 열기"}</button></form>{error && <p role="alert">{error}</p>}</section></main>;

  const pageTitle = Object.fromEntries(nav.map(([id, , label]) => [id, label]));
  return <div className="layout"><aside><a className="logo" href="https://app.gageyo.com/">✳ 가게요</a><div className="shop-switch"><span>파트너 관리</span><b>{workshop?.name || "업체"}</b></div><small>관리 메뉴</small>{nav.map(([id, icon, label]) => <button key={id} onClick={() => setTab(id)} className={tab === id ? "on" : ""}><span>{icon}</span>{label}{id === "quotes" && quotes.length > 0 && <i>{quotes.length}</i>}</button>)}<div className="aside-bottom"><div className="avatar">{workshop?.name?.slice(0, 1) || "가"}</div><div><b>{workshop?.name}</b><small>{workshop?.district} · {workshop?.status === "approved" ? "승인 완료" : "승인 대기"}</small></div><button onClick={() => { sessionStorage.removeItem("gageyo_partner_key"); setPartnerKey(""); setConnected(false); }}>나가기</button></div></aside><main><header><div><span className="crumb">파트너 센터　/　</span><b>{pageTitle[tab]}</b></div><div className="header-right"><span className="open-pill">● {workshop?.status === "approved" ? "운영 중" : "승인 대기"}</span><button onClick={loadData} aria-label="새로고침">↻</button><a href="https://app.gageyo.com/">파트너 찾기 ↗</a></div></header><div className="content"><div className="welcome"><div><h1>{tab === "overview" ? `${workshop?.name} 파트너 대시보드` : `${pageTitle[tab]} 관리`}</h1><p>{workshop?.district} · {workshop?.category} · 데이터는 가게요 API에서 불러옵니다.</p></div><button className="primary" onClick={() => setTab("products")}>＋ 상품 등록</button></div>
    {error && <div className="tip" role="alert">{error}</div>}{loading && <div className="tip">데이터를 불러오는 중…</div>}
    {tab === "overview" && <><section className="metrics">{[["견적 대기", `${quotes.length}건`, "응답 가능한 요청"], ["예약", `${bookings.length}건`, "현재 파트너 예약"], ["서비스", `${services.length}개`, "예약 가능한 작업"], ["판매 상품", `${products.length}개`, "등록된 상품"]].map(([label, value, note]) => <article key={label}><span>{label}</span><b>{value}</b><small>{note}</small></article>)}</section><section className="panel"><div className="panel-title"><div><h2>새 견적 요청</h2><p>지역과 서비스가 맞는 고객 요청입니다.</p></div><button onClick={() => setTab("quotes")}>전체 보기 →</button></div>{quotes.length ? quotes.slice(0, 5).map((item) => <div className="tr" key={item.id}><b>{item.vehicle}</b><span>{item.service_category} · {item.district}</span><span>{item.description}</span><button className="primary" onClick={() => answerQuote(item)}>견적 보내기</button></div>) : <p>새 견적 요청이 없습니다.</p>}</section></>}
    {tab === "quotes" && <section className="panel"><div className="panel-title"><div><h2>견적 요청</h2><p>업체 지역과 전문 분야가 일치하는 요청만 표시합니다.</p></div><button onClick={loadData}>↻ 새로고침</button></div>{quotes.map((item) => <article className="tr" key={item.id}><span><b>{item.vehicle}</b><small>{item.district} · {item.customer_phone}</small></span><span>{item.service_category}</span><span>{item.description}</span><button className="primary" onClick={() => answerQuote(item)}>견적 제출</button></article>)}{!quotes.length && <p>대기 중인 견적 요청이 없습니다.</p>}</section>}
    {tab === "bookings" && <section className="panel"><div className="panel-title"><div><h2>예약 일정</h2><p>고객 예약 요청을 확정하거나 거절할 수 있습니다.</p></div><button onClick={loadData}>↻ 새로고침</button></div>{bookings.map((item) => <article className="tr" key={item.id}><span><b>{item.customer_name}</b><small>{item.customer_phone} · {item.vehicle}</small></span><span>{new Date(item.starts_at).toLocaleString("ko-KR")}</span><span>{item.note || "메모 없음"}</span><span>{item.status}</span><select value={item.status} onChange={(e) => updateBooking(item.id, e.target.value)}><option value="requested">요청됨</option><option value="confirmed">확정</option><option value="completed">완료</option><option value="declined">거절</option><option value="cancelled">취소</option></select></article>)}{!bookings.length && <p>예약 내역이 없습니다.</p>}</section>}
    {tab === "services" && <><section className="panel settings"><h2>예약 서비스 등록</h2><form onSubmit={saveService}><label>서비스명<input required value={serviceForm.name} onChange={(e) => setServiceForm({ ...serviceForm, name: e.target.value })}/></label><label>설명<textarea value={serviceForm.description} onChange={(e) => setServiceForm({ ...serviceForm, description: e.target.value })}/></label><label>시작 가격<input type="number" min="1" required value={serviceForm.price_from} onChange={(e) => setServiceForm({ ...serviceForm, price_from: e.target.value })}/></label><label>예상 소요 시간 (분)<input type="number" min="1" required value={serviceForm.duration_minutes} onChange={(e) => setServiceForm({ ...serviceForm, duration_minutes: e.target.value })}/></label><button className="primary">서비스 등록</button></form></section><section className="panel"><h2>등록 서비스</h2>{services.map((item) => <article className="tr" key={item.id}><span><b>{item.name}</b><small>{item.description}</small></span><span>부터 {money(item.price_from)}</span><span>{item.duration_minutes}분</span></article>)}{!services.length && <p>등록한 서비스가 없습니다.</p>}</section></>}
    {tab === "products" && <><section className="panel settings"><h2>{editingProduct ? "상품 수정" : "상품 등록"}</h2><form onSubmit={saveProduct}><label>상품명<input required value={productForm.name} onChange={(e) => setProductForm({ ...productForm, name: e.target.value })}/></label><label>분류<input required value={productForm.category} onChange={(e) => setProductForm({ ...productForm, category: e.target.value })}/></label><label>설명<textarea value={productForm.description} onChange={(e) => setProductForm({ ...productForm, description: e.target.value })}/></label><label>가격<input type="number" min="1" required value={productForm.price} onChange={(e) => setProductForm({ ...productForm, price: e.target.value })}/></label><label>재고<input type="number" min="0" required value={productForm.stock} onChange={(e) => setProductForm({ ...productForm, stock: e.target.value })}/></label><label>이미지 URL<input type="url" value={productForm.image_url} onChange={(e) => setProductForm({ ...productForm, image_url: e.target.value })}/></label><button className="primary">{editingProduct ? "변경 저장" : "상품 등록"}</button>{editingProduct && <button type="button" onClick={() => { setEditingProduct(null); setProductForm(emptyProduct); }}>취소</button>}</form></section><section className="panel"><div className="panel-title"><div><h2>등록 상품</h2><p>재고가 0인 상품은 자동으로 품절 처리됩니다.</p></div></div>{products.map((item) => <article className="tr" key={item.id}><span><b>{item.name}</b><small>{item.category}</small></span><span>{money(item.price)}</span><span>재고 {item.stock}</span><span>{item.stock === 0 ? "품절" : "판매 중"}</span><button onClick={() => { setEditingProduct(item.id); setProductForm({ name: item.name, category: item.category, description: item.description, price: String(item.price), stock: String(item.stock), image_url: item.image_url || "" }); window.scrollTo({ top: 0, behavior: "smooth" }); }}>수정</button></article>)}{!products.length && <p>등록한 상품이 없습니다.</p>}</section></>}
    {tab === "orders" && <section className="panel"><div className="panel-title"><div><h2>주문 관리</h2><p>결제 연동 전 주문은 접수 상태이며, 결제가 완료되지 않았습니다.</p></div><button onClick={loadData}>↻ 새로고침</button></div>{orders.map((item) => <article className="tr" key={`${item.id}-${item.product_name}`}><span><b>#{item.id.slice(0, 8)} · {item.product_name}</b><small>{item.customer_name} · {item.customer_phone}</small></span><span>{item.quantity}개</span><span>{item.address}</span><span>{item.status}</span><span>{money(item.total_amount)}</span></article>)}{!orders.length && <p>주문 내역이 없습니다.</p>}</section>}
    {tab === "profile" && <section className="panel settings"><h2>업체 정보</h2><label>업체명<input value={workshop?.name || ""} readOnly/></label><label>지역<input value={workshop?.district || ""} readOnly/></label><label>주요 업종<input value={workshop?.category || ""} readOnly/></label><p>업체 기본 정보 수정은 운영자 확인 후 제공됩니다.</p></section>}
    </div></main>{notice && <div className="toast">{notice}</div>}</div>;
}
