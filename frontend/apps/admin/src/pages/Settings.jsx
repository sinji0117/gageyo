import { useEffect, useState } from "react";
import { Field, Loading, ErrorBox, useLoad } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

/** 플랫폼 수수료·무통장 입금 계좌·고객센터 정보. 결제 키·메일 서버는 보안상 API 앱 환경 변수로만 정합니다. */
export default function Settings() {
  const { toast } = useAdmin();
  const { data, loading, error, reload, setData } = useLoad(() => aapi("/admin/settings"), []);
  const [form, setForm] = useState(null);
  useEffect(() => {
    if (!data) return;
    setForm({
      fee_rate_percent: (Number(data.fee_rate_bp || 0) / 100).toString(),
      bank_name: data.bank_name || "",
      bank_account: data.bank_account || "",
      bank_holder: data.bank_holder || "",
      bank_due_hours: data.bank_due_hours || "48",
      support_phone: data.support_phone || "",
      support_email: data.support_email || "",
      company_info: data.company_info || "",
    });
  }, [data]);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });

  async function save(e) {
    e.preventDefault();
    try {
      const { fee_rate_percent, bank_due_hours, ...rest } = form;
      const updated = await aapi("/admin/settings", {
        method: "PUT",
        body: { ...rest, fee_rate_bp: Math.round(Number(fee_rate_percent) * 100), bank_due_hours: Number(bank_due_hours) },
      });
      setData(updated);
      toast("설정을 저장했습니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  if (loading && !data) return <Loading />;
  if (!form) return <ErrorBox error={error} onRetry={reload} />;
  return (
    <>
      <div className="page-title">
        <div>
          <h1>결제·운영 설정</h1>
          <p>바꾼 내용은 작업 기록에 남습니다.</p>
        </div>
      </div>
      <div className="status-cards">
        <div className={data.toss_enabled ? "ok" : ""}>
          <b>카드·간편결제 (토스페이먼츠)</b>
          <span>{data.toss_enabled ? (data.toss_test_mode ? "테스트 키로 켜짐" : "실제 키로 켜짐") : "꺼짐 — API 앱에 TOSS_CLIENT_KEY·TOSS_SECRET_KEY 설정"}</span>
        </div>
        <div className={data.mail_enabled ? "ok" : ""}>
          <b>메일 알림</b>
          <span>{data.mail_enabled ? "켜짐" : "꺼짐 — API 앱에 SMTP_URL·MAIL_FROM 설정 (지금은 로그에만 남음)"}</span>
        </div>
        <div className={data.signup_email_verify ? "ok" : ""}>
          <b>가입 이메일 인증</b>
          <span>{data.signup_email_verify ? "켜짐" : "꺼짐 (SIGNUP_EMAIL_VERIFY · 메일 설정 필요)"}</span>
        </div>
      </div>
      <form className="settings-grid" onSubmit={save}>
        <section className="box gy-form">
          <h2>수수료</h2>
          <Field label="플랫폼 수수료율 (%)" hint="카드·무통장으로 플랫폼이 받은 결제에만 적용, 결제 때 줄마다 고정됩니다 (0~30%)">
            <input type="number" min="0" max="30" step="0.01" value={form.fee_rate_percent} onChange={set("fee_rate_percent")} />
          </Field>
        </section>
        <section className="box gy-form">
          <h2>무통장 입금 계좌</h2>
          <p className="gy-muted small">세 칸을 모두 채우면 주문서에 무통장입금이 보입니다. 비우면 꺼집니다.</p>
          <div className="gy-row">
            <Field label="은행">
              <input value={form.bank_name} onChange={set("bank_name")} placeholder="국민은행" />
            </Field>
            <Field label="예금주">
              <input value={form.bank_holder} onChange={set("bank_holder")} />
            </Field>
          </div>
          <Field label="계좌번호">
            <input value={form.bank_account} onChange={set("bank_account")} inputMode="numeric" />
          </Field>
          <Field label="입금 기한 (시간)">
            <input type="number" min="1" max="168" value={form.bank_due_hours} onChange={set("bank_due_hours")} />
          </Field>
        </section>
        <section className="box gy-form">
          <h2>고객센터·사업자 정보</h2>
          <div className="gy-row">
            <Field label="고객센터 전화">
              <input value={form.support_phone} onChange={set("support_phone")} />
            </Field>
            <Field label="고객센터 이메일">
              <input type="email" value={form.support_email} onChange={set("support_email")} />
            </Field>
          </div>
          <Field label="사업자 정보" hint="고객 화면 아래쪽에 보입니다 (상호·대표·사업자번호·통신판매업 신고번호·주소 등)">
            <textarea rows={4} value={form.company_info} onChange={set("company_info")} />
          </Field>
        </section>
        <div className="settings-save">
          <button className="gy-button">설정 저장</button>
        </div>
      </form>
    </>
  );
}
