import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { Field } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

/** 내 보안: OTP(2단계 인증) 켜기·끄기. QR 은 이 브라우저에서 만들고 밖으로 보내지 않습니다. */
export default function Security() {
  const { me, toast } = useAdmin();
  const [enabled, setEnabled] = useState(Boolean(me.otp_enabled));
  const [setup, setSetup] = useState(null);
  const [qr, setQr] = useState("");
  const [code, setCode] = useState("");

  useEffect(() => {
    if (!setup) return;
    QRCode.toDataURL(setup.otpauth_url, { margin: 1, width: 200 })
      .then(setQr)
      .catch(() => setQr(""));
  }, [setup]);

  if (me.via_key) {
    return (
      <div className="box">
        <h2>내 보안</h2>
        <p>API 키로 접속한 상태입니다. OTP 는 운영자 계정으로 로그인한 뒤 켤 수 있습니다.</p>
      </div>
    );
  }
  async function start() {
    try {
      setSetup(await aapi("/admin/auth/otp/setup", { method: "POST" }));
      setCode("");
    } catch (e) {
      toast(e.message);
    }
  }
  async function enable(e) {
    e.preventDefault();
    try {
      await aapi("/admin/auth/otp/enable", { method: "POST", body: { secret: setup.secret, code } });
      setEnabled(true);
      setSetup(null);
      toast("2단계 인증을 켰습니다. 다음 로그인부터 코드가 필요합니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  async function disable(e) {
    e.preventDefault();
    try {
      await aapi("/admin/auth/otp/disable", { method: "POST", body: { code } });
      setEnabled(false);
      setCode("");
      toast("2단계 인증을 껐습니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>내 보안</h1>
          <p>{me.email}</p>
        </div>
      </div>
      <div className="box security">
        <h2>2단계 인증 (OTP) {enabled ? "켜짐" : "꺼짐"}</h2>
        <p className="gy-muted">Google Authenticator·Microsoft Authenticator 같은 OTP 앱의 6자리 코드를 로그인 때 함께 넣습니다. 같은 코드는 한 번만 쓸 수 있습니다.</p>
        {!enabled && !setup && (
          <button type="button" className="gy-button" onClick={start}>
            OTP 켜기 시작
          </button>
        )}
        {setup && (
          <form className="gy-form otp-setup" onSubmit={enable}>
            <div className="qr">{qr ? <img src={qr} alt="OTP 등록 QR 코드" width="200" height="200" /> : <span>QR 을 만들지 못했습니다</span>}</div>
            <div>
              <p>1. OTP 앱에서 QR 을 찍거나 아래 키를 직접 넣으세요.</p>
              <code className="secret">{setup.secret.replace(/(.{4})/g, "$1 ").trim()}</code>
              <p>2. 앱에 보이는 6자리 코드를 넣어 확인합니다.</p>
              <Field label="확인 코드">
                <input required inputMode="numeric" autoComplete="one-time-code" maxLength={7} value={code} onChange={(e) => setCode(e.target.value)} />
              </Field>
              <div className="gy-actions">
                <button type="button" className="gy-button ghost" onClick={() => setSetup(null)}>
                  취소
                </button>
                <button className="gy-button">확인하고 켜기</button>
              </div>
            </div>
          </form>
        )}
        {enabled && (
          <form className="gy-form narrow" onSubmit={disable}>
            <Field label="끄려면 지금 OTP 코드를 넣으세요">
              <input required inputMode="numeric" maxLength={7} value={code} onChange={(e) => setCode(e.target.value)} />
            </Field>
            <button className="gy-button danger">OTP 끄기</button>
          </form>
        )}
        <p className="gy-muted small">OTP 앱을 잃어버리면 서버 DB 에서 users.totp_secret 을 비워 끈 뒤 다시 등록합니다 (docs/operations.md 참고).</p>
      </div>
    </>
  );
}
