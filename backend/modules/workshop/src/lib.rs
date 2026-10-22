use axum::{extract::{Path, Query, State}, http::StatusCode, routing::{get, post}, Json, Router};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use uuid::Uuid;

#[derive(Clone)] pub struct WorkshopState { pub db: MySqlPool }
#[derive(Serialize, sqlx::FromRow)] pub struct Workshop { pub id: String, pub name: String, pub district: String, pub address: String, pub category: String, pub rating: f32, pub review_count: i32, pub latitude: Option<f64>, pub longitude: Option<f64> }
#[derive(Serialize, sqlx::FromRow)] pub struct WorkshopService { pub id: String, pub name: String, pub description: String, pub price_from: u64, pub duration_minutes: u16 }
#[derive(Serialize, sqlx::FromRow)] pub struct Offer { pub id: String, pub workshop_id: String, pub amount: u64, pub description: String, pub valid_until: Option<NaiveDateTime>, pub created_at: NaiveDateTime }
#[derive(Deserialize)] pub struct Search { pub q: Option<String>, pub category: Option<String>, pub district: Option<String> }
#[derive(Deserialize)] pub struct QuoteRequest { pub vehicle: String, pub service_category: String, pub description: String, pub district: String, pub customer_phone: String }
#[derive(Deserialize)] pub struct OfferRequest { pub workshop_id: String, pub amount: i64, pub description: String, pub valid_until: Option<NaiveDateTime> }
#[derive(Deserialize)] pub struct BookingRequest { pub workshop_id: String, pub service_id: Option<String>, pub vehicle: String, pub starts_at: NaiveDateTime, pub customer_name: String, pub customer_phone: String, pub note: Option<String> }

pub fn routes() -> Router<WorkshopState> {
    Router::new().route("/workshops", get(list_workshops)).route("/workshops/{id}", get(workshop_detail))
        .route("/quotes", post(create_quote)).route("/quotes/{id}/offers", get(list_offers).post(create_offer))
        .route("/bookings", post(create_booking)).route("/workshops/{id}/services", get(list_services))
}
async fn list_workshops(State(s): State<WorkshopState>, Query(q): Query<Search>) -> Result<Json<Vec<Workshop>>, (StatusCode,String)> {
    let rows = sqlx::query_as::<_, Workshop>(r#"SELECT id, name, district, address, category, rating, review_count, latitude, longitude FROM workshops WHERE status='approved' AND (? IS NULL OR CONCAT(name,' ',district,' ',category) LIKE CONCAT('%',?,'%')) AND (? IS NULL OR category=?) AND (? IS NULL OR district=?) ORDER BY rating DESC LIMIT 100"#)
        .bind(q.q.clone()).bind(q.q).bind(q.category.clone()).bind(q.category).bind(q.district.clone()).bind(q.district).fetch_all(&s.db).await.map_err(internal)?;
    Ok(Json(rows))
}
async fn workshop_detail(State(s): State<WorkshopState>, Path(id): Path<String>) -> Result<Json<Workshop>, (StatusCode,String)> {
    sqlx::query_as::<_, Workshop>("SELECT id, name, district, address, category, rating, review_count, latitude, longitude FROM workshops WHERE id=? AND status='approved'").bind(id)
        .fetch_optional(&s.db).await.map_err(internal)?.map(Json).ok_or((StatusCode::NOT_FOUND,"업체를 찾을 수 없습니다".into()))
}
async fn list_services(State(s): State<WorkshopState>, Path(id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, (StatusCode,String)> {
    let rows=sqlx::query_as::<_,WorkshopService>("SELECT id, name, description, price_from, duration_minutes FROM workshop_services WHERE workshop_id=? AND is_active=1 ORDER BY sort_order").bind(id).fetch_all(&s.db).await.map_err(internal)?;
    Ok(Json(rows.into_iter().map(|r|serde_json::json!({"id":r.id,"name":r.name,"description":r.description,"price_from":r.price_from,"duration_minutes":r.duration_minutes})).collect()))
}
async fn create_quote(State(s): State<WorkshopState>, Json(req): Json<QuoteRequest>) -> Result<(StatusCode,Json<serde_json::Value>), (StatusCode,String)> {
    if req.vehicle.trim().is_empty() || req.description.trim().is_empty() || req.customer_phone.trim().is_empty() { return Err((StatusCode::BAD_REQUEST,"차량·요청 내용·연락처를 입력해 주세요".into())); }
    let id=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO quote_requests (id, vehicle, service_category, description, district, customer_phone, status) VALUES (?, ?, ?, ?, ?, ?, 'open')").bind(&id).bind(req.vehicle).bind(req.service_category).bind(req.description).bind(req.district).bind(req.customer_phone).execute(&s.db).await.map_err(internal)?;
    Ok((StatusCode::CREATED,Json(serde_json::json!({"id":id,"status":"open"}))))
}
async fn list_offers(State(s): State<WorkshopState>, Path(id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, (StatusCode,String)> {
    let rows=sqlx::query_as::<_,Offer>("SELECT id, workshop_id, amount, description, valid_until, created_at FROM quote_offers WHERE quote_id=? ORDER BY amount").bind(id).fetch_all(&s.db).await.map_err(internal)?;
    Ok(Json(rows))
}
async fn create_offer(State(s): State<WorkshopState>, Path(id): Path<String>, Json(req): Json<OfferRequest>) -> Result<(StatusCode,Json<serde_json::Value>), (StatusCode,String)> {
    if req.amount < 0 {return Err((StatusCode::BAD_REQUEST,"견적 금액은 0 이상이어야 합니다".into()));}
    let offer_id=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO quote_offers (id, quote_id, workshop_id, amount, description, valid_until) VALUES (?, ?, ?, ?, ?, ?)").bind(&offer_id).bind(id).bind(req.workshop_id).bind(req.amount).bind(req.description).bind(req.valid_until).execute(&s.db).await.map_err(internal)?;
    Ok((StatusCode::CREATED,Json(serde_json::json!({"id":offer_id}))))
}
async fn create_booking(State(s): State<WorkshopState>, Json(req): Json<BookingRequest>) -> Result<(StatusCode,Json<serde_json::Value>), (StatusCode,String)> {
    let id=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO bookings (id, workshop_id, service_id, vehicle, starts_at, customer_name, customer_phone, note, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'requested')").bind(&id).bind(req.workshop_id).bind(req.service_id).bind(req.vehicle).bind(req.starts_at).bind(req.customer_name).bind(req.customer_phone).bind(req.note).execute(&s.db).await.map_err(internal)?;
    Ok((StatusCode::CREATED,Json(serde_json::json!({"id":id,"status":"requested"}))))
}
fn internal(e: sqlx::Error)->(StatusCode,String){tracing::error!(error=%e,"database request failed");(StatusCode::INTERNAL_SERVER_ERROR,"요청을 처리하지 못했습니다".into())}
