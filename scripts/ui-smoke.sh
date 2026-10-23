#!/usr/bin/env bash
# 화면 점검 준비: API 를 8081 에 띄우고, 검사용 업체·상품·고객·견적을 만들고, 빌드한 화면 네 개를 vite preview 로 띄운 뒤
# scripts/ui-smoke.mjs(Playwright)로 화면을 찍습니다.
#
#   필요: MariaDB(DATABASE_URL), 빌드한 API(backend/target/debug/gageyo-api), 빌드한 화면(frontend npm run build),
#         Chrome, playwright-core (NODE_PATH 로 찾을 수 있게)
#   실행: DATABASE_URL=... OUT=/tmp/shots bash scripts/ui-smoke.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
: "${DATABASE_URL:?DATABASE_URL 을 설정해 주세요}"
OUT="${OUT:-/tmp/ui-shots}"
WORK="$(mktemp -d)"
KEY="ui-admin-key-$RANDOM$RANDOM"
RUN="$(date +%s)"
BASE="http://127.0.0.1:8081/api"
PIDS=()
cleanup() { for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null; done; }
trap cleanup EXIT

python3 "$ROOT/scripts/mock_toss.py" 18091 > "$WORK/toss.log" 2>&1 & PIDS+=($!)
env DATABASE_URL="$DATABASE_URL" PORT=8081 ADMIN_EMAILS="ui-ops@gageyo.test" ADMIN_API_KEY="$KEY" COOKIE_SECURE=false \
  SIGNUP_EMAIL_VERIFY=off TOSS_CLIENT_KEY=test_ck_x TOSS_SECRET_KEY=test_sk_x TOSS_API_BASE=http://127.0.0.1:18091 \
  UPLOAD_DIR="$WORK/uploads" NO_COLOR=1 "$ROOT/backend/target/debug/gageyo-api" > "$WORK/api.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 60); do curl -sf "$BASE/health" > /dev/null && break; sleep 1; done

j() { jq -r "$1"; }
P="$WORK/p.jar"; C="$WORK/c.jar"
PASS="smoke-pass-1234"
P_EMAIL="ui-shop-$RUN@gageyo.test"
C_EMAIL="ui-driver-$RUN@gageyo.test"

# 검사용 이미지 (작은 PNG 두 장)
python3 - "$WORK" <<'PY'
import struct, sys, zlib
def png(path, rgb):
    w, h = 64, 48
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))
    c = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    open(path, "wb").write(b"\x89PNG\r\n\x1a\n" + c(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + c(b"IDAT", zlib.compress(raw)) + c(b"IEND", b""))
png(sys.argv[1] + "/shop.png", (60, 90, 70))
png(sys.argv[1] + "/oil.png", (200, 160, 40))
PY

WID=$(curl -s -c "$P" -b "$P" -H 'Content-Type: application/json' "$BASE/partner/apply" --data "$(jq -n --arg e "$P_EMAIL" --arg p "$PASS" '{email:$e,password:$p,display_name:"박정비",phone:"010-3333-4444",agree_terms:true,
  workshop:{name:"스모크 모터스",categories:["자동차 정비","디테일링세차/광택"],district:"강남구",address:"서울 강남구 테헤란로 10",business_number:"220-81-62517",
  representative:"박정비",phone:"02-555-0000",description:"엔진오일·브레이크·광택 전문. 수입차 정비 경력 15년.",latitude:37.501,longitude:127.037}}')" | j .workshop_id)
curl -s -o /dev/null -X PATCH -H "x-admin-key: $KEY" -H 'Content-Type: application/json' "$BASE/admin/workshops/$WID" --data '{"status":"approved"}'
curl -s -o /dev/null -X PUT -c "$P" -b "$P" -H 'Content-Type: application/json' "$BASE/partner/profile" \
  --data '{"description":"엔진오일·브레이크·광택 전문. 수입차 정비 경력 15년.","phone":"02-555-0000","open_time":"09:00","close_time":"19:00","closed_days":"0","latitude":37.501,"longitude":127.037}'
SHOP_URL=$(curl -s -c "$P" -b "$P" -F "file=@$WORK/shop.png" "$BASE/files" | j .url)
curl -s -o /dev/null -c "$P" -b "$P" -H 'Content-Type: application/json' "$BASE/partner/photos" --data "{\"url\":\"$SHOP_URL\"}"
OIL_URL=$(curl -s -c "$P" -b "$P" -F "file=@$WORK/oil.png" "$BASE/files" | j .url)
curl -s -o /dev/null -c "$P" -b "$P" -H 'Content-Type: application/json' "$BASE/partner/services" --data '{"name":"엔진오일 교환","description":"합성유 4L 포함","price_from":55000,"duration_minutes":40}'
curl -s -o /dev/null -c "$P" -b "$P" -H 'Content-Type: application/json' "$BASE/partner/services" --data '{"name":"브레이크 패드 교체","price_from":90000,"duration_minutes":60}'
curl -s -o /dev/null -c "$P" -b "$P" -H 'Content-Type: application/json' "$BASE/partner/products" --data "{\"name\":\"합성 엔진오일 5W-30 4L\",\"category\":\"엔진오일\",\"description\":\"API SP 등급\",\"price\":42000,\"list_price\":52000,\"stock\":20,\"image_url\":\"$OIL_URL\"}"
curl -s -o /dev/null -X POST -H 'Content-Type: application/json' -c "$C" -b "$C" "$BASE/auth/register" --data "{\"email\":\"$C_EMAIL\",\"password\":\"$PASS\",\"display_name\":\"이운전\",\"phone\":\"010-5555-6666\"}"
curl -s -o /dev/null -c "$C" -b "$C" -H 'Content-Type: application/json' "$BASE/me/vehicles" --data '{"model":"현대 아반떼","model_year":2021,"label":"출퇴근차"}'
QID=$(curl -s -c "$C" -b "$C" -H 'Content-Type: application/json' "$BASE/quotes" --data '{"vehicle":"현대 아반떼 2021","service_category":"자동차 정비","description":"엔진오일 교환과 브레이크 점검 부탁드립니다.","district":"서울 강남구 역삼동","customer_phone":"010-5555-6666"}' | j .id)
curl -s -o /dev/null -c "$P" -b "$P" -H 'Content-Type: application/json' "$BASE/partner/quotes/$QID/offers" --data '{"amount":65000,"description":"합성유 교환 + 브레이크 패드 점검 무료. 약 50분 소요."}'
curl -s -o /dev/null -c "$C" -b "$C" -H 'Content-Type: application/json' "$BASE/quotes" --data '{"vehicle":"기아 K5 2019","service_category":"디테일링세차/광택","description":"광택과 실내 세차","district":"서울 강남구 삼성동","customer_phone":"010-5555-6666"}'
curl -s -o /dev/null -H "x-admin-key: $KEY" -H 'Content-Type: application/json' "$BASE/admin/notices" --data '{"audience":"partner","title":"가게요 파트너 정산일 안내","body":"매월 10일에 지난달 정산금을 보내 드립니다.","pinned":true}'
curl -s -o /dev/null -H 'Content-Type: application/json' "$BASE/support/inquiries" --data '{"name":"비회원","email":"guest@gageyo.test","subject":"예약 변경 문의","body":"예약 시간을 바꿀 수 있나요?"}'

for app in site:4173 console:4174 admin:4175 www:4176; do
  name=${app%%:*}; port=${app##*:}
  (cd "$ROOT/frontend/apps/$name" && npx vite preview --port "$port" --strictPort --host 127.0.0.1 > "$WORK/preview-$name.log" 2>&1) & PIDS+=($!)
done
for port in 4173 4174 4175 4176; do for _ in $(seq 1 30); do curl -sf "http://127.0.0.1:$port/" > /dev/null && break; sleep 1; done; done

OUT="$OUT" CUSTOMER_EMAIL="$C_EMAIL" PARTNER_EMAIL="$P_EMAIL" PASSWORD="$PASS" ADMIN_KEY="$KEY" QUOTE_ID="$QID" node "$ROOT/scripts/ui-smoke.mjs"
STATUS=$?
[ "$STATUS" -ne 0 ] && tail -n 40 "$WORK/api.log"
exit $STATUS
