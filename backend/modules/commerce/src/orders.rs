//! 주문: 장바구니 → 주문서 → 결제(카드·무통장) → 업체별 배송.
//!
//! - 배송(`delivery`): 주소로 보냄. 여러 업체 상품을 한 번에 주문할 수 있습니다.
//! - 방문 장착(`visit`): 한 업체 상품만, 희망 방문 시간을 고르면 그 업체에 예약 신청이 함께 만들어집니다.
//! - 비회원 주문은 주문번호와 연락처로 확인·취소합니다. 회원 주문은 마이페이지에 쌓입니다.

use std::collections::BTreeMap;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDateTime, NaiveTime};
use gageyo_core::{
    auth::{client_ip, Customer, MaybeCustomer, SessionUser},
    ratelimit, schedule, settings,
    util::{carrier, kst_now, new_id, normalize_email, normalize_phone, opt_text, phone_digits, req_line, tracking_url},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub const MAX_LINES: usize = 30;
pub const MAX_QTY: u32 = 99;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/checkout/config", get(checkout_config))
        .route("/orders", post(create_order))
        .route("/orders/lookup", post(lookup_order))
        .route("/orders/{id}", get(order_status))
        .route("/orders/{id}/cancel", post(customer_cancel))
        .route("/me/orders", get(my_orders))
        .route("/me/orders/{id}", get(my_order_detail))
}

/// 쓸 수 있는 결제 수단
pub struct Methods {
    pub toss: bool,
    pub bank: Option<settings::BankAccount>,
}

impl Methods {
    pub async fn load(s: &AppState) -> AppResult<Self> {
        Ok(Self { toss: s.cfg.toss_enabled(), bank: settings::bank_account(s).await? })
    }
    /// 결제 수단을 하나도 설정하지 않은 서버에서는 결제 없이 접수만 (예전 동작)
    pub fn none_allowed(&self) -> bool {
        !self.toss && self.bank.is_none()
    }
    pub fn pick(&self, requested: Option<&str>) -> AppResult<&'static str> {
        match requested.unwrap_or("") {
            "toss" | "card" if self.toss => Ok("toss"),
            "bank" if self.bank.is_some() => Ok("bank"),
            "none" if self.none_allowed() => Ok("none"),
            "" if self.toss => Ok("toss"),
            "" if self.bank.is_some() => Ok("bank"),
            "" if self.none_allowed() => Ok("none"),
            _ => Err(AppError::bad("선택한 결제 수단을 지금 쓸 수 없습니다")),
        }
    }
}

async fn checkout_config(State(s): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let m = Methods::load(&s).await?;
    Ok(Json(serde_json::json!({
        "toss": m.toss.then(|| serde_json::json!({ "client_key": s.cfg.toss_client_key })),
        "bank": m.bank,
        "none_allowed": m.none_allowed(),
        "carriers": gageyo_core::util::carriers_json(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct OrderItem {
    pub product_id: String,
    pub quantity: u32,
}

#[derive(Debug, Deserialize)]
pub struct OrderRequest {
    pub customer_name: String,
    pub customer_phone: String,
    pub email: Option<String>,
    pub address: Option<String>,
    /// delivery(기본) · visit
    pub fulfillment: Option<String>,
    pub visit_at: Option<NaiveDateTime>,
    pub vehicle: Option<String>,
    pub memo: Option<String>,
    /// toss · bank · none (비우면 쓸 수 있는 첫 수단)
    pub payment_method: Option<String>,
    pub depositor_name: Option<String>,
    pub items: Vec<OrderItem>,
}

/// 같은 상품 줄을 합치고 수량을 검사
pub fn merge_lines(items: &[OrderItem]) -> AppResult<Vec<(String, u32)>> {
    if items.is_empty() {
        return Err(AppError::bad("상품을 선택해 주세요"));
    }
    let mut map: BTreeMap<String, u32> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for it in items {
        if it.quantity == 0 {
            return Err(AppError::bad("수량을 확인해 주세요"));
        }
        let id = it.product_id.trim().to_string();
        if id.is_empty() || id.len() > 36 {
            return Err(AppError::bad("상품을 다시 선택해 주세요"));
        }
        let q = map.entry(id.clone()).or_insert(0);
        if *q == 0 {
            order.push(id);
        }
        *q += it.quantity;
        if *q > MAX_QTY {
            return Err(AppError::bad(format!("한 상품은 {MAX_QTY}개까지 주문할 수 있습니다")));
        }
    }
    if order.len() > MAX_LINES {
        return Err(AppError::bad(format!("한 번에 {MAX_LINES}가지 상품까지 주문할 수 있습니다")));
    }
    Ok(order.into_iter().map(|id| {
        let q = map[&id];
        (id, q)
    }).collect())
}

#[derive(sqlx::FromRow)]
struct LockedProduct {
    workshop_id: String,
    name: String,
    price: u64,
    stock: u32,
}

async fn create_order(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    headers: HeaderMap,
    Json(req): Json<OrderRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let name = req_line(&req.customer_name, "주문자 이름", 100)?;
    let phone = normalize_phone(&req.customer_phone)?;
    let email = match req.email.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
        Some(e) => Some(normalize_email(e)?),
        None => None,
    };
    let fulfillment = match req.fulfillment.as_deref().unwrap_or("delivery") {
        "delivery" => "delivery",
        "visit" => "visit",
        _ => return Err(AppError::bad("받는 방법을 다시 골라 주세요")),
    };
    let address = if fulfillment == "delivery" {
        Some(req_line(req.address.as_deref().unwrap_or(""), "배송 주소", 500)?)
    } else {
        opt_text(req.address.as_deref(), "주소", 500)?
    };
    let vehicle = if fulfillment == "visit" {
        Some(req_line(req.vehicle.as_deref().unwrap_or(""), "차량 정보", 160)?)
    } else {
        opt_text(req.vehicle.as_deref(), "차량 정보", 160)?
    };
    let memo = opt_text(req.memo.as_deref(), "요청 사항", 500)?;
    let lines = merge_lines(&req.items)?;
    let methods = Methods::load(&s).await?;
    let method = methods.pick(req.payment_method.as_deref())?;
    let depositor = if method == "bank" {
        Some(opt_text(req.depositor_name.as_deref(), "입금자명", 60)?.unwrap_or_else(|| name.clone()))
    } else {
        None
    };
    let ip = client_ip(&headers);
    if !ip.is_empty() && !ratelimit::hit(&s.db, &format!("order:{ip}"), 3600, 30).await? {
        return Err(AppError::TooMany("주문이 너무 많습니다. 잠시 후 다시 시도해 주세요".into()));
    }

    let order_id = new_id();
    let mut tx = s.db.begin().await?;
    let mut total: u64 = 0;
    let mut priced: Vec<(String, LockedProduct, u32)> = Vec::new();
    for (pid, qty) in &lines {
        let p = sqlx::query_as::<_, LockedProduct>(
            "SELECT p.workshop_id, p.name, p.price, p.stock FROM products p JOIN workshops w ON w.id = p.workshop_id \
             WHERE p.id = ? AND p.status IN ('active','sold_out') AND w.status = 'approved' FOR UPDATE",
        )
        .bind(pid)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("판매하지 않는 상품이 있습니다. 장바구니를 확인해 주세요"))?;
        if p.stock < *qty {
            return Err(AppError::Conflict(if p.stock == 0 {
                format!("{} 상품이 품절되었습니다", p.name)
            } else {
                format!("{} 상품 재고가 {}개 남았습니다", p.name, p.stock)
            }));
        }
        total = total.saturating_add(p.price.saturating_mul(u64::from(*qty)));
        priced.push((pid.clone(), p, *qty));
    }
    if total > 100_000_000 {
        return Err(AppError::bad("한 번에 1억 원 넘게 주문할 수 없습니다"));
    }
    let visit_workshop = if fulfillment == "visit" {
        let first = priced[0].1.workshop_id.clone();
        if priced.iter().any(|(_, p, _)| p.workshop_id != first) {
            return Err(AppError::bad("방문 장착은 한 업체 상품만 함께 주문할 수 있습니다"));
        }
        let visit_at = req.visit_at.ok_or_else(|| AppError::bad("방문 희망 시간을 골라 주세요"))?;
        let (open, close, closed): (Option<NaiveTime>, Option<NaiveTime>, String) =
            sqlx::query_as("SELECT open_time, close_time, closed_days FROM workshops WHERE id = ?").bind(&first).fetch_one(&mut *tx).await?;
        schedule::check_visit_time(visit_at, kst_now(), open, close, &closed)?;
        Some((first, visit_at))
    } else {
        None
    };
    sqlx::query(
        "INSERT INTO orders (id, customer_user_id, customer_name, customer_phone, email, address, status, total_amount, fulfillment, \
         visit_workshop_id, visit_at, vehicle, memo, payment_method, depositor_name) \
         VALUES (?, ?, ?, ?, ?, ?, 'payment_pending', ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&order_id)
    .bind(user.as_ref().map(|u| u.id.clone()))
    .bind(&name)
    .bind(&phone)
    .bind(&email)
    .bind(address.as_deref().unwrap_or(""))
    .bind(total)
    .bind(fulfillment)
    .bind(visit_workshop.as_ref().map(|v| v.0.clone()))
    .bind(visit_workshop.as_ref().map(|v| v.1))
    .bind(&vehicle)
    .bind(&memo)
    .bind(method)
    .bind(&depositor)
    .execute(&mut *tx)
    .await?;
    for (pid, p, qty) in &priced {
        sqlx::query(
            "INSERT INTO order_items (id, order_id, product_id, workshop_id, product_name, unit_price, quantity, status) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')",
        )
        .bind(new_id())
        .bind(&order_id)
        .bind(pid)
        .bind(&p.workshop_id)
        .bind(&p.name)
        .bind(p.price)
        .bind(qty)
        .execute(&mut *tx)
        .await?;
        // 같은 UPDATE 안에서 왼쪽 열부터 계산되므로 status 는 줄어든 재고로 판단합니다.
        sqlx::query("UPDATE products SET stock = stock - ?, status = IF(stock = 0, 'sold_out', status) WHERE id = ?")
            .bind(qty)
            .bind(pid)
            .execute(&mut *tx)
            .await?;
    }
    let mut booking_id = None;
    if let Some((wid, at)) = &visit_workshop {
        let bid = new_id();
        let names: Vec<String> = priced.iter().map(|(_, p, q)| format!("{} ×{q}", p.name)).collect();
        sqlx::query(
            "INSERT INTO bookings (id, customer_user_id, workshop_id, order_id, vehicle, starts_at, customer_name, customer_phone, note, status) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'requested')",
        )
        .bind(&bid)
        .bind(user.as_ref().map(|u| u.id.clone()))
        .bind(wid)
        .bind(&order_id)
        .bind(vehicle.as_deref().unwrap_or(""))
        .bind(at)
        .bind(&name)
        .bind(&phone)
        .bind(format!("상품 방문 장착: {}{}", names.join(", "), memo.as_deref().map(|m| format!(" / {m}")).unwrap_or_default()))
        .execute(&mut *tx)
        .await?;
        booking_id = Some(bid);
    }
    tx.commit().await?;

    crate::notify::order_received(&s, &order_id, total, methods.bank.as_ref().filter(|_| method == "bank"), depositor.as_deref()).await;
    if method == "none" {
        crate::notify::partners_new_order(&s, &order_id).await;
    }
    let order_name = match priced.len() {
        1 => priced[0].1.name.clone(),
        n => format!("{} 외 {}건", priced[0].1.name, n - 1),
    };
    let order_name: String = order_name.chars().take(100).collect();
    let mut res = serde_json::json!({
        "id": order_id, "total_amount": total, "status": "payment_pending", "payment_method": method,
        "order_name": order_name, "booking_id": booking_id,
    });
    match method {
        "toss" => {
            res["toss"] = serde_json::json!({
                "client_key": s.cfg.toss_client_key, "order_id": order_id, "order_name": order_name, "amount": total,
                "customer_name": name, "customer_email": email.or_else(|| user.as_ref().map(|u| u.email.clone())),
                "customer_key": user.as_ref().map(|u| u.id.clone()),
            })
        }
        "bank" => res["bank"] = serde_json::to_value(&methods.bank)?,
        _ => {}
    }
    Ok((StatusCode::CREATED, Json(res)))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrderRow {
    pub id: String,
    pub customer_user_id: Option<String>,
    pub customer_name: String,
    pub customer_phone: String,
    pub email: Option<String>,
    pub address: String,
    pub status: String,
    pub total_amount: u64,
    pub fulfillment: String,
    pub visit_workshop_id: Option<String>,
    pub visit_at: Option<NaiveDateTime>,
    pub vehicle: Option<String>,
    pub memo: Option<String>,
    pub payment_method: String,
    pub pay_detail: Option<String>,
    pub depositor_name: Option<String>,
    pub paid_at: Option<NaiveDateTime>,
    pub cancelled_at: Option<NaiveDateTime>,
    pub cancel_reason: Option<String>,
    pub refunded_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

pub const ORDER_COLS: &str = "id, customer_user_id, customer_name, customer_phone, email, address, status, total_amount, fulfillment, \
    visit_workshop_id, visit_at, vehicle, memo, payment_method, pay_detail, depositor_name, paid_at, cancelled_at, cancel_reason, refunded_at, created_at";

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ItemRow {
    pub id: String,
    pub product_id: String,
    pub workshop_id: Option<String>,
    pub workshop_name: Option<String>,
    pub product_name: String,
    pub unit_price: u64,
    pub quantity: u32,
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_no: Option<String>,
    pub shipped_at: Option<NaiveDateTime>,
    pub delivered_at: Option<NaiveDateTime>,
}

pub async fn load_order(s: &AppState, id: &str) -> AppResult<OrderRow> {
    sqlx::query_as::<_, OrderRow>(&format!("SELECT {ORDER_COLS} FROM orders WHERE id = ?"))
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| AppError::not_found("주문을 찾을 수 없습니다"))
}

pub async fn load_items(s: &AppState, order_id: &str) -> AppResult<Vec<ItemRow>> {
    Ok(sqlx::query_as::<_, ItemRow>(
        "SELECT oi.id, oi.product_id, oi.workshop_id, w.name AS workshop_name, oi.product_name, oi.unit_price, oi.quantity, oi.status, \
         oi.carrier, oi.tracking_no, oi.shipped_at, oi.delivered_at FROM order_items oi LEFT JOIN workshops w ON w.id = oi.workshop_id \
         WHERE oi.order_id = ? ORDER BY w.name, oi.product_name",
    )
    .bind(order_id)
    .fetch_all(&s.db)
    .await?)
}

/// 주문 상세 JSON (고객·운영자 공통)
pub async fn order_json(s: &AppState, o: &OrderRow) -> AppResult<serde_json::Value> {
    let items = load_items(s, &o.id).await?;
    let mut v = serde_json::to_value(o)?;
    v["items"] = serde_json::json!(items
        .iter()
        .map(|it| {
            let mut iv = serde_json::to_value(it).unwrap_or_default();
            let code = it.carrier.as_deref().unwrap_or("");
            iv["carrier_name"] = serde_json::json!(carrier(code).map(|c| c.1).unwrap_or(""));
            iv["tracking_url"] = serde_json::json!(tracking_url(code, it.tracking_no.as_deref().unwrap_or("")));
            iv["amount"] = serde_json::json!(it.unit_price * u64::from(it.quantity));
            iv
        })
        .collect::<Vec<_>>());
    v["status_label"] = serde_json::json!(crate::order_status_label(&o.status));
    v["cancellable"] = serde_json::json!(o.status == "payment_pending");
    if o.status == "payment_pending" && o.payment_method == "bank" {
        v["bank"] = serde_json::to_value(settings::bank_account(s).await?)?;
    }
    if o.status == "payment_pending" && o.payment_method == "toss" {
        v["toss"] = serde_json::json!({ "client_key": s.cfg.toss_client_key, "order_id": o.id, "amount": o.total_amount,
            "order_name": order_name(&items), "customer_name": o.customer_name, "customer_email": o.email });
    }
    let booking: Option<(String, String)> = sqlx::query_as("SELECT id, status FROM bookings WHERE order_id = ? LIMIT 1")
        .bind(&o.id)
        .fetch_optional(&s.db)
        .await?;
    v["booking"] = booking.map(|(id, st)| serde_json::json!({"id": id, "status": st})).unwrap_or(serde_json::Value::Null);
    Ok(v)
}

pub fn order_name(items: &[ItemRow]) -> String {
    let n: String = match items.len() {
        0 => "가게요 주문".into(),
        1 => items[0].product_name.clone(),
        n => format!("{} 외 {}건", items[0].product_name, n - 1),
    };
    n.chars().take(100).collect()
}

/// 주문을 볼 수 있는지: 회원 주문은 그 회원, 아니면 연락처가 맞을 때
pub fn can_access(o: &OrderRow, user: Option<&SessionUser>, phone: Option<&str>) -> bool {
    let owner_ok = matches!((&o.customer_user_id, user), (Some(owner), Some(u)) if owner == &u.id);
    let phone_ok = phone.map(phone_digits).is_some_and(|p| !p.is_empty() && p == phone_digits(&o.customer_phone));
    owner_ok || phone_ok
}

#[derive(Debug, Deserialize)]
pub struct LookupInput {
    pub order_id: String,
    pub phone: String,
}

async fn lookup_order(State(s): State<AppState>, headers: HeaderMap, Json(b): Json<LookupInput>) -> AppResult<Json<serde_json::Value>> {
    let ip = client_ip(&headers);
    if !ratelimit::hit(&s.db, &format!("lookup:{ip}"), 900, 30).await? {
        return Err(AppError::TooMany("잠시 후 다시 시도해 주세요".into()));
    }
    let o = load_order(&s, b.order_id.trim()).await.map_err(|_| AppError::not_found("주문번호와 연락처를 다시 확인해 주세요"))?;
    if !can_access(&o, None, Some(&b.phone)) {
        return Err(AppError::not_found("주문번호와 연락처를 다시 확인해 주세요"));
    }
    Ok(Json(order_json(&s, &o).await?))
}

/// 예전 화면 호환: 주문번호만으로 상태·금액 확인 (개인정보는 내려주지 않음)
async fn order_status(State(s): State<AppState>, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let o = load_order(&s, &id).await?;
    Ok(Json(serde_json::json!({ "id": o.id, "status": o.status, "status_label": crate::order_status_label(&o.status),
        "total_amount": o.total_amount, "created_at": o.created_at })))
}

async fn my_orders(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<Vec<serde_json::Value>>> {
    let rows: Vec<(String, String, u64, String, String, NaiveDateTime, String, i64)> = sqlx::query_as(
        "SELECT o.id, o.status, o.total_amount, o.fulfillment, o.payment_method, o.created_at, \
           COALESCE((SELECT oi.product_name FROM order_items oi WHERE oi.order_id = o.id ORDER BY oi.product_name LIMIT 1), ''), \
           (SELECT COUNT(*) FROM order_items oi WHERE oi.order_id = o.id) \
         FROM orders o WHERE o.customer_user_id = ? ORDER BY o.created_at DESC LIMIT 200",
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, status, total, fulfillment, method, created_at, first, n)| {
                serde_json::json!({"id": id, "status": status, "status_label": crate::order_status_label(&status), "total_amount": total,
                    "fulfillment": fulfillment, "payment_method": method, "created_at": created_at,
                    "title": if n > 1 { format!("{first} 외 {}건", n - 1) } else { first }})
            })
            .collect(),
    ))
}

async fn my_order_detail(State(s): State<AppState>, Customer(u): Customer, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let o = load_order(&s, &id).await?;
    if o.customer_user_id.as_deref() != Some(u.id.as_str()) {
        return Err(AppError::not_found("주문을 찾을 수 없습니다"));
    }
    Ok(Json(order_json(&s, &o).await?))
}

#[derive(Debug, Deserialize)]
pub struct CancelInput {
    pub phone: Option<String>,
    pub reason: Option<String>,
}

/// 고객 취소: 결제 전 주문만 (결제 후 취소는 고객센터·운영자)
async fn customer_cancel(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Json(b): Json<CancelInput>,
) -> AppResult<StatusCode> {
    let o = load_order(&s, &id).await?;
    if !can_access(&o, user.as_ref(), b.phone.as_deref()) {
        return Err(AppError::Forbidden("주문할 때 적은 연락처를 입력해 주세요".into()));
    }
    if o.status != "payment_pending" {
        return Err(AppError::Conflict("결제가 끝난 주문은 고객센터로 취소를 요청해 주세요".into()));
    }
    let reason = opt_text(b.reason.as_deref(), "취소 사유", 300)?.unwrap_or_else(|| "고객 취소".into());
    cancel_in_db(&s, &id, "cancelled", &reason).await?;
    crate::notify::order_cancelled(&s, &id, false, Some(&reason)).await;
    Ok(StatusCode::NO_CONTENT)
}

/// 주문을 취소 상태로 바꾸고 재고·예약을 되돌립니다 (결제 취소는 부르는 쪽에서 먼저).
pub async fn cancel_in_db(s: &AppState, order_id: &str, final_status: &str, reason: &str) -> AppResult<()> {
    let mut tx = s.db.begin().await?;
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM orders WHERE id = ? FOR UPDATE")
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await?;
    let status = status.ok_or_else(|| AppError::not_found("주문을 찾을 수 없습니다"))?;
    if ["cancelled", "refunded"].contains(&status.as_str()) {
        return Err(AppError::Conflict("이미 취소된 주문입니다".into()));
    }
    let items: Vec<(String, String, u32)> =
        sqlx::query_as("SELECT id, product_id, quantity FROM order_items WHERE order_id = ? AND status <> 'cancelled' FOR UPDATE")
            .bind(order_id)
            .fetch_all(&mut *tx)
            .await?;
    for (_, product_id, qty) in &items {
        sqlx::query("UPDATE products SET stock = stock + ?, status = IF(status = 'sold_out', 'active', status) WHERE id = ?")
            .bind(qty)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE order_items SET status = 'cancelled' WHERE order_id = ?").bind(order_id).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE orders SET status = ?, cancelled_at = NOW(), cancel_reason = ?, refunded_at = IF(? = 'refunded', NOW(), refunded_at) WHERE id = ?",
    )
    .bind(final_status)
    .bind(reason)
    .bind(final_status)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE bookings SET status = 'cancelled', decline_reason = '주문 취소' WHERE order_id = ? AND status IN ('requested','confirmed')")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, q: u32) -> OrderItem {
        OrderItem { product_id: id.into(), quantity: q }
    }

    #[test]
    fn merges_and_limits() {
        let m = merge_lines(&[item("a", 2), item("b", 1), item("a", 3)]).unwrap();
        assert_eq!(m, vec![("a".to_string(), 5), ("b".to_string(), 1)]);
        assert!(merge_lines(&[]).is_err());
        assert!(merge_lines(&[item("a", 0)]).is_err());
        assert!(merge_lines(&[item("a", 60), item("a", 40)]).is_err());
        let many: Vec<OrderItem> = (0..31).map(|i| item(&format!("p{i}"), 1)).collect();
        assert!(merge_lines(&many).is_err());
    }

    #[test]
    fn payment_method_pick() {
        let none = Methods { toss: false, bank: None };
        assert_eq!(none.pick(None).unwrap(), "none");
        assert!(none.pick(Some("toss")).is_err());
        let toss = Methods { toss: true, bank: None };
        assert_eq!(toss.pick(None).unwrap(), "toss");
        assert_eq!(toss.pick(Some("card")).unwrap(), "toss");
        assert!(toss.pick(Some("none")).is_err(), "결제 수단이 있으면 결제 없이 접수할 수 없음");
        assert!(toss.pick(Some("bank")).is_err());
        let bank = Methods { toss: false, bank: Some(gageyo_core::settings::BankAccount::default()) };
        assert_eq!(bank.pick(None).unwrap(), "bank");
    }
}
