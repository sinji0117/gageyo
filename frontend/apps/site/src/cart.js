import { useCallback, useEffect, useMemo, useState } from "react";

const KEY = "gageyo_cart_v1";

function read() {
  try {
    const raw = window.localStorage.getItem(KEY);
    const items = raw ? JSON.parse(raw) : [];
    return Array.isArray(items) ? items.filter((i) => i && i.id && i.quantity > 0) : [];
  } catch {
    return [];
  }
}

/** 장바구니. 이 브라우저에만 남습니다 (가격·재고는 주문할 때 서버가 다시 확인). */
export function useCart() {
  const [items, setItems] = useState(read);
  useEffect(() => {
    try {
      window.localStorage.setItem(KEY, JSON.stringify(items));
    } catch {
      /* 저장소를 쓸 수 없는 브라우저 (사생활 보호 모드 등) */
    }
  }, [items]);

  const add = useCallback((product, quantity = 1) => {
    setItems((list) => {
      const found = list.find((i) => i.id === product.id);
      const max = Math.min(99, product.stock ?? 99);
      if (found) return list.map((i) => (i.id === product.id ? { ...i, quantity: Math.min(max, i.quantity + quantity) } : i));
      return [
        ...list,
        {
          id: product.id,
          name: product.name,
          price: product.price,
          image_url: product.image_url,
          workshop_id: product.workshop_id,
          workshop_name: product.workshop_name,
          stock: product.stock,
          quantity: Math.min(max, quantity),
        },
      ];
    });
  }, []);
  const setQuantity = useCallback((id, quantity) => {
    setItems((list) => list.map((i) => (i.id === id ? { ...i, quantity: Math.max(1, Math.min(99, quantity)) } : i)));
  }, []);
  const remove = useCallback((id) => setItems((list) => list.filter((i) => i.id !== id)), []);
  const clear = useCallback(() => setItems([]), []);
  const count = useMemo(() => items.reduce((sum, i) => sum + i.quantity, 0), [items]);
  const total = useMemo(() => items.reduce((sum, i) => sum + i.price * i.quantity, 0), [items]);
  const workshops = useMemo(() => [...new Set(items.map((i) => i.workshop_id))], [items]);
  return { items, add, setQuantity, remove, clear, count, total, workshops };
}
