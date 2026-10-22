use axum::{extract::{Path, Query, State}, http::StatusCode, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use uuid::Uuid;

#[derive(Clone)] pub struct CommerceState { pub db: MySqlPool }
#[derive(Serialize, sqlx::FromRow)] pub struct Product { pub id:String, pub workshop_id:String, pub name:String, pub category:String, pub description:String, pub price:u64, pub stock:u32, pub image_url:Option<String> }
#[derive(Serialize, sqlx::FromRow)] pub struct OrderStatus { pub id:String, pub status:String, pub total_amount:u64, pub created_at:chrono::NaiveDateTime }
#[derive(sqlx::FromRow)] struct LockedProduct { name:String, price:u64, stock:u32 }
#[derive(Deserialize)] pub struct ProductQuery { pub q:Option<String>, pub category:Option<String> }
#[derive(Deserialize)] pub struct OrderRequest { pub customer_name:String, pub customer_phone:String, pub address:String, pub items:Vec<OrderItem> }
#[derive(Deserialize)] pub struct OrderItem { pub product_id:String, pub quantity:u32 }
pub fn routes()->Router<CommerceState>{Router::new().route("/products",get(products)).route("/products/{id}",get(product_detail)).route("/orders",post(create_order)).route("/orders/{id}",get(order_status))}

async fn products(State(s):State<CommerceState>,Query(q):Query<ProductQuery>)->Result<Json<Vec<Product>>,(StatusCode,String)>{
 let rows=sqlx::query_as::<_,Product>("SELECT id, workshop_id, name, category, description, price, stock, image_url FROM products WHERE status='active' AND (? IS NULL OR category=?) AND (? IS NULL OR name LIKE CONCAT('%',?,'%')) ORDER BY created_at DESC LIMIT 100").bind(q.category.clone()).bind(q.category).bind(q.q.clone()).bind(q.q).fetch_all(&s.db).await.map_err(internal)?;
 Ok(Json(rows))
}
async fn product_detail(State(s):State<CommerceState>,Path(id):Path<String>)->Result<Json<Product>,(StatusCode,String)>{
 let product=sqlx::query_as::<_,Product>("SELECT id,workshop_id,name,category,description,price,stock,image_url FROM products WHERE id=? AND status='active'").bind(id).fetch_optional(&s.db).await.map_err(internal)?.ok_or((StatusCode::NOT_FOUND,"상품을 찾을 수 없습니다".into()))?;
 Ok(Json(product))
}
async fn create_order(State(s):State<CommerceState>,Json(req):Json<OrderRequest>)->Result<(StatusCode,Json<serde_json::Value>),(StatusCode,String)>{
 if req.items.is_empty(){return Err((StatusCode::BAD_REQUEST,"상품을 선택해 주세요".into()));}
 let mut tx=s.db.begin().await.map_err(internal)?; let order_id=Uuid::new_v4().to_string();
 sqlx::query("INSERT INTO orders (id,customer_name,customer_phone,address,status,total_amount) VALUES (?,?,?,?,'payment_pending',0)").bind(&order_id).bind(req.customer_name).bind(req.customer_phone).bind(req.address).execute(&mut *tx).await.map_err(internal)?;
 let mut total:u64=0;
 for item in req.items {
  if item.quantity==0{return Err((StatusCode::BAD_REQUEST,"수량을 확인해 주세요".into()));}
  let p=sqlx::query_as::<_,LockedProduct>("SELECT name,price,stock FROM products WHERE id=? AND status='active' FOR UPDATE").bind(&item.product_id).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or((StatusCode::NOT_FOUND,"상품을 찾을 수 없습니다".into()))?;
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
