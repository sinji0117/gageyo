import { useState } from "react";
import { api, Field } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { setAdminKey } from "../aapi.js";

/**
 * 운영자 로그인. 서버 설정 ADMIN_EMAILS 에 있는 이메일만 들어올 수 있습니다.
 * 처음에는 [비밀번호 설정 메일 받기]로 비밀번호를 정합니다 (메일 설정이 없으면 서버 로그에 링크가 남음).
 * 예전 방식 ADMIN_API_KEY 는 비상용으로 남겨 둡니다.
 */
export default function Login({ resetToken, onLoggedIn }) {
  const { toast } = useAdmin();
  const [mode, setMode] = useState(resetToken ? "reset" : "login");
  const [form, setForm] = useState({ email: "", password: "", otp: "", key: "", confirm: "" });
  const [needOtp, setNeedOtp] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });

  async function run(fn) {
    setBusy(true);
    setError("");
    try {
      await fn();
    } catch (e) {
      if (e.code === "otp_required") {
        setNeedOtp(true);
        setError("OTP 앱의 6자리 코드를 입력해 주세요.");
      } else setError(e.status === 423 ? "로그인을 5번 틀려 15분 동안 잠겼습니다." : e.message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="admin-login">
      <section>
        <div className="brand">
          ✳ 가게요 <small>PLATFORM OPS</small>
        </div>
        {mode === "login" && (
          <form
            className="gy-form"
            onSubmit={(e) => {
              e.preventDefault();
              run(async () => {
                setAdminKey("");
                await api("/admin/auth/login", { method: "POST", body: { email: form.email, password: form.password, otp: form.otp || undefined } });
                await onLoggedIn();
              });
            }}
          >
            <h1>운영자 로그인</h1>
            <Field label="이메일">
              <input type="email" required value={form.email} onChange={set("email")} autoComplete="username" />
            </Field>
            <Field label="비밀번호">
              <input type="password" required value={form.password} onChange={set("password")} autoComplete="current-password" />
            </Field>
            {needOtp && (
              <Field label="OTP 코드">
                <input inputMode="numeric" autoComplete="one-time-code" required maxLength={7} value={form.otp} onChange={set("otp")} autoFocus />
              </Field>
            )}
            {error && <div className="gy-error">{error}</div>}
            <button className="gy-button block" disabled={busy}>
              {busy ? "확인 중…" : "로그인"}
            </button>
            <div className="login-links">
              <button type="button" className="link-button" onClick={() => { setMode("forgot"); setError(""); }}>
                비밀번호 설정 메일 받기
              </button>
              <button type="button" className="link-button" onClick={() => { setMode("key"); setError(""); }}>
                API 키로 접속
              </button>
            </div>
          </form>
        )}
        {mode === "forgot" && (
          <form
            className="gy-form"
            onSubmit={(e) => {
              e.preventDefault();
              run(async () => {
                await api("/auth/password/forgot", { method: "POST", body: { email: form.email, scope: "admin" } });
                setNotice("ADMIN_EMAILS 에 있는 이메일이면 비밀번호 설정 링크를 보냈습니다. 메일 설정(SMTP_URL)이 없는 서버는 API 앱 로그에 링크가 남습니다.");
              });
            }}
          >
            <h1>비밀번호 설정</h1>
            {notice ? <div className="gy-note">{notice}</div> : null}
            <Field label="운영자 이메일">
              <input type="email" required value={form.email} onChange={set("email")} />
            </Field>
            {error && <div className="gy-error">{error}</div>}
            <button className="gy-button block" disabled={busy}>
              설정 링크 받기
            </button>
            <button type="button" className="link-button" onClick={() => { setMode("login"); setNotice(""); }}>
              ← 로그인
            </button>
          </form>
        )}
        {mode === "reset" && (
          <form
            className="gy-form"
            onSubmit={(e) => {
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
                setForm({ ...form, password: "", confirm: "" });
              });
            }}
          >
            <h1>새 비밀번호</h1>
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
        )}
        {mode === "key" && (
          <form
            className="gy-form"
            onSubmit={(e) => {
              e.preventDefault();
              run(async () => {
                await api("/admin/auth/me", { headers: { "x-admin-key": form.key.trim() } });
                setAdminKey(form.key.trim());
                await onLoggedIn();
              });
            }}
          >
            <h1>API 키로 접속</h1>
            <p className="gy-muted">Rpanel API 앱의 ADMIN_API_KEY 입니다. 이 브라우저 탭에만 보관되고, 작업 기록에는 'api-key' 로 남습니다. 평소에는 운영자 계정으로 로그인하세요.</p>
            <Field label="ADMIN_API_KEY">
              <input type="password" required value={form.key} onChange={set("key")} autoComplete="off" />
            </Field>
            {error && <div className="gy-error">{error}</div>}
            <button className="gy-button block" disabled={busy}>
              접속
            </button>
            <button type="button" className="link-button" onClick={() => setMode("login")}>
              ← 로그인
            </button>
          </form>
        )}
      </section>
    </main>
  );
}
