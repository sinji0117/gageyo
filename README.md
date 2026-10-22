# 가게요 (gageyo)

자동차 정비·관리 업체와 운전자를 연결하는 지역 기반 서비스 플랫폼입니다. bandiuri의 모듈형 Rust API, React 워크스페이스, MariaDB·Redis 구성 방식을 바탕으로 자동차 도메인에 맞게 설계합니다.

   제공할 서비스

- **운전자**: 주변 정비소 탐색, 요청서 한 번으로 여러 업체 견적 받기, 업체 예약, 자동차 상품 구매와 주문 확인
- **업체**: 업체 프로필과 서비스·상품 관리, 견적 제출, 예약 일정 관리, 주문 처리
- **운영자**: 업체 승인, 회원·상품·예약·주문 현황과 신고 관리

   구조

```text
gageyo/
├── backend/                 Rust workspace (Axum API)
│   ├── crates/core/         설정, 공통 오류와 응답
│   ├── crates/api/          API 실행 파일과 라우터
│   ├── modules/workshop/    업체, 서비스, 견적, 예약 도메인
│   ├── modules/commerce/    상품, 장바구니 주문 도메인
│   └── migrations/          MariaDB 스키마
├── frontend/                React 19 + Vite npm workspaces
│   ├── apps/site/           고객용 탐색·견적·예약·스토어
│   ├── apps/console/        업체 관리 화면
│   ├── apps/admin/          플랫폼 운영 화면
│   ├── apps/www/            마케팅 사이트
│   └── packages/ui/         API 클라이언트와 공용 UI
├── deploy/                  배포 환경 예시
├── docs/                    개발·구조·API 안내
└── docker-compose.dev.yml   MariaDB와 Redis
```

   로컬 개발

필수: Rust stable, Node.js 22+, npm, Docker.

```bash
docker compose -f docker-compose.dev.yml up -d
cd frontend && npm install
npm run dev:site
```

API 개발 서버는 `backend`에서 `DATABASE_URL`과 `REDIS_URL`을 설정한 뒤 `cargo run -p gageyo-api`로 실행합니다. 환경 변수 예시는 `.env.example`을 참고하세요.

Rpanel Git 자동 배포 설정은 [Rpanel 배포 안내](docs/rpanel-setup.md)를 참고하세요.

   진행 상태

서비스 주소는 `gageyo.com`(서비스 소개), `app.gageyo.com`(파트너 탐색·견적·예약·상품), `partner.gageyo.com`(파트너 관리), `admin.gageyo.com`(운영자 관리)입니다. 상세 구현 현황과 남은 기능은 [기능 현황](docs/features.md)을 확인하세요.

로그인·본인 확인, 지도 SDK, PG 결제·환불·정산, SMS/이메일 알림, 이미지 저장소는 아직 연결되지 않았습니다. 주문 접수는 결제가 완료된 상태가 아닙니다.
