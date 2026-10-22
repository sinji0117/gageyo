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
| POST | `/quotes/{id}/offers` | 업체 견적 제안 생성 |
| POST | `/bookings` | 예약 요청 생성 |
| GET | `/products?q=&category=` | 판매 상품 조회 |
| GET | `/products/{id}` | 상품 상세 |
| POST | `/orders` | 상품 주문과 재고 차감 |
| GET | `/orders/{id}` | 주문 상태 조회 |

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

인증과 소유자 권한 검사, 페이징, 위치 반경 검색, 요청 검증은 후속 구현이 필요합니다.
