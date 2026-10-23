import { createContext, useContext } from "react";

/**
 * 고객 화면 전체가 함께 쓰는 값
 * - user: 로그인한 고객 (없으면 null)
 * - config: /api/config (결제 수단·가입 코드 인증·고객센터)
 * - categories: 서비스 카테고리
 * - cart: 장바구니 (cart.js)
 * - toast(message)
 * - open(dialog) / close(): 대화 상자 열기 ({ type: "workshop", id } 등)
 * - go(view): 화면 바꾸기 ("home" · "products" · "my")
 */
export const AppContext = createContext(null);

export function useApp() {
  return useContext(AppContext);
}
