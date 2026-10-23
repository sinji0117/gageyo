import { api } from "@gageyo/ui";

// 운영자 API 요청. 운영자 계정 로그인은 쿠키로 처리되고,
// 비상용 ADMIN_API_KEY 로 들어온 경우에만 x-admin-key 헤더를 붙입니다 (이 탭에만 보관).
const KEY = "gageyo_admin_key";

export function adminKey() {
  try {
    return window.sessionStorage.getItem(KEY) || "";
  } catch {
    return "";
  }
}

export function setAdminKey(value) {
  try {
    if (value) window.sessionStorage.setItem(KEY, value);
    else window.sessionStorage.removeItem(KEY);
  } catch {
    /* 저장소 없음 */
  }
}

export function aapi(path, options = {}) {
  const key = adminKey();
  return api(path, key ? { ...options, headers: { ...(options.headers || {}), "x-admin-key": key } } : options);
}
