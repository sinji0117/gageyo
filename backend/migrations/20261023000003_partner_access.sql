CREATE TABLE IF NOT EXISTS workshop_access_tokens (
  token_hash CHAR(64) PRIMARY KEY,
  workshop_id CHAR(36) NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
  INDEX idx_workshop_access_tokens_workshop(workshop_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

