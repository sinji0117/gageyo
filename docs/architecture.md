# 시스템 구조

```text
브라우저
 ├─ site      고객 탐색·견적·예약·스토어 (React)
 ├─ console   업체 관리 (React)
 ├─ admin     플랫폼 운영 (React)
 └─ www       소개 페이지 (React)
       │ /api
       ▼
gageyo-api (Rust / Axum)
 ├─ workshop 모듈: 업체·서비스·견적·예약
 ├─ commerce 모듈: 상품·주문·재고
 └─ MariaDB: 영구 데이터
       └─ Redis: 이후 세션·검색 캐시·요청 제한에 사용
```

Rust 코드는 `backend/` Cargo workspace로 분리했습니다. 각 도메인 모듈은 독립 router와 상태(`MySqlPool`)를 받아 API 실행기에 연결됩니다. React 화면은 `frontend/` npm workspace이고 `packages/ui`에서 공용 API 요청과 금액 서식을 제공합니다.

## 주요 데이터 경계

- 업체 소유권은 `workshops.owner_user_id`로 연결합니다.
- 견적 요청은 지역과 서비스 종류로 업체에 공개되고 업체별 제안은 `quote_offers`에 보관합니다.
- 예약은 업체와 선택 서비스, 방문 예정 시각을 참조합니다.
- 주문 생성은 트랜잭션 안에서 상품 재고를 잠그고 수량을 차감합니다.

## 미구현 운영 보안

현재 API 시작점에는 공개 읽기/요청 생성 라우트와 개발 CORS 설정이 포함됩니다. 업체 로그인과 권한 확인, 개인정보 열람 정책, 결제 승인·환불, 감사 로그를 붙이기 전에는 실제 고객 데이터나 운영 결제를 연결하지 마세요. 이 항목은 다음 구현 단계입니다.
