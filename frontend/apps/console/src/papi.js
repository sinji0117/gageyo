import { api } from "@gageyo/ui";

// 파트너 API 요청. 계정 로그인은 쿠키로 처리되고,
// 예전 방식(운영자가 발급한 파트너 키)으로 들어온 경우에만 x-partner-key 헤더를 붙입니다.
const KEY = "gageyo_partner_key";

export function legacyKey() {
  try {
    return window.sessionStorage.getItem(KEY) || "";
  } catch {
    return "";
  }
}

export function setLegacyKey(value) {
  try {
    if (value) window.sessionStorage.setItem(KEY, value);
    else window.sessionStorage.removeItem(KEY);
  } catch {
    /* 저장소 없음 */
  }
}

export function papi(path, options = {}) {
  const key = legacyKey();
  return api(path, key ? { ...options, headers: { ...(options.headers || {}), "x-partner-key": key } } : options);
}
