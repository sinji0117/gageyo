import { useEffect, useMemo, useState } from "react";
import { api, money, Modal, Field, Loading, ErrorBox, useLoad, hoursText, todayIso, toLocalIso, DAY_NAMES } from "@gageyo/ui";
import { useApp } from "../context.js";

function slots(open, close) {
  if (!open || !close) return null;
  const toMin = (t) => Number(t.slice(0, 2)) * 60 + Number(t.slice(3, 5));
  const out = [];
  for (let m = toMin(open); m < toMin(close); m += 30) out.push(`${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`);
  return out;
}

export default function BookingDialog({ workshopId, workshopName, serviceId = "", quoteOfferId = "", vehicle = "", name = "", phone = "" }) {
  const { close, user, toast, go } = useApp();
  const { data: w, loading, error, reload } = useLoad(() => api(`/workshops/${workshopId}`), [workshopId]);
  const [vehicles, setVehicles] = useState([]);
  const [form, setForm] = useState({
    service_id: serviceId,
    date: todayIso(1),
    time: "10:00",
    vehicle,
    customer_name: name || user?.display_name || "",
    customer_phone: phone || user?.phone || "",
    note: "",
  });
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(null);
  const [formError, setFormError] = useState("");

  useEffect(() => {
    if (user) api("/me/vehicles").then(setVehicles).catch(() => setVehicles([]));
  }, [user]);
  useEffect(() => {
    if (!form.vehicle && vehicles.length) setForm((f) => ({ ...f, vehicle: `${vehicles[0].model}${vehicles[0].model_year ? ` ${vehicles[0].model_year}년식` : ""}` }));
  }, [vehicles, form.vehicle]);

  const timeSlots = useMemo(() => slots(w?.open_time, w?.close_time), [w]);
  const closedDays = (w?.closed_days || "").split(",").filter(Boolean).map(Number);
  const pickedDay = new Date(`${form.date}T00:00:00`).getDay();
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });

  async function submit(e) {
    e.preventDefault();
    setFormError("");
    if (closedDays.includes(pickedDay)) {
      setFormError(`${DAY_NAMES[pickedDay]}요일은 휴무입니다. 다른 날을 골라 주세요.`);
      return;
    }
    setBusy(true);
    try {
      const res = await api("/bookings", {
        method: "POST",
        body: {
          workshop_id: workshopId,
          service_id: form.service_id || null,
          quote_offer_id: quoteOfferId || null,
          vehicle: form.vehicle,
          starts_at: toLocalIso(form.date, form.time),
          customer_name: form.customer_name,
          customer_phone: form.customer_phone,
          note: form.note,
        },
      });
      setDone(res);
      toast("예약 신청을 보냈습니다.");
    } catch (err) {
      setFormError(err.message);
    } finally {
      setBusy(false);
    }
  }

  if (done) {
    return (
      <Modal title="예약 신청 완료" eyebrow={workshopName} onClose={close}>
        <div className="done-box">
          <p>
            <b>{workshopName}</b>에 예약 신청을 보냈습니다. 업체가 확정하면 {user ? "메일과 마이페이지에서" : "연락처로"} 알려 드립니다.
          </p>
          <dl className="info-list">
            <dt>예약 번호</dt>
            <dd className="mono">{done.id}</dd>
            <dt>방문 시간</dt>
            <dd>
              {form.date} {form.time}
            </dd>
          </dl>
          {!user && <p className="gy-note">비회원 예약은 예약 번호와 연락처로 [주문·예약 조회]에서 확인·취소할 수 있습니다. 예약 번호를 꼭 저장해 두세요.</p>}
          <div className="cta-row">
            {user && (
              <button type="button" className="gy-button" onClick={() => { close(); go("my", "bookings"); }}>
                내 예약 보기
              </button>
            )}
            <button type="button" className="gy-button ghost" onClick={close}>
              닫기
            </button>
          </div>
        </div>
      </Modal>
    );
  }

  return (
    <Modal title={`${workshopName || w?.name || "업체"} 예약`} eyebrow="방문 예약" onClose={close}>
      {loading && !w && <Loading />}
      <ErrorBox error={error} onRetry={reload} />
      {w && (
        <form className="gy-form" onSubmit={submit}>
          <p className="gy-muted small">영업시간: {hoursText(w)}</p>
          <Field label="서비스">
            <select value={form.service_id} onChange={set("service_id")}>
              <option value="">선택 안 함 (메모에 적기)</option>
              {w.services.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name} · {money(s.price_from)}~ · {s.duration_minutes}분
                </option>
              ))}
            </select>
          </Field>
          <div className="gy-row">
            <Field label="방문 날짜" required hint={closedDays.includes(pickedDay) ? `${DAY_NAMES[pickedDay]}요일 휴무` : undefined}>
              <input type="date" required min={todayIso(0)} max={todayIso(89)} value={form.date} onChange={set("date")} />
            </Field>
            <Field label="시간" required>
              {timeSlots ? (
                <select value={form.time} onChange={set("time")}>
                  {timeSlots.map((t) => (
                    <option key={t}>{t}</option>
                  ))}
                </select>
              ) : (
                <input type="time" required value={form.time} onChange={set("time")} step="1800" />
              )}
            </Field>
          </div>
          <Field label="차량" required>
            {vehicles.length > 0 && (
              <select onChange={(e) => e.target.value && setForm({ ...form, vehicle: e.target.value })} defaultValue="">
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
          <div className="gy-row">
            <Field label="이름" required>
              <input required maxLength={100} value={form.customer_name} onChange={set("customer_name")} autoComplete="name" />
            </Field>
            <Field label="연락처" required>
              <input required type="tel" value={form.customer_phone} onChange={set("customer_phone")} placeholder="010-0000-0000" autoComplete="tel" />
            </Field>
          </div>
          <Field label="요청 사항">
            <textarea maxLength={1000} value={form.note} onChange={set("note")} placeholder="증상이나 원하는 작업, 방문 전 전달할 내용" />
          </Field>
          {formError && <div className="gy-error">{formError}</div>}
          <button className="gy-button block" disabled={busy}>
            {busy ? "보내는 중…" : "예약 신청하기"}
          </button>
        </form>
      )}
    </Modal>
  );
}
