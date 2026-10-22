import { useEffect, useState } from "react";
import { api } from "@gageyo/ui";

const emptyForm = { group_name: "차량 관리", name: "", sort_order: 10, is_active: true };

export default function App() {
  const [tab, setTab] = useState("overview");
  const [notice, setNotice] = useState("");
  const [apiKey, setApiKey] = useState(() => sessionStorage.getItem("gageyo_admin_key") || "");
  const [keyDraft, setKeyDraft] = useState(() => sessionStorage.getItem("gageyo_admin_key") || "");
  const [connected, setConnected] = useState(false);
  const [categories, setCategories] = useState([]);
  const [form, setForm] = useState(emptyForm);
  const [editingId, setEditingId] = useState(null);
  const [loading, setLoading] = useState(false);
  const [records, setRecords] = useState({ workshops: [], quotes: [], bookings: [], products: [], orders: [] });
  const [recordsLoading, setRecordsLoading] = useState(false);
  function flash(message) { setNotice(message); window.setTimeout(() => setNotice(""), 2600); }

  async function loadCategories(key = apiKey) {
    if (!key) { setConnected(false); return; }
    setLoading(true);
    try {
      const rows = await api("/admin/categories", { headers: { "x-admin-key": key } });
      setCategories(rows); setConnected(true); setApiKey(key); setKeyDraft(key);
      sessionStorage.setItem("gageyo_admin_key", key);
    } catch (error) { setConnected(false); flash(error.message || "관리자 연결에 실패했습니다."); }
    finally { setLoading(false); }
  }
  useEffect(() => { if (apiKey) loadCategories(apiKey); }, []);
  async function loadRecords(key = apiKey) {
    setRecordsLoading(true);
    try {
      const headers = { "x-admin-key": key };
      const [workshops, quotes, bookings, products, orders] = await Promise.all([
        api("/admin/workshops", { headers }), api("/admin/quotes", { headers }), api("/admin/bookings", { headers }),
        api("/admin/products", { headers }), api("/admin/orders", { headers }),
      ]);
      setRecords({ workshops, quotes, bookings, products, orders });
    } catch (error) { flash(error.message || "운영 데이터를 불러오지 못했습니다."); }
    finally { setRecordsLoading(false); }
  }
  useEffect(() => {
    if (!apiKey || !connected || !["overview", "workshops", "quotes", "catalog"].includes(tab)) return;
    loadRecords(apiKey);
  }, [apiKey, connected, tab]);

  async function changeStatus(path, id, status) {
    try {
      await api(`${path}/${id}`, { method: "PATCH", headers: { "x-admin-key": apiKey }, body: JSON.stringify({ status }) });
      flash("상태를 변경했습니다.");
      await loadRecords(apiKey);
    } catch (error) { flash(error.message || "상태를 변경하지 못했습니다."); }
  }
  async function issuePartnerKey(workshop) {
    if (!window.confirm(`${workshop.name}의 파트너 키를 발급할까요? 기존 키가 있다면 즉시 무효화됩니다.`)) return;
    try {
      const result = await api(`/admin/workshops/${workshop.id}/partner-key`, { method: "POST", headers: { "x-admin-key": apiKey } });
      try { await navigator.clipboard.writeText(result.partner_key); } catch { /* clipboard may be unavailable on non-HTTPS preview */ }
      window.alert(`파트너 키가 발급되었습니다. 이 키는 다시 표시되지 않습니다.\n\n${result.partner_key}\n\n클립보드 복사를 시도했습니다. 업체에 안전하게 전달해 주세요.`);
    } catch (error) { flash(error.message || "파트너 키를 발급하지 못했습니다."); }
  }

  async function saveCategory(event) {
    event.preventDefault();
    const method = editingId ? "PUT" : "POST";
    const path = editingId ? `/admin/categories/${editingId}` : "/admin/categories";
    try {
      await api(path, { method, headers: { "x-admin-key": apiKey }, body: JSON.stringify({ ...form, sort_order: Number(form.sort_order) }) });
      setForm(emptyForm); setEditingId(null); await loadCategories(); flash("카테고리를 저장했습니다.");
    } catch (error) { flash(error.message || "저장하지 못했습니다."); }
  }
  async function removeCategory(item) {
    if (!window.confirm(`‘${item.name}’ 카테고리를 삭제할까요?`)) return;
    try { await api(`/admin/categories/${item.id}`, { method: "DELETE", headers: { "x-admin-key": apiKey } }); await loadCategories(); flash("카테고리를 삭제했습니다."); }
    catch (error) { flash(error.message || "삭제하지 못했습니다."); }
  }
  function editCategory(item) { setEditingId(item.id); setForm({ group_name: item.group_name, name: item.name, sort_order: item.sort_order, is_active: item.is_active }); window.scrollTo({ top: 0, behavior: "smooth" }); }

  const pageNames = { overview: "현황 대시보드", workshops: "업체 관리", users: "회원 관리", quotes: "견적·예약", catalog: "상품·주문", categories: "카테고리 관리", reports: "신고·문의" };
  return <div className="admin"><aside><div className="brand">✳ 가게요 <small>PLATFORM OPS</small></div>{[["overview", "▦", "현황 대시보드"], ["workshops", "⚒", "업체 관리"], ["users", "♙", "회원 관리"], ["quotes", "☷", "견적·예약"], ["catalog", "◇", "상품·주문"], ["categories", "▤", "카테고리"], ["reports", "⚑", "신고·문의"]].map(([id, icon, label]) => <button className={tab === id ? "selected" : ""} onClick={() => setTab(id)} key={id}><span>{icon}</span>{label}</button>)}<div className="admin-user">운영 관리자<small>플랫폼 운영</small></div></aside><main><header><div><small>ADMIN / </small><b>{pageNames[tab]}</b></div><button onClick={() => flash("운영자 계정 메뉴입니다.")}>운영자　⌄</button></header><section className="page"><div className="page-title"><div><h1>{tab === "overview" ? "플랫폼 현황" : pageNames[tab]}</h1><p>{tab === "categories" ? "고객 화면에 표시할 서비스와 매매 카테고리를 관리합니다." : "가게요 플랫폼의 주요 운영 지표를 확인하고 관리합니다."}</p></div>{tab !== "categories" && <button onClick={() => flash("최신 데이터로 갱신했습니다.")}>↻　새로고침</button>}</div>

    {tab !== "categories" && <article className="box admin-key-box"><div><h2>운영 데이터 연결</h2><p>Rpanel의 gageyo-api 앱에 설정한 ADMIN_API_KEY를 입력하면 실제 운영 자료를 불러옵니다.</p></div><div className="key-connect"><input type="password" value={keyDraft} onChange={(e) => setKeyDraft(e.target.value)} placeholder="ADMIN_API_KEY" autoComplete="current-password"/><button className="approve" onClick={() => loadCategories(keyDraft)}>{loading ? "확인 중…" : connected ? "다시 연결" : "연결"}</button>{connected && <span className="key-ok">● 연결됨</span>}</div></article>}
    {tab === "categories" ? <>
      <article className="box admin-key-box"><div><h2>관리자 연결</h2><p>Rpanel의 gageyo-api 앱에 설정한 ADMIN_API_KEY를 입력하세요. 키는 이 브라우저 탭에서만 보관됩니다.</p></div><div className="key-connect"><input type="password" value={keyDraft} onChange={(e) => setKeyDraft(e.target.value)} placeholder="ADMIN_API_KEY" autoComplete="current-password"/><button className="approve" onClick={() => loadCategories(keyDraft)}>{loading ? "확인 중…" : connected ? "다시 연결" : "연결"}</button>{connected && <span className="key-ok">● 연결됨</span>}</div></article>
      {connected && <div className="category-admin-grid"><article className="box category-form-box"><div className="box-title"><div><h2>{editingId ? "카테고리 수정" : "카테고리 추가"}</h2><p>고객 서비스 화면에 바로 반영됩니다.</p></div>{editingId && <button onClick={() => { setEditingId(null); setForm(emptyForm); }}>취소</button>}</div><form onSubmit={saveCategory}><label>분류 그룹<select value={form.group_name} onChange={(e) => setForm({ ...form, group_name: e.target.value })}><option>차량 관리</option><option>자동차 매매</option></select></label><label>카테고리명<input required maxLength={80} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="예: 자동차 썬팅"/></label><label>정렬 순서<input type="number" min="0" step="10" value={form.sort_order} onChange={(e) => setForm({ ...form, sort_order: e.target.value })}/></label><label className="check-label"><input type="checkbox" checked={form.is_active} onChange={(e) => setForm({ ...form, is_active: e.target.checked })}/>고객 화면에 표시</label><button className="primary">{editingId ? "변경 저장" : "카테고리 추가"}</button></form></article>
        <article className="box category-list-box"><div className="box-title"><div><h2>등록된 카테고리 <span className="count-pill">{categories.length}</span></h2><p>그룹과 정렬 순서대로 고객 화면에 표시됩니다.</p></div><button onClick={() => loadCategories()}>{loading ? "불러오는 중…" : "↻ 새로고침"}</button></div>{[...new Set(categories.map((item) => item.group_name))].map((group) => <section className="admin-category-group" key={group}><h3>{group}<small>{categories.filter((item) => item.group_name === group).length}개</small></h3>{categories.filter((item) => item.group_name === group).map((item) => <div className={`admin-category-row ${item.is_active ? "" : "inactive"}`} key={item.id}><span className="sort-number">{item.sort_order}</span><b>{item.name}</b><em>{item.is_active ? "노출 중" : "숨김"}</em><button onClick={() => editCategory(item)}>수정</button><button className="remove-category" onClick={() => removeCategory(item)}>삭제</button></div>)}</section>)}{!categories.length && <div className="category-empty">관리자 키를 연결하면 카테고리 목록이 표시됩니다.</div>}</article></div>}
    </> : !connected ? <article className="box"><h2>관리자 연결이 필요합니다</h2><p>관리자 키를 연결하면 이 화면에 실제 데이터와 관리 기능이 표시됩니다.</p></article> : <>
      {tab === "overview" && <div className="metric-grid">{[["등록 업체", records.workshops.length, "전체 등록 수"], ["견적 요청", records.quotes.length, "전체 요청 수"], ["예약", records.bookings.length, "전체 예약 수"], ["승인 대기", records.workshops.filter((item) => item.status === "pending").length, "검토가 필요합니다"]].map(([label, value, note]) => <article key={label}><span>{label}</span><b>{value}</b><small>{note}</small></article>)}</div>}
      {tab === "overview" || tab === "workshops" ? <article className="box"><div className="box-title"><div><h2>{tab === "overview" ? "업체 승인 요청" : "등록 업체"}</h2><p>실제 DB에 저장된 파트너 업체입니다. 승인 업체만 앱의 파트너 탐색에 노출됩니다.</p></div><button onClick={() => loadRecords()}>↻ 새로고침</button></div><div className="list"><div className="row head"><span>업체명</span><span>지역</span><span>업종</span><span>등록일</span><span>상태</span><span>관리</span></div>{(tab === "overview" ? records.workshops.filter((item) => item.status === "pending") : records.workshops).map((item) => <div className="row" key={item.id}><b>{item.name}</b><span>{item.district}</span><span>{item.category}</span><span>{new Date(item.created_at).toLocaleDateString("ko-KR")}</span><span>{item.status}</span><span>{item.status === "pending" ? <><button className="approve" onClick={() => changeStatus("/admin/workshops", item.id, "approved")}>승인</button><button onClick={() => changeStatus("/admin/workshops", item.id, "rejected")}>반려</button></> : item.status === "approved" ? <><button className="approve" onClick={() => issuePartnerKey(item)}>파트너 키 발급/재발급</button><button onClick={() => changeStatus("/admin/workshops", item.id, "suspended")}>중지</button></> : <button className="approve" onClick={() => changeStatus("/admin/workshops", item.id, "approved")}>승인</button>}</span></div>)}</div>{!records.workshops.length && <p>등록된 업체가 없습니다.</p>}</article> : null}
      {tab === "quotes" && <><article className="box"><div className="box-title"><div><h2>견적 요청</h2><p>고객이 제출한 요청과 연락처입니다.</p></div><span>{recordsLoading ? "불러오는 중…" : `${records.quotes.length}건`}</span></div><div className="list"><div className="row head"><span>차량</span><span>서비스 / 지역</span><span>요청 내용</span><span>연락처</span><span>상태</span></div>{records.quotes.map((item) => <div className="row" key={item.id}><b>{item.vehicle}</b><span>{item.service_category} · {item.district}</span><span>{item.description}</span><span>{item.customer_phone}</span><span>{item.status}</span></div>)}</div>{!records.quotes.length && <p>견적 요청이 없습니다.</p>}</article><article className="box"><div className="box-title"><div><h2>예약</h2><p>파트너가 접수한 예약을 운영자가 확인합니다.</p></div><span>{records.bookings.length}건</span></div><div className="list"><div className="row head"><span>업체</span><span>고객 / 차량</span><span>방문 시간</span><span>연락처</span><span>상태</span><span>변경</span></div>{records.bookings.map((item) => <div className="row" key={item.id}><b>{item.workshop_name}</b><span>{item.customer_name} · {item.vehicle}</span><span>{new Date(item.starts_at).toLocaleString("ko-KR")}</span><span>{item.customer_phone}</span><span>{item.status}</span><span><select value={item.status} onChange={(e) => changeStatus("/admin/bookings", item.id, e.target.value)}><option value="requested">요청</option><option value="confirmed">확정</option><option value="completed">완료</option><option value="cancelled">취소</option><option value="declined">거절</option></select></span></div>)}</div></article></>}
      {tab === "catalog" && <><article className="box"><div className="box-title"><div><h2>상품 검수</h2><p>파트너 상품의 판매 상태를 관리합니다.</p></div><span>{records.products.length}개</span></div><div className="list"><div className="row head"><span>상품</span><span>파트너</span><span>분류</span><span>가격</span><span>재고</span><span>상태</span></div>{records.products.map((item) => <div className="row" key={item.id}><b>{item.name}</b><span>{item.workshop_name}</span><span>{item.category}</span><span>{Number(item.price).toLocaleString("ko-KR")}원</span><span>{item.stock}</span><span><select value={item.status} onChange={(e) => changeStatus("/admin/products", item.id, e.target.value)}><option value="draft">임시</option><option value="active">판매 중</option><option value="sold_out">품절</option><option value="archived">숨김</option></select></span></div>)}</div></article><article className="box"><div className="box-title"><div><h2>주문 관리</h2><p>주문 상태를 운영자가 갱신합니다. 결제 대행 연동 전에는 결제 금액을 수납하지 않습니다.</p></div><span>{records.orders.length}건</span></div><div className="list"><div className="row head"><span>주문번호</span><span>주문자</span><span>연락처</span><span>주소</span><span>금액</span><span>상태</span></div>{records.orders.map((item) => <div className="row" key={item.id}><b>{item.id.slice(0, 8)}</b><span>{item.customer_name}</span><span>{item.customer_phone}</span><span>{item.address}</span><span>{Number(item.total_amount).toLocaleString("ko-KR")}원</span><span><select value={item.status} onChange={(e) => changeStatus("/admin/orders", item.id, e.target.value)}><option value="payment_pending">결제 대기</option><option value="paid">결제 완료</option><option value="preparing">준비 중</option><option value="shipped">배송 중</option><option value="delivered">배송 완료</option><option value="cancelled">취소</option><option value="refunded">환불</option></select></span></div>)}</div></article></>}
      {["users", "reports"].includes(tab) && <article className="box"><h2>{pageNames[tab]} 기능은 아직 준비 중입니다</h2><p>실제 회원 인증과 신고 접수 데이터 모델이 연결되기 전까지 허위 운영 지표를 표시하지 않습니다.</p></article>}
    </>}
    </section></main>{notice && <div className="toast">{notice}</div>}</div>;
}
