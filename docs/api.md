# API 초안

기본 주소는 `/api`입니다. 요청과 응답은 JSON이며, 금액 단위는 원입니다.

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/health` | API 상태 |
| GET | `/workshops?q=&category=&district=` | 승인된 주변 업체 검색 |
| GET | `/workshops/{id}` | 업체 상세 |
| GET | `/workshops/{id}/services` | 업체 서비스 목록 |
| POST | `/quotes` | 견적 요청 생성 |
| GET | `/quotes/{id}/offers` | 해당 요청의 업체 제안 목록 |
| POST | `/quotes/{id}/offers` | 파트너 키로 견적 제안 생성 |
| POST | `/bookings` | 예약 요청 생성 |
| GET | `/products?q=&category=` | 판매 상품 조회 |
| GET | `/products/{id}` | 상품 상세 |
| POST | `/orders` | 상품 주문과 재고 차감 |
| GET | `/orders/{id}` | 주문 상태 조회 |
| GET | `/partner/me` | 파트너 키에 연결된 업체 확인 |
| GET | `/partner/quotes` | 지역·업종에 맞는 미응답 견적 요청 |
| GET/PATCH | `/partner/bookings[/{id}]` | 파트너 예약 조회 및 상태 갱신 |
| GET/POST/PATCH | `/partner/products[/{id}]` | 파트너 상품 조회·등록·수정 |
| GET | `/partner/orders` | 해당 파트너 상품의 주문 항목 |
| GET | `/admin/workshops`, `/admin/quotes`, `/admin/bookings` | 운영자 데이터 조회 |
| GET/PATCH | `/admin/products[/{id}]`, `/admin/orders[/{id}]` | 운영자 상품·주문 조회 및 상태 갱신 |
| POST | `/admin/workshops/{id}/partner-key` | 파트너 키 발급·교체 |

운영자 API는 `ADMIN_API_KEY` 환경변수와 `x-admin-key` 헤더를 사용합니다. 파트너 API는 업체별 `x-partner-key` 헤더를 사용하며 키 원문은 발급 시 한 번만 반환되고 DB에는 SHA-256 해시로 저장됩니다. 운영자 화면의 업체 관리에서 승인된 업체의 파트너 키를 발급해 업체에 안전하게 전달하세요. 재발급하면 이전 키는 즉시 무효가 됩니다.

견적 요청 예시:

```json
{
  "vehicle": "현대 아반떼 2021년식",
  "service_category": "정비",
  "description": "엔진오일 교환과 기본 점검",
  "district": "서울 강남구 역삼동",
  "customer_phone": "010-0000-0000"
}
```

고객 요청과 주문 조회는 로그인 없이 연락처와 주문/요청 UUID를 이용하는 초기 버전입니다. 회원 계정·비밀번호 로그인, SMS 본인 확인, 지도 좌표 기반 반경 검색, 결제 대행사 연동은 아직 구현되지 않았습니다. 주문 접수는 결제가 되었다는 뜻이 아니며, `payment_pending` 상태로 저장됩니다.
