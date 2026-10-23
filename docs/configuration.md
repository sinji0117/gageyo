[← 문서 목차](README.md)

# 환경 변수

API 서버(`gageyo-api`)는 환경 변수로만 설정합니다. 로컬에서는 저장소 루트의 `.env` 를 읽고,
Rpanel 에서는 API 앱의 **환경 변수** 탭에 넣습니다. 값을 바꾸면 API 앱을 다시 시작해야 적용됩니다.

비밀값(`ADMIN_API_KEY`, `DATA_KEY`, `SMTP_URL`, `TOSS_SECRET_KEY`)은 저장소·문서·채팅에 적지 마세요.

## 꼭 필요한 값

| 이름 | 예시 | 설명 |
| --- | --- | --- |
| `DATABASE_URL` | `mysql://gageyo_user:비밀번호@127.0.0.1:3306/gageyo_db` | MariaDB 주소. Rpanel 은 DB 를 연결하면 자동으로 넣어 줍니다. |
| `ADMIN_EMAILS` | `ops@gageyo.com, boss@gageyo.com` | 운영자 화면에 로그인할 수 있는 이메일 (쉼표로 구분). 여기서 빼면 다음 요청부터 바로 막힙니다. |

## 운영에 권장하는 값

| 이름 | 예시 | 설명 |
| --- | --- | --- |
| `DATA_KEY` | `openssl rand -hex 32` 로 만든 64자 | 운영자 OTP 비밀값 암호화와 인증 코드 해시에 쓰는 키. 없으면 `DATABASE_URL` 에서 만들지만, DB 주소·비밀번호를 바꾸면 OTP 를 다시 등록해야 하므로 따로 두세요. **한 번 정하면 바꾸지 마세요.** |
| `SMTP_URL` | `smtps://아이디:앱비밀번호@smtp.gmail.com:465` | 메일 서버. 없으면 메일을 보내지 않고 API 로그에만 남깁니다 (가입 코드·비밀번호 설정 링크도 로그에). |
| `MAIL_FROM` | `가게요 <no-reply@gageyo.com>` | 보내는 사람. `SMTP_URL` 이 있으면 꼭 필요합니다. |
| `OPS_EMAIL` | `ops@gageyo.com` | 새 입점 신청·변경 요청·문의 알림을 받을 주소. 비우면 `ADMIN_EMAILS` 모두에게 보냅니다. |
| `TOSS_CLIENT_KEY` | `test_ck_…` / `live_ck_…` | 토스페이먼츠 결제창 클라이언트 키. 두 키가 모두 있어야 카드·간편결제가 켜집니다. |
| `TOSS_SECRET_KEY` | `test_sk_…` / `live_sk_…` | 토스페이먼츠 시크릿 키 (서버에서만 씀). 테스트·실제 키를 섞으면 서버가 켜지지 않습니다. |
| `APP_URL` | `https://app.gageyo.com` | 고객 화면 주소 (메일 링크·사이트맵에 씀) |
| `PARTNER_URL` | `https://partner.gageyo.com` | 파트너 센터 주소 |
| `ADMIN_URL` | `https://admin.gageyo.com` | 운영자 화면 주소 |

## 선택 값

| 이름 | 기본값 | 설명 |
| --- | --- | --- |
| `PORT` / `BIND_ADDR` | `127.0.0.1:8081` | 듣는 주소. Rpanel 은 `PORT` 를 넣어 줍니다 (`127.0.0.1:$PORT`). |
| `ADMIN_API_KEY` | (없음) | 스크립트·비상용 운영자 키 (`x-admin-key` 헤더). 비우면 키 접속을 막습니다. 평소에는 운영자 계정 로그인을 쓰세요. |
| `COOKIE_SECURE` | `true` | 로그인 쿠키에 `Secure` 를 붙일지. https 가 아닌 로컬 개발에서만 `false`. |
| `CORS_ORIGINS` | (없음) | 다른 주소에서 API 를 부를 때 허용할 origin (쉼표). 화면이 같은 주소의 `/api` 를 프록시하면 필요 없습니다. |
| `SIGNUP_EMAIL_VERIFY` | `auto` | 가입·입점 신청 이메일 코드 인증. `auto` = 메일 설정이 있으면 켬, `on`, `off`. |
| `UPLOAD_DIR` | Rpanel `data/uploads` 자동 감지, 아니면 `data/uploads` | 업로드 이미지 저장 폴더. 배포해도 지워지지 않는 곳이어야 합니다. |
| `UPLOAD_MAX_MB` | `10` | 이미지 한 장 최대 크기 (MB) |
| `TOSS_API_BASE` | `https://api.tosspayments.com` | 토스 API 주소 (테스트용 모의 서버를 가리킬 때만) |
| `RUST_LOG` | `info,gageyo_api=debug` | 로그 단계 |

## 화면(프런트엔드) 빌드 값

| 이름 | 설명 |
| --- | --- |
| `VITE_API_URL` | 화면이 부를 API 주소. 기본은 같은 주소의 `/api` 라 보통 비워 둡니다. |

## 운영자 화면에서 바꾸는 값

수수료율, 무통장 입금 계좌, 입금 기한, 고객센터 전화·이메일, 사업자 정보는 환경 변수가 아니라
운영자 화면 **결제·운영 설정**에서 바꿉니다 (DB `platform_settings` 표, 바꾸면 작업 기록에 남음).

## 쓰지 않는 값

- `REDIS_URL`: 가게요는 세션·인증 코드·요청 제한을 모두 MariaDB 로 처리합니다. Redis 를 만들거나 연결하지 않아도 됩니다.
- `MONGODB_URI`: 로그 저장소 연결은 아직 없습니다. 로그는 표준 출력(Rpanel 앱 로그)으로만 나갑니다.
