import { useCallback, useEffect, useMemo, useState } from "react";
import { api, useToast, Badge, Loading } from "@gageyo/ui";
import { PartnerContext } from "./context.js";
import { papi, legacyKey, setLegacyKey } from "./papi.js";
import Login from "./pages/Login.jsx";
import Dashboard from "./pages/Dashboard.jsx";
import Quotes from "./pages/Quotes.jsx";
import Offers from "./pages/Offers.jsx";
import Bookings from "./pages/Bookings.jsx";
import Services from "./pages/Services.jsx";
import Products from "./pages/Products.jsx";
import Orders from "./pages/Orders.jsx";
import Reviews from "./pages/Reviews.jsx";
import Settlements from "./pages/Settlements.jsx";
import Profile from "./pages/Profile.jsx";
import Support from "./pages/Support.jsx";

// [id, 아이콘, 이름, 승인 필요]
const NAV = [
  ["dashboard", "▦", "대시보드", false],
  ["quotes", "☷", "견적 요청", true],
  ["offers", "✉", "보낸 견적", true],
  ["bookings", "▣", "예약 일정", true],
  ["services", "◷", "서비스", false],
  ["products", "◇", "상품", false],
  ["orders", "▤", "주문·배송", true],
  ["reviews", "★", "리뷰", false],
  ["settlements", "₩", "정산", true],
  ["profile", "⚙", "업체 정보", false],
  ["support", "✆", "공지·문의", false],
];
const PAGES = { dashboard: Dashboard, quotes: Quotes, offers: Offers, bookings: Bookings, services: Services, products: Products, orders: Orders, reviews: Reviews, settlements: Settlements, profile: Profile, support: Support };

export default function App() {
  const params = new URLSearchParams(window.location.search);
  const [me, setMe] = useState(null);
  const [checking, setChecking] = useState(true);
  const [tab, setTab] = useState(() => (PAGES[params.get("tab")] ? params.get("tab") : "dashboard"));
  const [counts, setCounts] = useState({});
  const [menuOpen, setMenuOpen] = useState(false);
  const [toastNode, toast] = useToast();
  const resetToken = params.get("reset");

  const reload = useCallback(async () => {
    try {
      const data = await papi("/partner/me");
      setMe(data);
      return data;
    } catch (e) {
      if (e.status === 401 || e.status === 403) {
        if (legacyKey() && e.status === 401) setLegacyKey("");
        setMe(null);
        if (e.status === 403) toast(e.message);
      }
      return null;
    } finally {
      setChecking(false);
    }
  }, [toast]);

  useEffect(() => {
    if (!resetToken) reload();
    else setChecking(false);
  }, [reload, resetToken]);

  const loadCounts = useCallback(async () => {
    if (!me || me.status !== "approved") return;
    try {
      const [d, q] = await Promise.all([papi("/partner/dashboard"), papi("/partner/quotes")]);
      setCounts({ quotes: q.length, bookings: d.bookings_requested, orders: d.orders_to_ship, reviews: d.reviews_unanswered });
    } catch {
      /* 숫자는 없어도 됨 */
    }
  }, [me]);
  useEffect(() => {
    loadCounts();
  }, [loadCounts, tab]);

  const go = useCallback((t) => {
    setTab(t);
    setMenuOpen(false);
    window.history.replaceState(null, "", t === "dashboard" ? window.location.pathname : `?tab=${t}`);
    window.scrollTo({ top: 0 });
  }, []);

  async function logout() {
    try {
      if (!legacyKey()) await api("/partner/auth/logout", { method: "POST" });
    } finally {
      setLegacyKey("");
      setMe(null);
    }
  }

  const value = useMemo(() => ({ me, reload, toast, go, loadCounts }), [me, reload, toast, go, loadCounts]);

  if (checking) return <Loading text="파트너 센터를 여는 중…" />;
  if (!me || resetToken) {
    return (
      <PartnerContext.Provider value={value}>
        <Login resetToken={resetToken} onLoggedIn={async () => { window.history.replaceState(null, "", window.location.pathname); await reload(); }} />
        {toastNode}
      </PartnerContext.Provider>
    );
  }

  const approved = me.status === "approved";
  const current = NAV.find(([id]) => id === tab) || NAV[0];
  const Page = PAGES[tab];
  return (
    <PartnerContext.Provider value={value}>
      <div className="layout">
        <aside className={menuOpen ? "open" : ""}>
          <a className="logo" href={window.location.pathname}>
            ✳ 가게요 <small>PARTNER</small>
          </a>
          <div className="shop-switch">
            <b>{me.name}</b>
            <span>
              {me.district} · <Badge kind="workshop" value={me.status} />
            </span>
          </div>
          <nav>
            {NAV.map(([id, icon, label, needApproval]) => (
              <button key={id} type="button" className={tab === id ? "on" : ""} onClick={() => go(id)} disabled={needApproval && !approved} title={needApproval && !approved ? "운영자 승인 뒤에 쓸 수 있습니다" : ""}>
                <span aria-hidden="true">{icon}</span>
                {label}
                {counts[id] > 0 && <i>{counts[id]}</i>}
              </button>
            ))}
          </nav>
          <div className="aside-bottom">
            <div className="avatar">{me.name?.[0] || "가"}</div>
            <div>
              <b>{me.account?.display_name || "파트너 키 접속"}</b>
              <small>{me.account?.email || "계정으로 옮겨 주세요"}</small>
            </div>
            <button type="button" onClick={logout}>
              로그아웃
            </button>
          </div>
        </aside>
        <main>
          <header>
            <button type="button" className="menu-toggle" onClick={() => setMenuOpen(!menuOpen)} aria-label="메뉴">
              ☰
            </button>
            <div>
              <span className="crumb">파트너 센터 / </span>
              <b>{current[2]}</b>
            </div>
            <div className="header-right">
              <a href="https://app.gageyo.com/" target="_blank" rel="noreferrer">
                고객 화면 ↗
              </a>
            </div>
          </header>
          <div className="content">
            {!approved && (
              <div className={`status-banner ${me.status}`}>
                {me.status === "pending" && (
                  <>
                    <b>입점 심사 중입니다.</b> 운영자가 확인하면 메일로 알려 드립니다. 그동안 업체 정보·사진·서비스·상품을 미리 준비해 두세요.
                  </>
                )}
                {me.status === "rejected" && (
                  <>
                    <b>입점 신청이 반려되었습니다.</b> 사유: {me.reject_reason || "-"} — [업체 정보]에서 고친 뒤 다시 심사를 요청해 주세요.
                  </>
                )}
              </div>
            )}
            {me.account?.via_key && (
              <div className="status-banner pending">
                예전 파트너 키로 접속했습니다. 운영자에게 <b>계정 비밀번호 설정 링크</b>를 요청하면 이메일·비밀번호로 로그인할 수 있습니다.
              </div>
            )}
            {(!current[3] || approved) && <Page />}
          </div>
        </main>
      </div>
      {toastNode}
    </PartnerContext.Provider>
  );
}
