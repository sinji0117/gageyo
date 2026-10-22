# 로컬 개발

필요 도구: Rust stable, Node.js 22 이상, npm, Docker Compose. MongoDB 로그 저장을 개발 환경에서 확인하는 단계는 로그 저장 계층을 연결한 뒤 추가합니다.

1. 저장소 루트에서 `docker compose -f docker-compose.dev.yml up -d`로 MariaDB와 Redis를 시작합니다.
2. `.env.example`을 `.env`로 복사하고 DB URL을 확인합니다.
3. `cd frontend && npm install` 후 `npm run dev:site`를 실행합니다. 고객 화면은 `http://localhost:5173`에서 확인합니다.
4. 저장소 루트에서 `cargo run --manifest-path backend/Cargo.toml -p gageyo-api`를 실행합니다. 루트의 `.env`를 읽고, 처음 실행할 때 DB migration을 적용합니다. API는 `http://localhost:8081`에서 열립니다.
5. 파트너 화면은 `npm run dev:console`, 운영 화면은 `npm run dev:admin`, 소개 페이지는 `npm run dev:www`로 시작합니다.

API가 꺼져 있어도 고객 화면은 화면 구성 확인용 샘플 카드로 렌더링됩니다. 견적·예약 등록은 API에 연결될 때 데이터베이스에 저장됩니다.
