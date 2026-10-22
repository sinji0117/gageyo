# Rpanel에 gageyo 배포하기

이 문서는 GitHub 저장소 `sinji0117/gageyo`를 Rpanel의 **프로젝트**로 연결해 API와 웹 화면을 배포하는 순서입니다. gageyo는 한 저장소 안에 Rust API와 네 개의 React/Vite 앱이 있습니다. 따라서 Rpanel 프로젝트 하나에 백엔드 앱 하나와 정적 프런트엔드 앱 네 개를 등록합니다.

> 현재 단계는 배포 가능한 프로젝트 골격입니다. 고객·업체 로그인과 권한 검증, 결제 연동, 운영자 실데이터 화면은 아직 완성되지 않았습니다. 실제 고객 개인정보나 결제 데이터를 받기 전에 [운영 전 확인 사항](#운영-전에-필요한-구현)을 완료하세요.

## 1. 준비하기

- Rpanel이 설치된 Ubuntu 서버와 패널 관리자 계정
- `gageyo` 저장소에 접근할 수 있는 GitHub 계정
- 서비스에 연결할 도메인과 DNS 관리 권한
- 충분한 디스크 공간과 MariaDB 백업 위치

Rpanel은 Git 배포에서 저장소를 서버에서 빌드합니다. Rust 릴리스 빌드는 메모리를 사용하므로 서버 여유 메모리를 확인하세요.

## 2. DNS를 서버로 연결하기

DNS 관리 화면에서 서비스 도메인마다 서버의 공인 IP를 가리키는 `A` 레코드를 추가합니다. 예를 들면 아래처럼 나눌 수 있습니다.

| 화면 | 예시 도메인 | 용도 |
|---|---|---|
| 마케팅 소개 | `gageyo.com` | `frontend/apps/www` |
| 운전자 서비스 | `app.gageyo.com` | 업체 탐색·견적·예약·상품 |
| 업체 센터 | `partner.gageyo.com` | 업체 견적·예약·상품 관리 |
| 플랫폼 운영 | `admin.gageyo.com` | 운영자 화면 |

기본 도메인은 `gageyo.com`입니다. DNS 관리 화면에서 `@`, `app`, `partner`, `admin` 호스트에 서버 공인 IP를 가리키는 A 레코드를 각각 추가하세요. DNS가 전파된 뒤 Rpanel 각 앱의 SSL 탭에서 HTTPS 인증서를 발급합니다.

## 3. Rpanel 프로젝트와 API 앱 만들기

1. Rpanel 왼쪽의 **프로젝트**에서 **새 프로젝트**를 만듭니다. 예: `gageyo`.
2. 프로젝트 안에서 **새 앱**을 만들고 **백엔드 (API 서버)** 역할과 **Rust**를 선택합니다.
3. Git 저장소 주소와 빌드 항목을 다음처럼 설정합니다.

| 설정 | 값 |
|---|---|
| 이름 | `gageyo-api` |
| Git 저장소 | `git@github.com:sinji0117/gageyo.git` |
| 브랜치 | `main` |
| 앱 폴더 (Root Directory) | `backend` |
| 바이너리 이름 | `gageyo-api` |
| 도메인 | 비워 둠 (프런트엔드에서 API 프록시로 연결) |
| 상태 확인 경로 | `/api/health` |

Cargo workspace가 `backend/Cargo.toml`에 있고 실행 바이너리 이름은 `backend/crates/api/Cargo.toml`의 `gageyo-api`입니다. Rpanel이 Rust 바이너리를 빌드하고 실행합니다. 앱 포트는 Rpanel이 지정하므로 `PORT`를 별도로 고정하지 마세요. API는 그 포트의 `127.0.0.1`에서 수신합니다.

## 4. MariaDB 연결하기

1. `gageyo-api` 앱의 **데이터베이스** 탭을 엽니다.
2. **MariaDB 데이터베이스 추가**를 선택하고 데이터베이스를 만듭니다. 예: `gageyo_db`.
3. 생성한 DB를 이 앱에 연결합니다.
4. Rpanel이 앱에 `DATABASE_URL`을 주입하는지 앱 설정에서 확인합니다.

카테고리 관리 API를 사용하려면 같은 API 앱의 **환경 변수**에 `ADMIN_API_KEY`를 추가하세요. 값을 저장한 뒤 API 앱을 재시작 또는 재배포합니다. 키는 운영자가 직접 생성한 충분히 긴 임의 문자열로 설정하고 Git 저장소나 문서에 기록하지 마세요. 운영자 화면(`admin.gageyo.com`)의 **카테고리** 메뉴에서 동일한 키를 입력해 연결하면 카테고리 추가·수정·활성 상태 변경·삭제를 할 수 있습니다. 브라우저에는 현재 세션 동안만 보관됩니다. 키가 비어 있거나 API가 재시작되지 않으면 관리 API는 사용할 수 없습니다.

API가 시작될 때 `backend/migrations/`의 SQL migration을 적용합니다. 별도 SQL 명령을 직접 실행할 필요는 없습니다. Rpanel의 DB 백업을 설정한 뒤 운영 배포를 진행하세요.

**Redis는 현재 gageyo API에서 사용하지 않습니다.** Rpanel에 Redis를 만들거나 연결할 필요가 없습니다. 이후 세션·캐시 기능을 구현할 때 별도로 붙입니다.

### 애플리케이션 로그용 MongoDB

로그 저장소는 MongoDB를 사용할 계획입니다. MongoDB 인스턴스를 준비한 뒤 API 앱 환경 변수에 연결 문자열 `MONGODB_URI`를 등록할 수 있도록 구성하세요. 단, 현재 Rust API의 `tracing` 로그는 표준 출력으로만 기록되며 MongoDB 저장 계층은 아직 연결되지 않았습니다. MongoDB에 실제 로그를 저장하려면 후속으로 API의 MongoDB 연결과 tracing 저장 계층 구현이 필요합니다. 기존 Rpanel 배포 로그도 함께 확인할 수 있도록 유지하세요.

## 5. GitHub 비공개 저장소 접근 설정

`gageyo`가 비공개 저장소라면 HTTPS URL 대신 SSH 주소를 사용합니다.

1. 앱의 **Git · 웹훅** 탭에서 Deploy key를 생성합니다.
2. 표시된 공개 키를 복사합니다.
3. GitHub의 `gageyo` 저장소에서 **Settings → Deploy keys → Add deploy key**를 엽니다.
4. 공개 키를 붙여 넣고 **Allow write access는 선택하지 않은 상태**로 저장합니다. Rpanel은 코드를 읽고 배포하므로 저장소 쓰기 권한이 필요하지 않습니다.
5. Rpanel의 **Git · 웹훅** 탭에서 저장소 URL이 SSH 주소인지 확인합니다.

## 6. 프런트엔드 앱 네 개 등록하기

각 정적 앱은 같은 Git 저장소를 사용합니다. **새 앱 → 프런트엔드 (웹 화면) → Static**으로 만들고, 아래 표의 폴더·빌드 명령·출력 폴더·도메인을 각각 넣습니다.

| 화면 | 화면 용도 | 앱 폴더 (Root Directory) | 빌드 명령 | 출력 폴더 | 도메인 예시 |
|---|---|---|---|---|---|
| 고객 서비스 | 사용자 화면 | `frontend` | `npm run build -w @gageyo/site` | `apps/site/dist` | `app.gageyo.com` |
| 업체 센터 | 관리자 화면 | `frontend` | `npm run build -w @gageyo/console` | `apps/console/dist` | `partner.gageyo.com` |
| 운영자 화면 | 관리자 화면 | `frontend` | `npm run build -w @gageyo/admin` | `apps/admin/dist` | `admin.gageyo.com` |
| 마케팅 소개 | 사용자 화면 | `frontend` | `npm run build -w @gageyo/www` | `apps/www/dist` | `gageyo.com` |

모든 정적 앱에서 다음 값도 동일하게 설정합니다.

- Git 저장소: `git@github.com:sinji0117/gageyo.git`
- 브랜치: `main`
- Rpanel 프로젝트: 2단계에서 만든 `gageyo`
- API 연결: 3단계의 `gageyo-api`

Rpanel은 `frontend/package.json`을 기준으로 의존성을 설치합니다. 현재 `package-lock.json`은 저장소에 없으므로 Rpanel이 `npm install`을 사용합니다. 패키지 버전을 고정하는 lock 파일을 추가하면 다음 배포부터 재현 가능한 설치에 `npm ci`를 쓸 수 있습니다.

각 빌드 명령은 지정한 React workspace만 빌드하고, 출력 폴더는 `frontend` 기준 상대 경로입니다. 앱 폴더를 `frontend/apps/site`처럼 개별 앱 폴더로 지정하면 표의 명령과 출력 폴더가 맞지 않으므로 **앱 폴더는 네 앱 모두 `frontend`로 설정**하세요.

## 7. `/api` 연결과 HTTPS 설정

1. Rpanel 프로젝트 화면에서 각 프런트엔드 앱의 **API 연결**을 `gageyo-api`로 설정합니다.
2. 연결 경로는 `/api/`로 둡니다. 프런트엔드는 기본 API 주소 `/api`를 사용하므로 브라우저 요청이 같은 도메인에서 API 앱으로 전달됩니다.
3. 각 앱의 **SSL** 탭에서 도메인 인증서를 발급하고 HTTPS를 켭니다.
4. 고객 화면에서 업체 목록과 `/api/health` 응답을 확인합니다.

프런트엔드 앱은 API와 같은 origin으로 요청하므로 별도의 `VITE_API_URL` 환경 변수가 필요하지 않습니다. 로컬 Vite 개발 서버는 `frontend/apps/site/vite.config.js` 및 `frontend/apps/admin/vite.config.js`가 `/api`를 `127.0.0.1:8081`로 프록시합니다. 고객 화면의 카테고리는 API에서 활성 카테고리를 불러오며, 초기 항목은 DB migration에서 등록됩니다.

## 8. 배포와 자동 배포 웹훅

첫 배포는 프로젝트 화면의 **전체 배포**를 사용하세요. 백엔드가 먼저 배포되고 연결된 프런트엔드가 이어서 빌드됩니다. 각 앱별 **배포 로그**에서 Cargo/npm 설치 및 빌드 오류를 확인할 수 있습니다.

Git push 때 자동 배포하려면 앱별 **Git · 웹훅** 탭의 URL과 Secret을 사용해 GitHub 저장소의 **Settings → Webhooks → Add webhook**에 등록합니다.

- Payload URL: Rpanel 앱에 표시된 Webhook URL
- Content type: `application/json`
- Secret: Rpanel 앱에 표시된 Secret
- 이벤트: `Just the push event`

웹훅을 받은 서버가 외부에서 접근 가능해야 합니다. 저장소 브랜치 `main`에 push한 뒤 앱의 배포 로그에서 새 커밋이 반영됐는지 확인합니다. 웹훅을 아직 등록하지 않았다면 프로젝트 화면에서 수동으로 전체 배포할 수 있습니다.

## 9. 운영 전에 필요한 구현

현재 저장소는 DB migration과 초기 도메인 API 및 화면 뼈대를 갖춘 개발 단계입니다. 실제 공개 서비스로 운영하기 전 아래 기능을 구현하고 확인해야 합니다.

- 회원가입·로그인·이메일 확인과 API 인증
- 업체 계정 소유권 검사, 운영자 권한과 업체 승인 흐름
- 고객 전화번호 등 개인정보 접근 통제, 보관·삭제 정책
- 예약 중복 방지와 업체 영업 시간·휴무 관리
- 실제 지도 SDK와 위치·거리 검색
- 결제 승인·취소·환불과 웹훅 서명 검증
- 주문 취소 시 재고 복구, 업체별 주문·배송 처리
- 실데이터를 사용하는 업체·운영자 관리 화면
- 제한된 CORS 원본, DB 백업 복원 점검, 오류 알림

현재 API CORS는 개발 편의를 위해 모든 origin을 허용합니다. 같은 도메인 `/api` 프록시 배포만으로 CORS 정책이 제한되는 것은 아니므로, 운영 전 허용 도메인을 지정하도록 서버 설정을 개선해야 합니다.

## 참고 문서

- [Rpanel Git 자동 배포](https://github.com/sinji0117/Rpanel/blob/main/docs/git-deploy.md)
- [Rpanel 데이터베이스 연결](https://github.com/sinji0117/Rpanel/blob/main/docs/database.md)
- [gageyo API 설정 예시](../deploy/rpanel.env.example)
- [gageyo 프로젝트 구조](architecture.md)
