import { useState } from "react";
import { Field, Empty, Loading, ErrorBox, useLoad, dateText } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

const AUDIENCE = { all: "전체", customer: "고객", partner: "파트너" };
const empty = { audience: "partner", title: "", body: "", pinned: false, is_published: true };

export default function Notices() {
  const { toast } = useAdmin();
  const { data, loading, error, reload } = useLoad(() => aapi("/admin/notices"), []);
  const [form, setForm] = useState(empty);
  const [editing, setEditing] = useState(null);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.type === "checkbox" ? e.target.checked : e.target.value });

  async function save(e) {
    e.preventDefault();
    try {
      await aapi(editing ? `/admin/notices/${editing}` : "/admin/notices", { method: editing ? "PUT" : "POST", body: form });
      toast("공지를 저장했습니다.");
      setForm(empty);
      setEditing(null);
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  async function remove(n) {
    if (!window.confirm(`'${n.title}' 공지를 지울까요?`)) return;
    await aapi(`/admin/notices/${n.id}`, { method: "DELETE" }).catch((err) => toast(err.message));
    reload();
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>공지</h1>
          <p>고객 공지는 고객 화면 아래쪽에, 파트너 공지는 파트너 센터 대시보드·공지 화면에 보입니다.</p>
        </div>
      </div>
      <div className="category-admin-grid">
        <form className="box gy-form" onSubmit={save}>
          <h2>{editing ? "공지 고치기" : "새 공지"}</h2>
          <Field label="대상">
            <select value={form.audience} onChange={set("audience")}>
              {Object.entries(AUDIENCE).map(([k, v]) => (
                <option key={k} value={k}>
                  {v}
                </option>
              ))}
            </select>
          </Field>
          <Field label="제목">
            <input required maxLength={200} value={form.title} onChange={set("title")} />
          </Field>
          <Field label="내용">
            <textarea required rows={8} maxLength={10000} value={form.body} onChange={set("body")} />
          </Field>
          <label className="gy-check">
            <input type="checkbox" checked={form.pinned} onChange={set("pinned")} /> 맨 위 고정
          </label>
          <label className="gy-check">
            <input type="checkbox" checked={form.is_published} onChange={set("is_published")} /> 공개
          </label>
          <div className="gy-actions">
            {editing && (
              <button type="button" className="gy-button ghost" onClick={() => { setEditing(null); setForm(empty); }}>
                취소
              </button>
            )}
            <button className="gy-button">{editing ? "저장" : "올리기"}</button>
          </div>
        </form>
        <section className="box">
          <ErrorBox error={error} onRetry={reload} />
          {loading && !data && <Loading />}
          {data?.length === 0 && <Empty>공지가 없습니다.</Empty>}
          <ul className="notice-admin">
            {data?.map((n) => (
              <li key={n.id} className={n.is_published ? "" : "inactive"}>
                <div>
                  <b>
                    {n.pinned ? "📌 " : ""}
                    {n.title}
                  </b>
                  <small>
                    {AUDIENCE[n.audience]} · {dateText(n.created_at)} {n.is_published ? "" : "· 숨김"}
                  </small>
                </div>
                <div className="gy-actions">
                  <button
                    type="button"
                    className="gy-button ghost small"
                    onClick={() => {
                      setEditing(n.id);
                      setForm({ audience: n.audience, title: n.title, body: n.body, pinned: n.pinned, is_published: n.is_published });
                    }}
                  >
                    수정
                  </button>
                  <button type="button" className="gy-button danger small" onClick={() => remove(n)}>
                    삭제
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </>
  );
}
