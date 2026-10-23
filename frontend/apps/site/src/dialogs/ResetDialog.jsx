import { useState } from "react";
import { api, Modal, Field } from "@gageyo/ui";
import { useApp } from "../context.js";

/** 메일 링크(?reset=)로 들어와 새 비밀번호 정하기 */
export default function ResetDialog({ token }) {
  const { close, open, toast } = useApp();
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(e) {
    e.preventDefault();
    if (password !== confirm) {
      setError("두 비밀번호가 다릅니다.");
      return;
    }
    setBusy(true);
    setError("");
    try {
      await api("/auth/password/reset", { method: "POST", body: { token, password } });
      toast("비밀번호를 바꿨습니다. 새 비밀번호로 로그인해 주세요.");
      open({ type: "auth" });
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title="새 비밀번호 정하기" eyebrow="가게요 계정" onClose={close}>
      <form className="gy-form" onSubmit={submit}>
        <Field label="새 비밀번호" required hint="10자 이상">
          <input type="password" required minLength={10} maxLength={128} value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" />
        </Field>
        <Field label="한 번 더" required>
          <input type="password" required value={confirm} onChange={(e) => setConfirm(e.target.value)} autoComplete="new-password" />
        </Field>
        {error && <div className="gy-error">{error}</div>}
        <button className="gy-button block" disabled={busy}>
          {busy ? "바꾸는 중…" : "비밀번호 바꾸기"}
        </button>
      </form>
    </Modal>
  );
}
