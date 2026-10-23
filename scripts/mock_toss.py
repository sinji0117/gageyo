"""토스페이먼츠 결제 API 모의 서버 (종단 검사용).

- POST /v1/payments/confirm        : 시크릿 키 test_sk_x 만 받음, paymentKey=reject-me 는 카드 거절
- GET  /v1/payments/{paymentKey}   : 이미 승인된 결제 조회
- GET  /v1/payments/orders/{orderId} : 주문번호로 결제 조회 (없으면 404)
- POST /v1/payments/{key}/cancel   : 취소 (Idempotency-Key 필요)

실행: python3 scripts/mock_toss.py 18090
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

AUTH = "Basic dGVzdF9za194Og=="  # base64("test_sk_x:")
PAID = {}
ORDERS = {}  # orderId -> paymentKey


class Handler(BaseHTTPRequestHandler):
    def _send(self, code, body):
        data = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n) or b"{}")

    def do_GET(self):
        if self.headers.get("Authorization") != AUTH:
            return self._send(401, {"code": "UNAUTHORIZED_KEY", "message": "인증되지 않은 시크릿 키"})
        key = self.path.rsplit("/", 1)[-1]
        if self.path.startswith("/v1/payments/orders/"):
            key = ORDERS.get(key)
            if key is None:
                return self._send(404, {"code": "NOT_FOUND_PAYMENT", "message": "결제 없음"})
            return self._send(200, {"status": "DONE", "paymentKey": key, "totalAmount": PAID[key], "method": "카드"})
        if key in PAID:
            return self._send(200, {"status": "DONE", "totalAmount": PAID[key], "method": "카드"})
        return self._send(404, {"code": "NOT_FOUND_PAYMENT", "message": "결제 없음"})

    def do_POST(self):
        if self.headers.get("Authorization") != AUTH:
            return self._send(401, {"code": "UNAUTHORIZED_KEY", "message": "인증되지 않은 시크릿 키"})
        body = self._body()
        if self.path == "/v1/payments/confirm":
            key = body.get("paymentKey")
            if key == "reject-me":
                return self._send(400, {"code": "REJECT_CARD_COMPANY", "message": "카드사에서 거절했습니다"})
            if key in PAID:
                return self._send(400, {"code": "ALREADY_PROCESSED_PAYMENT", "message": "이미 처리된 결제"})
            PAID[key] = body.get("amount")
            ORDERS[body.get("orderId")] = key
            return self._send(200, {"status": "DONE", "totalAmount": body.get("amount"), "method": "간편결제", "easyPay": {"provider": "토스페이"}})
        if self.path.endswith("/cancel"):
            if not self.headers.get("Idempotency-Key"):
                return self._send(400, {"code": "INVALID_REQUEST", "message": "멱등 키 없음"})
            return self._send(200, {"status": "CANCELED"})
        return self._send(404, {"code": "NOT_FOUND", "message": "없는 주소"})

    def log_message(self, fmt, *args):
        sys.stderr.write("mock-toss: " + (fmt % args) + "\n")


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18090
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
