# 가게요 (gageyo)

자동차 정비·관리 업체와 운전자를 연결하는 지역 기반 플랫폼입니다.
[bandiuri](https://github.com/sinji0117/bandiuri) 의 모듈형 Rust API · React 워크스페이스 구조를 바탕으로 자동차 도메인에 맞게 만들었습니다.

| 주소 | 화면 | 하는 일 |
| --- | --- | --- |
| `gageyo.com` | 소개 | 서비스 소개, 파트너 입점 안내, 자주 묻는 질문 |
| `app.gageyo.com` | 고객 | 업체 찾기(내 주변), 견적 요청·비교·선택, 방문 예약, 용품 주문(카드·무통장), 리뷰, 마이페이지 |
| `partner.gageyo.com` | 파트너 센터 | 입점 신청, 업체 정보·사진, 견적 답변, 예약 처리, 상품·발송, 리뷰 답글, 정산 |
| `admin.gageyo.com` | 운영자 | 입점 심사, 업체·회원, 주문·입금·환불, 정산 지급, 문의·신고, 공지, 설정 (OTP) |

## 구조

```text
gageyo/
├── backend/                 Rust 워크스페이스 (Axum 0.8 · sqlx · MariaDB)
│   ├── crates/core/         설정 · 오류 · 인증/세션 · 메일 · OTP · 암호화 · 이미지 · 토스 결제 · 요청 제한
│   ├── crates/api/          실행 파일 · 계정 · 운영자 · 업로드 · 사이트맵
│   ├── modules/workshop/    업체 · 견적 · 예약 · 리뷰 · 파트너 · 입점 심사
│   ├── modules/commerce/    상품 · 주문 · 결제/환불 · 배송 · 정산
│   ├── modules/support/     문의 · 신고 · 공지
│   └── migrations/          MariaDB 스키마 (API 시작 때 자동 적용)
├── frontend/                React 19 + Vite npm 워크스페이스
│   ├── apps/{site,console,admin,www}/
│   └── packages/ui/         API 클라이언트 · 공용 컴포넌트 · 스타일
├── scripts/                 종단 검사(e2e.sh) · 화면 점검(ui-smoke.sh)
├── deploy/                  Rpanel 환경 변수 예시
└── docs/                    문서
```

Redis 는 쓰지 않습니다 — 세션·인증 코드·요청 제한까지 MariaDB 하나로 동작합니다.

## 빠르게 띄우기

필요: Rust stable, Node.js 22+, Docker.

```bash
docker compose -f docker-compose.dev.yml up -d mariadb
cp .env.example .env                                    # ADMIN_EMAILS 에 내 이메일
cargo run --manifest-path backend/Cargo.toml -p gageyo-api
cd frontend && npm install && npm run dev:site          # dev:console · dev:admin · dev:www
```

자세히: [로컬 개발](docs/development.md) · 배포: [Rpanel 배포 따라 하기](docs/rpanel-setup.md) · 전체 문서: [docs/](docs/README.md)

## 검사

```bash
cd backend && cargo test
DATABASE_URL=mysql://gageyo_user:devpass@127.0.0.1:3306/gageyo_db bash scripts/e2e.sh
```

[테스트](docs/testing.md) · [변경 기록](CHANGELOG.md)
