import { useEffect, useState } from "react";
import { api, Modal, Field, Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateText, dateTimeText, qs, hoursText, timeText, DAY_NAMES } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

const STATUS_TABS = [
  ["pending", "승인 대기"],
  ["approved", "승인"],
  ["rejected", "반려"],
  ["suspended", "중지"],
  ["", "전체"],
];

export default function Workshops() {
  const { tabParams, toast, refreshCounts } = useAdmin();
  const [status, setStatus] = useState(tabParams.status ?? "pending");
  const [q, setQ] = useState("");
  const [term, setTerm] = useState("");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/workshops${qs({ status, q: term, size: 200 })}`), [status, term]);
  const [detail, setDetail] = useState(null);
  const [creating, setCreating] = useState(false);

  return (
    <>
      <div className="page-title">
        <div>
          <h1>업체 관리</h1>
          <p>입점 신청을 심사하고, 업체 정보를 고치고, 이용을 중지합니다. 승인된 업체만 고객 화면에 보입니다.</p>
        </div>
        <button type="button" onClick={() => setCreating(true)}>
          ＋ 업체 직접 등록
        </button>
      </div>
      <Tabs tabs={STATUS_TABS} value={status} onChange={setStatus} />
      <form
        className="search-row"
        onSubmit={(e) => {
          e.preventDefault();
          setTerm(q);
        }}
      >
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="상호·지역·이메일·사업자번호" />
        <button className="gy-button ghost small">검색</button>
      </form>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>업체가 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>업체</th>
                <th>지역·분야</th>
                <th>담당자</th>
                <th>사업자번호</th>
                <th>평점</th>
                <th>상태</th>
                <th>신청일</th>
              </tr>
            </thead>
            <tbody>
              {data.map((w) => (
                <tr key={w.id} className="clickable" onClick={() => setDetail(w.id)}>
                  <td>
                    <b>{w.name}</b>
                    {w.pending_changes > 0 && <small className="warn-text">변경 요청 {w.pending_changes}건</small>}
                  </td>
                  <td>
                    {w.district}
                    <small>{w.category}</small>
                  </td>
                  <td>{w.owner_email || "-"}</td>
                  <td>{w.business_number || "-"}</td>
                  <td>
                    ★ {Number(w.rating).toFixed(1)} <small>({w.review_count})</small>
                  </td>
                  <td>
                    <Badge kind="workshop" value={w.status} />
                  </td>
                  <td>{dateText(w.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {detail && (
        <WorkshopDetail
          id={detail}
          onClose={() => setDetail(null)}
          onChanged={() => {
            reload();
            refreshCounts();
          }}
        />
      )}
      {creating && (
        <WorkshopForm
          onClose={() => setCreating(false)}
          onSaved={(r) => {
            setCreating(false);
            reload();
            if (r.owner_link) window.prompt("메일 설정이 없어 주인 계정의 비밀번호 설정 링크를 직접 전달해 주세요 (30분 유효)", r.owner_link);
            toast("업체를 등록했습니다.");
          }}
        />
      )}
    </>
  );
}

function WorkshopDetail({ id, onClose, onChanged }) {
  const { toast } = useAdmin();
  const { data: w, loading, error, reload } = useLoad(() => aapi(`/admin/workshops/${id}`), [id]);
  const [editing, setEditing] = useState(false);

  async function setStatus(status) {
    let reason = null;
    if (["rejected", "suspended"].includes(status)) {
      reason = window.prompt(status === "rejected" ? "반려 사유 (업체에 메일로 안내됩니다)" : "중지 사유 (업체에 메일로 안내됩니다)", "");
      if (!reason) return;
    } else if (!window.confirm(status === "approved" ? `${w.name}을(를) 승인할까요? 고객 화면에 바로 보입니다.` : "상태를 바꿀까요?")) return;
    try {
      await aapi(`/admin/workshops/${id}`, { method: "PATCH", body: { status, reason } });
      toast("상태를 바꿨습니다.");
      reload();
      onChanged();
    } catch (e) {
      toast(e.message);
    }
  }
  async function ownerLink() {
    if (!window.confirm(`${w.owner?.email} 로 파트너 센터 비밀번호 설정 링크를 보낼까요?`)) return;
    try {
      const r = await aapi(`/admin/workshops/${id}/owner-link`, { method: "POST" });
      if (r.link) window.prompt("메일 설정이 없어 링크를 직접 전달해 주세요 (30분 유효)", r.link);
      else toast("설정 링크를 메일로 보냈습니다.");
    } catch (e) {
      toast(e.message);
    }
  }
  async function partnerKey() {
    if (!window.confirm("예전 방식 파트너 키를 발급할까요? 기존 키는 바로 무효가 됩니다. 가능하면 계정 비밀번호 설정 링크를 쓰세요.")) return;
    try {
      const r = await aapi(`/admin/workshops/${id}/partner-key`, { method: "POST" });
      window.prompt("파트너 키 (다시 표시되지 않습니다)", r.partner_key);
      reload();
    } catch (e) {
      toast(e.message);
    }
  }

  return (
    <Modal title={w?.name || "업체"} eyebrow="업체 상세" onClose={onClose} wide>
      {loading && !w && <Loading />}
      <ErrorBox error={error} onRetry={reload} />
      {w && !editing && (
        <div className="detail">
          <div className="detail-actions">
            <Badge kind="workshop" value={w.status} />
            {w.status !== "approved" && (
              <button type="button" className="gy-button small" onClick={() => setStatus("approved")}>
                승인
              </button>
            )}
            {w.status === "pending" && (
              <button type="button" className="gy-button ghost small" onClick={() => setStatus("rejected")}>
                반려
              </button>
            )}
            {w.status === "approved" && (
              <button type="button" className="gy-button danger small" onClick={() => setStatus("suspended")}>
                이용 중지
              </button>
            )}
            <button type="button" className="gy-button ghost small" onClick={() => setEditing(true)}>
              정보 고치기
            </button>
            <button type="button" className="gy-button ghost small" onClick={ownerLink}>
              비밀번호 설정 링크
            </button>
            <button type="button" className="gy-button ghost small" onClick={partnerKey}>
              파트너 키{w.has_partner_key ? " 재발급" : ""}
            </button>
            <a className="gy-button ghost small" href={`https://app.gageyo.com/?shop=${w.id}`} target="_blank" rel="noreferrer">
              고객 화면 ↗
            </a>
          </div>
          {w.reject_reason && <p className="gy-note warn">사유: {w.reject_reason}</p>}
          <dl className="kv">
            <dt>분야</dt>
            <dd>{(w.categories || []).join(", ")}</dd>
            <dt>주소</dt>
            <dd>
              {w.district} · {w.address}
              {w.latitude != null && (
                <small>
                  {" "}
                  ({w.latitude}, {w.longitude})
                </small>
              )}
            </dd>
            <dt>전화</dt>
            <dd>{w.phone}</dd>
            <dt>사업자</dt>
            <dd>
              {w.business_number || "-"} · 대표 {w.representative || "-"}{" "}
              {w.business_number && (
                <a href="https://www.bizno.net/" target="_blank" rel="noreferrer" className="small-link">
                  사업자 조회 ↗
                </a>
              )}
            </dd>
            <dt>영업</dt>
            <dd>{hoursText(w)}</dd>
            <dt>담당자</dt>
            <dd>
              {w.owner ? (
                <>
                  {w.owner.display_name} · {w.owner.email} · {w.owner.phone || "-"} <Badge kind="user" value={w.owner.status} />
                  <small> 최근 로그인 {w.owner.last_login_at ? dateTimeText(w.owner.last_login_at) : "없음"}</small>
                </>
              ) : (
                "-"
              )}
            </dd>
            <dt>숫자</dt>
            <dd>
              서비스 {w.counts.services} · 상품 {w.counts.products} · 예약 {w.counts.bookings}(완료 {w.counts.completed_bookings}) · 견적 {w.counts.offers} · ★{" "}
              {Number(w.rating).toFixed(1)} ({w.review_count})
            </dd>
            <dt>신청·승인</dt>
            <dd>
              {dateText(w.created_at)} 신청{w.approved_at ? ` · ${dateText(w.approved_at)} 승인` : ""}
            </dd>
          </dl>
          <h3>소개</h3>
          <p className="pre">{w.description}</p>
          {w.photos.length > 0 && (
            <div className="photo-strip">
              {w.photos.map((p) => (
                <a key={p.id} href={p.url} target="_blank" rel="noreferrer" style={{ backgroundImage: `url(${p.url})` }} aria-label="사진" />
              ))}
            </div>
          )}
          {w.change_requests.length > 0 && (
            <>
              <h3>정보 변경 요청</h3>
              <ul className="plain">
                {w.change_requests.map((c) => (
                  <li key={c.id}>
                    {dateText(c.created_at)} · {c.changes?.name} · {(c.changes?.categories || []).join(", ")} · {c.status}
                    {c.reason && ` (${c.reason})`}
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      )}
      {w && editing && (
        <WorkshopForm
          initial={w}
          inline
          onClose={() => setEditing(false)}
          onSaved={() => {
            setEditing(false);
            reload();
            onChanged();
            toast("업체 정보를 고쳤습니다.");
          }}
        />
      )}
    </Modal>
  );
}

/** 업체 등록(운영자) · 정보 고치기 */
function WorkshopForm({ initial = null, inline = false, onClose, onSaved }) {
  const { toast } = useAdmin();
  const [categories, setCategories] = useState([]);
  const [form, setForm] = useState(() => ({
    owner_email: "",
    owner_name: "",
    owner_phone: "",
    name: initial?.name || "",
    categories: initial?.categories || [],
    district: initial?.district || "",
    address: initial?.address || "",
    business_number: initial?.business_number || "",
    representative: initial?.representative || "",
    phone: initial?.phone || "",
    description: initial?.description || "",
    latitude: initial?.latitude ?? "",
    longitude: initial?.longitude ?? "",
    open_time: timeText(initial?.open_time),
    close_time: timeText(initial?.close_time),
    closed_days: (initial?.closed_days || "").split(",").filter(Boolean).map(Number),
    approve: true,
  }));
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api("/categories").then(setCategories).catch(() => {});
  }, []);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.type === "checkbox" ? e.target.checked : e.target.value });
  const toggleCat = (n) => setForm({ ...form, categories: form.categories.includes(n) ? form.categories.filter((c) => c !== n) : [...form.categories, n] });
  const toggleDay = (d) => setForm({ ...form, closed_days: form.closed_days.includes(d) ? form.closed_days.filter((x) => x !== d) : [...form.closed_days, d] });

  async function submit(e) {
    e.preventDefault();
    setBusy(true);
    const body = {
      ...form,
      closed_days: form.closed_days.join(","),
      latitude: form.latitude === "" ? null : Number(form.latitude),
      longitude: form.longitude === "" ? null : Number(form.longitude),
    };
    try {
      const r = initial ? await aapi(`/admin/workshops/${initial.id}`, { method: "PUT", body }) : await aapi("/admin/workshops", { method: "POST", body });
      onSaved(r);
    } catch (err) {
      toast(err.message);
    } finally {
      setBusy(false);
    }
  }
  const content = (
    <form className="gy-form" onSubmit={submit}>
      {!initial && (
        <>
          <p className="gy-muted">전화·방문으로 받은 업체를 직접 등록합니다. 주인 이메일로 파트너 계정이 없으면 만들고, 비밀번호 설정 링크를 보냅니다.</p>
          <div className="gy-row">
            <Field label="주인 이메일" required>
              <input type="email" required value={form.owner_email} onChange={set("owner_email")} />
            </Field>
            <Field label="담당자 이름" required>
              <input required value={form.owner_name} onChange={set("owner_name")} />
            </Field>
          </div>
          <Field label="담당자 연락처">
            <input type="tel" value={form.owner_phone} onChange={set("owner_phone")} />
          </Field>
        </>
      )}
      <div className="gy-row">
        <Field label="상호" required>
          <input required value={form.name} onChange={set("name")} />
        </Field>
        <Field label="업체 전화" required>
          <input required type="tel" value={form.phone} onChange={set("phone")} />
        </Field>
      </div>
      <Field label="분야" required>
        <div className="cat-picker">
          {categories.map((c) => (
            <button key={c.id} type="button" className={form.categories.includes(c.name) ? "on" : ""} onClick={() => toggleCat(c.name)}>
              {c.name}
            </button>
          ))}
        </div>
      </Field>
      <div className="gy-row">
        <Field label="지역 (구·군)" required>
          <input required value={form.district} onChange={set("district")} />
        </Field>
        <Field label="주소" required>
          <input required value={form.address} onChange={set("address")} />
        </Field>
      </div>
      <div className="gy-row">
        <Field label="사업자등록번호">
          <input value={form.business_number} onChange={set("business_number")} />
        </Field>
        <Field label="대표자">
          <input value={form.representative} onChange={set("representative")} />
        </Field>
      </div>
      <Field label="소개" required>
        <textarea required rows={4} value={form.description} onChange={set("description")} />
      </Field>
      <div className="gy-row">
        <Field label="여는 시간">
          <input type="time" value={form.open_time} onChange={set("open_time")} />
        </Field>
        <Field label="닫는 시간">
          <input type="time" value={form.close_time} onChange={set("close_time")} />
        </Field>
      </div>
      <Field label="휴무">
        <div className="cat-picker">
          {DAY_NAMES.map((d, i) => (
            <button key={d} type="button" className={form.closed_days.includes(i) ? "on" : ""} onClick={() => toggleDay(i)}>
              {d}
            </button>
          ))}
        </div>
      </Field>
      <div className="gy-row">
        <Field label="위도">
          <input inputMode="decimal" value={form.latitude} onChange={set("latitude")} />
        </Field>
        <Field label="경도">
          <input inputMode="decimal" value={form.longitude} onChange={set("longitude")} />
        </Field>
      </div>
      {!initial && (
        <label className="gy-check">
          <input type="checkbox" checked={form.approve} onChange={set("approve")} />
          바로 승인 (고객 화면에 표시)
        </label>
      )}
      <div className="gy-actions">
        <button type="button" className="gy-button ghost" onClick={onClose}>
          취소
        </button>
        <button className="gy-button" disabled={busy}>
          {initial ? "저장" : "등록"}
        </button>
      </div>
    </form>
  );
  if (inline) return content;
  return (
    <Modal title="업체 직접 등록" onClose={onClose} wide>
      {content}
    </Modal>
  );
}
