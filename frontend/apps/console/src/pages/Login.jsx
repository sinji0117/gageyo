import { useEffect, useState } from "react";
import { api, Field, CodeButton, AddressInput } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { setLegacyKey } from "../papi.js";

const emptyApply = {
  email: "",
  email_code: "",
  password: "",
  display_name: "",
  phone: "",
  name: "",
  categories: [],
  district: "",
  address: "",
  business_number: "",
  representative: "",
  shop_phone: "",
  description: "",
  latitude: "",
  longitude: "",
  agree: false,
};

/** 파트너 로그인 · 입점 신청 · 비밀번호 찾기/설정 · 예전 파트너 키 */
export default function Login({ resetToken, onLoggedIn }) {
  const { toast } = usePartner();
  const [mode, setMode] = useState(resetToken ? "reset" : "login");
  const [form, setForm] = useState({ email: "", password: "", key: "", confirm: "" });
  const [apply, setApply] = useState(emptyApply);
  const [categories, setCategories] = useState([]);
  const [config, setConfig] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    api("/categories").then(setCategories).catch(() => {});
    api("/config").then(setConfig).catch(() => {});
  }, []);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });
  const setA = (k) => (e) => setApply({ ...apply, [k]: e.target.type === "checkbox" ? e.target.checked : e.target.value });

  async function run(fn) {
    setBusy(true);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(e.status === 423 ? "로그인을 5번 틀려 15분 동안 잠겼습니다. 비밀번호 찾기를 이용해 주세요." : e.message);
    } finally {
      setBusy(false);
    }
  }

  const submitLogin = (e) => {
    e.preventDefault();
    run(async () => {
      setLegacyKey("");
      await api("/partner/auth/login", { method: "POST", body: { email: form.email, password: form.password } });
      await onLoggedIn();
    });
  };
  const submitKey = (e) => {
    e.preventDefault();
    run(async () => {
      setLegacyKey(form.key.trim());
      try {
        await api("/partner/me", { headers: { "x-partner-key": form.key.trim() } });
      } catch (err) {
        setLegacyKey("");
        throw err;
      }
      await onLoggedIn();
    });
  };
  const submitForgot = (e) => {
    e.preventDefault();
    run(async () => {
      await api("/auth/password/forgot", { method: "POST", body: { email: form.email, scope: "partner" } });
      setNotice("가입된 파트너 이메일이라면 비밀번호 설정 메일을 보냈습니다. 링크는 30분 동안 쓸 수 있습니다.");
    });
  };
  const submitReset = (e) => {
    e.preventDefault();
    if (form.password !== form.confirm) {
      setError("두 비밀번호가 다릅니다.");
      return;
    }
    run(async () => {
      await api("/auth/password/reset", { method: "POST", body: { token: resetToken, password: form.password } });
      window.history.replaceState(null, "", window.location.pathname);
      toast("비밀번호를 정했습니다. 로그인해 주세요.");
      setMode("login");
    });
  };
  const submitApply = (e) => {
    e.preventDefault();
    if (!apply.categories.length) {
      setError("서비스 분야를 하나 이상 골라 주세요.");
      return;
    }
    run(async () => {
      setLegacyKey("");
      await api("/partner/apply", {
        method: "POST",
        body: {
          email: apply.email,
          email_code: apply.email_code,
          password: apply.password,
          display_name: apply.display_name,
          phone: apply.phone,
          agree_terms: apply.agree,
          workshop: {
            name: apply.name,
            categories: apply.categories,
            district: apply.district,
            address: apply.address,
            business_number: apply.business_number,
            representative: apply.representative,
            phone: apply.shop_phone,
            description: apply.description,
            latitude: apply.latitude ? Number(apply.latitude) : null,
            longitude: apply.longitude ? Number(apply.longitude) : null,
          },
        },
      });
      toast("입점 신청을 받았습니다. 심사 결과는 메일로 알려 드립니다.");
      await onLoggedIn();
    });
  };
  function toggleCategory(name) {
    setApply((a) => ({ ...a, categories: a.categories.includes(name) ? a.categories.filter((c) => c !== name) : [...a.categories, name].slice(0, 8) }));
  }
  function fillMyLocation() {
    navigator.geolocation?.getCurrentPosition(
      (p) => setApply((a) => ({ ...a, latitude: p.coords.latitude.toFixed(6), longitude: p.coords.longitude.toFixed(6) })),
      () => toast("위치를 가져오지 못했습니다. 직접 입력하거나 비워 두세요."),
    );
  }

  const groups = [...new Set(categories.map((c) => c.group_name))];
  return (
    <main className={`partner-connect ${mode === "apply" ? "wide" : ""}`}>
      <section className="panel">
        <a className="logo" href="https://app.gageyo.com/">
          ✳ 가게요 <small>PARTNER</small>
        </a>
        {mode === "login" && (
          <>
            <h1>파트너 센터 로그인</h1>
            <p className="gy-muted">견적 요청에 답하고, 예약·주문을 관리하세요.</p>
            <form className="gy-form" onSubmit={submitLogin}>
              <Field label="이메일">
                <input type="email" required value={form.email} onChange={set("email")} autoComplete="email" />
              </Field>
              <Field label="비밀번호">
                <input type="password" required value={form.password} onChange={set("password")} autoComplete="current-password" />
              </Field>
              {error && <div className="gy-error">{error}</div>}
              <button className="gy-button block" disabled={busy}>
                {busy ? "확인 중…" : "로그인"}
              </button>
            </form>
            <div className="login-links">
              <button type="button" className="link-button" onClick={() => { setMode("apply"); setError(""); }}>
                입점 신청하기
              </button>
              <button type="button" className="link-button" onClick={() => { setMode("forgot"); setError(""); }}>
                비밀번호 찾기·설정
              </button>
              <button type="button" className="link-button muted" onClick={() => { setMode("key"); setError(""); }}>
                예전 파트너 키로 접속
              </button>
            </div>
          </>
        )}
        {mode === "key" && (
          <>
            <h1>파트너 키로 접속</h1>
            <p className="gy-muted">운영자가 예전에 발급한 파트너 키입니다. 키는 이 탭에만 보관됩니다. 계정 로그인으로 옮기려면 운영자에게 비밀번호 설정 링크를 요청하세요.</p>
            <form className="gy-form" onSubmit={submitKey}>
              <Field label="파트너 키">
                <input type="password" required value={form.key} onChange={set("key")} autoComplete="off" />
              </Field>
              {error && <div className="gy-error">{error}</div>}
              <button className="gy-button block" disabled={busy}>
                접속
              </button>
            </form>
            <button type="button" className="link-button" onClick={() => setMode("login")}>
              ← 로그인
            </button>
          </>
        )}
        {mode === "forgot" && (
          <>
            <h1>비밀번호 찾기·설정</h1>
            <p className="gy-muted">파트너 계정 이메일로 비밀번호 설정 링크를 보냅니다. 운영자가 대신 등록한 업체도 여기서 처음 비밀번호를 정할 수 있습니다.</p>
            {notice ? (
              <div className="gy-note">{notice}</div>
            ) : (
              <form className="gy-form" onSubmit={submitForgot}>
                <Field label="이메일">
                  <input type="email" required value={form.email} onChange={set("email")} />
                </Field>
                {error && <div className="gy-error">{error}</div>}
                <button className="gy-button block" disabled={busy}>
                  설정 링크 받기
                </button>
              </form>
            )}
            <button type="button" className="link-button" onClick={() => { setMode("login"); setNotice(""); }}>
              ← 로그인
            </button>
          </>
        )}
        {mode === "reset" && (
          <>
            <h1>새 비밀번호 정하기</h1>
            <form className="gy-form" onSubmit={submitReset}>
              <Field label="새 비밀번호" hint="10자 이상">
                <input type="password" required minLength={10} value={form.password} onChange={set("password")} autoComplete="new-password" />
              </Field>
              <Field label="한 번 더">
                <input type="password" required value={form.confirm} onChange={set("confirm")} autoComplete="new-password" />
              </Field>
              {error && <div className="gy-error">{error}</div>}
              <button className="gy-button block" disabled={busy}>
                비밀번호 정하기
              </button>
            </form>
          </>
        )}
        {mode === "apply" && (
          <>
            <h1>가게요 입점 신청</h1>
            <p className="gy-muted">계정과 업체 정보를 적어 주세요. 운영자가 사업자 정보를 확인하고 승인하면 고객 화면에 업체가 보입니다.</p>
            <form className="gy-form" onSubmit={submitApply}>
              <h2 className="form-section">1. 담당자 계정</h2>
              <Field label="이메일" required>
                <div className="gy-inline">
                  <input type="email" required value={apply.email} onChange={setA("email")} autoComplete="email" />
                  {config?.signup_email_verify && (
                    <CodeButton
                      onSend={async () => {
                        try {
                          await api("/partner/auth/send-code", { method: "POST", body: { email: apply.email } });
                          toast("인증 코드를 메일로 보냈습니다.");
                        } catch (err) {
                          setError(err.message);
                          throw err;
                        }
                      }}
                    />
                  )}
                </div>
              </Field>
              {config?.signup_email_verify && (
                <Field label="인증 코드" required>
                  <input required inputMode="numeric" value={apply.email_code} onChange={setA("email_code")} />
                </Field>
              )}
              <div className="gy-row">
                <Field label="비밀번호" required hint="10자 이상">
                  <input type="password" required minLength={10} value={apply.password} onChange={setA("password")} autoComplete="new-password" />
                </Field>
                <Field label="담당자 이름" required>
                  <input required value={apply.display_name} onChange={setA("display_name")} />
                </Field>
              </div>
              <Field label="담당자 연락처" required>
                <input type="tel" required value={apply.phone} onChange={setA("phone")} />
              </Field>

              <h2 className="form-section">2. 업체 정보</h2>
              <div className="gy-row">
                <Field label="상호" required>
                  <input required maxLength={160} value={apply.name} onChange={setA("name")} />
                </Field>
                <Field label="업체 전화" required>
                  <input type="tel" required value={apply.shop_phone} onChange={setA("shop_phone")} />
                </Field>
              </div>
              <Field label="서비스 분야" required hint="고른 분야와 지역이 맞는 견적 요청을 받습니다 (최대 8개, 첫 번째가 대표 분야)">
                <div className="category-picker">
                  {groups.map((g) => (
                    <div key={g}>
                      <small>{g}</small>
                      {categories
                        .filter((c) => c.group_name === g)
                        .map((c) => (
                          <button key={c.id} type="button" className={apply.categories.includes(c.name) ? "on" : ""} onClick={() => toggleCategory(c.name)}>
                            {c.name}
                          </button>
                        ))}
                    </div>
                  ))}
                </div>
              </Field>
              <div className="gy-row">
                <Field label="지역 (구·군)" required hint="예: 강남구 — 이 지역 견적 요청을 받습니다">
                  <input required maxLength={100} value={apply.district} onChange={setA("district")} />
                </Field>
                <Field label="주소" required>
                  <AddressInput
                    required
                    value={apply.address}
                    onChange={(v) => setApply((a) => ({ ...a, address: v }))}
                    onPick={(r) => r.sigungu && setApply((a) => ({ ...a, district: r.sigungu }))}
                  />
                </Field>
              </div>
              <div className="gy-row">
                <Field label="사업자등록번호" required>
                  <input required value={apply.business_number} onChange={setA("business_number")} placeholder="000-00-00000" />
                </Field>
                <Field label="대표자">
                  <input maxLength={100} value={apply.representative} onChange={setA("representative")} />
                </Field>
              </div>
              <Field label="업체 소개" required>
                <textarea required maxLength={3000} value={apply.description} onChange={setA("description")} placeholder="주요 작업, 보유 장비, 자격, 보증 등" />
              </Field>
              <Field label="위치 (선택)" hint="넣으면 고객의 '내 주변 업체' 검색에 나옵니다. 업체에서 [현재 위치]를 누르면 편해요.">
                <div className="gy-inline">
                  <input inputMode="decimal" placeholder="위도 37.5" value={apply.latitude} onChange={setA("latitude")} />
                  <input inputMode="decimal" placeholder="경도 127.0" value={apply.longitude} onChange={setA("longitude")} />
                  <button type="button" className="gy-button ghost" onClick={fillMyLocation}>
                    현재 위치
                  </button>
                </div>
              </Field>
              <label className="gy-check">
                <input type="checkbox" required checked={apply.agree} onChange={setA("agree")} />
                파트너 이용약관과 개인정보 처리(고객 연락처는 견적·예약·주문 처리에만 사용)에 동의합니다.
              </label>
              {error && <div className="gy-error">{error}</div>}
              <button className="gy-button block" disabled={busy}>
                {busy ? "보내는 중…" : "입점 신청하기"}
              </button>
            </form>
            <button type="button" className="link-button" onClick={() => setMode("login")}>
              ← 로그인
            </button>
          </>
        )}
      </section>
    </main>
  );
}
