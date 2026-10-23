[← 문서 목차](README.md)

# Rpanel 에 gageyo 배포하기

GitHub 저장소 `sinji0117/gageyo` 를 Rpanel **프로젝트** 하나로 연결해 API 앱 하나와 정적 화면 앱 네 개를 배포하는 순서입니다.

> 처음 올린다면 [첫 배포 체크리스트](first-deploy-checklist.md)를 옆에 두고 따라 하세요.

## 1. 준비

- Rpanel 이 설치된 Ubuntu 서버와 패널 관리자 계정 (Rust 릴리스 빌드에 메모리 4GB 이상 권장)
- 저장소에 접근할 수 있는 GitHub 계정, 도메인과 DNS 관리 권한
- (권장) 메일 발송 계정(SMTP), 토스페이먼츠 테스트 키

## 2. DNS

DNS 관리 화면에서 아래 호스트를 서버 공인 IP 로 `A` 레코드를 넣습니다.

| 호스트 | 도메인 | 화면 |
| --- | --- | --- |
| `@` | `gageyo.com` | 소개 (`frontend/apps/www`) |
| `app` | `app.gageyo.com` | 고객 (`frontend/apps/site`) |
| `partner` | `partner.gageyo.com` | 파트너 센터 (`frontend/apps/console`) |
| `admin` | `admin.gageyo.com` | 운영자 (`frontend/apps/admin`) |

## 3. API 앱 만들기

Rpanel → **프로젝트** → **새 프로젝트**(`gageyo`) → **새 앱** → **백엔드 (API 서버)** · **Rust**

| 설정 | 값 |
| --- | --- |
| 이름 | `gageyo-api` |
| Git 저장소 | `git@github.com:sinji0117/gageyo.git` (비공개면 SSH + Deploy key, 5단계) |
| 브랜치 | `main` |
| 앱 폴더 (Root Directory) | `backend` |
| 바이너리 이름 | `gageyo-api` |
| 도메인 | 비워 둠 (화면 앱이 `/api/` 로 프록시) |
| 상태 확인 경로 | `/api/health` (DB 연결까지 확인) |

**앱 폴더는 꼭 `backend`** 로 둡니다. 그래야 `Cargo.toml`(워크스페이스)·`crates/`·`modules/`·`migrations/` 가 함께 보입니다.
`backend/crates/api` 만 지정하면 워크스페이스와 마이그레이션을 찾지 못합니다.
포트는 Rpanel 이 정해 `PORT` 로 넣어 주고, API 는 `127.0.0.1:$PORT` 에서 듣습니다.

## 4. MariaDB 와 환경 변수

1. `gageyo-api` 앱 → **데이터베이스** 탭 → **MariaDB 데이터베이스 추가**(`gageyo_db`) → 이 앱에 연결 → `DATABASE_URL` 이 들어왔는지 확인
2. **환경 변수** 탭에 [deploy/rpanel.env.example](../deploy/rpanel.env.example) 를 보고 넣습니다. 최소한:

| 이름 | 값 |
| --- | --- |
| `ADMIN_EMAILS` | 운영자 이메일 (쉼표 구분) |
| `DATA_KEY` | `openssl rand -hex 32` 로 만든 64자 — **한 번 정하면 바꾸지 않기** |
| `APP_URL` · `PARTNER_URL` · `ADMIN_URL` | `https://app.gageyo.com` 등 (메일 링크용) |
| `SMTP_URL` · `MAIL_FROM` · `OPS_EMAIL` | 메일 (권장. 없으면 메일 대신 로그) |
| `TOSS_CLIENT_KEY` · `TOSS_SECRET_KEY` | 토스페이먼츠 (테스트 키로 먼저) |

전체 목록과 설명: [환경 변수](configuration.md). 값을 바꾼 뒤에는 API 앱을 다시 시작해야 적용됩니다.

- API 가 켜질 때 `backend/migrations/` 를 DB 에 적용합니다. 따로 SQL 을 실행할 필요가 없습니다.
- **Redis 는 필요 없습니다.** 세션·가입 코드·요청 횟수 제한을 모두 MariaDB 로 처리합니다.
- 업로드 이미지는 Rpanel 앱의 `data/uploads` 폴더(배포해도 남고 앱 백업에 포함)를 자동으로 찾아 씁니다.
- Rpanel 의 DB 백업을 켠 뒤 운영을 시작하세요.

## 5. 비공개 저장소 접근

앱의 **Git · 웹훅** 탭 → Deploy key 생성 → GitHub `gageyo` 저장소 **Settings → Deploy keys → Add deploy key** 에 붙여 넣기
(**Allow write access 는 선택하지 않음**) → 저장소 주소가 SSH(`git@github.com:…`)인지 확인.

## 6. 화면 앱 네 개

각각 **새 앱 → 프런트엔드 (웹 화면) → Static**, 같은 저장소·브랜치 `main`·프로젝트 `gageyo`·API 연결 `gageyo-api`.

| 화면 | 앱 폴더 | 빌드 명령 | 출력 폴더 | 도메인 |
| --- | --- | --- | --- | --- |
| 고객 | `frontend` | `npm run build -w @gageyo/site` | `apps/site/dist` | `app.gageyo.com` |
| 파트너 센터 | `frontend` | `npm run build -w @gageyo/console` | `apps/console/dist` | `partner.gageyo.com` |
| 운영자 | `frontend` | `npm run build -w @gageyo/admin` | `apps/admin/dist` | `admin.gageyo.com` |
| 소개 | `frontend` | `npm run build -w @gageyo/www` | `apps/www/dist` | `gageyo.com` |

**앱 폴더는 네 개 모두 `frontend`** 입니다 (npm 워크스페이스라 `packages/ui` 가 함께 필요). 출력 폴더는 `frontend` 기준 상대 경로입니다.
Rpanel 은 `frontend/package.json` 기준으로 `npm install` 합니다.

## 7. `/api` 연결과 https

1. 화면 앱마다 **API 연결**을 `gageyo-api`, 경로 `/api/` 로 둡니다. 화면은 같은 주소의 `/api` 를 부르므로 `VITE_API_URL` 은 필요 없습니다.
2. 각 앱의 **SSL** 탭에서 인증서를 발급하고 https 를 켭니다. 로그인 쿠키에 `Secure` 가 붙으므로 **https 가 아니면 로그인이 되지 않습니다**.
3. `https://app.gageyo.com/api/health` 가 `{"status":"ok"}` 인지 확인합니다.

## 8. 첫 운영자 설정

1. `https://admin.gageyo.com` → **비밀번호 설정 메일 받기** → `ADMIN_EMAILS` 에 넣은 이메일
2. 메일의 링크(30분)로 비밀번호를 정합니다. 메일 설정이 없으면 API 앱 **로그**에 `reset=…` 링크가 찍힙니다.
3. 로그인 → **내 보안**에서 OTP(2단계 인증)를 켭니다.
4. **결제·운영 설정**에서 수수료율, 무통장 입금 계좌(선택), 고객센터·사업자 정보를 넣습니다.

## 9. 배포와 자동 배포

첫 배포는 프로젝트 화면의 **전체 배포**(API 먼저, 이어서 화면)를 씁니다. push 때 자동 배포하려면 앱마다 **Git · 웹훅** 의
URL·Secret 을 GitHub **Settings → Webhooks → Add webhook** 에 넣습니다 (`application/json`, push 이벤트만).

## 이전 버전에서 올릴 때

- 새 마이그레이션(`20261024000001_platform.sql`)이 상태 열을 `ENUM` → `VARCHAR`, 좌표를 `DECIMAL` → `DOUBLE` 로 바꾸고 표를 추가합니다. 기존 데이터는 그대로입니다.
- 예전 `ADMIN_API_KEY` 와 **파트너 키**는 계속 쓸 수 있습니다. 운영자 화면 업체 상세의 **비밀번호 설정 링크**로 업체를 계정 로그인으로 옮기세요.
- 예전 고객 로그인 쿠키(`gageyo_session`)는 쓰지 않으므로 고객은 한 번 다시 로그인합니다.
- `REDIS_URL`·`MONGODB_URI` 는 쓰지 않습니다. 넣어 두어도 무시합니다.

## 자주 보이는 증상

| 증상 | 확인할 것 |
| --- | --- |
| `could not find Cargo.toml` · `failed to resolve workspace member` | API 앱 폴더가 `backend` 인지 |
| 시작 로그에 `설정 오류: 환경변수 DATABASE_URL …` | MariaDB 를 API 앱에 연결했는지 |
| `설정 오류: DATA_KEY 는 16진수 64자…` · `TOSS_… 키의 테스트/실제 종류가 다릅니다` | 환경 변수 값 형식 |
| 마이그레이션 오류로 시작 안 됨 | 시작 로그의 SQL 오류. 이미 적용된 마이그레이션 파일을 고치면 체크섬 오류가 납니다 (고치지 말고 새 파일로) |
| `/api/health` 가 503 `db_unavailable` | DB 가 꺼졌거나 연결 정보가 바뀜 |
| 화면에서 API 502 | 화면 앱의 API 연결이 `gageyo-api`·`/api/` 인지, API 앱이 켜져 있는지 |
| 로그인해도 바로 풀림 | https 인지 (`COOKIE_SECURE` 기본 true). 로컬 http 개발에서만 `COOKIE_SECURE=false` |
| 운영자 로그인 401 | 이메일이 `ADMIN_EMAILS` 에 있는지, 바꾼 뒤 API 를 다시 시작했는지 |
| 메일이 안 옴 | `SMTP_URL`·`MAIL_FROM` 둘 다 있는지, 시작 로그의 `메일:` 줄, 스팸함 |
| 카드 결제 버튼이 없음 | `TOSS_CLIENT_KEY`·`TOSS_SECRET_KEY` 둘 다, 운영자 화면 결제·운영 설정의 상태 카드 |
| 업로드 이미지가 배포 뒤 사라짐 | `UPLOAD_DIR` 이 릴리스 폴더 안이 아닌지 (기본은 앱 `data/uploads`) |
| 화면이 404·빈 페이지 | 화면 앱 폴더가 `frontend`, 빌드 명령의 워크스페이스 이름·출력 폴더가 표와 같은지 |

## 참고

- [Rpanel Git 자동 배포](https://github.com/sinji0117/Rpanel/blob/main/docs/git-deploy.md) · [Rpanel 데이터베이스](https://github.com/sinji0117/Rpanel/blob/main/docs/database.md)
- [환경 변수](configuration.md) · [운영 안내](operations.md) · [구조](architecture.md)
