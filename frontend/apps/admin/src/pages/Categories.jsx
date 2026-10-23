import { useState } from "react";
import { Field, Empty, Loading, ErrorBox, useLoad } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

const emptyForm = { group_name: "차량 관리", name: "", sort_order: 10, is_active: true };

/** 서비스 카테고리. 이름을 바꾸면 그 이름을 쓰던 업체 분야도 함께 바뀝니다. 업체가 쓰는 카테고리는 지우지 말고 숨기세요. */
export default function Categories() {
  const { toast } = useAdmin();
  const { data, loading, error, reload } = useLoad(() => aapi("/admin/categories"), []);
  const [form, setForm] = useState(emptyForm);
  const [editing, setEditing] = useState(null);
  const groups = [...new Set([...(data || []).map((c) => c.group_name), "차량 관리", "자동차 매매"])];

  async function save(e) {
    e.preventDefault();
    try {
      await aapi(editing ? `/admin/categories/${editing}` : "/admin/categories", { method: editing ? "PUT" : "POST", body: { ...form, sort_order: Number(form.sort_order) } });
      setForm(emptyForm);
      setEditing(null);
      toast("카테고리를 저장했습니다.");
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  async function remove(c) {
    if (!window.confirm(`'${c.name}' 카테고리를 지울까요?`)) return;
    try {
      await aapi(`/admin/categories/${c.id}`, { method: "DELETE" });
      toast("지웠습니다.");
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>카테고리</h1>
          <p>고객 화면의 서비스 분야이자 업체 분야·견적 매칭 기준입니다.</p>
        </div>
      </div>
      <div className="category-admin-grid">
        <form className="box gy-form" onSubmit={save}>
          <h2>{editing ? "카테고리 고치기" : "카테고리 추가"}</h2>
          <Field label="분류 그룹">
            <input list="category-groups" required value={form.group_name} onChange={(e) => setForm({ ...form, group_name: e.target.value })} />
            <datalist id="category-groups">
              {groups.map((g) => (
                <option key={g} value={g} />
              ))}
            </datalist>
          </Field>
          <Field label="이름">
            <input required maxLength={80} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="예: 자동차 썬팅" />
          </Field>
          <Field label="정렬 순서">
            <input type="number" min="0" step="10" value={form.sort_order} onChange={(e) => setForm({ ...form, sort_order: e.target.value })} />
          </Field>
          <label className="gy-check">
            <input type="checkbox" checked={form.is_active} onChange={(e) => setForm({ ...form, is_active: e.target.checked })} />
            고객 화면에 표시
          </label>
          <div className="gy-actions">
            {editing && (
              <button type="button" className="gy-button ghost" onClick={() => { setEditing(null); setForm(emptyForm); }}>
                취소
              </button>
            )}
            <button className="gy-button">{editing ? "저장" : "추가"}</button>
          </div>
        </form>
        <section className="box">
          <ErrorBox error={error} onRetry={reload} />
          {loading && !data && <Loading />}
          {data?.length === 0 && <Empty>카테고리가 없습니다.</Empty>}
          {[...new Set((data || []).map((c) => c.group_name))].map((g) => (
            <div key={g} className="admin-category-group">
              <h3>
                {g} <small>{data.filter((c) => c.group_name === g).length}개</small>
              </h3>
              {data
                .filter((c) => c.group_name === g)
                .map((c) => (
                  <div key={c.id} className={`admin-category-row ${c.is_active ? "" : "inactive"}`}>
                    <span className="sort-number">{c.sort_order}</span>
                    <b>{c.name}</b>
                    <small>업체 {c.workshop_count}곳</small>
                    <em>{c.is_active ? "노출" : "숨김"}</em>
                    <button
                      type="button"
                      onClick={() => {
                        setEditing(c.id);
                        setForm({ group_name: c.group_name, name: c.name, sort_order: c.sort_order, is_active: c.is_active });
                      }}
                    >
                      수정
                    </button>
                    <button type="button" className="remove" onClick={() => remove(c)} disabled={c.workshop_count > 0} title={c.workshop_count > 0 ? "업체가 쓰는 카테고리는 숨김으로 바꾸세요" : ""}>
                      삭제
                    </button>
                  </div>
                ))}
            </div>
          ))}
        </section>
      </div>
    </>
  );
}
