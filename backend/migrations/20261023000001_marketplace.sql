CREATE TABLE IF NOT EXISTS users (
  id CHAR(36) PRIMARY KEY, email VARCHAR(254) NOT NULL UNIQUE,
  phone VARCHAR(32) NULL, display_name VARCHAR(100) NOT NULL,
  role ENUM('customer','workshop','admin') NOT NULL DEFAULT 'customer',
  password_hash VARCHAR(255) NULL, email_verified_at DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS workshops (
  id CHAR(36) PRIMARY KEY, owner_user_id CHAR(36) NOT NULL,
  name VARCHAR(160) NOT NULL, slug VARCHAR(120) NOT NULL UNIQUE,
  category VARCHAR(60) NOT NULL, description TEXT NOT NULL,
  phone VARCHAR(32) NOT NULL, address VARCHAR(255) NOT NULL,
  district VARCHAR(100) NOT NULL, latitude DECIMAL(10,7) NULL, longitude DECIMAL(10,7) NULL,
  status ENUM('pending','approved','rejected','suspended') NOT NULL DEFAULT 'pending',
  rating FLOAT NOT NULL DEFAULT 0, review_count INT NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT fk_workshop_owner FOREIGN KEY(owner_user_id) REFERENCES users(id),
  INDEX idx_workshop_search(status, district, category, rating)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS workshop_services (
  id CHAR(36) PRIMARY KEY, workshop_id CHAR(36) NOT NULL,
  name VARCHAR(160) NOT NULL, description TEXT NOT NULL,
  price_from BIGINT UNSIGNED NOT NULL, duration_minutes SMALLINT UNSIGNED NOT NULL DEFAULT 60,
  is_active BOOLEAN NOT NULL DEFAULT TRUE, sort_order INT NOT NULL DEFAULT 0,
  FOREIGN KEY(workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
  INDEX idx_services_active(workshop_id,is_active)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS quote_requests (
  id CHAR(36) PRIMARY KEY, customer_user_id CHAR(36) NULL,
  vehicle VARCHAR(160) NOT NULL, service_category VARCHAR(80) NOT NULL,
  description TEXT NOT NULL, district VARCHAR(100) NOT NULL, customer_phone VARCHAR(32) NOT NULL,
  status ENUM('open','matched','accepted','closed','cancelled') NOT NULL DEFAULT 'open',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_quote_discovery(status,district,service_category,created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS quote_offers (
  id CHAR(36) PRIMARY KEY, quote_id CHAR(36) NOT NULL, workshop_id CHAR(36) NOT NULL,
  amount BIGINT UNSIGNED NOT NULL, description TEXT NOT NULL, valid_until DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(quote_id) REFERENCES quote_requests(id) ON DELETE CASCADE,
  FOREIGN KEY(workshop_id) REFERENCES workshops(id),
  UNIQUE KEY uq_quote_workshop(quote_id,workshop_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS bookings (
  id CHAR(36) PRIMARY KEY, customer_user_id CHAR(36) NULL,
  workshop_id CHAR(36) NOT NULL, service_id CHAR(36) NULL,
  vehicle VARCHAR(160) NOT NULL, starts_at DATETIME NOT NULL,
  customer_name VARCHAR(100) NOT NULL, customer_phone VARCHAR(32) NOT NULL, note TEXT NULL,
  status ENUM('requested','confirmed','completed','cancelled','declined') NOT NULL DEFAULT 'requested',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(workshop_id) REFERENCES workshops(id), FOREIGN KEY(service_id) REFERENCES workshop_services(id),
  INDEX idx_booking_calendar(workshop_id,starts_at,status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS products (
  id CHAR(36) PRIMARY KEY, workshop_id CHAR(36) NOT NULL,
  name VARCHAR(180) NOT NULL, category VARCHAR(80) NOT NULL, description TEXT NOT NULL,
  price BIGINT UNSIGNED NOT NULL, stock INT UNSIGNED NOT NULL DEFAULT 0, image_url VARCHAR(500) NULL,
  status ENUM('draft','active','sold_out','archived') NOT NULL DEFAULT 'draft',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(workshop_id) REFERENCES workshops(id), INDEX idx_product_discovery(status,category,created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS orders (
  id CHAR(36) PRIMARY KEY, customer_user_id CHAR(36) NULL,
  customer_name VARCHAR(100) NOT NULL, customer_phone VARCHAR(32) NOT NULL, address VARCHAR(500) NOT NULL,
  status ENUM('payment_pending','paid','preparing','shipped','delivered','cancelled','refunded') NOT NULL,
  total_amount BIGINT UNSIGNED NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, INDEX idx_order_customer(customer_phone,created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS order_items (
  id CHAR(36) PRIMARY KEY, order_id CHAR(36) NOT NULL, product_id CHAR(36) NOT NULL,
  product_name VARCHAR(180) NOT NULL, unit_price BIGINT UNSIGNED NOT NULL, quantity INT UNSIGNED NOT NULL,
  FOREIGN KEY(order_id) REFERENCES orders(id) ON DELETE CASCADE,
  FOREIGN KEY(product_id) REFERENCES products(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
