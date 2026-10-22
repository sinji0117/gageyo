const API_BASE = import.meta.env?.VITE_API_URL || "http://localhost:8081/api";
export async function api(path, options = {}) {
  const response = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: { "Content-Type": "application/json", ...(options.headers || {}) },
  });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(data.error || "요청을 처리하지 못했습니다.");
  return data;
}
export const money = (value) => new Intl.NumberFormat("ko-KR").format(value) + "원";
