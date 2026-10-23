[← 문서 목차](README.md)

# 개발 규칙

## API

- 오류는 `AppError` 로 돌려줍니다 → `{"error": "사람이 읽을 문구", "code": "bad_request"}`. 화면은 `error` 를 그대로 보여 줍니다.
- 입력은 `gageyo_core::util` 의 `req_text`·`opt_text`·`req_line`·`normalize_phone` 등으로 검사하고 길이 제한을 둡니다.
- 금액은 원 단위 정수(`u64`), 합계는 `CAST(SUM(…) AS SIGNED)` 로 읽습니다.
- 시각은 한국 시간 `NaiveDateTime` (DB 연결 시간대 +09:00). 지금은 `kst_now()`.
- 상태 값은 `VARCHAR` + 앱에서 허용 목록 검사. **`ENUM` 을 새로 쓰지 않습니다.**

## 권한

- 추출기로만 권한을 얻습니다: `Customer`·`MaybeCustomer`·`Partner`·`ApprovedPartner`·`Admin`.
- 파트너 API 의 모든 쿼리는 `workshop_id = 로그인한 업체` 조건을 넣습니다.
- 남의 데이터는 403 이 아니라 **404** 로 숨깁니다.
- 비회원 데이터는 UUID + 연락처 숫자 일치로 확인합니다.
- 운영자가 상태를 바꾸는 API 는 `audit::log` 로 기록하고, 거절·정지·숨김에는 사유를 받습니다.

## DB

- 새 표·열은 `backend/migrations/` 에 **새 파일**로 추가합니다. 이미 배포한 파일은 고치지 않습니다 (체크섬이 바뀌어 서버가 켜지지 않음).
- `ADD COLUMN IF NOT EXISTS` 처럼 다시 실행해도 안전하게 씁니다.

## 메일

- 알림은 모듈의 `notify.rs` 에 모읍니다. 실패해도 요청은 성공해야 하므로 `mailer.send` 는 기다리지 않습니다.
- 받는 사람이 없거나 메일 설정이 없으면 로그만 남습니다.

## 화면

- 공용 코드는 `@gageyo/ui` (`api()`, 형식 함수, `Modal`·`Field`·`Badge`·`AddressInput` …). 앱끼리 서로 가져다 쓰지 않습니다.
- 서버가 보낸 오류 문구를 그대로 토스트로 보여 줍니다. 화면에서 같은 검사를 반복하지 않아도 됩니다(편의용 검사는 OK).
- 브라우저 저장소(`localStorage`)는 장바구니 같은 편의용만, `try/catch` 로 감쌉니다.

## 검사

- 규칙(순수 함수)은 단위 테스트, 흐름은 `scripts/e2e.sh` 에 한 줄씩 추가합니다.
- 화면을 바꾸면 `scripts/ui-smoke.sh` 스크린샷을 확인합니다. → [테스트](testing.md)
