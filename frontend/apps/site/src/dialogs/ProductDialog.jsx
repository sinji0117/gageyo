import { useState } from "react";
import { api, money, Modal, Loading, ErrorBox, useLoad } from "@gageyo/ui";
import { useApp } from "../context.js";
import { Cover } from "../components/WorkshopCard.jsx";

export default function ProductDialog({ id }) {
  const { close, cart, toast, open } = useApp();
  const { data: p, loading, error, reload } = useLoad(() => api(`/products/${id}`), [id]);
  const [qty, setQty] = useState(1);
  const soldOut = p && (p.status === "sold_out" || p.stock === 0);
  return (
    <Modal title={p?.name || "상품"} eyebrow={p?.category} onClose={close} wide>
      {loading && !p && <Loading />}
      <ErrorBox error={error} onRetry={reload} />
      {p && (
        <div className="product-detail">
          <Cover url={p.image_url} name={p.name} className="product-hero" />
          <div>
            <p className="price-line">
              {p.list_price > p.price && (
                <>
                  <s>{money(p.list_price)}</s> <em>{Math.round((1 - p.price / p.list_price) * 100)}%</em>{" "}
                </>
              )}
              <b>{money(p.price)}</b>
            </p>
            <p className="gy-muted">
              판매:{" "}
              <button type="button" className="link-button" onClick={() => open({ type: "workshop", id: p.workshop_id })}>
                {p.workshop_name}
              </button>{" "}
              · {p.workshop_district}
            </p>
            <p className="description">{p.description || "상품 설명이 없습니다."}</p>
            <p className="gy-muted small">{soldOut ? "품절" : `재고 ${p.stock}개`} · 택배 또는 판매 업체 방문 장착</p>
            <div className="cta-row">
              <div className="qty">
                <button type="button" onClick={() => setQty(Math.max(1, qty - 1))} aria-label="하나 빼기">
                  −
                </button>
                <span>{qty}</span>
                <button type="button" onClick={() => setQty(Math.min(p.stock || 1, 99, qty + 1))} aria-label="하나 더">
                  ＋
                </button>
              </div>
              <button
                type="button"
                className="gy-button"
                disabled={soldOut}
                onClick={() => {
                  cart.add(p, qty);
                  toast("장바구니에 담았어요.");
                }}
              >
                장바구니 담기
              </button>
              <button
                type="button"
                className="gy-button dark"
                disabled={soldOut}
                onClick={() => {
                  cart.add(p, qty);
                  open({ type: "cart" });
                }}
              >
                바로 주문
              </button>
            </div>
            <button type="button" className="link-button small" onClick={() => open({ type: "report", targetType: "product", targetId: p.id, back: { type: "product", id } })}>
              상품 신고
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}
