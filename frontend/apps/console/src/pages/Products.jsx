import { useState } from "react";
import { money, Field, Empty, Loading, ErrorBox, Badge, ImageInput, useLoad } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

const empty = { name: "", category: "", description: "", price: "", list_price: "", stock: "", image_url: "", status: "active" };

export default function Products() {
  const { toast, me } = usePartner();
  const { data, loading, error, reload } = useLoad(() => papi("/partner/products"), []);
  const [form, setForm] = useState(empty);
  const [editing, setEditing] = useState(null);
  const [current, setCurrent] = useState(null);
  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });

  async function save(e) {
    e.preventDefault();
    const body = {
      ...form,
      price: Number(form.price),
      list_price: form.list_price ? Number(form.list_price) : null,
      stock: Number(form.stock),
      image_url: form.image_url || null,
    };
    try {
      await papi(editing ? `/partner/products/${editing}` : "/partner/products", { method: editing ? "PATCH" : "POST", body });
      toast(editing ? "상품을 고쳤습니다." : "상품을 등록했습니다.");
      setForm(empty);
      setEditing(null);
      setCurrent(null);
      reload();
    } catch (err) {
      toast(err.message);
    }
  }
  function edit(p) {
    setEditing(p.id);
    setCurrent(p);
    setForm({
      name: p.name,
      category: p.category,
      description: p.description,
      price: String(p.price),
      list_price: p.list_price ? String(p.list_price) : "",
      stock: String(p.stock),
      image_url: p.image_url || "",
      status: ["draft", "archived"].includes(p.status) ? p.status : "active",
    });
    window.scrollTo({ top: 0, behavior: "smooth" });
  }
  async function remove(p) {
    if (!window.confirm(`'${p.name}'을(를) 지울까요? 주문 기록이 있는 상품은 숨김 처리됩니다.`)) return;
    try {
      const r = await papi(`/partner/products/${p.id}`, { method: "DELETE" });
      toast(r.deleted ? "상품을 지웠습니다." : "주문 기록이 있어 숨김 처리했습니다.");
      reload();
    } catch (err) {
      toast(err.message);
    }
  }

  return (
    <div className="grid-form">
      <form className="panel gy-form" onSubmit={save}>
        <h2>{editing ? "상품 고치기" : "상품 등록"}</h2>
        {me.status !== "approved" && <p className="gy-note warn">승인 전에는 고객에게 보이지 않습니다. 미리 등록해 두세요.</p>}
        {current?.status === "blocked" && <p className="gy-note warn">운영자가 판매를 중지한 상품입니다. 내용은 고칠 수 있지만 다시 판매하려면 고객센터에 문의해 주세요.</p>}
        <ImageInput label="대표 이미지" value={form.image_url} onChange={(url) => setForm({ ...form, image_url: url })} onError={toast} />
        <Field label="상품명" required>
          <input required maxLength={180} value={form.name} onChange={set("name")} />
        </Field>
        <Field label="분류" required hint="예: 엔진오일, 타이어, 블랙박스">
          <input required maxLength={80} value={form.category} onChange={set("category")} list="product-categories" />
          <datalist id="product-categories">
            {[...new Set((data || []).map((p) => p.category))].map((c) => (
              <option key={c} value={c} />
            ))}
          </datalist>
        </Field>
        <Field label="설명">
          <textarea maxLength={5000} rows={5} value={form.description} onChange={set("description")} placeholder="규격, 호환 차종, 장착 가능 여부 등" />
        </Field>
        <div className="gy-row">
          <Field label="판매가 (원)" required>
            <input type="number" min="1" required value={form.price} onChange={set("price")} />
          </Field>
          <Field label="정가 (원)" hint="할인 표시용, 선택">
            <input type="number" min="0" value={form.list_price} onChange={set("list_price")} />
          </Field>
        </div>
        <div className="gy-row">
          <Field label="재고" required hint="0이면 품절로 보입니다">
            <input type="number" min="0" required value={form.stock} onChange={set("stock")} />
          </Field>
          <Field label="상태">
            <select value={form.status} onChange={set("status")} disabled={current?.status === "blocked"}>
              <option value="active">판매</option>
              <option value="draft">준비 중 (안 보임)</option>
              <option value="archived">숨김</option>
            </select>
          </Field>
        </div>
        <div className="gy-actions">
          {editing && (
            <button type="button" className="gy-button ghost" onClick={() => { setEditing(null); setCurrent(null); setForm(empty); }}>
              취소
            </button>
          )}
          <button className="gy-button">{editing ? "저장" : "등록"}</button>
        </div>
      </form>
      <section className="panel">
        <h2>
          등록한 상품 <small className="gy-muted">{data?.length || 0}개</small>
        </h2>
        <ErrorBox error={error} onRetry={reload} />
        {loading && !data && <Loading />}
        {data?.length === 0 && <Empty>등록한 상품이 없습니다.</Empty>}
        <ul className="row-list products">
          {data?.map((p) => (
            <li key={p.id}>
              <div className="thumb" style={p.image_url ? { backgroundImage: `url(${p.image_url})` } : undefined} />
              <div>
                <b>{p.name}</b>
                <small>
                  {p.category} · 재고 {p.stock} · 판매 {p.sold}
                </small>
              </div>
              <span>
                {money(p.price)} <Badge kind="product" value={p.status} />
              </span>
              <div className="gy-actions">
                <button type="button" className="gy-button ghost small" onClick={() => edit(p)}>
                  고치기
                </button>
                <button type="button" className="gy-button danger small" onClick={() => remove(p)}>
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
