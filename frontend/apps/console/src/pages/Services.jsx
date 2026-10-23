import { useState } from "react";
import { money, Field, Empty, Loading, ErrorBox, useLoad } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

const empty = { name: "", description: "", price_from: "", duration_minutes: "60", sort_order: "0", is_active: true };

/** 예약 서비스: 등록·수정·숨기기 (예전에는 등록만 되고 수정·삭제가 화면에 없었음) */
export default function Services() {
  const { toast } = usePartner();
  const { data, loading, error, reload } = useLoad(() => papi("/partner/services"), []);
  const [form, setForm] = useState(empty);
  const [editing, setEditing] = useState(null);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.type === "checkbox" ? e.target.checked : e.target.value });

  async function save(e) {
    e.preventDefault();
    const body = { ...form, price_from: Number(form.price_from), duration_minutes: Number(form.duration_minutes), sort_order: Number(form.sort_order) };
    try {
      await papi(editing ? `/partner/services/${editing}` : "/partner/services", { method: editing ? "PATCH" : "POST", body });
      toast(editing ? "서비스를 고쳤습니다." : "서비스를 등록했습니다.");
      setForm(empty);
      setEditing(null);
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  async function remove(s) {
    if (!window.confirm(`'${s.name}' 서비스를 지울까요? 예약에 쓰인 서비스는 숨김 처리됩니다.`)) return;
    try {
      await papi(`/partner/services/${s.id}`, { method: "DELETE" });
      toast("지웠습니다.");
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  function edit(s) {
    setEditing(s.id);
    setForm({ name: s.name, description: s.description, price_from: String(s.price_from), duration_minutes: String(s.duration_minutes), sort_order: String(s.sort_order), is_active: s.is_active });
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  return (
    <div className="grid-form">
      <form className="panel gy-form" onSubmit={save}>
        <h2>{editing ? "서비스 고치기" : "서비스 등록"}</h2>
        <Field label="서비스명" required>
          <input required maxLength={160} value={form.name} onChange={set("name")} placeholder="예: 엔진오일 교환" />
        </Field>
        <Field label="설명">
          <textarea maxLength={2000} value={form.description} onChange={set("description")} />
        </Field>
        <div className="gy-row">
          <Field label="시작 가격 (원)" required>
            <input type="number" min="1" required value={form.price_from} onChange={set("price_from")} />
          </Field>
          <Field label="소요 시간 (분)" required>
            <input type="number" min="1" required value={form.duration_minutes} onChange={set("duration_minutes")} />
          </Field>
        </div>
        <div className="gy-row">
          <Field label="정렬 순서" hint="작을수록 위">
            <input type="number" min="0" value={form.sort_order} onChange={set("sort_order")} />
          </Field>
          <label className="gy-check">
            <input type="checkbox" checked={form.is_active} onChange={set("is_active")} />
            고객에게 보이기
          </label>
        </div>
        <div className="gy-actions">
          {editing && (
            <button type="button" className="gy-button ghost" onClick={() => { setEditing(null); setForm(empty); }}>
              취소
            </button>
          )}
          <button className="gy-button">{editing ? "저장" : "등록"}</button>
        </div>
      </form>
      <section className="panel">
        <h2>등록한 서비스</h2>
        <ErrorBox error={error} onRetry={reload} />
        {loading && !data && <Loading />}
        {data?.length === 0 && <Empty>서비스를 등록하면 고객이 예약할 때 고를 수 있습니다.</Empty>}
        <ul className="row-list">
          {data?.map((s) => (
            <li key={s.id} className={s.is_active ? "" : "inactive"}>
              <div>
                <b>{s.name}</b>
                <small>{s.description}</small>
              </div>
              <span>
                {money(s.price_from)}~ · {s.duration_minutes}분{!s.is_active && " · 숨김"}
              </span>
              <div className="gy-actions">
                <button type="button" className="gy-button ghost small" onClick={() => edit(s)}>
                  고치기
                </button>
                <button type="button" className="gy-button danger small" onClick={() => remove(s)}>
                  삭제
                </button>
              </div>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
