//! 고객 화면의 업체 검색·상세·서비스·통계, 관심 업체.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use chrono::NaiveTime;
use gageyo_core::{
    auth::{Customer, MaybeCustomer},
    util::{like_term, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, QueryBuilder};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workshops", get(list_workshops))
        .route("/workshops/{id}", get(workshop_detail))
        .route("/workshops/{id}/services", get(list_services))
        .route("/stats", get(public_stats))
        .route("/me/favorites", get(my_favorites))
        .route("/me/favorites/{workshop_id}", put(add_favorite).delete(remove_favorite))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkshopCard {
    pub id: String,
    pub name: String,
    pub district: String,
    pub address: String,
    pub category: String,
    pub rating: f32,
    pub review_count: i32,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub cover_image_url: Option<String>,
    pub open_time: Option<NaiveTime>,
    pub close_time: Option<NaiveTime>,
    pub distance_km: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Search {
    pub q: Option<String>,
    pub category: Option<String>,
    pub district: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    /// 반경 (km, 기본 10, 최대 100). lat·lng 가 있을 때만
    pub radius_km: Option<f64>,
    /// rating(기본) · distance · reviews · recent
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

/// 하버사인 거리(km) SQL. `w.latitude/longitude` 와 바인드 3개(lat, lat, lng)를 씁니다.
const DISTANCE_SQL: &str = "6371 * 2 * ASIN(SQRT(POWER(SIN(RADIANS(w.latitude - ";

fn push_distance(qb: &mut QueryBuilder<'_, MySql>, lat: f64, lng: f64) {
    qb.push(DISTANCE_SQL);
    qb.push_bind(lat);
    qb.push(") / 2), 2) + COS(RADIANS(");
    qb.push_bind(lat);
    qb.push(")) * COS(RADIANS(w.latitude)) * POWER(SIN(RADIANS(w.longitude - ");
    qb.push_bind(lng);
    qb.push(") / 2), 2)))");
}

async fn list_workshops(State(s): State<AppState>, Query(q): Query<Search>) -> AppResult<Json<serde_json::Value>> {
    let coords = match (q.lat, q.lng) {
        (Some(a), Some(o)) => Some(gageyo_core::util::check_coords(Some(a), Some(o))?).map(|_| (a, o)),
        (None, None) => None,
        _ => return Err(AppError::bad("위치는 위도·경도를 함께 보내 주세요")),
    };
    let radius = q.radius_km.unwrap_or(10.0).clamp(0.5, 100.0);
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(24, 60);
    let term = like_term(q.q.as_deref());
    let district = like_term(q.district.as_deref());
    let category = q.category.as_deref().map(str::trim).filter(|c| !c.is_empty() && *c != "전체");

    let mut qb: QueryBuilder<MySql> = QueryBuilder::new(
        "SELECT w.id, w.name, w.district, w.address, w.category, w.rating, w.review_count, w.latitude, w.longitude, \
         w.cover_image_url, w.open_time, w.close_time, ",
    );
    match coords {
        Some((lat, lng)) => push_distance(&mut qb, lat, lng),
        None => {
            qb.push("NULL");
        }
    }
    qb.push(" AS distance_km FROM workshops w WHERE w.status = 'approved'");
    if let Some(t) = &term {
        qb.push(" AND (w.name LIKE CONCAT('%', ");
        qb.push_bind(t.clone());
        qb.push(", '%') OR w.district LIKE CONCAT('%', ");
        qb.push_bind(t.clone());
        qb.push(", '%') OR w.address LIKE CONCAT('%', ");
        qb.push_bind(t.clone());
        qb.push(", '%') OR EXISTS (SELECT 1 FROM workshop_categories c WHERE c.workshop_id = w.id AND c.category LIKE CONCAT('%', ");
        qb.push_bind(t.clone());
        qb.push(", '%')))");
    }
    if let Some(c) = category {
        qb.push(" AND EXISTS (SELECT 1 FROM workshop_categories c WHERE c.workshop_id = w.id AND c.category = ");
        qb.push_bind(c.to_string());
        qb.push(")");
    }
    if let Some(d) = &district {
        qb.push(" AND (w.district LIKE CONCAT('%', ");
        qb.push_bind(d.clone());
        qb.push(", '%') OR w.address LIKE CONCAT('%', ");
        qb.push_bind(d.clone());
        qb.push(", '%'))");
    }
    if let Some((lat, lng)) = coords {
        qb.push(" AND w.latitude IS NOT NULL AND ");
        push_distance(&mut qb, lat, lng);
        qb.push(" <= ");
        qb.push_bind(radius);
    }
    let order = match (q.sort.as_deref(), coords.is_some()) {
        (Some("distance"), true) => " ORDER BY distance_km ASC, w.rating DESC",
        (Some("reviews"), _) => " ORDER BY w.review_count DESC, w.rating DESC",
        (Some("recent"), _) => " ORDER BY w.approved_at DESC, w.created_at DESC",
        _ => " ORDER BY w.rating DESC, w.review_count DESC, w.created_at DESC",
    };
    qb.push(order);
    qb.push(" LIMIT ");
    qb.push_bind(limit + 1);
    qb.push(" OFFSET ");
    qb.push_bind(offset);
    let mut rows = qb.build_query_as::<WorkshopCard>().fetch_all(&s.db).await?;
    let has_more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);
    // 업체별 분야 (카드에 작은 꼬리표로 보임)
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let cats = categories_for(&s, &ids).await?;
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let c = cats.iter().filter(|(w, _)| w == &r.id).map(|(_, c)| c.clone()).collect::<Vec<_>>();
            let mut v = serde_json::to_value(&r).unwrap_or_default();
            v["categories"] = serde_json::json!(c);
            v
        })
        .collect();
    Ok(Json(serde_json::json!({ "items": items, "has_more": has_more })))
}

pub async fn categories_for(s: &AppState, ids: &[String]) -> AppResult<Vec<(String, String)>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let mut qb: QueryBuilder<MySql> = QueryBuilder::new("SELECT workshop_id, category FROM workshop_categories WHERE workshop_id IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(id.clone());
    }
    qb.push(") ORDER BY category");
    Ok(qb.build_query_as::<(String, String)>().fetch_all(&s.db).await?)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkshopDetail {
    pub id: String,
    pub name: String,
    pub district: String,
    pub address: String,
    pub category: String,
    pub description: String,
    pub phone: String,
    pub rating: f32,
    pub review_count: i32,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub cover_image_url: Option<String>,
    pub open_time: Option<NaiveTime>,
    pub close_time: Option<NaiveTime>,
    pub closed_days: String,
    pub business_number: Option<String>,
    pub representative: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkshopService {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price_from: u64,
    pub duration_minutes: u16,
}

async fn workshop_detail(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let w = sqlx::query_as::<_, WorkshopDetail>(
        "SELECT id, name, district, address, category, description, phone, rating, review_count, latitude, longitude, \
         cover_image_url, open_time, close_time, closed_days, business_number, representative \
         FROM workshops WHERE id = ? AND status = 'approved'",
    )
    .bind(&id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| AppError::not_found("업체를 찾을 수 없습니다"))?;
    let services = active_services(&s, &id).await?;
    let photos: Vec<(String, String)> =
        sqlx::query_as("SELECT id, url FROM workshop_photos WHERE workshop_id = ? ORDER BY sort_order, created_at")
            .bind(&id)
            .fetch_all(&s.db)
            .await?;
    let categories: Vec<String> = categories_for(&s, std::slice::from_ref(&id)).await?.into_iter().map(|(_, c)| c).collect();
    let reviews = crate::reviews::public_reviews(&s, &id, 5, 0).await?;
    let rating_counts: Vec<(i8, i64)> = sqlx::query_as(
        "SELECT rating, COUNT(*) FROM reviews WHERE workshop_id = ? AND status = 'visible' GROUP BY rating",
    )
    .bind(&id)
    .fetch_all(&s.db)
    .await?;
    let is_favorite = match &user {
        Some(u) => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM favorites WHERE user_id = ? AND workshop_id = ?")
                .bind(&u.id)
                .bind(&id)
                .fetch_one(&s.db)
                .await?
                > 0
        }
        None => false,
    };
    let products: Vec<(String, String, u64, Option<String>)> = sqlx::query_as(
        "SELECT id, name, price, image_url FROM products WHERE workshop_id = ? AND status = 'active' ORDER BY created_at DESC LIMIT 8",
    )
    .bind(&id)
    .fetch_all(&s.db)
    .await?;
    let mut v = serde_json::to_value(&w)?;
    v["services"] = serde_json::to_value(&services)?;
    v["photos"] = serde_json::json!(photos.into_iter().map(|(id, url)| serde_json::json!({"id": id, "url": url})).collect::<Vec<_>>());
    v["categories"] = serde_json::json!(categories);
    v["reviews"] = serde_json::to_value(&reviews)?;
    v["rating_counts"] =
        serde_json::json!(rating_counts.into_iter().map(|(r, n)| serde_json::json!({"rating": r, "count": n})).collect::<Vec<_>>());
    v["is_favorite"] = serde_json::json!(is_favorite);
    v["products"] = serde_json::json!(products
        .into_iter()
        .map(|(id, name, price, image_url)| serde_json::json!({"id": id, "name": name, "price": price, "image_url": image_url}))
        .collect::<Vec<_>>());
    Ok(Json(v))
}

pub async fn active_services(s: &AppState, workshop_id: &str) -> AppResult<Vec<WorkshopService>> {
    Ok(sqlx::query_as::<_, WorkshopService>(
        "SELECT id, name, description, price_from, duration_minutes FROM workshop_services \
         WHERE workshop_id = ? AND is_active = 1 ORDER BY sort_order, name",
    )
    .bind(workshop_id)
    .fetch_all(&s.db)
    .await?)
}

async fn list_services(State(s): State<AppState>, Path(id): Path<String>) -> AppResult<Json<Vec<WorkshopService>>> {
    Ok(Json(active_services(&s, &id).await?))
}

/// 첫 화면 숫자 (예전에는 '1,240+ 등록 업체' 같은 고정된 가짜 숫자를 보여 줬음)
async fn public_stats(State(s): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let (workshops, reviews, avg, quotes, bookings): (i64, i64, Option<f64>, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM workshops WHERE status = 'approved'), \
                (SELECT COUNT(*) FROM reviews WHERE status = 'visible'), \
                (SELECT CAST(AVG(rating) AS DOUBLE) FROM reviews WHERE status = 'visible'), \
                (SELECT COUNT(*) FROM quote_requests), \
                (SELECT COUNT(*) FROM bookings WHERE status = 'completed')",
    )
    .fetch_one(&s.db)
    .await?;
    Ok(Json(serde_json::json!({
        "workshops": workshops, "reviews": reviews, "average_rating": avg.map(|a| (a * 10.0).round() / 10.0),
        "quotes": quotes, "completed_bookings": bookings,
    })))
}

async fn my_favorites(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<Vec<WorkshopCard>>> {
    let rows = sqlx::query_as::<_, WorkshopCard>(
        "SELECT w.id, w.name, w.district, w.address, w.category, w.rating, w.review_count, w.latitude, w.longitude, \
         w.cover_image_url, w.open_time, w.close_time, NULL AS distance_km \
         FROM favorites f JOIN workshops w ON w.id = f.workshop_id \
         WHERE f.user_id = ? AND w.status = 'approved' ORDER BY f.created_at DESC",
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn add_favorite(State(s): State<AppState>, Customer(u): Customer, Path(workshop_id): Path<String>) -> AppResult<StatusCode> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshops WHERE id = ? AND status = 'approved'")
        .bind(&workshop_id)
        .fetch_one(&s.db)
        .await?;
    if exists == 0 {
        return Err(AppError::not_found("업체를 찾을 수 없습니다"));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM favorites WHERE user_id = ?").bind(&u.id).fetch_one(&s.db).await?;
    if count >= 200 {
        return Err(AppError::bad("관심 업체는 200곳까지 저장할 수 있습니다"));
    }
    sqlx::query("INSERT IGNORE INTO favorites (user_id, workshop_id) VALUES (?, ?)")
        .bind(&u.id)
        .bind(&workshop_id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_favorite(State(s): State<AppState>, Customer(u): Customer, Path(workshop_id): Path<String>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM favorites WHERE user_id = ? AND workshop_id = ?")
        .bind(&u.id)
        .bind(&workshop_id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
