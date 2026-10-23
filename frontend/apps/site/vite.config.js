import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// 개발 서버는 /api 를 로컬 API(8081)로 넘깁니다. 배포에서는 Rpanel 이 같은 주소의 /api/ 를 API 앱으로 프록시합니다.
export default defineConfig({
  plugins: [react()],
  server: { port: 5173, proxy: { "/api": "http://127.0.0.1:8081" } },
  preview: { port: 4173, proxy: { "/api": "http://127.0.0.1:8081" } },
});
