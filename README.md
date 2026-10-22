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

저장소에는 초기에 프로젝트 골격과 핵심 데이터 모델/API, 고객·업체·운영자 화면의 시작점을 구성합니다. 이메일 인증, 실제 지도 SDK, 결제 게이트웨이, 알림, 프로덕션 배포 설정은 각 제공자 자격 정보와 별도 구현이 필요한 후속 작업입니다.
