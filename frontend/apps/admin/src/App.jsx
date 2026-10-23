import { useCallback, useEffect, useMemo, useState } from "react";
import { api, useToast, Loading } from "@gageyo/ui";
import { AdminContext } from "./context.js";
import { aapi, adminKey, setAdminKey } from "./aapi.js";
import Login from "./pages/Login.jsx";
import Dashboard from "./pages/Dashboard.jsx";
import Workshops from "./pages/Workshops.jsx";
import ChangeRequests from "./pages/ChangeRequests.jsx";
import Members from "./pages/Members.jsx";
import Quotes from "./pages/Quotes.jsx";
import Bookings from "./pages/Bookings.jsx";
import Reviews from "./pages/Reviews.jsx";
import Products from "./pages/Products.jsx";
import Orders from "./pages/Orders.jsx";
import Settlements from "./pages/Settlements.jsx";
import Categories from "./pages/Categories.jsx";
import Inquiries from "./pages/Inquiries.jsx";
import Notices from "./pages/Notices.jsx";
import Settings from "./pages/Settings.jsx";
import Audit from "./pages/Audit.jsx";
import Security from "./pages/Security.jsx";

const NAV = [
  ["현황", [["dashboard", "▦", "대시보드"]]],
  ["업체", [["workshops", "⚒", "업체 관리", "pendingWorkshops"], ["changes", "✎", "정보 변경 요청", "changeRequests"]]],
  ["이용", [["quotes", "☷", "견적"], ["bookings", "▣", "예약"], ["reviews", "★", "리뷰"]]],
  ["쇼핑", [["products", "◇", "상품"], ["orders", "▤", "주문", "awaitingDeposit"], ["settlements", "₩", "정산"]]],
  ["회원", [["members", "♙", "회원 관리"]]],
  ["고객센터", [["inquiries", "✆", "문의·신고", "openInquiries"], ["notices", "📢", "공지"]]],
  ["설정", [["categories", "▤", "카테고리"], ["settings", "⚙", "결제·운영 설정"], ["audit", "⌛", "작업 기록"], ["security", "🔒", "내 보안"]]],
];
const PAGES = {
  dashboard: Dashboard,
  workshops: Workshops,
  changes: ChangeRequests,
  members: Members,
  quotes: Quotes,
  bookings: Bookings,
  reviews: Reviews,
  products: Products,
  orders: Orders,
  settlements: Settlements,
  categories: Categories,
  inquiries: Inquiries,
  notices: Notices,
  settings: Settings,
  audit: Audit,
  security: Security,
};
const TITLES = Object.fromEntries(NAV.flatMap(([, items]) => items.map(([id, , label]) => [id, label])));

export default function App() {
  const params = new URLSearchParams(window.location.search);
  const [me, setMe] = useState(null);
  const [checking, setChecking] = useState(true);
  const [tab, setTab] = useState(() => (PAGES[params.get("tab")] ? params.get("tab") : "dashboard"));
  const [tabParams, setTabParams] = useState({});
  const [counts, setCounts] = useState({});
  const [menuOpen, setMenuOpen] = useState(false);
  const [toastNode, toast] = useToast();
  const resetToken = params.get("reset");

  const loadMe = useCallback(async () => {
    try {
      setMe(await aapi("/admin/auth/me"));
    } catch (e) {
      if (adminKey() && e.status === 401) setAdminKey("");
      setMe(null);
    } finally {
      setChecking(false);
    }
  }, []);
  useEffect(() => {
    if (resetToken) setChecking(false);
    else loadMe();
  }, [loadMe, resetToken]);

  const refreshCounts = useCallback(async () => {
    try {
      const d = await aapi("/admin/dashboard");
      const open = d.open_inquiries || {};
      setCounts({
        pendingWorkshops: d.workshops.pending,
        changeRequests: d.workshops.change_requests,
        awaitingDeposit: d.commerce.awaiting_deposit,
        openInquiries: (open.customer || 0) + (open.partner || 0) + (open.report || 0),
      });
      return d;
    } catch {
      return null;
    }
  }, []);
  useEffect(() => {
    if (me) refreshCounts();
  }, [me, refreshCounts]);

  const go = useCallback((t, p = {}) => {
    setTab(t);
    setTabParams(p);
    setMenuOpen(false);
    window.history.replaceState(null, "", t === "dashboard" ? window.location.pathname : `?tab=${t}`);
    window.scrollTo({ top: 0 });
  }, []);

  async function logout() {
    try {
      if (!adminKey()) await api("/admin/auth/logout", { method: "POST" });
    } finally {
      setAdminKey("");
      setMe(null);
    }
  }

  const value = useMemo(() => ({ me, toast, go, counts, refreshCounts, tabParams }), [me, toast, go, counts, refreshCounts, tabParams]);
  if (checking) return <Loading text="운영자 화면을 여는 중…" />;
  if (!me || resetToken) {
    return (
      <AdminContext.Provider value={value}>
        <Login
          resetToken={resetToken}
          onLoggedIn={async () => {
            window.history.replaceState(null, "", window.location.pathname);
            await loadMe();
          }}
        />
        {toastNode}
      </AdminContext.Provider>
    );
  }
  const Page = PAGES[tab];
  return (
    <AdminContext.Provider value={value}>
      <div className="admin">
        <aside className={menuOpen ? "open" : ""}>
          <div className="brand">
            ✳ 가게요 <small>PLATFORM OPS</small>
          </div>
          <nav>
            {NAV.map(([group, items]) => (
              <div key={group} className="nav-group">
                <small>{group}</small>
                {items.map(([id, icon, label, countKey]) => (
                  <button key={id} type="button" className={tab === id ? "selected" : ""} onClick={() => go(id)}>
                    <span aria-hidden="true">{icon}</span>
                    {label}
                    {countKey && counts[countKey] > 0 && <i>{counts[countKey]}</i>}
                  </button>
                ))}
              </div>
            ))}
          </nav>
          <div className="admin-user">
            <b>{me.email}</b>
            <small>{me.via_key ? "API 키 접속" : me.otp_enabled ? "2단계 인증 켜짐" : "2단계 인증 꺼짐"}</small>
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
              <small>ADMIN / </small>
              <b>{TITLES[tab]}</b>
            </div>
            <div className="header-right">
              {!me.mail_enabled && <span className="pill warn">메일 미설정</span>}
              {!me.toss_enabled && <span className="pill">카드 결제 꺼짐</span>}
            </div>
          </header>
          {!me.via_key && !me.otp_enabled && (
            <div className="otp-banner">
              2단계 인증(OTP)이 꺼져 있습니다.{" "}
              <button type="button" onClick={() => go("security")}>
                지금 켜기 →
              </button>
            </div>
          )}
          <section className="page">
            <Page key={tab + JSON.stringify(tabParams)} />
          </section>
        </main>
      </div>
      {toastNode}
    </AdminContext.Provider>
  );
}
