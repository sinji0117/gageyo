import { useEffect, useState } from "react";
import { api } from "@gageyo/ui";

const metrics = [["등록 업체", "1,248", "이번 달 +86"], ["가입 운전자", "18,420", "이번 달 +1,204"], ["이번 달 거래액", "₩ 284.6M", "지난달 대비 +18.2%"], ["승인 대기", "17", "확인이 필요해요"]];
const shops = [["오토랩 강남점", "서울 강남구", "자동차 정비", "2026.10.22", "승인 대기"], ["카인드 디테일링", "서울 서초구", "디테일링세차/광택", "2026.10.22", "승인 대기"], ["타이어프로 대치점", "서울 강남구", "자동차 정비", "2026.10.21", "승인 완료"], ["EV 모터스", "서울 송파구", "전기차충전기 설치 및 수리", "2026.10.20", "승인 완료"]];
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

    {tab === "categories" ? <>
      <article className="box admin-key-box"><div><h2>관리자 연결</h2><p>Rpanel의 gageyo-api 앱에 설정한 ADMIN_API_KEY를 입력하세요. 키는 이 브라우저 탭에서만 보관됩니다.</p></div><div className="key-connect"><input type="password" value={keyDraft} onChange={(e) => setKeyDraft(e.target.value)} placeholder="ADMIN_API_KEY" autoComplete="current-password"/><button className="approve" onClick={() => loadCategories(keyDraft)}>{loading ? "확인 중…" : connected ? "다시 연결" : "연결"}</button>{connected && <span className="key-ok">● 연결됨</span>}</div></article>
      {connected && <div className="category-admin-grid"><article className="box category-form-box"><div className="box-title"><div><h2>{editingId ? "카테고리 수정" : "카테고리 추가"}</h2><p>고객 서비스 화면에 바로 반영됩니다.</p></div>{editingId && <button onClick={() => { setEditingId(null); setForm(emptyForm); }}>취소</button>}</div><form onSubmit={saveCategory}><label>분류 그룹<select value={form.group_name} onChange={(e) => setForm({ ...form, group_name: e.target.value })}><option>차량 관리</option><option>자동차 매매</option></select></label><label>카테고리명<input required maxLength={80} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="예: 자동차 썬팅"/></label><label>정렬 순서<input type="number" min="0" step="10" value={form.sort_order} onChange={(e) => setForm({ ...form, sort_order: e.target.value })}/></label><label className="check-label"><input type="checkbox" checked={form.is_active} onChange={(e) => setForm({ ...form, is_active: e.target.checked })}/>고객 화면에 표시</label><button className="primary">{editingId ? "변경 저장" : "카테고리 추가"}</button></form></article>
        <article className="box category-list-box"><div className="box-title"><div><h2>등록된 카테고리 <span className="count-pill">{categories.length}</span></h2><p>그룹과 정렬 순서대로 고객 화면에 표시됩니다.</p></div><button onClick={() => loadCategories()}>{loading ? "불러오는 중…" : "↻ 새로고침"}</button></div>{[...new Set(categories.map((item) => item.group_name))].map((group) => <section className="admin-category-group" key={group}><h3>{group}<small>{categories.filter((item) => item.group_name === group).length}개</small></h3>{categories.filter((item) => item.group_name === group).map((item) => <div className={`admin-category-row ${item.is_active ? "" : "inactive"}`} key={item.id}><span className="sort-number">{item.sort_order}</span><b>{item.name}</b><em>{item.is_active ? "노출 중" : "숨김"}</em><button onClick={() => editCategory(item)}>수정</button><button className="remove-category" onClick={() => removeCategory(item)}>삭제</button></div>)}</section>)}{!categories.length && <div className="category-empty">관리자 키를 연결하면 카테고리 목록이 표시됩니다.</div>}</article></div>}
    </> : <>{tab === "overview" && <div className="metric-grid">{metrics.map(([a, b, c]) => <article key={a}><span>{a}</span><b>{b}</b><small>{c}</small></article>)}</div>}<article className="box"><div className="box-title"><div><h2>{tab === "overview" ? "업체 승인 요청" : "업체 목록"}</h2><p>사업자 정보와 서비스 등록 내용을 확인하세요.</p></div><button onClick={() => flash("목록 전체 보기")}>전체 보기 →</button></div><div className="list"><div className="row head"><span>업체명</span><span>지역</span><span>업종</span><span>등록일</span><span>상태</span><span>관리</span></div>{shops.map((r) => <div className="row" key={r[0]}><b>{r[0]}</b><span>{r[1]}</span><span>{r[2]}</span><span>{r[3]}</span><span><em className={r[4] === "승인 대기" ? "pending" : "approved"}>{r[4]}</em></span><span>{r[4] === "승인 대기" ? <button className="approve" onClick={() => flash(`${r[0]} 승인 내용을 확인합니다.`)}>검토</button> : <button className="more" onClick={() => flash(`${r[0]} 상세 정보를 엽니다.`)}>···</button>}</span></div>)}</div></article><article className="box revenue"><div><h2>서비스 운영 메모</h2><p>견적 요청과 정비 예약, 업체가 등록한 상품 판매가 연결됩니다.</p></div><div className="steps"><span>01　업체 승인</span><span>02　서비스·상품 등록</span><span>03　견적·예약 운영</span></div></article></>}
    </section></main>{notice && <div className="toast">{notice}</div>}</div>;
}
