import { useState } from "react";
import { api, Modal, Field, CodeButton } from "@gageyo/ui";
import { useApp } from "../context.js";

export default function AuthDialog({ mode: initialMode = "login", then = null }) {
  const { close, setUser, toast, config, go } = useApp();
  const [mode, setMode] = useState(initialMode);
  const [form, setForm] = useState({ email: "", password: "", display_name: "", phone: "", email_code: "", agree: false });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [sent, setSent] = useState(false);
  const needCode = Boolean(config?.signup_email_verify);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.type === "checkbox" ? e.target.checked : e.target.value });

  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      if (mode === "forgot") {
        await api("/auth/password/forgot", { method: "POST", body: { email: form.email } });
        setSent(true);
        return;
      }
      const profile =
        mode === "register"
          ? await api("/auth/register", {
              method: "POST",
              body: { email: form.email, password: form.password, display_name: form.display_name, phone: form.phone, email_code: form.email_code, agree_terms: form.agree },
            })
          : await api("/auth/login", { method: "POST", body: { email: form.email, password: form.password } });
      setUser(profile);
      toast(`${profile.display_name}님, 반갑습니다.`);
      close();
      if (then?.view) go(then.view, then.tab);
    } catch (err) {
      setError(err.status === 423 ? "로그인을 5번 틀려 15분 동안 잠겼습니다. 비밀번호 찾기를 이용해 주세요." : err.message);
    } finally {
      setBusy(false);
    }
  }

  const title = { login: "로그인", register: "회원가입", forgot: "비밀번호 찾기" }[mode];
  return (
    <Modal title={title} eyebrow="가게요 계정" onClose={close}>
      {mode === "forgot" && sent ? (
        <div className="done-box">
          <p>
            <b>{form.email}</b>로 가입되어 있다면 비밀번호 설정 메일을 보냈습니다. 메일의 링크는 30분 동안 쓸 수 있습니다.
          </p>
          <button type="button" className="gy-button" onClick={() => { setMode("login"); setSent(false); }}>
            로그인으로
          </button>
        </div>
      ) : (
        <form className="gy-form" onSubmit={submit}>
          {mode === "register" && <p className="gy-muted">가입하면 견적·예약·주문을 마이페이지에서 한 번에 보고, 리뷰를 남길 수 있어요.</p>}
          {mode === "register" && (
            <div className="gy-row">
              <Field label="이름" required>
                <input required maxLength={100} value={form.display_name} onChange={set("display_name")} autoComplete="name" />
              </Field>
              <Field label="연락처">
                <input type="tel" value={form.phone} onChange={set("phone")} placeholder="010-0000-0000" autoComplete="tel" />
              </Field>
            </div>
          )}
          <Field label="이메일" required>
            <div className="gy-inline">
              <input type="email" required value={form.email} onChange={set("email")} autoComplete="email" placeholder="name@example.com" />
              {mode === "register" && needCode && (
                <CodeButton
                  onSend={async () => {
                    try {
                      await api("/auth/send-code", { method: "POST", body: { email: form.email } });
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
          {mode === "register" && needCode && (
            <Field label="인증 코드" required hint="메일로 받은 6자리 숫자 (10분 동안 유효)">
              <input required inputMode="numeric" maxLength={7} value={form.email_code} onChange={set("email_code")} />
            </Field>
          )}
          {mode !== "forgot" && (
            <Field label="비밀번호" required hint={mode === "register" ? "10자 이상" : undefined}>
              <input
                type="password"
                required
                minLength={mode === "register" ? 10 : 1}
                maxLength={128}
                value={form.password}
                onChange={set("password")}
                autoComplete={mode === "register" ? "new-password" : "current-password"}
              />
            </Field>
          )}
          {mode === "register" && (
            <label className="gy-check">
              <input type="checkbox" required checked={form.agree} onChange={set("agree")} />
              이용약관과 개인정보 처리(견적·예약 시 업체에 연락처 제공)에 동의합니다.
            </label>
          )}
          {error && <div className="gy-error">{error}</div>}
          <button className="gy-button block" disabled={busy}>
            {busy ? "처리 중…" : mode === "register" ? "가입하기" : mode === "forgot" ? "설정 메일 받기" : "로그인"}
          </button>
          <div className="auth-links">
            {mode !== "login" && (
              <button type="button" className="link-button" onClick={() => { setMode("login"); setError(""); }}>
                로그인
              </button>
            )}
            {mode !== "register" && (
              <button type="button" className="link-button" onClick={() => { setMode("register"); setError(""); }}>
                회원가입
              </button>
            )}
            {mode !== "forgot" && (
              <button type="button" className="link-button" onClick={() => { setMode("forgot"); setError(""); }}>
                비밀번호 찾기
              </button>
            )}
          </div>
        </form>
      )}
    </Modal>
  );
}
