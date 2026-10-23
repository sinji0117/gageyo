[← 문서 목차](README.md)

# API

- 기본 주소는 `/api` 입니다. 화면 네 개는 같은 주소의 `/api/` 를 API 서버로 프록시합니다.
- 요청·응답은 JSON, 금액 단위는 원(KRW), 날짜·시각은 한국 시간이며 시간대 표시 없이 `2026-10-24T14:30:00` 형식입니다.
- 오류는 항상 `{"error": "안내 문구", "code": "bad_request"}` 입니다. 화면은 `error` 를 그대로 보여 줍니다.

  | HTTP | code | 뜻 |
  | --- | --- | --- |
  | 400 | `bad_request` | 입력 오류 (안내 문구에 고칠 점) |
  | 401 | `unauthorized` | 로그인 필요·잘못된 키 |
  | 401 | `otp_required` | 운영자 OTP 코드 필요 |
  | 402 | `payment_rejected` | 결제사 거절 (카드 거절·환불 실패) |
  | 403 | `forbidden` | 권한 없음·승인 전 업체·연락처 불일치 |
  | 404 | `not_found` | 없음 (남의 것도 404 로 숨김) |
  | 409 | `conflict` | 중복·이미 처리됨·상태가 맞지 않음 |
  | 423 | `locked` | 로그인 5번 실패 → 15분 잠금 |
  | 429 | `too_many_requests` | 요청 횟수 제한 |
  | 503 | `unavailable` | 서버 설정 없음·결제사 통신 실패 |

## 인증

| 누구 | 방법 |
| --- | --- |
| 고객 | `POST /auth/login` → HttpOnly 쿠키 `gy_c` (30일) |
| 파트너 | `POST /partner/auth/login`·`/partner/apply` → 쿠키 `gy_p` (14일). 예전 방식 `x-partner-key` 헤더도 받음 |
| 운영자 | `POST /admin/auth/login` → 쿠키 `gy_a` (12시간), 이메일이 `ADMIN_EMAILS` 에 있어야 함. 비상용 `x-admin-key: ADMIN_API_KEY` |

쿠키는 `Path=/api; HttpOnly; SameSite=Lax; Secure`(설정에 따라) 이고, 세션은 DB 에 토큰 해시로만 저장합니다.
세션마다 범위(고객·파트너·운영자)가 있어 다른 범위의 API 에 쓸 수 없습니다. 계정을 정지하거나 비밀번호를 바꾸면 세션이 모두 지워집니다.

## 공개

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| GET | `/health` | 상태 (DB 연결까지 확인) |
| GET | `/config` | 가입 코드 인증 여부, 결제 수단(토스 클라이언트 키·무통장 여부), 고객센터, 택배사 목록 |
| GET | `/stats` | 승인 업체 수, 리뷰 수, 평균 평점, 견적 수, 완료 예약 수 |
| GET | `/sitemap.xml` | 고객 화면·승인 업체 주소 |
| GET | `/categories` | 켜진 서비스 카테고리 |
| GET | `/workshops?q=&category=&district=&lat=&lng=&radius_km=&sort=&page=&size=` | 승인 업체 검색 → `{items, has_more}`. `sort`: rating·distance·reviews·recent, 위치가 있으면 `distance_km` |
| GET | `/workshops/{id}` | 업체 상세 (서비스·사진·분야·리뷰 5개·별점 분포·상품 8개·`is_favorite`) |
| GET | `/workshops/{id}/services` | 서비스 목록 |
| GET | `/workshops/{id}/reviews?page=&size=` | 리뷰 (작성자 이름 가림) |
| GET | `/products?q=&category=&workshop_id=&sort=&page=&size=` | 판매 중·품절 상품 → `{items, has_more}` |
| GET | `/products/categories` | 상품 분류와 개수 |
| GET | `/products/{id}` | 상품 상세 |
| GET | `/notices?limit=` | 고객 대상 공지 |
| GET | `/files/{yyyymm}/{name}` | 업로드 이미지 |

## 고객 계정

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| POST | `/auth/send-code` | `{email}` 가입 인증 코드 메일 |
| POST | `/auth/register` | `{email, password, display_name, phone?, email_code?, agree_terms?}` → 201 + 쿠키 |
| POST | `/auth/login` | `{email, password}` |
| POST | `/auth/logout` | |
| GET | `/auth/me` | 내 정보 + 숫자(받는 견적·다가오는 예약·진행 주문·관심 업체) |
| POST | `/auth/password/forgot` | `{email, scope?: customer·partner·admin}` 설정 링크 메일 (항상 같은 응답) |
| POST | `/auth/password/reset` | `{token, password}` |
| PUT | `/me/profile` | `{display_name, phone?}` |
| PUT | `/me/password` | `{current_password, new_password}` (다른 세션 모두 끊음) |
| DELETE | `/me` | `{password}` 탈퇴 |
| GET/POST | `/me/vehicles` | 내 차량 (최대 10대) `{model, label?, model_year?, plate?, mileage?}` |
| PUT/DELETE | `/me/vehicles/{id}` | |
| GET | `/me/favorites` · PUT/DELETE `/me/favorites/{workshop_id}` | 관심 업체 |
| GET | `/me/quotes` · `/me/bookings` · `/me/orders` · `/me/orders/{id}` · `/me/reviews` · `/me/inquiries` | 내 기록 |

## 견적·예약·리뷰 (고객)

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| POST | `/quotes` | `{vehicle, service_category, description, district, customer_phone, customer_name?}` (회원이면 계정에 연결) |
| GET | `/quotes/{id}?phone=` | 요청과 받은 견적 (회원 본인 또는 연락처 일치) |
| GET | `/quotes/{id}/offers` | 받은 견적만 (예전 화면 호환, 개인정보 없음) |
| POST | `/quotes/{id}/accept` | `{offer_id, phone?}` → 업체 정보 (예약 신청서용) |
| POST | `/quotes/{id}/cancel` | `{phone?}` |
| POST | `/bookings` | `{workshop_id, service_id?, quote_offer_id?, vehicle, starts_at, customer_name, customer_phone, note?}` |
| POST | `/bookings/lookup` | `{booking_id, phone}` 비회원 조회 |
| POST | `/bookings/{id}/cancel` | `{phone?, reason?}` |
| POST | `/me/reviews` | `{booking_id, rating 1~5, content}` (완료된 내 예약) |
| PUT/DELETE | `/me/reviews/{id}` | |

## 주문·결제 (고객)

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| GET | `/checkout/config` | 쓸 수 있는 결제 수단, 무통장 계좌, 택배사 |
| POST | `/orders` | 아래 예시. 응답에 `toss`(결제창 값) 또는 `bank`(계좌) |
| POST | `/orders/{id}/payments/toss` | `{payment_key, amount, phone?}` 결제창에서 돌아온 뒤 승인 |
| POST | `/orders/lookup` | `{order_id, phone}` 비회원 주문 상세 |
| GET | `/orders/{id}` | 상태·금액만 (예전 화면 호환) |
| POST | `/orders/{id}/cancel` | `{phone?, reason?}` 결제 전 주문만 |

```json
{
  "customer_name": "이운전", "customer_phone": "010-4444-5555", "email": "me@example.com",
  "fulfillment": "delivery", "address": "서울 강남구 테헤란로 1, 101호",
  "payment_method": "toss",
  "items": [{ "product_id": "…", "quantity": 2 }]
}
```

방문 장착은 `"fulfillment": "visit", "visit_at": "2026-10-28T10:00:00", "vehicle": "아반떼 2021"` (한 업체 상품만).
`payment_method`: `toss` · `bank`(+`depositor_name`) · `none`(결제 수단이 하나도 없는 서버에서만). 비우면 쓸 수 있는 첫 수단.

## 고객센터

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| POST | `/support/inquiries` | `{name?, email?, subject, body, website}` (`website` 는 봇 막기용 빈 칸) → `{id, token}` (비회원만 token) |
| GET | `/support/inquiries/{id}?token=` | 대화 |
| POST | `/support/inquiries/{id}/messages` | `{body, token?}` |
| POST | `/reports` | `{target_type: workshop·review·product, target_id, reason, email?}` |

## 파트너

모두 파트너 로그인(또는 `x-partner-key`)이 필요합니다. ★ 는 **승인된 업체**만.

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| POST | `/partner/auth/send-code` | 입점 신청 인증 코드 |
| POST | `/partner/apply` | 계정 + 업체 (아래 예시) → 201 + 쿠키, 업체 `pending` |
| POST | `/partner/auth/login` · `/partner/auth/logout` | |
| GET | `/partner/me` | 업체 정보·분야·계정·심사 중 변경 요청 수 |
| GET | `/partner/dashboard` | 대시보드 숫자 |
| PUT | `/partner/profile` | `{description, phone, open_time?, close_time?, closed_days?("0,6"), cover_image_url?, latitude?, longitude?}` 바로 반영 |
| GET/POST | `/partner/change-requests` | 상호·분야·지역·주소·사업자 변경 (승인 업체는 요청, 아니면 바로 반영 후 재심사) |
| GET/POST | `/partner/photos` · DELETE `/partner/photos/{id}` | 업체 사진 `{url}` (최대 12장) |
| GET/POST | `/partner/services` · PATCH/DELETE `/partner/services/{id}` | 서비스 |
| GET/POST | `/partner/products` · PATCH/DELETE `/partner/products/{id}` | 상품 (`status`: active·draft·archived) |
| GET ★ | `/partner/quotes` | 맞는 견적 요청 (연락처 가림) |
| POST ★ | `/partner/quotes/{id}/offers` | `{amount, description, valid_until?}` |
| GET ★ · DELETE ★ | `/partner/offers` · `/partner/offers/{id}` | 보낸 견적 · 철회 |
| GET ★ | `/partner/bookings?status=&when=upcoming·past` | 예약 |
| PATCH ★ | `/partner/bookings/{id}` | `{status: confirmed·declined·completed·cancelled, reason?}` |
| GET ★ | `/partner/orders?view=todo·shipped·done·cancelled·all` | 우리 상품 줄 |
| PATCH ★ | `/partner/order-items/{id}` | `{status: preparing·shipped·delivered, carrier?, tracking_no?}` |
| GET | `/partner/reviews` · PUT `/partner/reviews/{id}/reply` | 리뷰·답글 `{reply}` (빈 값이면 지움) |
| GET ★ | `/partner/settlements?year=` | 월별 매출·수수료·지급액·지급 기록 |
| GET | `/partner/notices` | 파트너 대상 공지 |
| GET/POST | `/partner/inquiries` · GET `/partner/inquiries/{id}` · POST `…/messages` | 운영자 문의 |
| POST | `/files` | 이미지 올리기 (multipart `file`) → `{id, url, width, height}` |

```json
{
  "email": "shop@example.com", "password": "10자 이상", "display_name": "김사장", "phone": "010-2222-3333",
  "email_code": "123456", "agree_terms": true,
  "workshop": {
    "name": "역삼 모터스", "categories": ["자동차 정비", "자동차 썬팅"], "district": "강남구",
    "address": "서울 강남구 테헤란로 1", "business_number": "120-81-47521", "representative": "김사장",
    "phone": "02-555-1234", "description": "엔진오일·브레이크 전문", "latitude": 37.5006, "longitude": 127.0364
  }
}
```

## 운영자

모두 운영자 로그인(또는 `x-admin-key`)이 필요하고, 바꾸는 작업은 작업 기록(`admin_audit`)에 남습니다.

| 메서드 | 경로 | 설명 |
| --- | --- | --- |
| POST | `/admin/auth/login` | `{email, password, otp?}` (OTP 가 켜져 있으면 없을 때 401 `otp_required`) |
| POST | `/admin/auth/logout` · GET `/admin/auth/me` | |
| POST | `/admin/auth/otp/setup` · `/otp/enable {secret, code}` · `/otp/disable {code}` | 2단계 인증 |
| GET | `/admin/dashboard` | 현황·월별 거래액·최근 작업 기록 |
| GET/POST | `/admin/workshops?status=&q=` | 업체 목록 · 직접 등록 |
| GET/PATCH/PUT | `/admin/workshops/{id}` | 상세 · `{status, reason?}` 심사 · 정보 고치기 |
| POST | `/admin/workshops/{id}/owner-link` | 주인 계정 비밀번호 설정 링크 (메일 미설정이면 응답에 link) |
| POST | `/admin/workshops/{id}/partner-key` | 예전 방식 파트너 키 발급 |
| GET · PATCH | `/admin/change-requests?status=` · `/admin/change-requests/{id}` `{approve, reason?}` | 정보 변경 요청 |
| GET · PATCH | `/admin/quotes` · `/admin/quotes/{id}` `{status}` | 견적 |
| GET · PATCH | `/admin/bookings` · `/admin/bookings/{id}` `{status, reason?}` | 예약 |
| GET · PATCH | `/admin/reviews?status=` · `/admin/reviews/{id}` `{status: visible·hidden, reason?}` | 리뷰 |
| GET · PATCH | `/admin/products?status=&q=` · `/admin/products/{id}` `{status, reason?}` | 상품 (`blocked` = 판매 중지) |
| GET · GET · PATCH | `/admin/orders?status=&method=&q=` · `/admin/orders/{id}` · `{status}` | 주문 |
| POST | `/admin/orders/{id}/confirm-deposit` | 무통장 입금 확인 → 결제 완료 |
| POST | `/admin/orders/{id}/toss-sync` | 카드 결제 확인: 토스에서 주문번호로 결제를 찾아 승인돼 있으면 결제 완료로 (`{"paid": bool}`) |
| POST | `/admin/orders/{id}/cancel` | `{reason}` 취소 (카드는 자동 환불) → `{status, manual_refund_needed}` |
| GET · POST · DELETE | `/admin/settlements?period=YYYY-MM` · `{workshop_id, period, memo?}` · `?workshop_id=&period=` | 정산 · 지급 완료 기록 · 기록 지우기 |
| GET · PATCH | `/admin/users?role=&status=&q=` · `/admin/users/{id}` `{status, reason?}` | 회원 · 정지 |
| GET/POST · PUT/DELETE | `/admin/categories` · `/admin/categories/{id}` | 카테고리 |
| GET · GET/PATCH · POST | `/admin/inquiries?kind=&status=` · `/admin/inquiries/{id}` · `/admin/inquiries/{id}/reply` | 문의·신고 |
| GET/POST · PUT/DELETE | `/admin/notices` · `/admin/notices/{id}` | 공지 |
| GET/PUT | `/admin/settings` | 수수료율·무통장 계좌·고객센터·사업자 정보 |
| GET | `/admin/audit?target_type=&page=` | 작업 기록 |

## 요청 횟수 제한

| 대상 | 제한 |
| --- | --- |
| 로그인 실패 (계정별) | 15분에 5번 → 잠금 |
| 로그인 시도 (IP) | 15분에 50번 |
| 가입 인증 코드 | 같은 이메일 60초에 1번·1시간 5번, IP 1시간 30번 |
| 가입 (IP) | 1시간 20번 |
| 입점 신청 (IP) | 1시간 10번 |
| 견적 요청·예약 신청 (IP) | 1시간 20번 |
| 주문 (IP) | 1시간 30번 |
| 비회원 주문·예약 조회 (IP) | 15분 30번 |
| 문의·신고 (IP) | 1시간 10번 |
| 비밀번호 찾기 | IP 1시간 10번, 이메일 1시간 3번 |
| 이미지 올리기 | 업체(또는 계정) 1시간 300장 |
