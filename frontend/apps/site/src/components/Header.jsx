import { useState } from "react";
import { api } from "@gageyo/ui";
import { useApp } from "../context.js";

export default function Header() {
  const { user, setUser, cart, open, go, view, toast, config } = useApp();
  const [menu, setMenu] = useState(false);
  const partnerUrl = config?.urls?.partner || "https://partner.gageyo.com";

  async function logout() {
    try {
      await api("/auth/logout", { method: "POST" });
    } finally {
      setUser(null);
      go("home");
      toast("로그아웃했습니다.");
    }
  }
  const nav = (fn) => () => {
    setMenu(false);
    fn();
  };

  return (
    <header className="topbar">
      <button type="button" className="brand" onClick={nav(() => go("home"))}>
        <span className="brand-mark">✳</span>가게요
      </button>
      <nav className={menu ? "open" : ""} aria-label="주요 메뉴">
        <button type="button" className={view === "home" ? "on" : ""} onClick={nav(() => go("home"))}>
          업체 찾기
        </button>
        <button type="button" className={view === "products" ? "on" : ""} onClick={nav(() => go("products"))}>
          자동차 용품
        </button>
        <button type="button" onClick={nav(() => open({ type: "quote" }))}>
          견적 요청
        </button>
        <button type="button" onClick={nav(() => open({ type: "lookup" }))}>
          주문·예약 조회
        </button>
        <button type="button" onClick={nav(() => open({ type: "support" }))}>
          고객센터
        </button>
        <a href={partnerUrl} className="nav-partner">
          파트너 센터
        </a>
      </nav>
      <div className="top-actions">
        <button type="button" className="cart-button" onClick={() => open({ type: "cart" })} aria-label={`장바구니 ${cart.count}개`}>
          장바구니{cart.count > 0 && <b>{cart.count}</b>}
        </button>
        {user ? (
          <>
            <button type="button" className="text-button" onClick={() => go("my")}>
              {user.display_name}님
            </button>
            <button type="button" className="text-button muted" onClick={logout}>
              로그아웃
            </button>
          </>
        ) : (
          <button type="button" className="gy-button dark small" onClick={() => open({ type: "auth" })}>
            로그인
          </button>
        )}
        <button type="button" className="menu-button" aria-label="메뉴" aria-expanded={menu} onClick={() => setMenu(!menu)}>
          ☰
        </button>
      </div>
    </header>
  );
}
