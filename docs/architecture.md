[← 문서 목차](README.md)

# 구조

## 한눈에

```text
브라우저
 ├─ gageyo.com          www      소개·파트너 입점 안내          (React, 정적)
 ├─ app.gageyo.com      site     고객: 찾기·견적·예약·용품·마이페이지 (React, 정적)
 ├─ partner.gageyo.com  console  파트너 센터                    (React, 정적)
 └─ admin.gageyo.com    admin    운영자 화면                    (React, 정적)
        │  같은 주소의 /api/ 를 Rpanel(Nginx)이 API 앱으로 프록시
        ▼
gageyo-api  (Rust · Axum, 127.0.0.1:$PORT)
 ├─ crates/core       설정 · 오류 · 인증/세션 · 메일 · 가입 코드 · OTP · 비밀값 암호화 · 이미지 · 토스 결제 · 요청 제한 · 작업 기록
 ├─ modules/workshop  카테고리 · 업체 검색/상세 · 견적 · 예약 · 리뷰 · 관심 업체 · 파트너 계정/업체 정보 · 운영자 업체 관리
 ├─ modules/commerce  상품 · 주문 · 결제 승인/환불 · 배송 처리 · 정산 · 운영자 주문 관리
 ├─ modules/support   문의 · 신고 · 공지
 └─ crates/api        실행 파일 · 고객 계정 · 운영자 로그인/OTP · 회원 관리 · 대시보드 · 설정 · 이미지 업로드/내려주기 · 사이트맵
        │
        ├─ MariaDB   모든 업무 데이터 + 세션·인증 코드·요청 횟수 (Redis 쓰지 않음)
        ├─ 디스크     UPLOAD_DIR (업로드 이미지)
        ├─ SMTP      메일 알림 (없으면 로그만)
        └─ 토스페이먼츠 결제 승인·취소 (없으면 카드 결제 끔)
```

bandiuri 의 모듈형 Rust API · React 워크스페이스 구조를 따르되, 가게요는 **화면 네 개를 각각 정적 앱으로** Rpanel 에 올리고
API 를 따로 둡니다 (bandiuri 는 실행 파일 하나에 화면을 넣음). 그래서 화면과 API 를 따로 배포할 수 있습니다.

## 폴더

```text
backend/
├── Cargo.toml                 워크스페이스 (의존성 버전은 여기서만)
├── crates/core/src/
│   ├── config.rs              환경 변수 → Config
│   ├── error.rs               AppError → {"error","code"} JSON
│   ├── state.rs               AppState (DB 풀·설정·메일·비밀값·토스 클라이언트)
│   ├── auth.rs                비밀번호(argon2)·세션 쿠키·추출기(Customer·MaybeCustomer·Partner·ApprovedPartner·Admin)·로그인 잠금
│   ├── verify.rs              가입 이메일 코드
│   ├── ratelimit.rs           요청 횟수 제한 (DB 고정 창)
│   ├── totp.rs · secret.rs    운영자 OTP · AES-256-GCM 암호화와 HMAC
│   ├── files.rs               이미지 검사·다시 저장·디스크 기록
│   ├── toss.rs                토스페이먼츠 승인·조회·취소
│   ├── schedule.rs            영업시간·휴무일·방문 가능 시간 규칙
│   ├── settings.rs            운영자 설정 (수수료율·무통장 계좌·고객센터)
│   ├── audit.rs               작업 기록
│   └── util.rs                입력 검사(이메일·전화·사업자번호·좌표)·쪽수·LIKE 이스케이프·택배사
├── modules/workshop/src/      categories · workshops · quotes · bookings · reviews · partner · admin · notify
├── modules/commerce/src/      products · orders · payments · fulfillment · settlement · admin · notify
├── modules/support/src/       inquiries · notices
├── crates/api/src/            main · accounts · admin_auth · admin · files · public
└── migrations/                MariaDB 스키마 (새 파일로만 추가)
frontend/
├── packages/ui/               api()·uploadImage()·형식(money·날짜·상태 이름)·공용 컴포넌트(Modal·Field·Badge·Tabs·ImageInput·CodeButton·useLoad·useToast)·ui.css
└── apps/
    ├── site/src/              App(주소 처리·대화 상자) · pages(Home·Products·MyPage) · dialogs(업체·예약·견적·장바구니·주문·로그인·문의…) · cart.js · toss.js
    ├── console/src/           App(메뉴·승인 상태) · pages(대시보드·견적·예약·서비스·상품·주문·리뷰·정산·업체 정보·문의) · papi.js
    ├── admin/src/             App(메뉴·처리할 건수) · pages(대시보드·업체·변경 요청·회원·견적·예약·리뷰·상품·주문·정산·카테고리·문의·공지·설정·작업 기록·보안) · aapi.js
    └── www/src/               소개 페이지
scripts/
├── e2e.sh · mock_toss.py      종단 검사 (실제 MariaDB + 토스 모의 서버)
└── ui-smoke.sh · ui-smoke.mjs 화면 점검 (빌드한 화면 + 브라우저 스크린샷)
```

## 인증과 권한

- 세션은 `auth_sessions` 에 **토큰 해시**와 **범위(customer·partner·admin)** 로 저장합니다. 쿠키 이름도 범위마다 다릅니다(`gy_c`·`gy_p`·`gy_a`).
- 사용자 역할(`users.role`): `customer` · `workshop`(파트너) · `admin`(운영자 첫 설정으로 만든 계정).
  운영자 권한은 역할이 아니라 **`ADMIN_EMAILS` 에 이메일이 있는지**로 매 요청 확인합니다.
- 파트너는 계정 → `workshops.owner_user_id` 로 업체를 찾습니다. 예전 `workshop_access_tokens`(파트너 키)도 받습니다.
- 추출기
  - `Customer` / `MaybeCustomer`: 고객 세션 (비회원 허용 API 는 Maybe)
  - `Partner`: 파트너 세션 또는 파트너 키, 중지된 업체는 403
  - `ApprovedPartner`: 승인된 업체만 (견적·예약·주문·정산)
  - `Admin`: 운영자 세션 + `ADMIN_EMAILS`, 또는 `x-admin-key`
- 비회원 데이터(견적·예약·주문)는 **UUID + 연락처 숫자 일치**로 확인합니다. 문의는 48자 비밀 토큰(해시 저장).
- 남의 데이터를 요청하면 404 로 숨깁니다. 파트너 API 의 모든 쿼리는 `workshop_id = 로그인한 업체` 조건을 넣습니다.

## 상태 흐름

```text
업체        pending ──승인──▶ approved ──중지──▶ suspended
               └──반려──▶ rejected ──고쳐서 재신청──▶ pending

견적 요청    open ──고객 선택──▶ accepted        (14일 지나면 closed 로 보임)
               ├──고객 취소──▶ cancelled
               └──운영자 마감─▶ closed
견적 제안    sent ──▶ accepted / declined(다른 업체 선택) / withdrawn(업체 철회)

예약        requested ──확정──▶ confirmed ──완료──▶ completed ──▶ 리뷰 가능
               ├──거절(사유)──▶ declined      └──취소(사유)──▶ cancelled
               └──고객 취소(방문 전)──▶ cancelled

주문        payment_pending ──카드 승인·입금 확인──▶ paid ──▶ preparing ──▶ shipped ──▶ delivered
               └──고객 취소──▶ cancelled        └──운영자 취소──▶ refunded(카드 자동 환불·무통장 직접) / cancelled
주문 줄     pending ──결제──▶ ordered ──▶ preparing ──발송(택배사·운송장)──▶ shipped ──▶ delivered
                                            └─────────(방문 장착)──────────────────────▶ delivered
            주문 전체 상태 = 취소되지 않은 줄 상태로 계산 (모두 delivered → delivered, 모두 shipped 이상 → shipped …)

상품        draft · active ⇄ sold_out(재고 0 자동) · archived(파트너 숨김) · blocked(운영자 판매 중지, 파트너가 못 엶)
```

## 데이터

| 표 | 내용 |
| --- | --- |
| `users` | 계정 (role·status·마지막 로그인·OTP 비밀값(암호화)·탈퇴) |
| `auth_sessions` · `password_resets` · `email_codes` · `rate_limits` | 세션 · 비밀번호 설정 링크 · 가입 코드 · 요청 횟수 |
| `workshops` · `workshop_categories` · `workshop_photos` · `workshop_change_requests` | 업체 (영업시간·휴무·사업자·좌표·심사) · 분야 여러 개 · 사진 · 변경 요청 |
| `workshop_services` · `workshop_access_tokens` | 서비스 · 예전 파트너 키(해시) |
| `service_categories` | 서비스 카테고리 |
| `quote_requests` · `quote_offers` | 견적 요청 · 업체 견적 (업체당 한 번) |
| `bookings` | 예약 (견적·주문과 연결 가능) |
| `reviews` · `favorites` · `vehicles` | 리뷰(예약당 하나) · 관심 업체 · 내 차량 |
| `products` · `orders` · `order_items` | 상품 · 주문(결제·받는 방법·취소) · 줄(업체·배송·수수료) |
| `settlements` · `platform_settings` | 정산 지급 기록 · 운영 설정 |
| `files` | 업로드 이미지 |
| `inquiries` · `inquiry_messages` · `notices` | 문의·신고 · 대화 · 공지 |
| `admin_audit` | 작업 기록 |

- 문자열 상태 값은 `VARCHAR` 로 두고 앱에서 검사합니다 (sqlx 가 MariaDB `ENUM` 을 `String` 으로 읽지 못해 예전 목록 API 가 실패하던 문제를 고침).
- 좌표는 `DOUBLE` (예전 `DECIMAL` 은 `f64` 로 읽히지 않았음).
- DB 연결은 `time_zone = '+09:00'` 으로 맞춰 `NOW()`·기본값·화면에서 받은 시각이 모두 한국 시간입니다.
- 금액 합계는 `CAST(SUM(…) AS SIGNED)` 로 읽습니다 (MariaDB `SUM` 은 DECIMAL 을 돌려줌).

## 화면 공통

- `@gageyo/ui` 의 `api()` 는 `credentials: include` 로 쿠키를 보내고, 오류 응답의 `error` 문구를 그대로 던집니다.
- 고객 화면은 주소 매개변수로 메일 링크·결제창 복귀를 처리합니다: `?shop=` `?quote=` `?order=` `?my=` `?reset=` `?inquiry=&token=` `?pay=success|fail` `?request=quote|support|lookup|cart`.
- 장바구니·최근 견적 번호는 브라우저 `localStorage`, 비회원 결제 복귀용 연락처는 `sessionStorage` 에 둡니다 (저장소를 못 쓰는 브라우저에서도 화면은 동작).
