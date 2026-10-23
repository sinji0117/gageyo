import { useState } from "react";
import { api, money, Tabs, Badge, Empty, Loading, ErrorBox, Field, useLoad, dateText, dateTimeText, statusLabel } from "@gageyo/ui";
import { useApp } from "../context.js";
import WorkshopCard, { Stars } from "../components/WorkshopCard.jsx";

const TABS = [
  ["bookings", "예약"],
  ["quotes", "견적"],
  ["orders", "주문"],
  ["reviews", "내 후기"],
  ["favorites", "관심 업체"],
  ["vehicles", "내 차량"],
  ["inquiries", "문의"],
  ["account", "내 정보"],
];

export default function MyPage() {
  const { user, myTab, setMyTab } = useApp();
  const tab = TABS.some(([t]) => t === myTab) ? myTab : "bookings";
  const counts = user?.counts || {};
  return (
    <section className="main-wrap mypage">
      <div className="section-title">
        <div>
          <h2>{user.display_name}님의 가게요</h2>
          <p>{user.email}</p>
        </div>
        <div className="my-counts">
          <span>
            다가오는 예약 <b>{counts.upcoming_bookings || 0}</b>
          </span>
          <span>
            받는 중인 견적 <b>{counts.open_quotes || 0}</b>
          </span>
          <span>
            진행 중 주문 <b>{counts.active_orders || 0}</b>
          </span>
        </div>
      </div>
      <Tabs tabs={TABS} value={tab} onChange={setMyTab} />
      {tab === "bookings" && <MyBookings />}
      {tab === "quotes" && <MyQuotes />}
      {tab === "orders" && <MyOrders />}
      {tab === "reviews" && <MyReviews />}
      {tab === "favorites" && <MyFavorites />}
      {tab === "vehicles" && <MyVehicles />}
      {tab === "inquiries" && <MyInquiries />}
      {tab === "account" && <Account />}
    </section>
  );
}

function MyBookings() {
  const { open, toast, refreshUser } = useApp();
  const { data, loading, error, reload } = useLoad(() => api("/me/bookings"), []);
  async function cancel(b) {
    const reason = window.prompt("취소 사유를 적어 주세요 (선택)", "");
    if (reason === null) return;
    try {
      await api(`/bookings/${b.id}/cancel`, { method: "POST", body: { reason } });
      toast("예약을 취소했습니다.");
      reload();
      refreshUser();
    } catch (e) {
      toast(e.message);
    }
  }
  if (loading && !data) return <Loading />;
  return (
    <>
      <ErrorBox error={error} onRetry={reload} />
      {data?.length === 0 && (
        <Empty
          action={
            <button type="button" className="gy-button" onClick={() => open({ type: "quote" })}>
              견적 요청하기
            </button>
          }
        >
          예약 내역이 없습니다.
        </Empty>
      )}
      <ul className="card-list">
        {data?.map((b) => {
          const upcoming = ["requested", "confirmed"].includes(b.status) && new Date(b.starts_at) > new Date();
          return (
            <li key={b.id}>
              <div className="card-main">
                <Badge kind="booking" value={b.status} />
                <button type="button" className="link-button strong" onClick={() => open({ type: "workshop", id: b.workshop_id })}>
                  {b.workshop_name}
                </button>
                <p>
                  {dateTimeText(b.starts_at)} · {b.vehicle}
                  {b.service_name ? ` · ${b.service_name}` : ""}
                </p>
                <small className="gy-muted">
                  {b.workshop_address} · <a href={`tel:${b.workshop_phone}`}>{b.workshop_phone}</a>
                </small>
                {b.decline_reason && <small className="gy-muted">사유: {b.decline_reason}</small>}
              </div>
              <div className="card-side">
                {upcoming && (
                  <button type="button" className="gy-button ghost small" onClick={() => cancel(b)}>
                    예약 취소
                  </button>
                )}
                {b.status === "completed" && !b.review_id && (
                  <button type="button" className="gy-button small" onClick={() => open({ type: "review", bookingId: b.id, workshopName: b.workshop_name, onDone: reload })}>
                    후기 쓰기
                  </button>
                )}
                {b.review_id && <span className="gy-muted small">후기 작성함</span>}
              </div>
            </li>
          );
        })}
      </ul>
    </>
  );
}

function MyQuotes() {
  const { open } = useApp();
  const { data, loading, error, reload } = useLoad(() => api("/me/quotes"), []);
  if (loading && !data) return <Loading />;
  return (
    <>
      <ErrorBox error={error} onRetry={reload} />
      {data?.length === 0 && <Empty>견적 요청 내역이 없습니다.</Empty>}
      <ul className="card-list">
        {data?.map((q) => (
          <li key={q.id}>
            <div className="card-main">
              <Badge kind="quote" value={q.status} /> <b>{q.service_category}</b>
              <p>
                {q.vehicle} · {q.district}
              </p>
              <small className="gy-muted">{dateText(q.created_at)} 요청</small>
            </div>
            <div className="card-side">
              <span>
                견적 <b>{q.offer_count}</b>건{q.min_amount != null && ` · 최저 ${money(q.min_amount)}`}
              </span>
              <button type="button" className="gy-button small" onClick={() => open({ type: "quoteView", id: q.id })}>
                견적 보기
              </button>
            </div>
          </li>
        ))}
      </ul>
    </>
  );
}

function MyOrders() {
  const { open } = useApp();
  const { data, loading, error, reload } = useLoad(() => api("/me/orders"), []);
  if (loading && !data) return <Loading />;
  return (
    <>
      <ErrorBox error={error} onRetry={reload} />
      {data?.length === 0 && <Empty>주문 내역이 없습니다.</Empty>}
      <ul className="card-list">
        {data?.map((o) => (
          <li key={o.id}>
            <div className="card-main">
              <Badge kind="order" value={o.status} /> <b>{o.title}</b>
              <p>
                {money(o.total_amount)} · {statusLabel("payment", o.payment_method)} · {o.fulfillment === "visit" ? "방문 장착" : "택배"}
              </p>
              <small className="gy-muted">
                {dateText(o.created_at)} · {o.id.slice(0, 8).toUpperCase()}
              </small>
            </div>
            <div className="card-side">
              <button type="button" className="gy-button ghost small" onClick={() => open({ type: "order", id: o.id })}>
                주문 상세
              </button>
            </div>
          </li>
        ))}
      </ul>
    </>
  );
}

function MyReviews() {
  const { open, toast } = useApp();
  const { data, loading, error, reload } = useLoad(() => api("/me/reviews"), []);
  async function remove(r) {
    if (!window.confirm("후기를 지울까요?")) return;
    try {
      await api(`/me/reviews/${r.id}`, { method: "DELETE" });
      toast("후기를 지웠습니다.");
      reload();
    } catch (e) {
      toast(e.message);
    }
  }
  if (loading && !data) return <Loading />;
  return (
    <>
      <ErrorBox error={error} onRetry={reload} />
      {data?.length === 0 && <Empty>작업이 끝난 예약에 후기를 남길 수 있어요.</Empty>}
      <ul className="card-list">
        {data?.map((r) => (
          <li key={r.id}>
            <div className="card-main">
              <Stars value={r.rating} /> <b>{r.workshop_name}</b> {r.status === "hidden" && <Badge kind="review" value="hidden" />}
              <p>{r.content}</p>
              {r.reply && <p className="reply-inline">사장님: {r.reply}</p>}
              <small className="gy-muted">{dateText(r.created_at)}</small>
            </div>
            <div className="card-side">
              <button type="button" className="gy-button ghost small" onClick={() => open({ type: "review", review: r, workshopName: r.workshop_name, onDone: reload })}>
                고치기
              </button>
              <button type="button" className="link-button small" onClick={() => remove(r)}>
                삭제
              </button>
            </div>
          </li>
        ))}
      </ul>
    </>
  );
}

function MyFavorites() {
  const { data, loading, error, reload } = useLoad(() => api("/me/favorites"), []);
  if (loading && !data) return <Loading />;
  return (
    <>
      <ErrorBox error={error} onRetry={reload} />
      {data?.length === 0 && <Empty>업체 카드의 ♡ 를 눌러 관심 업체를 저장해 보세요.</Empty>}
      <div className="shop-grid">
        {data?.map((w) => (
          <WorkshopCard key={w.id} workshop={w} subtitle={w.district} />
        ))}
      </div>
    </>
  );
}

const emptyVehicle = { label: "", model: "", model_year: "", plate: "", mileage: "" };

function MyVehicles() {
  const { toast } = useApp();
  const { data, loading, error, reload } = useLoad(() => api("/me/vehicles"), []);
  const [form, setForm] = useState(emptyVehicle);
  const [editing, setEditing] = useState(null);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });
  async function save(e) {
    e.preventDefault();
    const body = {
      ...form,
      model_year: form.model_year ? Number(form.model_year) : null,
      mileage: form.mileage ? Number(form.mileage) : null,
    };
    try {
      await api(editing ? `/me/vehicles/${editing}` : "/me/vehicles", { method: editing ? "PUT" : "POST", body });
      setForm(emptyVehicle);
      setEditing(null);
      toast("저장했습니다.");
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  async function remove(v) {
    if (!window.confirm(`${v.label}을(를) 지울까요?`)) return;
    await api(`/me/vehicles/${v.id}`, { method: "DELETE" }).catch((e) => toast(e.message));
    reload();
  }
  return (
    <div className="two-col">
      <div>
        {loading && !data && <Loading />}
        <ErrorBox error={error} onRetry={reload} />
        {data?.length === 0 && <Empty>차량을 등록하면 견적·예약할 때 바로 고를 수 있어요.</Empty>}
        <ul className="card-list">
          {data?.map((v) => (
            <li key={v.id}>
              <div className="card-main">
                <b>{v.label}</b>
                <p>
                  {v.model}
                  {v.model_year ? ` · ${v.model_year}년식` : ""}
                  {v.plate ? ` · ${v.plate}` : ""}
                  {v.mileage != null ? ` · ${v.mileage.toLocaleString("ko-KR")}km` : ""}
                </p>
              </div>
              <div className="card-side">
                <button
                  type="button"
                  className="gy-button ghost small"
                  onClick={() => {
                    setEditing(v.id);
                    setForm({ label: v.label, model: v.model, model_year: v.model_year || "", plate: v.plate || "", mileage: v.mileage ?? "" });
                  }}
                >
                  고치기
                </button>
                <button type="button" className="link-button small" onClick={() => remove(v)}>
                  삭제
                </button>
              </div>
            </li>
          ))}
        </ul>
      </div>
      <form className="gy-form panel-box" onSubmit={save}>
        <h3>{editing ? "차량 고치기" : "차량 추가"}</h3>
        <Field label="차종" required>
          <input required maxLength={120} value={form.model} onChange={set("model")} placeholder="예: 현대 아반떼" />
        </Field>
        <Field label="별명">
          <input maxLength={60} value={form.label} onChange={set("label")} placeholder="예: 출퇴근차" />
        </Field>
        <div className="gy-row">
          <Field label="연식">
            <input type="number" min="1950" max="2100" value={form.model_year} onChange={set("model_year")} />
          </Field>
          <Field label="주행거리(km)">
            <input type="number" min="0" value={form.mileage} onChange={set("mileage")} />
          </Field>
        </div>
        <Field label="차량 번호" hint="선택 · 업체에 보이지 않습니다">
          <input maxLength={20} value={form.plate} onChange={set("plate")} />
        </Field>
        <div className="cta-row">
          {editing && (
            <button
              type="button"
              className="gy-button ghost"
              onClick={() => {
                setEditing(null);
                setForm(emptyVehicle);
              }}
            >
              취소
            </button>
          )}
          <button className="gy-button">{editing ? "저장" : "추가"}</button>
        </div>
      </form>
    </div>
  );
}

function MyInquiries() {
  const { open } = useApp();
  const { data, loading, error, reload } = useLoad(() => api("/me/inquiries"), []);
  if (loading && !data) return <Loading />;
  return (
    <>
      <ErrorBox error={error} onRetry={reload} />
      <div className="cta-row">
        <button type="button" className="gy-button" onClick={() => open({ type: "support" })}>
          새 문의
        </button>
      </div>
      {data?.length === 0 && <Empty>문의 내역이 없습니다.</Empty>}
      <ul className="card-list">
        {data?.map((q) => (
          <li key={q.id}>
            <div className="card-main">
              <Badge kind="inquiry" value={q.status} /> <b>{q.subject}</b>
              <small className="gy-muted">{dateTimeText(q.last_message_at)}</small>
            </div>
            <div className="card-side">
              <button type="button" className="gy-button ghost small" onClick={() => open({ type: "inquiry", id: q.id })}>
                보기
              </button>
            </div>
          </li>
        ))}
      </ul>
    </>
  );
}

function Account() {
  const { user, setUser, toast, go, refreshUser } = useApp();
  const [profile, setProfile] = useState({ display_name: user.display_name, phone: user.phone || "" });
  const [pw, setPw] = useState({ current_password: "", new_password: "" });
  const [withdrawPw, setWithdrawPw] = useState("");
  async function saveProfile(e) {
    e.preventDefault();
    try {
      await api("/me/profile", { method: "PUT", body: profile });
      await refreshUser();
      toast("내 정보를 저장했습니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  async function savePw(e) {
    e.preventDefault();
    try {
      await api("/me/password", { method: "PUT", body: pw });
      setPw({ current_password: "", new_password: "" });
      toast("비밀번호를 바꿨습니다. 다른 기기에서는 로그아웃됩니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  async function withdraw(e) {
    e.preventDefault();
    if (!window.confirm("정말 탈퇴할까요? 관심 업체·내 차량 정보가 지워지고 되돌릴 수 없습니다.")) return;
    try {
      await api("/me", { method: "DELETE", body: { password: withdrawPw } });
      setUser(null);
      go("home");
      toast("탈퇴했습니다. 그동안 이용해 주셔서 감사합니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  return (
    <div className="account-grid">
      <form className="gy-form panel-box" onSubmit={saveProfile}>
        <h3>내 정보</h3>
        <Field label="이메일">
          <input value={user.email} readOnly />
        </Field>
        <Field label="이름" required>
          <input required maxLength={100} value={profile.display_name} onChange={(e) => setProfile({ ...profile, display_name: e.target.value })} />
        </Field>
        <Field label="연락처" hint="견적·예약할 때 자동으로 채워집니다">
          <input type="tel" value={profile.phone} onChange={(e) => setProfile({ ...profile, phone: e.target.value })} />
        </Field>
        <button className="gy-button">저장</button>
      </form>
      <form className="gy-form panel-box" onSubmit={savePw}>
        <h3>비밀번호 바꾸기</h3>
        <Field label="지금 비밀번호" required>
          <input type="password" required value={pw.current_password} onChange={(e) => setPw({ ...pw, current_password: e.target.value })} autoComplete="current-password" />
        </Field>
        <Field label="새 비밀번호" required hint="10자 이상">
          <input type="password" required minLength={10} value={pw.new_password} onChange={(e) => setPw({ ...pw, new_password: e.target.value })} autoComplete="new-password" />
        </Field>
        <button className="gy-button">바꾸기</button>
      </form>
      <form className="gy-form panel-box danger-zone" onSubmit={withdraw}>
        <h3>회원 탈퇴</h3>
        <p className="gy-muted small">배송 중인 주문이나 다가오는 예약이 있으면 탈퇴할 수 없습니다. 주문·예약 기록은 거래 확인을 위해 남습니다.</p>
        <Field label="비밀번호 확인" required>
          <input type="password" required value={withdrawPw} onChange={(e) => setWithdrawPw(e.target.value)} autoComplete="current-password" />
        </Field>
        <button className="gy-button danger">탈퇴하기</button>
      </form>
    </div>
  );
}
