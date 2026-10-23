import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// 예전 설정에는 /api 프록시가 없어 로컬 개발에서 파트너 화면이 API 를 부르지 못했습니다.
export default defineConfig({
  plugins: [react()],
  server: { port: 5174, proxy: { "/api": "http://127.0.0.1:8081" } },
  preview: { port: 4174, proxy: { "/api": "http://127.0.0.1:8081" } },
});
