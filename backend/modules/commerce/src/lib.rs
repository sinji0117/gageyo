use axum::{extract::{Path, Query, State}, http::{HeaderMap, StatusCode}, routing::{get, patch, post}, Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use uuid::Uuid;

#[derive(Clone)] pub struct CommerceState { pub db: MySqlPool, pub admin_api_key: Option<String> }
#[derive(Serialize, sqlx::FromRow)] pub struct Product { pub id:String, pub workshop_id:String, pub name:String, pub category:String, pub description:String, pub price:u64, pub stock:u32, pub image_url:Option<String> }
#[derive(Serialize, sqlx::FromRow)] pub struct OrderStatus { pub id:String, pub status:String, pub total_amount:u64, pub created_at:chrono::NaiveDateTime }
#[derive(sqlx::FromRow)] struct LockedProduct { name:String, price:u64, stock:u32 }
#[derive(Serialize,sqlx::FromRow)] pub struct AdminOrder { pub id:String, pub customer_name:String, pub customer_phone:String, pub address:String, pub status:String, pub total_amount:u64, pub created_at:chrono::NaiveDateTime }
#[derive(Serialize,sqlx::FromRow)] pub struct AdminProduct { pub id:String, pub workshop_id:String, pub workshop_name:String, pub name:String, pub category:String, pub price:u64, pub stock:u32, pub status:String, pub created_at:chrono::NaiveDateTime }
#[derive(Serialize,sqlx::FromRow)] pub struct PartnerOrder { pub id:String, pub customer_name:String, pub customer_phone:String, pub address:String, pub status:String, pub total_amount:u64, pub product_name:String, pub quantity:u32, pub created_at:chrono::NaiveDateTime }
#[derive(Deserialize)] pub struct ProductInput { pub name:String, pub category:String, pub description:String, pub price:u64, pub stock:u32, pub image_url:Option<String> }
#[derive(Deserialize)] pub struct ProductQuery { pub q:Option<String>, pub category:Option<String> }
#[derive(Deserialize)] pub struct OrderRequest { pub customer_name:String, pub customer_phone:String, pub address:String, pub items:Vec<OrderItem> }
#[derive(Deserialize)] pub struct OrderItem { pub product_id:String, pub quantity:u32 }
pub fn routes()->Router<CommerceState>{Router::new().route("/products",get(products)).route("/products/{id}",get(product_detail)).route("/orders",post(create_order)).route("/orders/{id}",get(order_status)).route("/admin/orders",get(admin_orders)).route("/admin/orders/{id}",patch(update_order_status)).route("/admin/products",get(admin_products)).route("/admin/products/{id}",patch(update_product_status)).route("/partner/products",get(partner_products).post(create_partner_product)).route("/partner/products/{id}",patch(update_partner_product)).route("/partner/orders",get(partner_orders))}

fn require_admin(s:&CommerceState,headers:&HeaderMap)->Result<(),(StatusCode,String)>{
 let Some(expected)=s.admin_api_key.as_deref() else{return Err((StatusCode::SERVICE_UNAVAILABLE,"관리자 API 키가 서버에 설정되지 않았습니다".into()));};
 let provided=headers.get("x-admin-key").and_then(|v|v.to_str().ok()).unwrap_or("");
 if provided==expected {Ok(())} else {Err((StatusCode::UNAUTHORIZED,"관리자 API 키를 확인해 주세요".into()))}
}
async fn admin_orders(State(s):State<CommerceState>,headers:HeaderMap)->Result<Json<Vec<AdminOrder>>,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let rows=sqlx::query_as::<_,AdminOrder>("SELECT id,customer_name,customer_phone,address,status,total_amount,created_at FROM orders ORDER BY created_at DESC LIMIT 500").fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
#[derive(Deserialize)] struct OrderStatusInput { status:String }
async fn update_order_status(State(s):State<CommerceState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<OrderStatusInput>)->Result<StatusCode,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 if !["payment_pending","paid","preparing","shipped","delivered","cancelled","refunded"].contains(&input.status.as_str()){return Err((StatusCode::BAD_REQUEST,"주문 상태 값이 올바르지 않습니다".into()));}
 let result=sqlx::query("UPDATE orders SET status=? WHERE id=?").bind(input.status).bind(id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"주문을 찾을 수 없습니다".into()));}
 Ok(StatusCode::NO_CONTENT)
}
async fn admin_products(State(s):State<CommerceState>,headers:HeaderMap)->Result<Json<Vec<AdminProduct>>,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 let rows=sqlx::query_as::<_,AdminProduct>("SELECT p.id,p.workshop_id,w.name AS workshop_name,p.name,p.category,p.price,p.stock,p.status,p.created_at FROM products p JOIN workshops w ON w.id=p.workshop_id ORDER BY p.created_at DESC LIMIT 500").fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
#[derive(Deserialize)] struct ProductStatusInput { status:String }
async fn update_product_status(State(s):State<CommerceState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<ProductStatusInput>)->Result<StatusCode,(StatusCode,String)>{
 require_admin(&s,&headers)?;
 if !["draft","active","sold_out","archived"].contains(&input.status.as_str()){return Err((StatusCode::BAD_REQUEST,"상품 상태 값이 올바르지 않습니다".into()));}
 let result=sqlx::query("UPDATE products SET status=? WHERE id=?").bind(input.status).bind(id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"상품을 찾을 수 없습니다".into()));}
 Ok(StatusCode::NO_CONTENT)
}
fn token_hash(token:&str)->String { Sha256::digest(token.as_bytes()).iter().map(|byte|format!("{byte:02x}")).collect() }
async fn partner_workshop_id(s:&CommerceState,headers:&HeaderMap)->Result<String,(StatusCode,String)>{
 let token=headers.get("x-partner-key").and_then(|value|value.to_str().ok()).unwrap_or("");
 if token.is_empty(){return Err((StatusCode::UNAUTHORIZED,"파트너 키를 입력해 주세요".into()));}
 sqlx::query_scalar::<_,String>("SELECT workshop_id FROM workshop_access_tokens WHERE token_hash=?").bind(token_hash(token)).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::UNAUTHORIZED,"파트너 키를 확인해 주세요".into()))
}
async fn partner_products(State(s):State<CommerceState>,headers:HeaderMap)->Result<Json<Vec<Product>>,(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 let rows=sqlx::query_as::<_,Product>("SELECT id,workshop_id,name,category,description,price,stock,image_url FROM products WHERE workshop_id=? ORDER BY created_at DESC").bind(workshop_id).fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn create_partner_product(State(s):State<CommerceState>,headers:HeaderMap,Json(input):Json<ProductInput>)->Result<(StatusCode,Json<Product>),(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 if input.name.trim().is_empty()||input.category.trim().is_empty()||input.price==0{return Err((StatusCode::BAD_REQUEST,"상품명·분류·가격을 확인해 주세요".into()));}
 let id=Uuid::new_v4().to_string();
 sqlx::query("INSERT INTO products (id,workshop_id,name,category,description,price,stock,image_url,status) VALUES (?,?,?,?,?,?,?,?,?)").bind(&id).bind(&workshop_id).bind(input.name.trim()).bind(input.category.trim()).bind(input.description.trim()).bind(input.price).bind(input.stock).bind(input.image_url).bind(if input.stock==0{"sold_out"}else{"active"}).execute(&s.db).await.map_err(internal)?;
 let row=sqlx::query_as::<_,Product>("SELECT id,workshop_id,name,category,description,price,stock,image_url FROM products WHERE id=?").bind(id).fetch_one(&s.db).await.map_err(internal)?;
 Ok((StatusCode::CREATED,Json(row)))
}
async fn update_partner_product(State(s):State<CommerceState>,headers:HeaderMap,Path(id):Path<String>,Json(input):Json<ProductInput>)->Result<Json<Product>,(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 if input.name.trim().is_empty()||input.category.trim().is_empty()||input.price==0{return Err((StatusCode::BAD_REQUEST,"상품명·분류·가격을 확인해 주세요".into()));}
 let result=sqlx::query("UPDATE products SET name=?,category=?,description=?,price=?,stock=?,image_url=?,status=? WHERE id=? AND workshop_id=?").bind(input.name.trim()).bind(input.category.trim()).bind(input.description.trim()).bind(input.price).bind(input.stock).bind(input.image_url).bind(if input.stock==0{"sold_out"}else{"active"}).bind(&id).bind(workshop_id).execute(&s.db).await.map_err(internal)?;
 if result.rows_affected()==0{return Err((StatusCode::NOT_FOUND,"상품을 찾을 수 없습니다".into()));}
 let row=sqlx::query_as::<_,Product>("SELECT id,workshop_id,name,category,description,price,stock,image_url FROM products WHERE id=?").bind(id).fetch_one(&s.db).await.map_err(internal)?;
 Ok(Json(row))
}
async fn partner_orders(State(s):State<CommerceState>,headers:HeaderMap)->Result<Json<Vec<PartnerOrder>>,(StatusCode,String)>{
 let workshop_id=partner_workshop_id(&s,&headers).await?;
 let rows=sqlx::query_as::<_,PartnerOrder>("SELECT o.id,o.customer_name,o.customer_phone,o.address,o.status,o.total_amount,oi.product_name,oi.quantity,o.created_at FROM orders o JOIN order_items oi ON oi.order_id=o.id JOIN products p ON p.id=oi.product_id WHERE p.workshop_id=? ORDER BY o.created_at DESC LIMIT 500").bind(workshop_id).fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}

async fn products(State(s):State<CommerceState>,Query(q):Query<ProductQuery>)->Result<Json<Vec<Product>>,(StatusCode,String)>{
 let rows=sqlx::query_as::<_,Product>("SELECT p.id,p.workshop_id,p.name,p.category,p.description,p.price,p.stock,p.image_url FROM products p JOIN workshops w ON w.id=p.workshop_id WHERE p.status='active' AND w.status='approved' AND (? IS NULL OR p.category=?) AND (? IS NULL OR p.name LIKE CONCAT('%',?,'%')) ORDER BY p.created_at DESC LIMIT 100").bind(q.category.clone()).bind(q.category).bind(q.q.clone()).bind(q.q).fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn product_detail(State(s):State<CommerceState>,Path(id):Path<String>)->Result<Json<Product>,(StatusCode,String)>{
 let product=sqlx::query_as::<_,Product>("SELECT p.id,p.workshop_id,p.name,p.category,p.description,p.price,p.stock,p.image_url FROM products p JOIN workshops w ON w.id=p.workshop_id WHERE p.id=? AND p.status='active' AND w.status='approved'").bind(id).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::NOT_FOUND,"상품을 찾을 수 없습니다".into()))?;
 Ok(Json(product))
}
async fn create_order(State(s):State<CommerceState>,Json(req):Json<OrderRequest>)->Result<(StatusCode,Json<serde_json::Value>),(StatusCode,String)>{
 if req.items.is_empty(){return Err((StatusCode::BAD_REQUEST,"상품을 선택해 주세요".into()));}
 let mut tx=s.db.begin().await.map_err(internal)?; let order_id=Uuid::new_v4().to_string();
 sqlx::query("INSERT INTO orders (id,customer_name,customer_phone,address,status,total_amount) VALUES (?,?,?,?,'payment_pending',0)").bind(&order_id).bind(req.customer_name).bind(req.customer_phone).bind(req.address).execute(&mut *tx).await.map_err(internal)?;
 let mut total:u64=0;
 for item in req.items {
  if item.quantity==0{return Err((StatusCode::BAD_REQUEST,"수량을 확인해 주세요".into()));}
  let p=sqlx::query_as::<_,LockedProduct>("SELECT p.name,p.price,p.stock FROM products p JOIN workshops w ON w.id=p.workshop_id WHERE p.id=? AND p.status='active' AND w.status='approved' FOR UPDATE").bind(&item.product_id).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or((StatusCode::NOT_FOUND,"상품을 찾을 수 없습니다".into()))?;
  if p.stock<item.quantity{return Err((StatusCode::CONFLICT,format!("{} 상품 재고가 부족합니다",p.name)));}
  total+=p.price*(item.quantity as u64);
  sqlx::query("INSERT INTO order_items (id,order_id,product_id,product_name,unit_price,quantity) VALUES (?,?,?,?,?,?)").bind(Uuid::new_v4().to_string()).bind(&order_id).bind(&item.product_id).bind(p.name).bind(p.price).bind(item.quantity).execute(&mut *tx).await.map_err(internal)?;
  sqlx::query("UPDATE products SET stock=stock-? WHERE id=?").bind(item.quantity).bind(item.product_id).execute(&mut *tx).await.map_err(internal)?;
 }
 sqlx::query("UPDATE orders SET total_amount=? WHERE id=?").bind(total).bind(&order_id).execute(&mut *tx).await.map_err(internal)?;
 tx.commit().await.map_err(internal)?;
 Ok((StatusCode::CREATED,Json(serde_json::json!({"id":order_id,"total_amount":total,"status":"payment_pending"}))))
}
async fn order_status(State(s):State<CommerceState>,Path(id):Path<String>)->Result<Json<OrderStatus>,(StatusCode,String)>{
 let order=sqlx::query_as::<_,OrderStatus>("SELECT id,status,total_amount,created_at FROM orders WHERE id=?").bind(id).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::NOT_FOUND,"주문을 찾을 수 없습니다".into()))?;
 Ok(Json(order))
}
fn internal(e:sqlx::Error)->(StatusCode,String){tracing::error!(error=%e,"database request failed");(StatusCode::INTERNAL_SERVER_ERROR,"요청을 처리하지 못했습니다".into())}
