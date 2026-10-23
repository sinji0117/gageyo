//! 상품: 고객 목록·상세, 파트너 등록·수정·숨김, 운영자 검수(판매 중지).
//!
//! 상태: `draft`(준비 중) · `active`(판매 중) · `sold_out`(품절, 재고 0이면 자동) · `archived`(파트너가 숨김) · `blocked`(운영자 판매 중지)
//!
//! 예전에는 파트너가 상품을 수정하면 상태가 무조건 `active` 로 바뀌어, 운영자가 숨긴 상품이 다시 팔리는 문제가 있었습니다.
//! 이제 운영자가 막은(`blocked`) 상품은 파트너가 다시 열 수 없습니다.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::{Admin, Partner},
    files::check_image_url,
    util::{like_term, new_id, req_line, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/products", get(list_products))
        .route("/products/categories", get(product_categories))
        .route("/products/{id}", get(product_detail))
        .route("/partner/products", get(partner_products).post(create_product))
        .route("/partner/products/{id}", patch(update_product).delete(delete_product))
        .route("/admin/products", get(admin_products))
        .route("/admin/products/{id}", patch(admin_set_status))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Product {
    pub id: String,
    pub workshop_id: String,
    pub workshop_name: String,
    pub workshop_district: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub price: u64,
    pub list_price: Option<u64>,
    pub stock: u32,
    pub image_url: Option<String>,
    pub status: String,
}

const PRODUCT_SQL: &str = "SELECT p.id, p.workshop_id, w.name AS workshop_name, w.district AS workshop_district, p.name, p.category, \
    p.description, p.price, p.list_price, p.stock, p.image_url, p.status FROM products p JOIN workshops w ON w.id = p.workshop_id";

#[derive(Debug, Deserialize)]
pub struct ProductQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub workshop_id: Option<String>,
    /// recent(기본) · price_asc · price_desc
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

/// 고객 화면: 승인된 업체의 판매 중·품절 상품
async fn list_products(State(s): State<AppState>, Query(q): Query<ProductQuery>) -> AppResult<Json<serde_json::Value>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(24, 60);
    let term = like_term(q.q.as_deref());
    let category = q.category.as_deref().map(str::trim).filter(|c| !c.is_empty() && *c != "전체");
    let order = match q.sort.as_deref() {
        Some("price_asc") => "p.price ASC",
        Some("price_desc") => "p.price DESC",
        _ => "p.created_at DESC",
    };
    let mut rows = sqlx::query_as::<_, Product>(&format!(
        "{PRODUCT_SQL} WHERE p.status IN ('active','sold_out') AND w.status = 'approved' \
         AND (? IS NULL OR p.category = ?) AND (? IS NULL OR p.workshop_id = ?) \
         AND (? IS NULL OR p.name LIKE CONCAT('%', ?, '%') OR p.category LIKE CONCAT('%', ?, '%')) \
         ORDER BY (p.status = 'sold_out'), {order} LIMIT ? OFFSET ?"
    ))
    .bind(category)
    .bind(category)
    .bind(&q.workshop_id)
    .bind(&q.workshop_id)
    .bind(&term)
    .bind(&term)
    .bind(&term)
    .bind(limit + 1)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    let has_more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);
    Ok(Json(serde_json::json!({ "items": rows, "has_more": has_more })))
}

async fn product_categories(State(s): State<AppState>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT p.category, COUNT(*) FROM products p JOIN workshops w ON w.id = p.workshop_id \
         WHERE p.status IN ('active','sold_out') AND w.status = 'approved' GROUP BY p.category ORDER BY COUNT(*) DESC, p.category",
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(c, n)| serde_json::json!({"name": c, "count": n})).collect()))
}

async fn product_detail(State(s): State<AppState>, Path(id): Path<String>) -> AppResult<Json<Product>> {
    let p = sqlx::query_as::<_, Product>(&format!(
        "{PRODUCT_SQL} WHERE p.id = ? AND p.status IN ('active','sold_out') AND w.status = 'approved'"
    ))
    .bind(&id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| AppError::not_found("상품을 찾을 수 없습니다"))?;
    Ok(Json(p))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PartnerProduct {
    pub id: String,
    pub workshop_id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub price: u64,
    pub list_price: Option<u64>,
    pub stock: u32,
    pub image_url: Option<String>,
    pub status: String,
    pub sold: i64,
    pub created_at: NaiveDateTime,
}

const PARTNER_PRODUCT_SQL: &str = "SELECT p.id, p.workshop_id, p.name, p.category, p.description, p.price, p.list_price, p.stock, \
    p.image_url, p.status, CAST(COALESCE((SELECT SUM(oi.quantity) FROM order_items oi JOIN orders o ON o.id = oi.order_id \
      WHERE oi.product_id = p.id AND oi.status <> 'cancelled' AND o.status NOT IN ('payment_pending','cancelled','refunded')), 0) AS SIGNED) AS sold, \
    p.created_at FROM products p";

async fn partner_products(State(s): State<AppState>, p: Partner) -> AppResult<Json<Vec<PartnerProduct>>> {
    Ok(Json(
        sqlx::query_as::<_, PartnerProduct>(&format!("{PARTNER_PRODUCT_SQL} WHERE p.workshop_id = ? ORDER BY p.created_at DESC"))
            .bind(&p.workshop_id)
            .fetch_all(&s.db)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct ProductInput {
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub price: u64,
    pub list_price: Option<u64>,
    pub stock: u32,
    pub image_url: Option<String>,
    /// draft · active · archived (비우면 active)
    pub status: Option<String>,
}

struct CleanProduct {
    name: String,
    category: String,
    description: String,
    list_price: Option<u64>,
    image_url: Option<String>,
}

fn check_product(b: &ProductInput) -> AppResult<CleanProduct> {
    let name = req_line(&b.name, "상품명", 180)?;
    let category = req_line(&b.category, "분류", 80)?;
    let description = b.description.as_deref().unwrap_or("").trim().to_string();
    if description.chars().count() > 5000 {
        return Err(AppError::bad("상품 설명은 5000자 이내로 입력해 주세요"));
    }
    if b.price == 0 || b.price > 100_000_000 {
        return Err(AppError::bad("판매가를 확인해 주세요"));
    }
    let list_price = b.list_price.filter(|lp| *lp > 0);
    if let Some(lp) = list_price {
        if lp < b.price {
            return Err(AppError::bad("정가는 판매가보다 같거나 커야 합니다"));
        }
    }
    if b.stock > 100_000 {
        return Err(AppError::bad("재고는 100,000개까지 입력할 수 있습니다"));
    }
    Ok(CleanProduct { name, category, description, list_price, image_url: check_image_url(b.image_url.as_deref())? })
}

/// 파트너가 고른 상태 + 재고로 저장할 상태 계산
pub fn partner_status(requested: Option<&str>, stock: u32) -> AppResult<&'static str> {
    let s = match requested.unwrap_or("active") {
        "draft" => "draft",
        "archived" => "archived",
        "active" | "sold_out" => "active",
        _ => return Err(AppError::bad("상품 상태 값이 올바르지 않습니다")),
    };
    Ok(if s == "active" && stock == 0 { "sold_out" } else { s })
}

async fn product_by_id(s: &AppState, workshop_id: &str, id: &str) -> AppResult<PartnerProduct> {
    sqlx::query_as::<_, PartnerProduct>(&format!("{PARTNER_PRODUCT_SQL} WHERE p.id = ? AND p.workshop_id = ?"))
        .bind(id)
        .bind(workshop_id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| AppError::not_found("상품을 찾을 수 없습니다"))
}

async fn create_product(State(s): State<AppState>, p: Partner, Json(b): Json<ProductInput>) -> AppResult<(StatusCode, Json<PartnerProduct>)> {
    let c = check_product(&b)?;
    let status = partner_status(b.status.as_deref(), b.stock)?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE workshop_id = ?").bind(&p.workshop_id).fetch_one(&s.db).await?;
    if n >= 500 {
        return Err(AppError::bad("상품은 500개까지 등록할 수 있습니다"));
    }
    let id = new_id();
    sqlx::query(
        "INSERT INTO products (id, workshop_id, name, category, description, price, list_price, stock, image_url, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&p.workshop_id)
    .bind(&c.name)
    .bind(&c.category)
    .bind(&c.description)
    .bind(b.price)
    .bind(c.list_price)
    .bind(b.stock)
    .bind(&c.image_url)
    .bind(status)
    .execute(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(product_by_id(&s, &p.workshop_id, &id).await?)))
}

async fn update_product(State(s): State<AppState>, p: Partner, Path(id): Path<String>, Json(b): Json<ProductInput>) -> AppResult<Json<PartnerProduct>> {
    let c = check_product(&b)?;
    let current = product_by_id(&s, &p.workshop_id, &id).await?;
    let status = if current.status == "blocked" {
        // 운영자가 막은 상품: 내용은 고칠 수 있지만 상태는 그대로
        "blocked"
    } else {
        partner_status(b.status.as_deref().or(Some(current.status.as_str())), b.stock)?
    };
    sqlx::query(
        "UPDATE products SET name = ?, category = ?, description = ?, price = ?, list_price = ?, stock = ?, image_url = ?, status = ? \
         WHERE id = ? AND workshop_id = ?",
    )
    .bind(&c.name)
    .bind(&c.category)
    .bind(&c.description)
    .bind(b.price)
    .bind(c.list_price)
    .bind(b.stock)
    .bind(&c.image_url)
    .bind(status)
    .bind(&id)
    .bind(&p.workshop_id)
    .execute(&s.db)
    .await?;
    Ok(Json(product_by_id(&s, &p.workshop_id, &id).await?))
}

/// 주문된 적 없는 상품은 지우고, 주문 기록이 있으면 숨깁니다.
async fn delete_product(State(s): State<AppState>, p: Partner, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    product_by_id(&s, &p.workshop_id, &id).await?;
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM order_items WHERE product_id = ?").bind(&id).fetch_one(&s.db).await?;
    if used > 0 {
        sqlx::query("UPDATE products SET status = IF(status = 'blocked', 'blocked', 'archived') WHERE id = ? AND workshop_id = ?")
            .bind(&id)
            .bind(&p.workshop_id)
            .execute(&s.db)
            .await?;
        return Ok(Json(serde_json::json!({ "deleted": false, "archived": true })));
    }
    sqlx::query("DELETE FROM products WHERE id = ? AND workshop_id = ?").bind(&id).bind(&p.workshop_id).execute(&s.db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Debug, Deserialize)]
pub struct AdminProductQuery {
    pub status: Option<String>,
    pub q: Option<String>,
    pub workshop_id: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn admin_products(State(s): State<AppState>, _a: Admin, Query(q): Query<AdminProductQuery>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 500);
    let term = like_term(q.q.as_deref());
    let rows: Vec<(String, String, String, String, String, u64, u32, String, Option<String>, NaiveDateTime, i64)> = sqlx::query_as(
        "SELECT p.id, p.workshop_id, w.name, p.name, p.category, p.price, p.stock, p.status, p.image_url, p.created_at, \
           CAST(COALESCE((SELECT SUM(oi.quantity) FROM order_items oi WHERE oi.product_id = p.id AND oi.status <> 'cancelled'), 0) AS SIGNED) \
         FROM products p JOIN workshops w ON w.id = p.workshop_id \
         WHERE (? IS NULL OR p.status = ?) AND (? IS NULL OR p.workshop_id = ?) \
           AND (? IS NULL OR p.name LIKE CONCAT('%', ?, '%') OR w.name LIKE CONCAT('%', ?, '%')) \
         ORDER BY p.created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&q.status)
    .bind(&q.status)
    .bind(&q.workshop_id)
    .bind(&q.workshop_id)
    .bind(&term)
    .bind(&term)
    .bind(&term)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, wid, wname, name, cat, price, stock, status, img, created, sold)| {
                serde_json::json!({"id": id, "workshop_id": wid, "workshop_name": wname, "name": name, "category": cat, "price": price,
                    "stock": stock, "status": status, "image_url": img, "created_at": created, "sold": sold})
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct StatusInput {
    pub status: String,
    pub reason: Option<String>,
}

/// 운영자: 판매 중지(`blocked`) · 다시 열기(`active`, 재고 0이면 품절) · 숨김(`archived`)
async fn admin_set_status(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<StatusInput>) -> AppResult<StatusCode> {
    let stock: Option<u32> = sqlx::query_scalar("SELECT stock FROM products WHERE id = ?").bind(&id).fetch_optional(&s.db).await?;
    let stock = stock.ok_or_else(|| AppError::not_found("상품을 찾을 수 없습니다"))?;
    let status = match b.status.as_str() {
        "blocked" | "archived" | "draft" => b.status.as_str(),
        "active" | "sold_out" => {
            if stock == 0 {
                "sold_out"
            } else {
                "active"
            }
        }
        _ => return Err(AppError::bad("상품 상태 값이 올바르지 않습니다")),
    };
    sqlx::query("UPDATE products SET status = ? WHERE id = ?").bind(status).bind(&id).execute(&s.db).await?;
    audit::log(&s, &admin, &format!("product.{status}"), "product", &id, b.reason.clone()).await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::partner_status as ps;

    #[test]
    fn partner_status_rules() {
        assert_eq!(ps(None, 3).unwrap(), "active");
        assert_eq!(ps(None, 0).unwrap(), "sold_out");
        assert_eq!(ps(Some("sold_out"), 5).unwrap(), "active");
        assert_eq!(ps(Some("draft"), 0).unwrap(), "draft");
        assert_eq!(ps(Some("archived"), 5).unwrap(), "archived");
        assert!(ps(Some("blocked"), 5).is_err(), "partner cannot block/unblock");
        assert!(ps(Some("weird"), 5).is_err());
    }
}
