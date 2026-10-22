CREATE TABLE IF NOT EXISTS service_categories (
  id CHAR(36) PRIMARY KEY,
  group_name VARCHAR(60) NOT NULL,
  name VARCHAR(80) NOT NULL,
  slug VARCHAR(100) NOT NULL UNIQUE,
  sort_order INT NOT NULL DEFAULT 0,
  group_sort INT NOT NULL DEFAULT 2,
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  UNIQUE KEY uq_service_category_group_name(group_name,name),
  INDEX idx_service_category_public(is_active,group_sort,sort_order)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT IGNORE INTO service_categories (id,group_name,name,slug,sort_order,group_sort) VALUES
('6f100000-0000-4000-8000-000000000001','차량 관리','디테일링세차/광택','category-detailing-wash',10,0),
('6f100000-0000-4000-8000-000000000002','차량 관리','스팀 세차','category-steam-wash',20,0),
('6f100000-0000-4000-8000-000000000003','차량 관리','오토바이 튜닝','category-motorcycle-tuning',30,0),
('6f100000-0000-4000-8000-000000000004','차량 관리','자동차 썬팅','category-window-tint',40,0),
('6f100000-0000-4000-8000-000000000005','차량 관리','자동차 정비','category-auto-repair',50,0),
('6f100000-0000-4000-8000-000000000006','차량 관리','전기차충전기 설치 및 수리','category-ev-charger',60,0),
('6f100000-0000-4000-8000-000000000007','차량 관리','블랙박스/내비게이션 설치 및 수리','category-dashcam-navigation',70,0),
('6f100000-0000-4000-8000-000000000008','차량 관리','오토바이 정비 및 수리','category-motorcycle-repair',80,0),
('6f100000-0000-4000-8000-000000000009','차량 관리','자동차 랩핑','category-car-wrapping',90,0),
('6f100000-0000-4000-8000-000000000010','차량 관리','자동차 외부수리','category-body-repair',100,0),
('6f100000-0000-4000-8000-000000000011','차량 관리','자동차 튜닝','category-car-tuning',110,0),
('6f100000-0000-4000-8000-000000000012','차량 관리','출장 세차','category-mobile-wash',120,0),
('6f100000-0000-4000-8000-000000000013','자동차 매매','신차 구매','category-new-car',10,1),
('6f100000-0000-4000-8000-000000000014','자동차 매매','자동차 구매동행','category-car-buying-assist',20,1),
('6f100000-0000-4000-8000-000000000015','자동차 매매','자동차 리스/렌트','category-lease-rental',30,1),
('6f100000-0000-4000-8000-000000000016','자동차 매매','중고차 구매','category-used-car-buy',40,1),
('6f100000-0000-4000-8000-000000000017','자동차 매매','중고차 판매','category-used-car-sell',50,1),
('6f100000-0000-4000-8000-000000000018','자동차 매매','캠핑카 제작','category-camper-build',60,1);

