use axum::{extract::{Path, Query, State}, http::{HeaderMap, StatusCode}, routing::{get, patch, post, put}, Json, Router};
use argon2::{password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString}, Argon2};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use uuid::Uuid;

#[derive(Clone)] pub struct WorkshopState { pub db: MySqlPool, pub admin_api_key: Option<String> }
#[derive(Serialize, sqlx::FromRow)] pub struct Workshop { pub id: String, pub name: String, pub district: String, pub address: String, pub category: String, pub rating: f32, pub review_count: i32, pub latitude: Option<f64>, pub longitude: Option<f64> }
#[derive(Serialize, sqlx::FromRow)] pub struct ServiceCategory { pub id:String, pub group_name:String, pub name:String, pub slug:String, pub sort_order:i32, pub is_active:bool }
#[derive(Serialize, sqlx::FromRow)] pub struct WorkshopService { pub id: String, pub name: String, pub description: String, pub price_from: u64, pub duration_minutes: u16 }
#[derive(Serialize, sqlx::FromRow)] pub struct Offer { pub id: String, pub workshop_id: String, pub workshop_name:String, pub amount: u64, pub description: String, pub valid_until: Option<NaiveDateTime>, pub created_at: NaiveDateTime }
#[derive(Serialize, sqlx::FromRow)] pub struct AdminWorkshop { pub id:String, pub name:String, pub district:String, pub category:String, pub status:String, pub created_at:NaiveDateTime }
#[derive(Serialize, sqlx::FromRow)] pub struct AdminQuote { pub id:String, pub vehicle:String, pub service_category:String, pub description:String, pub district:String, pub customer_phone:String, pub status:String, pub created_at:NaiveDateTime }
#[derive(Serialize, sqlx::FromRow)] pub struct AdminBooking { pub id:String, pub workshop_id:String, pub workshop_name:String, pub vehicle:String, pub starts_at:NaiveDateTime, pub customer_name:String, pub customer_phone:String, pub status:String, pub created_at:NaiveDateTime }
#[derive(Deserialize)] pub struct Search { pub q: Option<String>, pub category: Option<String>, pub district: Option<String> }
#[derive(Deserialize)] pub struct QuoteRequest { pub vehicle: String, pub service_category: String, pub description: String, pub district: String, pub customer_phone: String }
#[derive(Deserialize)] pub struct OfferRequest { pub workshop_id: String, pub amount: i64, pub description: String, pub valid_until: Option<NaiveDateTime> }
#[derive(Deserialize)] pub struct BookingRequest { pub workshop_id: String, pub service_id: Option<String>, pub vehicle: String, pub starts_at: NaiveDateTime, pub customer_name: String, pub customer_phone: String, pub note: Option<String> }
#[derive(Deserialize)] pub struct CategoryInput { pub group_name:String, pub name:String, pub sort_order:Option<i32>, pub is_active:Option<bool> }

pub fn routes() -> Router<WorkshopState> {
    Router::new().route("/auth/register", post(register_customer))
        .route("/auth/login", post(login_customer))
        .route("/auth/logout", post(logout_customer))
        .route("/auth/me", get(current_customer))
        .route("/categories", get(public_categories))
        .route("/admin/categories", get(admin_categories).post(create_category))
        .route("/admin/categories/{id}", put(update_category).delete(delete_category))
        .route("/admin/workshops", get(admin_workshops))
        .route("/admin/workshops/{id}", patch(update_workshop_status))
        .route("/admin/quotes", get(admin_quotes))
        .route("/admin/bookings", get(admin_bookings))
        .route("/admin/bookings/{id}", patch(update_booking_status))
        .route("/admin/workshops/{id}/partner-key", post(create_partner_key))
        .route("/partner/me", get(partner_me))
        .route("/partner/quotes", get(partner_quotes))
        .route("/partner/quotes/{id}/offers", post(partner_create_offer))
        .route("/partner/bookings", get(partner_bookings))
        .route("/partner/bookings/{id}", patch(partner_update_booking))
        .route("/partner/services", get(partner_services).post(create_partner_service))
        .route("/partner/services/{id}", patch(update_partner_service).delete(delete_partner_service))
        .route("/workshops", get(list_workshops)).route("/workshops/{id}", get(workshop_detail))
        .route("/quotes", post(create_quote)).route("/quotes/{id}/offers", get(list_offers).post(partner_create_offer))
        .route("/bookings", post(create_booking)).route("/workshops/{id}/services", get(list_services))
}
#[derive(Deserialize)] struct RegisterInput { email:String, password:String, display_name:String, phone:Option<String> }
#[derive(Deserialize)] struct LoginInput { email:String, password:String }
#[derive(Serialize, sqlx::FromRow)] struct CustomerProfile { id:String, email:String, display_name:String, phone:Option<String>, role:String }
fn session_cookie(headers:&HeaderMap)->Option<String>{
 headers.get(axum::http::header::COOKIE)?.to_str().ok()?.split(';').find_map(|item|{let (name,value)=item.trim().split_once('=')?;(name=="gageyo_session").then(||value.to_owned())})
}
fn set_session_cookie(token:&str)->String{
 let secure=std::env::var("COOKIE_SECURE").map(|v|v!="false").unwrap_or(true);
 format!("gageyo_session={token}; Path=/api; Max-Age=2592000; HttpOnly; SameSite=Lax{}",if secure{"; Secure"}else{""})
}
fn clear_session_cookie()->String{"gageyo_session=; Path=/api; Max-Age=0; HttpOnly; SameSite=Lax; Secure".into()}
async fn register_customer(State(s):State<WorkshopState>,Json(input):Json<RegisterInput>)->Result<(StatusCode,[(axum::http::header::HeaderName,String);1],Json<CustomerProfile>),(StatusCode,String)>{
 let email=input.email.trim().to_lowercase();let name=input.display_name.trim();
 if !email.contains('@')||email.len()>254||name.is_empty()||name.chars().count()>100||input.password.chars().count()<10||input.password.len()>128{return Err((StatusCode::BAD_REQUEST,"이메일, 이름을 확인하고 비밀번호는 10~128자로 입력해 주세요".into()));}
 let salt=SaltString::generate(&mut OsRng);let password_hash=Argon2::default().hash_password(input.password.as_bytes(),&salt).map_err(|_|internal(sqlx::Error::Protocol("password hashing failed".into())))?.to_string();
 let id=Uuid::new_v4().to_string();
 let result=sqlx::query("INSERT INTO users (id,email,display_name,phone,role,password_hash) VALUES (?,?,?,?,'customer',?)").bind(&id).bind(&email).bind(name).bind(input.phone.as_deref().map(str::trim).filter(|v|!v.is_empty())).bind(password_hash).execute(&s.db).await;
 if let Err(e)=result { if let sqlx::Error::Database(db)=&e {if db.code().as_deref()==Some("1062"){return Err((StatusCode::CONFLICT,"이미 가입된 이메일입니다".into()));}} return Err(internal(e)); }
 let token=format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple());
 sqlx::query("INSERT INTO auth_sessions (id,user_id,token_hash,expires_at) VALUES (?,?,?,DATE_ADD(UTC_TIMESTAMP(), INTERVAL 30 DAY))").bind(Uuid::new_v4().to_string()).bind(&id).bind(token_hash(&token)).execute(&s.db).await.map_err(internal)?;
 let profile=sqlx::query_as::<_,CustomerProfile>("SELECT id,email,display_name,phone,role FROM users WHERE id=?").bind(id).fetch_one(&s.db).await.map_err(internal)?;
 Ok((StatusCode::CREATED,[(axum::http::header::SET_COOKIE,set_session_cookie(&token))],Json(profile)))
}
async fn login_customer(State(s):State<WorkshopState>,Json(input):Json<LoginInput>)->Result<([(axum::http::header::HeaderName,String);1],Json<CustomerProfile>),(StatusCode,String)>{
 let email=input.email.trim().to_lowercase();
 let row=sqlx::query_as::<_,(String,String,String,Option<String>,String,Option<String>)>("SELECT id,email,display_name,phone,role,password_hash FROM users WHERE email=?").bind(email).fetch_optional(&s.db).await.map_err(internal)?;
 let Some((id,email,display_name,phone,role,hash))=row else{return Err((StatusCode::UNAUTHORIZED,"이메일 또는 비밀번호를 확인해 주세요".into()));};
 let valid=hash.as_deref().and_then(|hash|PasswordHash::new(hash).ok()).is_some_and(|parsed|Argon2::default().verify_password(input.password.as_bytes(),&parsed).is_ok());
 if !valid||role!="customer"{return Err((StatusCode::UNAUTHORIZED,"이메일 또는 비밀번호를 확인해 주세요".into()));}
 let token=format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple());
 sqlx::query("INSERT INTO auth_sessions (id,user_id,token_hash,expires_at) VALUES (?,?,?,DATE_ADD(UTC_TIMESTAMP(), INTERVAL 30 DAY))").bind(Uuid::new_v4().to_string()).bind(&id).bind(token_hash(&token)).execute(&s.db).await.map_err(internal)?;
 Ok(([(axum::http::header::SET_COOKIE,set_session_cookie(&token))],Json(CustomerProfile{id,email,display_name,phone,role})))
}
async fn current_customer(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<CustomerProfile>,(StatusCode,String)>{
 let token=session_cookie(&headers).ok_or((StatusCode::UNAUTHORIZED,"로그인이 필요합니다".into()))?;
 let profile=sqlx::query_as::<_,CustomerProfile>("SELECT u.id,u.email,u.display_name,u.phone,u.role FROM auth_sessions a JOIN users u ON u.id=a.user_id WHERE a.token_hash=? AND a.expires_at>UTC_TIMESTAMP() AND u.role='customer'").bind(token_hash(&token)).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::UNAUTHORIZED,"로그인이 필요합니다".into()))?;
 Ok(Json(profile))
}
async fn logout_customer(State(s):State<WorkshopState>,headers:HeaderMap)->(StatusCode,[(axum::http::header::HeaderName,String)],Json<serde_json::Value>){
 if let Some(token)=session_cookie(&headers){let _=sqlx::query("DELETE FROM auth_sessions WHERE token_hash=?").bind(token_hash(&token)).execute(&s.db).await;}
 (StatusCode::OK,[(axum::http::header::SET_COOKIE,clear_session_cookie())],Json(serde_json::json!({"ok":true})))
}
fn token_hash(token:&str)->String { Sha256::digest(token.as_bytes()).iter().map(|byte|format!("{byte:02x}")).collect() }
async fn partner_workshop_id(s:&WorkshopState,headers:&HeaderMap)->Result<String,(StatusCode,String)>{
 let token=headers.get("x-partner-key").and_then(|value|value.to_str().ok()).unwrap_or("");
 if token.is_empty(){return Err((StatusCode::UNAUTHORIZED,"파트너 키를 입력해 주세요".into()));}
 sqlx::query_scalar::<_,String>("SELECT workshop_id FROM workshop_access_tokens WHERE token_hash=?").bind(token_hash(token)).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::UNAUTHORIZED,"파트너 키를 확인해 주세요".into()))
}
async fn create_partner_key(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>)->Result<(StatusCode,Json<serde_json::Value>),(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let exists=sqlx::query_scalar::<_,String>("SELECT id FROM workshops WHERE id=?").bind(&id).fetch_optional(&s.db).await.map_err(internal)?;
 if exists.is_none(){return Err((StatusCode::NOT_FOUND,"업체를 찾을 수 없습니다".into()));}
 let token=format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple());
 sqlx::query("DELETE FROM workshop_access_tokens WHERE workshop_id=?").bind(&id).execute(&s.db).await.map_err(internal)?;
 sqlx::query("INSERT INTO workshop_access_tokens (token_hash,workshop_id) VALUES (?,?)").bind(token_hash(&token)).bind(id).execute(&s.db).await.map_err(internal)?;
 Ok((StatusCode::CREATED,Json(serde_json::json!({"partner_key":token}))))
}
async fn partner_me(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<AdminWorkshop>,(StatusCode,String)>{
 let id=partner_workshop_id(&s,&headers).await?;
 let row=sqlx::query_as::<_,AdminWorkshop>("SELECT id,name,district,category,status,created_at FROM workshops WHERE id=?").bind(id).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::NOT_FOUND,"업체를 찾을 수 없습니다".into()))?;
 Ok(Json(row))
}
async fn partner_quotes(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<Vec<AdminQuote>>,(StatusCode,String)>{
 let id=partner_workshop_id(&s,&headers).await?;
 let rows=sqlx::query_as::<_,AdminQuote>("SELECT q.id,q.vehicle,q.service_category,q.description,q.district,q.customer_phone,q.status,q.created_at FROM quote_requests q JOIN workshops w ON w.id=? WHERE q.status='open' AND q.district LIKE CONCAT('%',w.district,'%') AND (q.service_category=w.category OR q.service_category='기타') AND NOT EXISTS (SELECT 1 FROM quote_offers o WHERE o.quote_id=q.id AND o.workshop_id=w.id) ORDER BY q.created_at DESC LIMIT 200").bind(id).fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn partner_create_offer(State(s):State<WorkshopState>,headers:HeaderMap,Path(quote_id):Path<String>,Json(input):Json<OfferRequest>)->Result<(StatusCode,Json<serde_json::Value>),(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 if input.amount<0 || input.description.trim().is_empty(){return Err((StatusCode::BAD_REQUEST,"견적 금액과 작업 내용을 입력해 주세요".into()));}
 let offer_id=Uuid::new_v4().to_string();
 let result=sqlx::query("INSERT INTO quote_offers (id,quote_id,workshop_id,amount,description,valid_until) SELECT ?,q.id,?,?,?,? FROM quote_requests q WHERE q.id=? AND q.status='open'")
   .bind(&offer_id).bind(&workshop_id).bind(input.amount as u64).bind(input.description.trim()).bind(input.valid_until).bind(&quote_id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"견적 요청을 찾을 수 없거나 마감되었습니다".into()));}
 Ok((StatusCode::CREATED,Json(serde_json::json!({"id":offer_id}))))
}
async fn partner_bookings(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<Vec<AdminBooking>>,(StatusCode,String)>{
 let id=partner_workshop_id(&s,&headers).await?;
 let rows=sqlx::query_as::<_,AdminBooking>("SELECT b.id,b.workshop_id,w.name AS workshop_name,b.vehicle,b.starts_at,b.customer_name,b.customer_phone,b.status,b.created_at FROM bookings b JOIN workshops w ON w.id=b.workshop_id WHERE b.workshop_id=? ORDER BY b.starts_at DESC LIMIT 500").bind(id).fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn partner_update_booking(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<BookingStatusInput>)->Result<StatusCode,(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 if !["confirmed","completed","cancelled","declined"].contains(&input.status.as_str()){return Err((StatusCode::BAD_REQUEST,"예약 상태 값이 올바르지 않습니다".into()));}
 let result=sqlx::query("UPDATE bookings SET status=? WHERE id=? AND workshop_id=?").bind(input.status).bind(id).bind(workshop_id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"예약을 찾을 수 없습니다".into()));}
 Ok(StatusCode::NO_CONTENT)
}
async fn partner_services(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<Vec<WorkshopService>>,(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 let rows=sqlx::query_as::<_,WorkshopService>("SELECT id,name,description,price_from,duration_minutes FROM workshop_services WHERE workshop_id=? ORDER BY sort_order").bind(workshop_id).fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
#[derive(Deserialize)] struct ServiceInput { name:String, description:String, price_from:u64, duration_minutes:u16, sort_order:Option<i32> }
async fn create_partner_service(State(s):State<WorkshopState>,headers:HeaderMap,Json(input):Json<ServiceInput>)->Result<(StatusCode,Json<WorkshopService>),(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 if input.name.trim().is_empty()||input.price_from==0||input.duration_minutes==0{return Err((StatusCode::BAD_REQUEST,"서비스명·가격·소요 시간을 확인해 주세요".into()));}
 let id=Uuid::new_v4().to_string();
 sqlx::query("INSERT INTO workshop_services (id,workshop_id,name,description,price_from,duration_minutes,is_active,sort_order) VALUES (?,?,?,?,?,?,1,?)").bind(&id).bind(workshop_id).bind(input.name.trim()).bind(input.description.trim()).bind(input.price_from).bind(input.duration_minutes).bind(input.sort_order.unwrap_or(0)).execute(&s.db).await.map_err(internal)?;
 let row=sqlx::query_as::<_,WorkshopService>("SELECT id,name,description,price_from,duration_minutes FROM workshop_services WHERE id=?").bind(id).fetch_one(&s.db).await.map_err(internal)?;
 Ok((StatusCode::CREATED,Json(row)))
}
async fn update_partner_service(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<ServiceInput>)->Result<Json<WorkshopService>,(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 if input.name.trim().is_empty()||input.price_from==0||input.duration_minutes==0{return Err((StatusCode::BAD_REQUEST,"서비스명·가격·소요 시간을 확인해 주세요".into()));}
 let result=sqlx::query("UPDATE workshop_services SET name=?,description=?,price_from=?,duration_minutes=?,sort_order=? WHERE id=? AND workshop_id=?").bind(input.name.trim()).bind(input.description.trim()).bind(input.price_from).bind(input.duration_minutes).bind(input.sort_order.unwrap_or(0)).bind(&id).bind(workshop_id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"서비스를 찾을 수 없습니다".into()));}
 let row=sqlx::query_as::<_,WorkshopService>("SELECT id,name,description,price_from,duration_minutes FROM workshop_services WHERE id=?").bind(id).fetch_one(&s.db).await.map_err(internal)?;
 Ok(Json(row))
}
async fn delete_partner_service(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>)->Result<StatusCode,(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 let result=sqlx::query("UPDATE workshop_services SET is_active=0 WHERE id=? AND workshop_id=?").bind(id).bind(workshop_id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"서비스를 찾을 수 없습니다".into()));}
 Ok(StatusCode::NO_CONTENT)
}
async fn admin_workshops(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<Vec<AdminWorkshop>>,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let rows=sqlx::query_as::<_,AdminWorkshop>("SELECT id,name,district,category,status,created_at FROM workshops ORDER BY created_at DESC LIMIT 500").fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
#[derive(Deserialize)] struct WorkshopStatusInput { status:String }
async fn update_workshop_status(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<WorkshopStatusInput>)->Result<StatusCode,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 if !["approved","rejected","suspended","pending"].contains(&input.status.as_str()){return Err((StatusCode::BAD_REQUEST,"업체 상태 값이 올바르지 않습니다".into()));}
 let result=sqlx::query("UPDATE workshops SET status=? WHERE id=?").bind(input.status).bind(id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"업체를 찾을 수 없습니다".into()));}
 Ok(StatusCode::NO_CONTENT)
}
async fn admin_quotes(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<Vec<AdminQuote>>,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let rows=sqlx::query_as::<_,AdminQuote>("SELECT id,vehicle,service_category,description,district,customer_phone,status,created_at FROM quote_requests ORDER BY created_at DESC LIMIT 500").fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn admin_bookings(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<Vec<AdminBooking>>,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let rows=sqlx::query_as::<_,AdminBooking>("SELECT b.id,b.workshop_id,w.name AS workshop_name,b.vehicle,b.starts_at,b.customer_name,b.customer_phone,b.status,b.created_at FROM bookings b JOIN workshops w ON w.id=b.workshop_id ORDER BY b.starts_at DESC LIMIT 500").fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
#[derive(Deserialize)] struct BookingStatusInput { status:String }
async fn update_booking_status(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<BookingStatusInput>)->Result<StatusCode,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 if !["requested","confirmed","completed","cancelled","declined"].contains(&input.status.as_str()){return Err((StatusCode::BAD_REQUEST,"예약 상태 값이 올바르지 않습니다".into()));}
 let result=sqlx::query("UPDATE bookings SET status=? WHERE id=?").bind(input.status).bind(id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"예약을 찾을 수 없습니다".into()));}
 Ok(StatusCode::NO_CONTENT)
}
async fn public_categories(State(s):State<WorkshopState>)->Result<Json<Vec<ServiceCategory>>,(StatusCode,String)>{
 let rows=sqlx::query_as::<_,ServiceCategory>("SELECT id,group_name,name,slug,sort_order,is_active FROM service_categories WHERE is_active=1 ORDER BY group_sort,sort_order,id").fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn admin_categories(State(s):State<WorkshopState>,headers:HeaderMap)->Result<Json<Vec<ServiceCategory>>,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let rows=sqlx::query_as::<_,ServiceCategory>("SELECT id,group_name,name,slug,sort_order,is_active FROM service_categories ORDER BY group_sort,sort_order,id").fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn create_category(State(s):State<WorkshopState>,headers:HeaderMap,Json(input):Json<CategoryInput>)->Result<(StatusCode,Json<ServiceCategory>),(StatusCode,String)>{
 require_admin(&s,&headers)?;validate_category(&input)?;
 let id=Uuid::new_v4().to_string();let slug=format!("category-{}",id.replace('-',""));
 sqlx::query("INSERT INTO service_categories (id,group_name,name,slug,sort_order,is_active,group_sort) VALUES (?,?,?,?,?,?,?)")
  .bind(&id).bind(input.group_name.trim()).bind(input.name.trim()).bind(slug).bind(input.sort_order.unwrap_or(0)).bind(input.is_active.unwrap_or(true)).bind(group_sort(&input.group_name)).execute(&s.db).await.map_err(internal)?;
 let row=category_by_id(&s,&id).await?;Ok((StatusCode::CREATED,Json(row)))
}
async fn update_category(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<CategoryInput>)->Result<Json<ServiceCategory>,(StatusCode,String)>{
 require_admin(&s,&headers)?;validate_category(&input)?;
 let result=sqlx::query("UPDATE service_categories SET group_name=?,name=?,sort_order=?,is_active=?,group_sort=? WHERE id=?")
  .bind(input.group_name.trim()).bind(input.name.trim()).bind(input.sort_order.unwrap_or(0)).bind(input.is_active.unwrap_or(true)).bind(group_sort(&input.group_name)).bind(&id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"카테고리를 찾을 수 없습니다".into()));}
 Ok(Json(category_by_id(&s,&id).await?))
}
async fn delete_category(State(s):State<WorkshopState>,headers:HeaderMap,Path(id):Path<String>)->Result<StatusCode,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let result=sqlx::query("DELETE FROM service_categories WHERE id=?").bind(id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"카테고리를 찾을 수 없습니다".into()));}
 Ok(StatusCode::NO_CONTENT)
}
async fn category_by_id(s:&WorkshopState,id:&str)->Result<ServiceCategory,(StatusCode,String)>{
 sqlx::query_as::<_,ServiceCategory>("SELECT id,group_name,name,slug,sort_order,is_active FROM service_categories WHERE id=?").bind(id).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::NOT_FOUND,"카테고리를 찾을 수 없습니다".into()))
}
fn require_admin(s:&WorkshopState,headers:&HeaderMap)->Result<(),(StatusCode,String)>{
 let Some(expected)=s.admin_api_key.as_deref() else{return Err((StatusCode::SERVICE_UNAVAILABLE,"관리자 API 키가 서버에 설정되지 않았습니다".into()));};
 let provided=headers.get("x-admin-key").and_then(|v|v.to_str().ok()).unwrap_or("");
 if provided==expected {Ok(())} else {Err((StatusCode::UNAUTHORIZED,"관리자 API 키를 확인해 주세요".into()))}
}
fn validate_category(input:&CategoryInput)->Result<(),(StatusCode,String)>{
 if input.group_name.trim().is_empty()||input.group_name.chars().count()>60{return Err((StatusCode::BAD_REQUEST,"카테고리 그룹명을 확인해 주세요".into()));}
 if input.name.trim().is_empty()||input.name.chars().count()>80{return Err((StatusCode::BAD_REQUEST,"카테고리 이름은 1~80자로 입력해 주세요".into()));}
 if input.sort_order.unwrap_or(0)<0{return Err((StatusCode::BAD_REQUEST,"정렬 순서는 0 이상이어야 합니다".into()));}Ok(())
}
fn group_sort(group:&str)->i32{match group.trim(){"차량 관리"=>0,"자동차 매매"=>1,_=>2}}
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
async fn list_offers(State(s): State<WorkshopState>, Path(id): Path<String>) -> Result<Json<Vec<Offer>>, (StatusCode,String)> {
    let rows=sqlx::query_as::<_,Offer>("SELECT o.id,o.workshop_id,w.name AS workshop_name,o.amount,o.description,o.valid_until,o.created_at FROM quote_offers o JOIN workshops w ON w.id=o.workshop_id WHERE o.quote_id=? ORDER BY o.amount").bind(id).fetch_all(&s.db).await.map_err(internal)?;
    Ok(Json(rows))
}
async fn create_booking(State(s): State<WorkshopState>, Json(req): Json<BookingRequest>) -> Result<(StatusCode,Json<serde_json::Value>), (StatusCode,String)> {
    let id=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO bookings (id, workshop_id, service_id, vehicle, starts_at, customer_name, customer_phone, note, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'requested')").bind(&id).bind(req.workshop_id).bind(req.service_id).bind(req.vehicle).bind(req.starts_at).bind(req.customer_name).bind(req.customer_phone).bind(req.note).execute(&s.db).await.map_err(internal)?;
    Ok((StatusCode::CREATED,Json(serde_json::json!({"id":id,"status":"requested"}))))
}
fn internal(e: sqlx::Error)->(StatusCode,String){tracing::error!(error=%e,"database request failed");(StatusCode::INTERNAL_SERVER_ERROR,"요청을 처리하지 못했습니다".into())}
