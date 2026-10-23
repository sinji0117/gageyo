#!/usr/bin/env bash
# 종단 검사: 실제 MariaDB 에 API 를 띄우고 고객·파트너·운영자 흐름을 curl 로 끝까지 확인합니다.
#
#   필요: MariaDB (DATABASE_URL), curl, jq, python3
#   실행: DATABASE_URL=mysql://gageyo_user:devpass@127.0.0.1:3306/gageyo_db bash scripts/e2e.sh
#
# 토스 결제는 로컬 모의 서버(scripts/mock_toss.py)로 승인·취소를 흉내 냅니다.
# DB 는 비어 있지 않아도 되지만, 검사용 데이터가 쌓이므로 개발·CI 전용 DB 에서 돌리세요.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
: "${DATABASE_URL:?DATABASE_URL 을 설정해 주세요}"
PORT="${E2E_PORT:-18081}"
TOSS_PORT="${E2E_TOSS_PORT:-18090}"
BASE="http://127.0.0.1:$PORT/api"
WORK="$(mktemp -d)"
LOG="$WORK/api.log"
PASS=0
FAIL=0
ADMIN_KEY="e2e-admin-key-$(date +%s)"
RUN="$(date +%s)$RANDOM"
ADMIN_EMAIL="ops-$RUN@gageyo.test"

cleanup() {
  [ -n "${API_PID:-}" ] && kill "$API_PID" 2>/dev/null
  [ -n "${TOSS_PID:-}" ] && kill "$TOSS_PID" 2>/dev/null
  if [ "$FAIL" -gt 0 ]; then
    echo "----- API 로그 (마지막 80줄) -----"
    tail -n 80 "$LOG" 2>/dev/null
  fi
}
trap cleanup EXIT

BIN="$ROOT/backend/target/debug/gageyo-api"
if [ ! -x "$BIN" ]; then
  (cd "$ROOT/backend" && cargo build -p gageyo-api) || exit 1
fi

python3 "$ROOT/scripts/mock_toss.py" "$TOSS_PORT" > "$WORK/toss.log" 2>&1 &
TOSS_PID=$!

env DATABASE_URL="$DATABASE_URL" PORT="$PORT" ADMIN_EMAILS="$ADMIN_EMAIL" ADMIN_API_KEY="$ADMIN_KEY" \
  COOKIE_SECURE=false SIGNUP_EMAIL_VERIFY=off TOSS_CLIENT_KEY=test_ck_x TOSS_SECRET_KEY=test_sk_x \
  TOSS_API_BASE="http://127.0.0.1:$TOSS_PORT" UPLOAD_DIR="$WORK/uploads" RUST_LOG=info NO_COLOR=1 \
  "$BIN" > "$LOG" 2>&1 &
API_PID=$!

for _ in $(seq 1 60); do
  curl -sf "$BASE/health" > /dev/null && break
  sleep 1
done

# ---------------------------------------------------------------------------
# 도움 함수
# ---------------------------------------------------------------------------
# call METHOD PATH JAR [JSON] [추가 curl 인자...] → STATUS, BODY
call() {
  local m=$1 p=$2 jar=$3 d=${4:-}
  shift 4 2>/dev/null || shift $#
  local args=(-s -o "$WORK/body" -w '%{http_code}' -X "$m" -H 'Content-Type: application/json')
  [ "$jar" != "-" ] && args+=(-c "$WORK/$jar.jar" -b "$WORK/$jar.jar")
  [ -n "$d" ] && args+=(--data "$d")
  STATUS=$(curl "${args[@]}" "$@" "$BASE$p")
  BODY=$(cat "$WORK/body")
}
admin() { # admin METHOD PATH [JSON]  (x-admin-key)
  call "$1" "$2" - "${3:-}" -H "x-admin-key: $ADMIN_KEY"
}
expect() { # expect 설명 기대코드
  if [ "$STATUS" = "$2" ]; then
    PASS=$((PASS + 1)); echo "  ok   $1"
  else
    FAIL=$((FAIL + 1)); echo "  FAIL $1 — 기대 $2, 받음 $STATUS: $BODY"
  fi
}
is() { # is 설명 실제 기대
  if [ "$2" = "$3" ]; then
    PASS=$((PASS + 1)); echo "  ok   $1"
  else
    FAIL=$((FAIL + 1)); echo "  FAIL $1 — 기대 [$3], 받음 [$2]"
  fi
}
j() { echo "$BODY" | jq -r "$1"; }
reset_token() { grep -o 'reset=[0-9a-f]\{64\}' "$LOG" | tail -n 1 | cut -d= -f2; }
totp() { python3 - "$1" <<'PY'
import base64, hashlib, hmac, struct, sys, time
s = sys.argv[1]
k = base64.b32decode(s + "=" * ((8 - len(s) % 8) % 8))
h = hmac.new(k, struct.pack(">Q", int(time.time()) // 30), hashlib.sha1).digest()
o = h[19] & 15
print("%06d" % ((struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF) % 1000000))
PY
}
# 다음 평일(월~토) 날짜
next_open_day() {
  local n=${1:-1}
  while :; do
    local d; d=$(TZ=Asia/Seoul date -d "+$n day" +%F)
    [ "$(TZ=Asia/Seoul date -d "$d" +%w)" != "0" ] && { echo "$d"; return; }
    n=$((n + 1))
  done
}
next_sunday() { local n=1; while [ "$(TZ=Asia/Seoul date -d "+$n day" +%w)" != "0" ]; do n=$((n + 1)); done; TZ=Asia/Seoul date -d "+$n day" +%F; }

echo "== 기본"
call GET /health - ""; expect "상태 확인" 200
call GET /categories - ""; expect "카테고리" 200
is "기본 카테고리 18개 이상" "$(j 'length >= 18')" true
call GET /nope - ""; expect "없는 주소는 JSON 404" 404
is "오류는 JSON error 문자열" "$(j '.error | type')" string
call GET /config - ""; expect "공개 설정" 200
is "토스 키 공개" "$(j '.payments.toss.client_key')" test_ck_x

echo "== 운영자 첫 설정 (비밀번호 설정 링크 → 로그인 → OTP)"
call POST /auth/password/forgot - "{\"email\":\"$ADMIN_EMAIL\",\"scope\":\"admin\"}"; expect "운영자 비밀번호 설정 요청" 200
sleep 1
TOKEN=$(reset_token)
is "설정 링크가 로그에 남음(메일 미설정)" "$([ -n "$TOKEN" ] && echo yes)" yes
call POST /auth/password/reset - "{\"token\":\"$TOKEN\",\"password\":\"admin-pass-1234\"}"; expect "운영자 비밀번호 설정" 200
call POST /auth/password/reset - "{\"token\":\"$TOKEN\",\"password\":\"admin-pass-1234\"}"; expect "같은 링크 재사용 거부" 400
call POST /admin/auth/login adm "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"admin-pass-1234\"}"; expect "운영자 로그인" 200
call GET /admin/auth/me adm ""; expect "운영자 내 정보" 200
call POST /admin/auth/login - "{\"email\":\"someone@else.test\",\"password\":\"admin-pass-1234\"}"; expect "ADMIN_EMAILS 밖은 거부" 401
call GET /admin/dashboard - ""; expect "로그인 없이 운영자 API 거부" 401
call GET /admin/dashboard - "" -H "x-admin-key: wrong"; expect "틀린 운영자 키 거부" 401
call POST /admin/auth/otp/setup adm ""; expect "OTP 등록 시작" 200
SECRET=$(j .secret)
call POST /admin/auth/otp/enable adm "{\"secret\":\"$SECRET\",\"code\":\"000000\"}"; expect "틀린 OTP 코드 거부" 400
call POST /admin/auth/otp/enable adm "{\"secret\":\"$SECRET\",\"code\":\"$(totp "$SECRET")\"}"; expect "OTP 켜기" 200
call POST /admin/auth/login adm2 "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"admin-pass-1234\"}"; expect "OTP 없이 로그인 → otp_required" 401
is "otp_required 코드" "$(j .code)" otp_required
sleep 31
call POST /admin/auth/login adm2 "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"admin-pass-1234\",\"otp\":\"$(totp "$SECRET")\"}"; expect "OTP 로 로그인" 200
call GET /admin/workshops adm2 ""; expect "운영자 세션으로 업체 목록" 200

echo "== 파트너 입점 신청 → 승인"
P_EMAIL="shop-$RUN@gageyo.test"
APPLY=$(jq -n --arg e "$P_EMAIL" '{email:$e,password:"partner-pass-1",display_name:"김사장",phone:"010-2222-3333",agree_terms:true,
  workshop:{name:"역삼 모터스",categories:["자동차 정비","자동차 썬팅"],district:"강남구",address:"서울 강남구 테헤란로 1",
  business_number:"120-81-47521",representative:"김사장",phone:"02-555-1234",description:"엔진오일·브레이크 전문",latitude:37.5006,longitude:127.0364}}')
call POST /partner/apply ptn "$APPLY"; expect "입점 신청" 201
WID=$(j .workshop_id)
call POST /partner/apply - "$APPLY"; expect "같은 이메일 재신청 거부" 409
BAD=$(echo "$APPLY" | jq '.email="x-'$RUN'@gageyo.test" | .workshop.business_number="1208147522"')
call POST /partner/apply - "$BAD"; expect "틀린 사업자번호 거부" 400
call GET /partner/me ptn ""; expect "파트너 내 정보" 200
is "승인 대기" "$(j .status)" pending
call GET /partner/quotes ptn ""; expect "승인 전 견적 요청 막힘" 403
call GET /workshops - ""; expect "고객 업체 목록" 200
is "승인 전 업체는 안 보임" "$(j "[.items[] | select(.id==\"$WID\")] | length")" 0
admin PATCH "/admin/workshops/$WID" '{"status":"rejected"}'; expect "반려 사유 없으면 거부" 400
admin PATCH "/admin/workshops/$WID" '{"status":"approved"}'; expect "운영자 승인" 204
call GET /partner/me ptn ""; is "승인됨" "$(j .status)" approved
call POST /partner/auth/logout ptn ""; expect "파트너 로그아웃" 200
call GET /partner/me ptn ""; expect "로그아웃 뒤 막힘" 401
call POST /partner/auth/login ptn "{\"email\":\"$P_EMAIL\",\"password\":\"partner-pass-1\"}"; expect "파트너 로그인" 200

echo "== 업체 정보·서비스·상품"
call PUT /partner/profile ptn '{"description":"엔진오일·브레이크·썬팅","phone":"02-555-1234","open_time":"09:00","close_time":"20:00","closed_days":"0","latitude":37.5006,"longitude":127.0364}'
expect "영업시간·휴무일" 200
is "휴무일 정리" "$(j .closed_days)" 0
call PUT /partner/profile ptn '{"description":"x","phone":"02-555-1234","open_time":"20:00","close_time":"09:00"}'; expect "영업시간 거꾸로 거부" 400
call POST /partner/services ptn '{"name":"엔진오일 교환","description":"합성유","price_from":55000,"duration_minutes":40}'; expect "서비스 등록" 201
SID=$(j .id)
call POST /partner/products ptn '{"name":"합성 엔진오일 4L","category":"엔진오일","description":"5W-30","price":10000,"list_price":12000,"stock":5}'; expect "상품 등록" 201
PID=$(j .id)
call POST /partner/products ptn '{"name":"와이퍼","category":"소모품","price":8000,"stock":1}'; expect "상품 2 등록" 201
PID2=$(j .id)
call POST /partner/products ptn '{"name":"x","category":"y","price":10000,"list_price":5000,"stock":1}'; expect "정가<판매가 거부" 400
admin PATCH "/admin/products/$PID2" '{"status":"blocked"}'; expect "운영자 판매 중지" 204
call PATCH "/partner/products/$PID2" ptn '{"name":"와이퍼","category":"소모품","price":8000,"stock":3,"status":"active"}'; expect "중지 상품 수정" 200
is "운영자가 막은 상품은 파트너가 못 엶" "$(j .status)" blocked
admin PATCH "/admin/products/$PID2" '{"status":"active"}'; expect "다시 판매" 204
call GET "/workshops?category=%EC%9E%90%EB%8F%99%EC%B0%A8%20%EC%A0%95%EB%B9%84" - ""; expect "분야 검색" 200
is "분야로 찾음" "$(j "[.items[] | select(.id==\"$WID\")] | length")" 1
call GET "/workshops?lat=37.4979&lng=127.0276&radius_km=3&sort=distance" - ""; expect "반경 검색" 200
is "3km 안에 있음" "$(j "[.items[] | select(.id==\"$WID\")] | length")" 1
is "거리 계산" "$(j "[.items[] | select(.id==\"$WID\")][0].distance_km < 3")" true
call GET "/workshops?lat=35.1&lng=129.0&radius_km=5" - ""; is "부산 반경에는 없음" "$(j "[.items[] | select(.id==\"$WID\")] | length")" 0
call GET "/workshops/$WID" - ""; expect "업체 상세" 200
is "서비스 표시" "$(j '.services | length')" 1
is "분야 2개" "$(j '.categories | length')" 2
call GET "/products?q=%EC%97%94%EC%A7%84" - ""; expect "상품 검색" 200
is "상품 찾음" "$(j "[.items[] | select(.id==\"$PID\")] | length")" 1

echo "== 고객 가입·로그인"
C_EMAIL="driver-$RUN@gageyo.test"
call POST /auth/register cus "{\"email\":\"$C_EMAIL\",\"password\":\"driver-pass-1\",\"display_name\":\"이운전\",\"phone\":\"01044445555\"}"; expect "고객 가입" 201
is "연락처 정리" "$(j .phone)" "010-4444-5555"
call GET /auth/me cus ""; expect "내 정보" 200
call POST /auth/register - "{\"email\":\"$C_EMAIL\",\"password\":\"driver-pass-1\",\"display_name\":\"x\"}"; expect "중복 가입 거부" 409
call POST /auth/login - "{\"email\":\"$P_EMAIL\",\"password\":\"partner-pass-1\"}"; expect "파트너 계정은 고객 로그인 불가" 401
call POST /me/vehicles cus '{"model":"현대 아반떼","model_year":2021,"label":"출퇴근차"}'; expect "내 차량 등록" 201
call PUT "/me/favorites/$WID" cus ""; expect "관심 업체" 204
call GET /me/favorites cus ""; is "관심 업체 목록" "$(j length)" 1

echo "== 견적 요청 → 업체 견적 → 고객 선택"
call POST /quotes cus '{"vehicle":"현대 아반떼 2021","service_category":"자동차 정비","description":"엔진오일 교환과 점검","district":"서울 강남구 역삼동","customer_phone":"010-4444-5555"}'
expect "견적 요청" 201
QID=$(j .id)
call POST /quotes - '{"vehicle":"x","service_category":"없는분야","description":"x","district":"강남구","customer_phone":"010-1111-2222"}'; expect "없는 분야 거부" 400
call GET /partner/quotes ptn ""; expect "파트너 견적 요청 목록" 200
is "분야·지역 맞는 요청 보임" "$(j "[.[] | select(.id==\"$QID\")] | length")" 1
is "답하기 전 연락처 가림" "$(j "[.[] | select(.id==\"$QID\")][0].customer_phone")" "010-****-5555"
call POST "/partner/quotes/$QID/offers" ptn '{"amount":50000,"description":"합성유 교환 + 무상 점검"}'; expect "견적 보내기" 201
OFFER=$(j .id)
call POST "/partner/quotes/$QID/offers" ptn '{"amount":40000,"description":"다시"}'; expect "같은 요청에 두 번 거부" 409
call GET "/quotes/$QID" - ""; expect "다른 사람은 연락처 없이 못 봄" 403
call GET "/quotes/$QID?phone=01044445555" - ""; expect "연락처로 확인" 200
call GET "/quotes/$QID" cus ""; expect "요청한 회원은 봄" 200
is "견적 1건" "$(j '.offers | length')" 1
call GET /me/quotes cus ""; is "마이페이지 견적" "$(j "[.[] | select(.id==\"$QID\")][0].offer_count")" 1
call POST "/quotes/$QID/accept" cus "{\"offer_id\":\"$OFFER\"}"; expect "견적 선택" 200
is "선택한 업체" "$(j .workshop_id)" "$WID"
call POST "/quotes/$QID/accept" cus "{\"offer_id\":\"$OFFER\"}"; expect "두 번 선택 거부" 409
call GET /partner/offers ptn ""; is "선택된 견적은 연락처 보임" "$(j "[.[] | select(.id==\"$OFFER\")][0].customer_phone")" "010-4444-5555"

echo "== 예약 → 확정 → 완료 → 리뷰"
DAY=$(next_open_day 1)
call POST /bookings cus "{\"workshop_id\":\"$WID\",\"service_id\":\"$SID\",\"vehicle\":\"아반떼\",\"starts_at\":\"${DAY}T21:00:00\",\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\"}"
expect "영업시간 밖 거부" 400
SUN=$(next_sunday)
call POST /bookings cus "{\"workshop_id\":\"$WID\",\"vehicle\":\"아반떼\",\"starts_at\":\"${SUN}T10:00:00\",\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\"}"
expect "휴무일 거부" 400
call POST /bookings cus "{\"workshop_id\":\"$WID\",\"vehicle\":\"아반떼\",\"starts_at\":\"2020-01-01T10:00:00\",\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\"}"
expect "지난 시간 거부" 400
call POST /bookings cus "{\"workshop_id\":\"$WID\",\"service_id\":\"$SID\",\"quote_offer_id\":\"$OFFER\",\"vehicle\":\"아반떼\",\"starts_at\":\"${DAY}T10:00:00\",\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\"}"
expect "예약 신청" 201
BID=$(j .id)
call POST /bookings cus "{\"workshop_id\":\"$WID\",\"vehicle\":\"아반떼\",\"starts_at\":\"${DAY}T10:00:00\",\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\"}"
expect "같은 시간 중복 거부" 409
call PATCH "/partner/bookings/$BID" ptn '{"status":"completed"}'; expect "신청 → 바로 완료 가능" 204
call PATCH "/partner/bookings/$BID" ptn '{"status":"requested"}'; expect "완료 → 신청 되돌리기 거부" 400
call GET /me/bookings cus ""; is "내 예약 완료" "$(j "[.[] | select(.id==\"$BID\")][0].status")" completed
call POST /me/reviews cus "{\"booking_id\":\"$BID\",\"rating\":5,\"content\":\"친절하고 꼼꼼해요\"}"; expect "리뷰 쓰기" 201
RID=$(j .id)
call POST /me/reviews cus "{\"booking_id\":\"$BID\",\"rating\":4,\"content\":\"한 번 더 씀\"}"; expect "리뷰 두 번 거부" 409
call GET "/workshops/$WID" - ""; is "평점 반영" "$(j .rating)" 5
is "리뷰 수" "$(j .review_count)" 1
call PUT "/partner/reviews/$RID/reply" ptn '{"reply":"감사합니다!"}'; expect "답글" 204
admin PATCH "/admin/reviews/$RID" '{"status":"hidden"}'; expect "리뷰 숨김" 204
call GET "/workshops/$WID" - ""; is "숨기면 평점에서 빠짐" "$(j .review_count)" 0
admin PATCH "/admin/reviews/$RID" '{"status":"visible"}'; expect "리뷰 다시 공개" 204
DAY2=$(next_open_day 2)
call POST /bookings - "{\"workshop_id\":\"$WID\",\"vehicle\":\"쏘나타\",\"starts_at\":\"${DAY2}T11:00:00\",\"customer_name\":\"박비회원\",\"customer_phone\":\"010-7777-8888\"}"
expect "비회원 예약" 201
GBID=$(j .id)
call POST /bookings/lookup - "{\"booking_id\":\"$GBID\",\"phone\":\"010-0000-0000\"}"; expect "틀린 연락처 조회 거부" 404
call POST /bookings/lookup - "{\"booking_id\":\"$GBID\",\"phone\":\"01077778888\"}"; expect "비회원 예약 조회" 200
call PATCH "/partner/bookings/$GBID" ptn '{"status":"declined"}'; expect "거절 사유 필요" 400
call POST "/bookings/$GBID/cancel" - '{"phone":"010-7777-8888"}'; expect "비회원 예약 취소" 204

echo "== 주문 (토스 카드)"
call GET /checkout/config - ""; expect "결제 설정" 200
call POST /orders - "{\"customer_name\":\"박비회원\",\"customer_phone\":\"010-7777-8888\",\"address\":\"서울 강남구 1\",\"items\":[{\"product_id\":\"$PID\",\"quantity\":9}]}"
expect "재고보다 많이 주문 거부" 409
call POST /orders - "{\"customer_name\":\"박비회원\",\"customer_phone\":\"010-7777-8888\",\"email\":\"guest-$RUN@gageyo.test\",\"address\":\"서울 강남구 1\",\"payment_method\":\"toss\",\"items\":[{\"product_id\":\"$PID\",\"quantity\":1},{\"product_id\":\"$PID\",\"quantity\":1}]}"
expect "토스 주문" 201
OID=$(j .id)
is "같은 상품 줄 합쳐 금액 계산" "$(j .total_amount)" 20000
is "결제창 정보" "$(j .toss.client_key)" test_ck_x
call GET /partner/products ptn ""; is "재고 차감" "$(j "[.[] | select(.id==\"$PID\")][0].stock")" 3
call POST "/orders/$OID/payments/toss" - '{"payment_key":"pay_e2e_1","amount":19000}'; expect "금액 다르면 거부" 400
call POST "/orders/$OID/payments/toss" - '{"payment_key":"reject-me","amount":20000}'; expect "카드 거절은 402" 402
call POST "/orders/$OID/payments/toss" - '{"payment_key":"pay_e2e_1","amount":20000,"phone":"010-7777-8888"}'; expect "결제 승인" 200
is "결제 완료" "$(j .status)" paid
call POST "/orders/$OID/payments/toss" - '{"payment_key":"pay_e2e_1","amount":20000}'; expect "같은 승인 다시 (새로고침)" 200
call GET /partner/orders ptn ""; expect "파트너 주문 목록" 200
ITEM=$(j "[.[] | select(.order_id==\"$OID\")][0].item_id")
is "업체 몫 금액만 보임" "$(j "[.[] | select(.order_id==\"$OID\")][0].amount")" 20000
call PATCH "/partner/order-items/$ITEM" ptn '{"status":"shipped"}'; expect "택배사 없이 발송 거부" 400
call PATCH "/partner/order-items/$ITEM" ptn '{"status":"shipped","carrier":"cj","tracking_no":"1234-5678-9012"}'; expect "발송 처리" 204
call POST /orders/lookup - "{\"order_id\":\"$OID\",\"phone\":\"010-7777-8888\"}"; expect "비회원 주문 조회" 200
is "주문 상태 발송" "$(j .status)" shipped
is "배송 조회 주소" "$(j '.items[0].tracking_url')" "https://trace.cjlogistics.com/next/tracking.html?wblNo=123456789012"
call POST /orders/lookup - "{\"order_id\":\"$OID\",\"phone\":\"010-0000-0000\"}"; expect "틀린 연락처 거부" 404
admin POST "/admin/orders/$OID/cancel" '{"reason":"테스트"}'; expect "발송된 주문 취소 거부" 409
call PATCH "/partner/order-items/$ITEM" ptn '{"status":"delivered"}'; expect "배송 완료" 204

echo "== 주문 (무통장 → 입금 확인 → 취소·환불)"
admin PUT /admin/settings '{"fee_rate_bp":350,"bank_name":"국민은행","bank_account":"123-456-7890","bank_holder":"가게요"}'; expect "수수료·입금 계좌 설정" 200
call POST /orders cus "{\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\",\"address\":\"서울 강남구 2\",\"payment_method\":\"bank\",\"items\":[{\"product_id\":\"$PID\",\"quantity\":1}]}"
expect "무통장 주문" 201
BOID=$(j .id)
is "입금 계좌 안내" "$(j .bank.bank_name)" 국민은행
call GET /me/orders cus ""; is "마이페이지 주문" "$(j "[.[] | select(.id==\"$BOID\")][0].status")" payment_pending
admin POST "/admin/orders/$BOID/confirm-deposit" ""; expect "입금 확인" 204
call GET "/me/orders/$BOID" cus ""; is "결제 완료" "$(j .status)" paid
admin POST "/admin/orders/$BOID/cancel" '{"reason":"고객 요청"}'; expect "운영자 취소" 200
is "무통장은 직접 환불 안내" "$(j .manual_refund_needed)" true
call GET /partner/products ptn ""; is "취소하면 재고 복원" "$(j "[.[] | select(.id==\"$PID\")][0].stock")" 3

echo "== 주문 (토스 결제 후 운영자 취소 → 자동 환불, 결제 전 고객 취소)"
call POST /orders cus "{\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\",\"address\":\"서울\",\"items\":[{\"product_id\":\"$PID2\",\"quantity\":1}]}"
expect "카드 주문" 201
TOID=$(j .id)
call POST "/orders/$TOID/payments/toss" cus '{"payment_key":"pay_e2e_2","amount":8000}'; expect "카드 결제" 200
admin POST "/admin/orders/$TOID/cancel" '{"reason":"품절"}'; expect "카드 결제 취소" 200
is "환불됨" "$(j .status)" refunded
call POST /orders cus "{\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\",\"address\":\"서울\",\"items\":[{\"product_id\":\"$PID\",\"quantity\":3}]}"
expect "결제 전 주문" 201
POID=$(j .id)
call GET /partner/products ptn ""; is "재고 0 → 품절" "$(j "[.[] | select(.id==\"$PID\")][0].status")" sold_out
call POST "/orders/$POID/cancel" cus '{}'; expect "결제 전 고객 취소" 204
call GET /partner/products ptn ""; is "취소 → 다시 판매 중" "$(j "[.[] | select(.id==\"$PID\")][0].status")" active

echo "== 방문 장착 주문"
call POST /orders cus "{\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\",\"fulfillment\":\"visit\",\"vehicle\":\"아반떼\",\"visit_at\":\"${DAY2}T15:00:00\",\"items\":[{\"product_id\":\"$PID\",\"quantity\":1}]}"
expect "방문 장착 주문" 201
is "예약 함께 생성" "$([ "$(j .booking_id)" != null ] && echo yes)" yes
call POST /orders cus "{\"customer_name\":\"이운전\",\"customer_phone\":\"010-4444-5555\",\"fulfillment\":\"visit\",\"vehicle\":\"아반떼\",\"items\":[{\"product_id\":\"$PID\",\"quantity\":1}]}"
expect "방문 시간 없으면 거부" 400

echo "== 정산"
call GET /partner/settlements ptn ""; expect "파트너 정산" 200
is "이번 달 매출(배송 완료 2만 원)" "$(j '.months[0].gross')" 20000
admin GET /admin/settlements ""; expect "운영자 정산" 200
admin POST /admin/settlements "{\"workshop_id\":\"$WID\",\"period\":\"$(TZ=Asia/Seoul date +%Y-%m)\"}"; expect "이번 달 지급 처리 거부" 400

echo "== 업체 정보 변경 요청"
call POST /partner/change-requests ptn '{"name":"역삼 모터스 2호점","categories":["자동차 정비"],"district":"강남구","address":"서울 강남구 테헤란로 2","business_number":"1208147521"}'
expect "변경 요청" 200
is "승인 업체는 요청으로 남음" "$(j .applied)" false
CRID=$(j .request_id)
call POST /partner/change-requests ptn '{"name":"x","categories":["자동차 정비"],"district":"강남구","address":"y"}'; expect "심사 중 중복 요청 거부" 409
admin PATCH "/admin/change-requests/$CRID" '{"approve":true}'; expect "변경 승인" 204
call GET /partner/me ptn ""; is "상호 반영" "$(j .name)" "역삼 모터스 2호점"
is "분야 반영" "$(j '.categories | length')" 1

echo "== 이미지"
python3 -c "import zlib,struct;w=h=4;raw=b''.join(b'\x00'+b'\xff\x00\x00'*w for _ in range(h));c=lambda t,d:struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d)&0xffffffff);open('$WORK/a.png','wb').write(b'\x89PNG\r\n\x1a\n'+c(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+c(b'IDAT',zlib.compress(raw))+c(b'IEND',b''))"
STATUS=$(curl -s -o "$WORK/body" -w '%{http_code}' -b "$WORK/ptn.jar" -F "file=@$WORK/a.png" "$BASE/files"); BODY=$(cat "$WORK/body")
expect "이미지 올리기" 201
URL=$(j .url)
STATUS=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT$URL"); expect "이미지 열기" 200
echo "not an image" > "$WORK/a.txt"
STATUS=$(curl -s -o "$WORK/body" -w '%{http_code}' -b "$WORK/ptn.jar" -F "file=@$WORK/a.txt" "$BASE/files"); BODY=$(cat "$WORK/body")
expect "이미지 아닌 파일 거부" 400
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -F "file=@$WORK/a.png" "$BASE/files"); expect "로그인 없이 업로드 거부" 401
call POST /partner/photos ptn "{\"url\":\"$URL\"}"; expect "업체 사진 등록" 201
call GET "/workshops/$WID" - ""; is "대표 사진 자동 지정" "$(j .cover_image_url)" "$URL"

echo "== 고객센터·신고·공지"
call POST /support/inquiries - "{\"name\":\"박비회원\",\"email\":\"guest-$RUN@gageyo.test\",\"subject\":\"환불 문의\",\"body\":\"언제 환불되나요?\"}"; expect "비회원 문의" 201
IID=$(j .id); ITOK=$(j .token)
is "비밀 링크 48자" "$(j '.token | length')" 48
call GET "/support/inquiries/$IID?token=wrong" - ""; expect "틀린 링크 거부" 404
call GET "/support/inquiries/$IID?token=$ITOK" - ""; expect "링크로 확인" 200
admin POST "/admin/inquiries/$IID/reply" '{"body":"3영업일 안에 환불됩니다."}'; expect "운영자 답변" 200
call POST "/support/inquiries/$IID/messages" - "{\"body\":\"감사합니다\",\"token\":\"$ITOK\"}"; expect "이어서 묻기" 200
is "다시 답변 대기" "$(j .status)" open
call POST /support/inquiries - '{"name":"봇","email":"bot@x.com","subject":"spam","body":"spam","website":"http://spam"}'; expect "봇 칸 채우면 조용히 무시" 201
call POST /reports cus "{\"target_type\":\"review\",\"target_id\":\"$RID\",\"reason\":\"광고성 리뷰 같아요\"}"; expect "리뷰 신고" 201
call POST /partner/inquiries ptn '{"subject":"정산 문의","body":"정산일이 언제인가요?"}'; expect "파트너 문의" 201
admin GET "/admin/inquiries?status=open" ""; expect "운영자 문의함" 200
is "신고 탭 숫자" "$(j '.open_counts.report >= 1')" true
admin POST /admin/notices '{"audience":"customer","title":"추석 연휴 안내","body":"연휴 동안 상담이 쉽니다.","pinned":true}'; expect "공지 작성" 201
call GET /notices - ""; is "고객 공지 보임" "$(j '[.[] | select(.title=="추석 연휴 안내")] | length >= 1')" true
call GET /partner/notices ptn ""; is "고객 공지는 파트너에게 안 보임" "$(j '[.[] | select(.title=="추석 연휴 안내")] | length')" 0

echo "== 운영자 회원 관리·작업 기록·대시보드"
admin GET "/admin/users?q=driver-$RUN" ""; expect "회원 검색" 200
CUID=$(j '.[0].id')
is "목록은 연락처 가림" "$(j '.[0].phone')" "010-****-5555"
admin PATCH "/admin/users/$CUID" '{"status":"suspended"}'; expect "정지 사유 필요" 400
admin PATCH "/admin/users/$CUID" '{"status":"suspended","reason":"테스트"}'; expect "회원 정지" 204
call GET /auth/me cus ""; expect "정지하면 바로 로그아웃" 401
call POST /auth/login cus "{\"email\":\"$C_EMAIL\",\"password\":\"driver-pass-1\"}"; expect "정지 계정 로그인 거부" 403
admin PATCH "/admin/users/$CUID" '{"status":"active"}'; expect "정지 해제" 204
admin GET /admin/audit ""; expect "작업 기록" 200
is "기록 쌓임" "$(j 'length > 5')" true
admin GET /admin/dashboard ""; expect "대시보드" 200
is "승인 업체 수" "$(j '.workshops.approved >= 1')" true
admin GET /admin/categories ""; expect "운영자 카테고리" 200
CAT=$(j '[.[] | select(.name=="자동차 정비")][0].id')
admin DELETE "/admin/categories/$CAT" ""; expect "쓰는 카테고리 삭제 거부" 409
admin POST "/admin/workshops/$WID/partner-key" ""; expect "예전 파트너 키 발급" 201
PKEY=$(j .partner_key)
call GET /partner/me - "" -H "x-partner-key: $PKEY"; expect "파트너 키로 접속" 200
call GET /sitemap.xml - ""; expect "사이트맵" 200
is "사이트맵에 업체" "$(grep -c "$WID" "$WORK/body")" 1

echo "== 비밀번호 재설정·로그인 잠금·탈퇴"
call POST /auth/password/forgot - "{\"email\":\"$C_EMAIL\"}"; expect "재설정 요청" 200
sleep 1
call POST /auth/password/reset - "{\"token\":\"$(reset_token)\",\"password\":\"driver-pass-2\"}"; expect "비밀번호 재설정" 200
call POST /auth/login cus "{\"email\":\"$C_EMAIL\",\"password\":\"driver-pass-2\"}"; expect "새 비밀번호로 로그인" 200
for i in 1 2 3 4 5; do call POST /auth/login - "{\"email\":\"$C_EMAIL\",\"password\":\"wrong-$i-password\"}"; done
call POST /auth/login - "{\"email\":\"$C_EMAIL\",\"password\":\"driver-pass-2\"}"; expect "5번 틀리면 잠금" 423
call DELETE /me cus '{"password":"driver-pass-2"}'; expect "다가오는 예약 있으면 탈퇴 거부" 409
call GET /me/bookings cus ""
for b in $(j '.[] | select(.status=="requested" or .status=="confirmed") | .id'); do call POST "/bookings/$b/cancel" cus '{}'; done
call DELETE /me cus '{"password":"driver-pass-2"}'; expect "탈퇴" 200
call GET /auth/me cus ""; expect "탈퇴 뒤 로그아웃" 401
call POST /auth/register - "{\"email\":\"$C_EMAIL\",\"password\":\"driver-pass-3\",\"display_name\":\"재가입\"}"; expect "같은 이메일 재가입" 201

echo
echo "통과 $PASS · 실패 $FAIL"
[ "$FAIL" -eq 0 ]
