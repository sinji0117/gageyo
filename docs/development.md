[← 문서 목차](README.md)

# 로컬 개발

필요한 것: Rust stable, Node.js 22 이상, npm, Docker(MariaDB 용). Redis 는 필요 없습니다.

## 1. DB

```bash
docker compose -f docker-compose.dev.yml up -d mariadb
```

`gageyo_db` / `gageyo_user` / `devpass` 로 `127.0.0.1:3306` 에 뜹니다.

## 2. API

```bash
cp .env.example .env          # DATABASE_URL·ADMIN_EMAILS·COOKIE_SECURE=false 등 확인
cargo run --manifest-path backend/Cargo.toml -p gageyo-api
```

- 루트의 `.env` 를 읽고, 처음 켤 때 `backend/migrations/` 를 DB 에 적용합니다.
- `http://127.0.0.1:8081/api/health` 가 `{"status":"ok"}` 이면 준비 끝입니다.
- 메일 설정(`SMTP_URL`)이 없으면 가입 코드·비밀번호 설정 링크가 API 로그에 찍힙니다.
- 카드 결제를 써 보려면 토스 테스트 키(`test_ck_`·`test_sk_`)를 `.env` 에 넣습니다. 실제 돈은 나가지 않습니다.

## 3. 화면

```bash
cd frontend && npm install
npm run dev:site      # http://localhost:5173  고객
npm run dev:console   # http://localhost:5174  파트너 센터
npm run dev:admin     # http://localhost:5175  운영자
npm run dev:www       # http://localhost:5176  소개
```

네 개발 서버 모두 `/api` 를 `127.0.0.1:8081` 로 넘깁니다 (예전에는 파트너·소개 화면 설정에 프록시가 없어 API 를 부르지 못했음).
로그인 쿠키는 화면마다 이름이 달라서(`gy_c`·`gy_p`·`gy_a`) 같은 `localhost` 에서 세 화면에 동시에 로그인해도 섞이지 않습니다.

## 4. 처음 쓰는 순서

1. **운영자**: `.env` 의 `ADMIN_EMAILS` 에 내 이메일을 넣고 API 를 켭니다. `localhost:5175` → **비밀번호 설정 메일 받기** →
   API 로그의 `reset=…` 링크를 열어(주소의 호스트를 `localhost:5175` 로 바꿔서) 비밀번호를 정하고 로그인합니다.
   - 또는 `.env` 의 `ADMIN_API_KEY` 로 **API 키로 접속**.
   - 로컬에서 링크가 `localhost` 로 오게 하려면 `.env` 에 `APP_URL=http://localhost:5173`, `PARTNER_URL=http://localhost:5174`, `ADMIN_URL=http://localhost:5175` 를 넣으세요.
2. **업체**: `localhost:5174` → 입점 신청 → 운영자 화면 업체 관리에서 승인 → 파트너 센터에서 서비스·상품 등록.
3. **고객**: `localhost:5173` → 업체 찾기·견적 요청·예약·주문.

## 빌드

```bash
cd backend && cargo build --release -p gageyo-api     # target/release/gageyo-api
cd frontend && npm run build                          # apps/*/dist
```

## 검사

```bash
cd backend && cargo test                                # 단위 테스트
DATABASE_URL=mysql://gageyo_user:devpass@127.0.0.1:3306/gageyo_db bash scripts/e2e.sh   # 종단 검사
```

자세한 내용은 [테스트](testing.md)를 보세요.
