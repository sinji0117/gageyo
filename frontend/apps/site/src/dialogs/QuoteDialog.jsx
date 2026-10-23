import { useEffect, useState } from "react";
import { api, Modal, Field } from "@gageyo/ui";
import { useApp } from "../context.js";

const SAVED_KEY = "gageyo_quote_ids";

export function rememberQuote(id, phone) {
  try {
    const saved = JSON.parse(window.localStorage.getItem(SAVED_KEY) || "[]").filter((q) => q && q.id !== id);
    window.localStorage.setItem(SAVED_KEY, JSON.stringify([{ id, phone, at: Date.now() }, ...saved].slice(0, 20)));
  } catch {
    /* 저장소 없음 */
  }
}

export function savedQuotes() {
  try {
    return JSON.parse(window.localStorage.getItem(SAVED_KEY) || "[]").filter((q) => q && typeof q === "object" && q.id);
  } catch {
    return [];
  }
}

/** 견적 요청서 (분야·지역이 맞는 업체들이 견적을 보냄) */
export default function QuoteDialog({ category = "" }) {
  const { close, open, categories, user, toast } = useApp();
  const [vehicles, setVehicles] = useState([]);
  const [form, setForm] = useState({
    service_category: category || categories[0]?.name || "기타",
    district: "",
    description: "",
    vehicle: "",
    customer_name: user?.display_name || "",
    customer_phone: user?.phone || "",
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (user) api("/me/vehicles").then(setVehicles).catch(() => {});
  }, [user]);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });

  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      const res = await api("/quotes", { method: "POST", body: form });
      rememberQuote(res.id, form.customer_phone);
      toast("견적 요청을 보냈습니다. 업체 견적이 도착하면 알려 드릴게요.");
      open({ type: "quoteView", id: res.id, phone: form.customer_phone, fresh: true });
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  const groups = [...new Set(categories.map((c) => c.group_name))];
  return (
    <Modal title="업체에 견적 요청" eyebrow="무료 견적" onClose={close}>
      <p className="gy-muted">차량과 원하는 작업을 적으면 분야와 지역이 맞는 업체들이 견적을 보냅니다. 받은 견적은 14일 동안 비교할 수 있어요.</p>
      <form className="gy-form" onSubmit={submit}>
        <Field label="서비스 종류" required>
          <select value={form.service_category} onChange={set("service_category")}>
            {groups.map((g) => (
              <optgroup key={g} label={g}>
                {categories
                  .filter((c) => c.group_name === g)
                  .map((c) => (
                    <option key={c.id}>{c.name}</option>
                  ))}
              </optgroup>
            ))}
            <option value="기타">기타</option>
          </select>
        </Field>
        <Field label="지역" required hint="구·동까지 적으면 가까운 업체가 받아요">
          <input required maxLength={100} value={form.district} onChange={set("district")} placeholder="예: 서울 강남구 역삼동" />
        </Field>
        <Field label="차량" required>
          {vehicles.length > 0 && (
            <select defaultValue="" onChange={(e) => e.target.value && setForm({ ...form, vehicle: e.target.value })}>
              <option value="">내 차량에서 고르기</option>
              {vehicles.map((v) => (
                <option key={v.id} value={`${v.model}${v.model_year ? ` ${v.model_year}년식` : ""}`}>
                  {v.label} · {v.model}
                </option>
              ))}
            </select>
          )}
          <input required maxLength={160} value={form.vehicle} onChange={set("vehicle")} placeholder="예: 현대 아반떼 2021년식" />
        </Field>
        <Field label="요청 내용" required>
          <textarea required maxLength={2000} value={form.description} onChange={set("description")} placeholder="증상, 원하는 작업, 희망 일정 등을 적어 주세요." />
        </Field>
        <div className="gy-row">
          <Field label="이름">
            <input maxLength={100} value={form.customer_name} onChange={set("customer_name")} autoComplete="name" />
          </Field>
          <Field label="연락처" required hint="견적을 고른 업체에만 보입니다">
            <input required type="tel" value={form.customer_phone} onChange={set("customer_phone")} placeholder="010-0000-0000" autoComplete="tel" />
          </Field>
        </div>
        {error && <div className="gy-error">{error}</div>}
        <button className="gy-button block" disabled={busy}>
          {busy ? "보내는 중…" : "무료 견적 요청하기"}
        </button>
      </form>
    </Modal>
  );
}
