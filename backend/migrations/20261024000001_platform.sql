-- 가게요 플랫폼 기능 확장 (bandiuri 기능 이식)
-- 계정 상태·세션 범위, 파트너 입점·정보 변경, 견적 수락, 리뷰, 관심 업체, 내 차량,
-- 결제(토스·무통장)·배송·정산, 이미지, 문의·신고·공지, 작업 기록, 인증 코드·요청 제한
--
-- 주의: 이미 적용된 이전 migration 파일은 고치지 않습니다(sqlx 체크섬). 모든 변경은 이 파일처럼 새 파일로 추가합니다.

-- ---------------------------------------------------------------------------
-- 기존 ENUM·DECIMAL 열을 VARCHAR·DOUBLE 로 바꿈
-- (sqlx 가 ENUM→String, DECIMAL→f64 를 읽지 못해 목록 조회가 실패하던 문제 수정)
-- ---------------------------------------------------------------------------
ALTER TABLE users MODIFY role VARCHAR(20) NOT NULL DEFAULT 'customer';
ALTER TABLE workshops MODIFY status VARCHAR(20) NOT NULL DEFAULT 'pending';
ALTER TABLE workshops MODIFY latitude DOUBLE NULL, MODIFY longitude DOUBLE NULL;
ALTER TABLE quote_requests MODIFY status VARCHAR(20) NOT NULL DEFAULT 'open';
ALTER TABLE bookings MODIFY status VARCHAR(20) NOT NULL DEFAULT 'requested';
ALTER TABLE products MODIFY status VARCHAR(20) NOT NULL DEFAULT 'draft';
ALTER TABLE orders MODIFY status VARCHAR(20) NOT NULL DEFAULT 'payment_pending';

-- ---------------------------------------------------------------------------
-- 계정
-- ---------------------------------------------------------------------------
ALTER TABLE users
  ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'active' AFTER role,
  ADD COLUMN IF NOT EXISTS last_login_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS last_login_ip VARCHAR(64) NULL,
  ADD COLUMN IF NOT EXISTS totp_secret VARCHAR(255) NULL,
  ADD COLUMN IF NOT EXISTS totp_last_step BIGINT NULL,
  ADD COLUMN IF NOT EXISTS withdrawn_at DATETIME NULL;

ALTER TABLE auth_sessions
  ADD COLUMN IF NOT EXISTS scope VARCHAR(16) NOT NULL DEFAULT 'customer' AFTER user_id,
  ADD COLUMN IF NOT EXISTS ip VARCHAR(64) NULL,
  ADD COLUMN IF NOT EXISTS user_agent VARCHAR(255) NULL;

CREATE TABLE IF NOT EXISTS password_resets (
  token_hash CHAR(64) PRIMARY KEY,
  user_id CHAR(36) NOT NULL,
  scope VARCHAR(16) NOT NULL,
  expires_at DATETIME NOT NULL,
  used_at DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT fk_password_resets_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
  INDEX idx_password_resets_user(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 회원가입 이메일 인증 코드 (코드 원문이 아니라 해시만 저장)
CREATE TABLE IF NOT EXISTS email_codes (
  scope VARCHAR(20) NOT NULL,
  email VARCHAR(254) NOT NULL,
  code_hash CHAR(64) NOT NULL,
  tries INT NOT NULL DEFAULT 0,
  expires_at DATETIME NOT NULL,
  sent_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY(scope, email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 요청 횟수 제한 (로그인 잠금·코드 발송·문의 등록)
CREATE TABLE IF NOT EXISTS rate_limits (
  bucket VARCHAR(191) PRIMARY KEY,
  hits INT NOT NULL DEFAULT 0,
  window_start DATETIME NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 내 차량
CREATE TABLE IF NOT EXISTS vehicles (
  id CHAR(36) PRIMARY KEY,
  user_id CHAR(36) NOT NULL,
  label VARCHAR(60) NOT NULL,
  model VARCHAR(120) NOT NULL,
  model_year SMALLINT NULL,
  plate VARCHAR(20) NULL,
  mileage INT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT fk_vehicles_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
  INDEX idx_vehicles_user(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- 업체
-- ---------------------------------------------------------------------------
ALTER TABLE workshops
  ADD COLUMN IF NOT EXISTS business_number VARCHAR(20) NULL,
  ADD COLUMN IF NOT EXISTS representative VARCHAR(100) NULL,
  ADD COLUMN IF NOT EXISTS open_time TIME NULL,
  ADD COLUMN IF NOT EXISTS close_time TIME NULL,
  ADD COLUMN IF NOT EXISTS closed_days VARCHAR(20) NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS cover_image_url VARCHAR(500) NULL,
  ADD COLUMN IF NOT EXISTS reject_reason VARCHAR(500) NULL,
  ADD COLUMN IF NOT EXISTS approved_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS updated_at DATETIME NULL ON UPDATE CURRENT_TIMESTAMP;

-- 업체가 하는 서비스 분야 (여러 개). 견적 요청 매칭에 씀
CREATE TABLE IF NOT EXISTS workshop_categories (
  workshop_id CHAR(36) NOT NULL,
  category VARCHAR(80) NOT NULL,
  PRIMARY KEY(workshop_id, category),
  CONSTRAINT fk_workshop_categories_workshop FOREIGN KEY(workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
  INDEX idx_workshop_categories_category(category)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
INSERT IGNORE INTO workshop_categories (workshop_id, category) SELECT id, category FROM workshops;

CREATE TABLE IF NOT EXISTS workshop_photos (
  id CHAR(36) PRIMARY KEY,
  workshop_id CHAR(36) NOT NULL,
  url VARCHAR(500) NOT NULL,
  sort_order INT NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT fk_workshop_photos_workshop FOREIGN KEY(workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
  INDEX idx_workshop_photos(workshop_id, sort_order)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 승인된 업체의 상호·업종·주소·사업자 정보 변경은 운영자 승인 뒤 반영
CREATE TABLE IF NOT EXISTS workshop_change_requests (
  id CHAR(36) PRIMARY KEY,
  workshop_id CHAR(36) NOT NULL,
  requested_by CHAR(36) NULL,
  payload LONGTEXT NOT NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'pending',
  reason VARCHAR(500) NULL,
  decided_by VARCHAR(254) NULL,
  decided_at DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT fk_change_requests_workshop FOREIGN KEY(workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
  INDEX idx_change_requests_status(status, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- 견적·예약·리뷰·관심 업체
-- ---------------------------------------------------------------------------
ALTER TABLE quote_requests
  ADD COLUMN IF NOT EXISTS customer_name VARCHAR(100) NULL,
  ADD COLUMN IF NOT EXISTS accepted_offer_id CHAR(36) NULL,
  ADD COLUMN IF NOT EXISTS updated_at DATETIME NULL ON UPDATE CURRENT_TIMESTAMP,
  ADD INDEX IF NOT EXISTS idx_quote_customer(customer_user_id, created_at);

ALTER TABLE quote_offers
  ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'sent';

ALTER TABLE bookings
  ADD COLUMN IF NOT EXISTS quote_offer_id CHAR(36) NULL,
  ADD COLUMN IF NOT EXISTS order_id CHAR(36) NULL,
  ADD COLUMN IF NOT EXISTS decline_reason VARCHAR(300) NULL,
  ADD COLUMN IF NOT EXISTS updated_at DATETIME NULL ON UPDATE CURRENT_TIMESTAMP,
  ADD INDEX IF NOT EXISTS idx_booking_customer(customer_user_id, starts_at);

CREATE TABLE IF NOT EXISTS reviews (
  id CHAR(36) PRIMARY KEY,
  workshop_id CHAR(36) NOT NULL,
  booking_id CHAR(36) NULL UNIQUE,
  user_id CHAR(36) NOT NULL,
  rating TINYINT NOT NULL,
  content TEXT NOT NULL,
  reply TEXT NULL,
  replied_at DATETIME NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'visible',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NULL ON UPDATE CURRENT_TIMESTAMP,
  CONSTRAINT fk_reviews_workshop FOREIGN KEY(workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
  CONSTRAINT fk_reviews_user FOREIGN KEY(user_id) REFERENCES users(id),
  INDEX idx_reviews_workshop(workshop_id, status, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS favorites (
  user_id CHAR(36) NOT NULL,
  workshop_id CHAR(36) NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY(user_id, workshop_id),
  CONSTRAINT fk_favorites_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_favorites_workshop FOREIGN KEY(workshop_id) REFERENCES workshops(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- 상품·주문·결제·배송·정산
-- ---------------------------------------------------------------------------
ALTER TABLE products
  ADD COLUMN IF NOT EXISTS list_price BIGINT UNSIGNED NULL,
  ADD COLUMN IF NOT EXISTS updated_at DATETIME NULL ON UPDATE CURRENT_TIMESTAMP;

ALTER TABLE orders
  ADD COLUMN IF NOT EXISTS email VARCHAR(254) NULL,
  ADD COLUMN IF NOT EXISTS fulfillment VARCHAR(20) NOT NULL DEFAULT 'delivery',
  ADD COLUMN IF NOT EXISTS visit_workshop_id CHAR(36) NULL,
  ADD COLUMN IF NOT EXISTS visit_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS vehicle VARCHAR(160) NULL,
  ADD COLUMN IF NOT EXISTS memo VARCHAR(500) NULL,
  ADD COLUMN IF NOT EXISTS payment_method VARCHAR(20) NOT NULL DEFAULT 'none',
  ADD COLUMN IF NOT EXISTS payment_key VARCHAR(200) NULL,
  ADD COLUMN IF NOT EXISTS pay_detail VARCHAR(60) NULL,
  ADD COLUMN IF NOT EXISTS depositor_name VARCHAR(60) NULL,
  ADD COLUMN IF NOT EXISTS paid_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS cancelled_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS cancel_reason VARCHAR(300) NULL,
  ADD COLUMN IF NOT EXISTS refunded_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS updated_at DATETIME NULL ON UPDATE CURRENT_TIMESTAMP,
  ADD INDEX IF NOT EXISTS idx_order_user(customer_user_id, created_at),
  ADD INDEX IF NOT EXISTS idx_order_status(status, created_at);

ALTER TABLE order_items
  ADD COLUMN IF NOT EXISTS workshop_id CHAR(36) NULL,
  ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'pending',
  ADD COLUMN IF NOT EXISTS carrier VARCHAR(30) NULL,
  ADD COLUMN IF NOT EXISTS tracking_no VARCHAR(40) NULL,
  ADD COLUMN IF NOT EXISTS shipped_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS delivered_at DATETIME NULL,
  ADD COLUMN IF NOT EXISTS fee_amount BIGINT UNSIGNED NOT NULL DEFAULT 0,
  ADD INDEX IF NOT EXISTS idx_order_items_workshop(workshop_id, status);
UPDATE order_items oi JOIN products p ON p.id = oi.product_id SET oi.workshop_id = p.workshop_id WHERE oi.workshop_id IS NULL;

-- 운영자가 화면에서 바꾸는 설정 (수수료율·무통장 입금 계좌 등)
CREATE TABLE IF NOT EXISTS platform_settings (
  k VARCHAR(60) PRIMARY KEY,
  v TEXT NOT NULL,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 월 정산 지급 기록 (금액은 지급 시점에 고정)
CREATE TABLE IF NOT EXISTS settlements (
  id CHAR(36) PRIMARY KEY,
  workshop_id CHAR(36) NOT NULL,
  period CHAR(7) NOT NULL,
  gross BIGINT UNSIGNED NOT NULL,
  fee BIGINT UNSIGNED NOT NULL,
  net BIGINT UNSIGNED NOT NULL,
  memo VARCHAR(300) NULL,
  paid_by VARCHAR(254) NULL,
  paid_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY uq_settlement(workshop_id, period),
  CONSTRAINT fk_settlements_workshop FOREIGN KEY(workshop_id) REFERENCES workshops(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- 이미지
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS files (
  id CHAR(36) PRIMARY KEY,
  path VARCHAR(255) NOT NULL UNIQUE,
  mime VARCHAR(40) NOT NULL,
  bytes INT UNSIGNED NOT NULL,
  width INT UNSIGNED NOT NULL,
  height INT UNSIGNED NOT NULL,
  original_name VARCHAR(200) NOT NULL DEFAULT '',
  uploader_user_id CHAR(36) NULL,
  workshop_id CHAR(36) NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_files_workshop(workshop_id, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- 문의·신고·공지·작업 기록
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS inquiries (
  id CHAR(36) PRIMARY KEY,
  kind VARCHAR(20) NOT NULL,
  user_id CHAR(36) NULL,
  workshop_id CHAR(36) NULL,
  name VARCHAR(100) NOT NULL,
  email VARCHAR(254) NULL,
  subject VARCHAR(200) NOT NULL,
  target_type VARCHAR(20) NULL,
  target_id CHAR(36) NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'open',
  token_hash CHAR(64) NULL,
  ip VARCHAR(64) NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_message_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_inquiries_status(kind, status, last_message_at),
  INDEX idx_inquiries_user(user_id),
  INDEX idx_inquiries_workshop(workshop_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS inquiry_messages (
  id CHAR(36) PRIMARY KEY,
  inquiry_id CHAR(36) NOT NULL,
  author VARCHAR(10) NOT NULL,
  body TEXT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT fk_inquiry_messages FOREIGN KEY(inquiry_id) REFERENCES inquiries(id) ON DELETE CASCADE,
  INDEX idx_inquiry_messages(inquiry_id, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS notices (
  id CHAR(36) PRIMARY KEY,
  audience VARCHAR(20) NOT NULL DEFAULT 'all',
  title VARCHAR(200) NOT NULL,
  body TEXT NOT NULL,
  pinned BOOLEAN NOT NULL DEFAULT FALSE,
  is_published BOOLEAN NOT NULL DEFAULT TRUE,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NULL ON UPDATE CURRENT_TIMESTAMP,
  INDEX idx_notices(audience, is_published, pinned, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS admin_audit (
  id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  actor VARCHAR(254) NOT NULL,
  action VARCHAR(60) NOT NULL,
  target_type VARCHAR(30) NOT NULL,
  target_id VARCHAR(64) NOT NULL,
  detail TEXT NULL,
  ip VARCHAR(64) NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_admin_audit(created_at),
  INDEX idx_admin_audit_target(target_type, target_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
