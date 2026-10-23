// API 요청 도우미. 모든 화면은 같은 주소의 /api 를 부릅니다 (Rpanel·Vite 가 API 서버로 프록시).
// 로그인은 HttpOnly 쿠키라 화면 코드는 토큰을 직접 다루지 않습니다.

const API_BASE = (typeof import.meta !== "undefined" && import.meta.env?.VITE_API_URL) || "/api";

export class ApiError extends Error {
  constructor(message, status, code) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

/**
 * api("/workshops") · api("/quotes", { method: "POST", body: {...} })
 * body 가 객체면 JSON 으로 보냅니다. 오류는 서버 안내 문구를 담은 ApiError 로 던집니다.
 */
export async function api(path, options = {}) {
  const { body, headers, ...rest } = options;
  const isForm = typeof FormData !== "undefined" && body instanceof FormData;
  let response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      credentials: "include",
      ...rest,
      headers: { ...(body !== undefined && !isForm ? { "Content-Type": "application/json" } : {}), ...(headers || {}) },
      body: body === undefined ? undefined : isForm || typeof body === "string" ? body : JSON.stringify(body),
    });
  } catch {
    throw new ApiError("서버에 연결하지 못했습니다. 인터넷 연결을 확인해 주세요.", 0, "network");
  }
  if (response.status === 204) return null;
  const data = await response.json().catch(() => ({}));
  if (!response.ok) {
    const message = typeof data.error === "string" ? data.error : data.error?.message;
    throw new ApiError(message || "요청을 처리하지 못했습니다.", response.status, data.code);
  }
  return data;
}

/** 이미지 올리기 → { id, url, width, height } */
export async function uploadImage(file) {
  const form = new FormData();
  form.append("file", file);
  return api("/files", { method: "POST", body: form });
}

/** 쿼리 문자열 만들기 (빈 값은 뺌) */
export function qs(params) {
  const p = new URLSearchParams();
  Object.entries(params || {}).forEach(([k, v]) => {
    if (v !== undefined && v !== null && v !== "") p.set(k, v);
  });
  const s = p.toString();
  return s ? `?${s}` : "";
}
