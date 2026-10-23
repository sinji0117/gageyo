import { useEffect, useState } from "react";
import { api, Field, Badge, ImageInput, Empty, useLoad, dateText, DAY_NAMES, timeText, AddressInput } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

/**
 * 업체 정보
 * - 소개·전화·영업시간·휴무일·위치·대표 사진: 바로 반영
 * - 상호·분야·지역·주소·사업자: 승인된 업체는 운영자 승인 뒤 반영 (예전에는 화면에서 고칠 방법이 없었음)
 */
export default function Profile() {
  const { me, reload, toast } = usePartner();
  const [form, setForm] = useState(null);
  const [core, setCore] = useState(null);
  const [categories, setCategories] = useState([]);
  const photos = useLoad(() => papi("/partner/photos"), []);
  const changes = useLoad(() => papi("/partner/change-requests"), []);

  useEffect(() => {
    api("/categories").then(setCategories).catch(() => {});
  }, []);
  useEffect(() => {
    setForm({
      description: me.description || "",
      phone: me.phone || "",
      open_time: timeText(me.open_time),
      close_time: timeText(me.close_time),
      closed_days: (me.closed_days || "").split(",").filter(Boolean).map(Number),
      cover_image_url: me.cover_image_url || "",
      latitude: me.latitude ?? "",
      longitude: me.longitude ?? "",
    });
    setCore({
      name: me.name,
      categories: me.categories || [],
      district: me.district,
      address: me.address,
      business_number: me.business_number || "",
      representative: me.representative || "",
    });
  }, [me]);
  if (!form || !core) return null;

  async function saveProfile(e) {
    e.preventDefault();
    try {
      await papi("/partner/profile", {
        method: "PUT",
        body: {
          ...form,
          closed_days: form.closed_days.join(","),
          cover_image_url: form.cover_image_url || null,
          latitude: form.latitude === "" ? null : Number(form.latitude),
          longitude: form.longitude === "" ? null : Number(form.longitude),
        },
      });
      await reload();
      toast("업체 정보를 저장했습니다.");
    } catch (err) {
      toast(err.message);
    }
  }
  async function saveCore(e) {
    e.preventDefault();
    try {
      const r = await papi("/partner/change-requests", { method: "POST", body: core });
      toast(r.applied ? "반영했습니다. 운영자가 다시 심사합니다." : "변경 요청을 보냈습니다. 운영자 승인 뒤 반영됩니다.");
      await reload();
      changes.reload();
    } catch (err) {
      toast(err.message);
    }
  }
  async function addPhoto(url) {
    if (!url) return;
    try {
      await papi("/partner/photos", { method: "POST", body: { url } });
      photos.reload();
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  async function removePhoto(p) {
    if (!window.confirm("이 사진을 지울까요?")) return;
    await papi(`/partner/photos/${p.id}`, { method: "DELETE" }).catch((err) => toast(err.message));
    photos.reload();
    reload();
  }
  const toggleDay = (d) => setForm({ ...form, closed_days: form.closed_days.includes(d) ? form.closed_days.filter((x) => x !== d) : [...form.closed_days, d] });
  const toggleCat = (name) => setCore({ ...core, categories: core.categories.includes(name) ? core.categories.filter((c) => c !== name) : [...core.categories, name].slice(0, 8) });
  const groups = [...new Set(categories.map((c) => c.group_name))];
  const pending = (changes.data || []).find((c) => c.status === "pending");

  return (
    <div className="profile-grid">
      <form className="panel gy-form" onSubmit={saveProfile}>
        <h2>소개·영업 정보</h2>
        <p className="gy-muted small">저장하면 바로 고객 화면에 반영됩니다.</p>
        <Field label="업체 소개" required>
          <textarea required rows={6} maxLength={3000} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} />
        </Field>
        <Field label="업체 전화" required>
          <input required type="tel" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} />
        </Field>
        <div className="gy-row">
          <Field label="여는 시간" hint="비우면 '영업시간 문의'">
            <input type="time" value={form.open_time} onChange={(e) => setForm({ ...form, open_time: e.target.value })} />
          </Field>
          <Field label="닫는 시간">
            <input type="time" value={form.close_time} onChange={(e) => setForm({ ...form, close_time: e.target.value })} />
          </Field>
        </div>
        <Field label="정기 휴무" hint="예약은 영업시간 안, 휴무일이 아닌 날만 받습니다">
          <div className="day-picker">
            {DAY_NAMES.map((d, i) => (
              <button key={d} type="button" className={form.closed_days.includes(i) ? "on" : ""} onClick={() => toggleDay(i)} aria-pressed={form.closed_days.includes(i)}>
                {d}
              </button>
            ))}
          </div>
        </Field>
        <Field label="위치" hint="고객의 '내 주변 업체'·거리순 검색에 씁니다">
          <div className="gy-inline">
            <input inputMode="decimal" placeholder="위도" value={form.latitude} onChange={(e) => setForm({ ...form, latitude: e.target.value })} />
            <input inputMode="decimal" placeholder="경도" value={form.longitude} onChange={(e) => setForm({ ...form, longitude: e.target.value })} />
            <button
              type="button"
              className="gy-button ghost"
              onClick={() =>
                navigator.geolocation?.getCurrentPosition(
                  (p) => setForm({ ...form, latitude: p.coords.latitude.toFixed(6), longitude: p.coords.longitude.toFixed(6) }),
                  () => toast("위치를 가져오지 못했습니다."),
                )
              }
            >
              현재 위치
            </button>
          </div>
        </Field>
        <Field label="대표 사진">
          <ImageInput label="대표 사진" value={form.cover_image_url} onChange={(url) => setForm({ ...form, cover_image_url: url })} onError={toast} />
        </Field>
        <button className="gy-button">저장</button>
      </form>

      <div className="stack">
        <section className="panel">
          <h2>업체 사진</h2>
          <p className="gy-muted small">작업장·장비·완성 사진을 올리면 고객 상세 화면에 보입니다 (최대 12장).</p>
          <div className="photo-grid">
            {(photos.data || []).map((p) => (
              <div key={p.id} className="photo" style={{ backgroundImage: `url(${p.url})` }}>
                {form.cover_image_url === p.url && <span>대표</span>}
                <button type="button" onClick={() => setForm({ ...form, cover_image_url: p.url })}>
                  대표로
                </button>
                <button type="button" onClick={() => removePhoto(p)}>
                  삭제
                </button>
              </div>
            ))}
          </div>
          <ImageInput label="사진 추가" value="" onChange={addPhoto} onError={toast} />
        </section>

        <form className="panel gy-form" onSubmit={saveCore}>
          <h2>기본 정보 {me.status === "approved" ? "변경 요청" : "고치기"}</h2>
          <p className="gy-muted small">
            {me.status === "approved" ? "상호·분야·지역·주소·사업자 정보는 운영자가 확인한 뒤 반영됩니다." : "고치면 바로 반영되고 다시 심사를 받습니다."}
          </p>
          {pending && (
            <p className="gy-note warn">
              심사 중인 변경 요청이 있습니다 ({dateText(pending.created_at)}). 결과가 나온 뒤 다시 요청할 수 있습니다.
            </p>
          )}
          <Field label="상호" required>
            <input required value={core.name} onChange={(e) => setCore({ ...core, name: e.target.value })} />
          </Field>
          <Field label="서비스 분야" required hint="첫 번째가 대표 분야">
            <div className="category-picker">
              {groups.map((g) => (
                <div key={g}>
                  <small>{g}</small>
                  {categories
                    .filter((c) => c.group_name === g)
                    .map((c) => (
                      <button key={c.id} type="button" className={core.categories.includes(c.name) ? "on" : ""} onClick={() => toggleCat(c.name)}>
                        {c.name}
                      </button>
                    ))}
                </div>
              ))}
            </div>
          </Field>
          <div className="gy-row">
            <Field label="지역 (구·군)" required>
              <input required value={core.district} onChange={(e) => setCore({ ...core, district: e.target.value })} />
            </Field>
            <Field label="주소" required>
              <AddressInput
                required
                value={core.address}
                onChange={(v) => setCore((c) => ({ ...c, address: v }))}
                onPick={(r) => r.sigungu && setCore((c) => ({ ...c, district: r.sigungu }))}
              />
            </Field>
          </div>
          <div className="gy-row">
            <Field label="사업자등록번호">
              <input value={core.business_number} onChange={(e) => setCore({ ...core, business_number: e.target.value })} />
            </Field>
            <Field label="대표자">
              <input value={core.representative} onChange={(e) => setCore({ ...core, representative: e.target.value })} />
            </Field>
          </div>
          <button className="gy-button" disabled={Boolean(pending)}>
            {me.status === "approved" ? "변경 요청 보내기" : "고치고 다시 심사 받기"}
          </button>
          {(changes.data || []).length > 0 && (
            <ul className="change-history">
              {changes.data.map((c) => (
                <li key={c.id}>
                  <Badge value={c.status === "approved" ? "approved" : c.status === "rejected" ? "rejected" : "pending"}>
                    {c.status === "approved" ? "승인" : c.status === "rejected" ? "반려" : "심사 중"}
                  </Badge>{" "}
                  {dateText(c.created_at)} · {c.changes?.name}
                  {c.reason && <small> — {c.reason}</small>}
                </li>
              ))}
            </ul>
          )}
          {!changes.data?.length && <Empty>변경 요청 기록이 없습니다.</Empty>}
        </form>
      </div>
    </div>
  );
}
