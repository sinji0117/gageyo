[← 문서 목차](README.md)

# 배포 구성

- **Rpanel**: [Rpanel 배포 따라 하기](rpanel-setup.md) — API 앱 1개 + 정적 화면 앱 4개, MariaDB 하나.
- **다른 Linux 서버**
  1. MariaDB 10.6 이상(utf8mb4)을 준비합니다.
  2. `cd backend && cargo build --release -p gageyo-api` → `target/release/gageyo-api` 를 서비스로 띄웁니다
     (환경 변수는 [환경 변수](configuration.md), 업로드 폴더 `UPLOAD_DIR` 은 배포해도 지워지지 않는 곳).
  3. `cd frontend && npm install && npm run build` → `apps/{site,console,admin,www}/dist` 를 각 도메인의 정적 파일로 올립니다.
  4. 각 도메인에서 `/api/` 를 API 로 프록시하고(`X-Real-IP` 헤더 전달), https 를 켭니다.

```nginx
location /api/ {
    proxy_pass http://127.0.0.1:8081;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    client_max_body_size 12m;   # 이미지 10MB + 여유
}
location / { try_files $uri /index.html; }
```

## 운영 전에 확인할 것

- [ ] https 와 `COOKIE_SECURE` 기본값(true)
- [ ] `DATA_KEY` 설정 (바꾸지 않기), `ADMIN_EMAILS`, 운영자 OTP
- [ ] 메일(`SMTP_URL`·`MAIL_FROM`) — 가입 인증·비밀번호 찾기·알림
- [ ] 토스 테스트 키로 결제·취소 확인 → 가맹 계약 후 live 키 ([결제 안내](payments-toss.md))
- [ ] 무통장 계좌·수수료율·고객센터·사업자 정보 (운영자 화면 결제·운영 설정)
- [ ] DB 백업·복원 연습, 업로드 폴더 백업
- [ ] 개인정보 처리방침·이용약관 문서 게시 (화면의 동의 문구와 맞추기)
- [ ] `CORS_ORIGINS` 는 다른 도메인에서 API 를 부를 때만 설정 (기본은 CORS 헤더 없음 = 같은 주소만)

자세한 순서: [첫 배포 체크리스트](first-deploy-checklist.md)
