import { useCallback, useEffect, useMemo, useState } from "react";
import { api, useToast } from "@gageyo/ui";
import { AppContext } from "./context.js";
import { useCart } from "./cart.js";
import { recallOrderPhone } from "./toss.js";
import Header from "./components/Header.jsx";
import Footer from "./components/Footer.jsx";
import Home from "./pages/Home.jsx";
import Products from "./pages/Products.jsx";
import MyPage from "./pages/MyPage.jsx";
import WorkshopDialog from "./dialogs/WorkshopDialog.jsx";
import BookingDialog from "./dialogs/BookingDialog.jsx";
import QuoteDialog from "./dialogs/QuoteDialog.jsx";
import QuoteView from "./dialogs/QuoteView.jsx";
import AuthDialog from "./dialogs/AuthDialog.jsx";
import ResetDialog from "./dialogs/ResetDialog.jsx";
import CartDialog from "./dialogs/CartDialog.jsx";
import OrderView from "./dialogs/OrderView.jsx";
import LookupDialog from "./dialogs/LookupDialog.jsx";
import ProductDialog from "./dialogs/ProductDialog.jsx";
import SupportDialog from "./dialogs/SupportDialog.jsx";
import InquiryView from "./dialogs/InquiryView.jsx";
import ReportDialog from "./dialogs/ReportDialog.jsx";
import ReviewDialog from "./dialogs/ReviewDialog.jsx";

const DIALOGS = {
  workshop: WorkshopDialog,
  booking: BookingDialog,
  quote: QuoteDialog,
  quoteView: QuoteView,
  auth: AuthDialog,
  reset: ResetDialog,
  cart: CartDialog,
  order: OrderView,
  lookup: LookupDialog,
  product: ProductDialog,
  support: SupportDialog,
  inquiry: InquiryView,
  report: ReportDialog,
  review: ReviewDialog,
};

function setUrl(params) {
  const url = new URL(window.location.href);
  url.search = "";
  Object.entries(params || {}).forEach(([k, v]) => v && url.searchParams.set(k, v));
  window.history.replaceState(null, "", url.toString());
}

export default function App() {
  const [user, setUser] = useState(null);
  const [userLoaded, setUserLoaded] = useState(false);
  const [config, setConfig] = useState(null);
  const [categories, setCategories] = useState([]);
  const [view, setView] = useState("home");
  const [myTab, setMyTab] = useState("bookings");
  const [dialog, setDialog] = useState(null);
  const cart = useCart();
  const [toastNode, toast] = useToast();

  const refreshUser = useCallback(async () => {
    try {
      const me = await api("/auth/me");
      setUser(me);
      return me;
    } catch {
      setUser(null);
      return null;
    } finally {
      setUserLoaded(true);
    }
  }, []);

  useEffect(() => {
    refreshUser();
    api("/config").then(setConfig).catch(() => setConfig({ payments: { none_allowed: true }, support: {} }));
    api("/categories").then(setCategories).catch(() => setCategories([]));
  }, [refreshUser]);

  const open = useCallback((d) => {
    setDialog(d);
    if (d?.type === "workshop") setUrl({ shop: d.id });
  }, []);
  const close = useCallback(() => {
    setDialog(null);
    if (new URL(window.location.href).search) setUrl({});
  }, []);
  const go = useCallback(
    (next, tab) => {
      if (next === "my" && !user) {
        open({ type: "auth", then: { view: "my", tab } });
        return;
      }
      if (tab) setMyTab(tab);
      setView(next);
      window.scrollTo({ top: 0, behavior: "smooth" });
    },
    [user, open],
  );

  // 메일 링크·결제창에서 돌아온 주소 처리 (?shop= ?quote= ?order= ?my= ?reset= ?inquiry= ?pay=)
  useEffect(() => {
    if (!userLoaded) return;
    const p = new URLSearchParams(window.location.search);
    if (p.get("pay") === "success" && p.get("orderId")) {
      const orderId = p.get("orderId");
      setUrl({});
      api(`/orders/${encodeURIComponent(orderId)}/payments/toss`, {
        method: "POST",
        body: { payment_key: p.get("paymentKey"), amount: Number(p.get("amount")), phone: recallOrderPhone(orderId) || undefined },
      })
        .then(() => {
          cart.clear();
          toast("결제가 완료되었습니다.");
          open({ type: "order", id: orderId, phone: recallOrderPhone(orderId) });
        })
        .catch((e) => {
          toast(e.message);
          open({ type: "order", id: orderId, phone: recallOrderPhone(orderId) });
        });
      return;
    }
    if (p.get("pay") === "fail") {
      const orderId = p.get("orderId");
      setUrl({});
      toast(p.get("message") || "결제가 취소되었습니다.");
      if (orderId) open({ type: "order", id: orderId, phone: recallOrderPhone(orderId) });
      return;
    }
    if (p.get("shop")) return open({ type: "workshop", id: p.get("shop") });
    if (p.get("quote")) return open({ type: "quoteView", id: p.get("quote") });
    if (p.get("order")) return open({ type: "order", id: p.get("order") });
    if (p.get("reset")) return open({ type: "reset", token: p.get("reset") });
    if (p.get("inquiry")) return open({ type: "inquiry", id: p.get("inquiry"), token: p.get("token") || "" });
    if (p.get("my")) {
      setUrl({});
      go("my", p.get("my"));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [userLoaded]);

  const value = useMemo(
    () => ({ user, setUser, refreshUser, config, categories, cart, toast, open, close, go, view, myTab, setMyTab }),
    [user, refreshUser, config, categories, cart, toast, open, close, go, view, myTab],
  );
  const Dialog = dialog ? DIALOGS[dialog.type] : null;

  return (
    <AppContext.Provider value={value}>
      <Header />
      <main id="top">
        {view === "home" && <Home />}
        {view === "products" && <Products />}
        {view === "my" && user && <MyPage />}
      </main>
      <Footer />
      {Dialog && <Dialog {...dialog} />}
      {toastNode}
    </AppContext.Provider>
  );
}
