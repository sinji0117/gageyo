//! 문의·신고 (고객 · 파트너 · 운영자 답변)

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use gageyo_core::{
    audit,
    auth::{self, client_ip, Admin, Customer, MaybeCustomer, Partner, SessionUser},
    mail::Mail,
    ratelimit,
    util::{new_id, normalize_email, req_line, req_text, Page},
    AppError, AppResult, AppState,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/support/inquiries", post(create_customer_inquiry))
        .route("/support/inquiries/{id}", get(customer_thread))
        .route("/support/inquiries/{id}/messages", post(customer_reply))
        .route("/me/inquiries", get(my_inquiries))
        .route("/reports", post(create_report))
        .route("/partner/inquiries", get(partner_inquiries).post(create_partner_inquiry))
        .route("/partner/inquiries/{id}", get(partner_thread))
        .route("/partner/inquiries/{id}/messages", post(partner_reply))
        .route("/admin/inquiries", get(admin_inquiries))
        .route("/admin/inquiries/{id}", get(admin_thread).patch(admin_set_status))
        .route("/admin/inquiries/{id}/reply", post(admin_reply))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Inquiry {
    pub id: String,
    pub kind: String,
    pub user_id: Option<String>,
    pub workshop_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub subject: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub status: String,
    #[serde(skip)]
    pub token_hash: Option<String>,
    pub created_at: NaiveDateTime,
    pub last_message_at: NaiveDateTime,
}

const COLS: &str = "id, kind, user_id, workshop_id, name, email, subject, target_type, target_id, status, token_hash, created_at, last_message_at";

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Message {
    pub id: String,
    pub author: String,
    pub body: String,
    pub created_at: NaiveDateTime,
}

async fn load(s: &AppState, id: &str) -> AppResult<Inquiry> {
    sqlx::query_as::<_, Inquiry>(&format!("SELECT {COLS} FROM inquiries WHERE id = ?"))
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| AppError::not_found("문의를 찾을 수 없습니다"))
}

async fn thread_json(s: &AppState, inq: &Inquiry) -> AppResult<serde_json::Value> {
    let messages = sqlx::query_as::<_, Message>("SELECT id, author, body, created_at FROM inquiry_messages WHERE inquiry_id = ? ORDER BY created_at, id")
        .bind(&inq.id)
        .fetch_all(&s.db)
        .await?;
    let mut v = serde_json::to_value(inq)?;
    v["messages"] = serde_json::to_value(messages)?;
    Ok(v)
}

async fn add_message(s: &AppState, inquiry_id: &str, author: &str, body: &str, status: &str) -> AppResult<()> {
    let mut tx = s.db.begin().await?;
    sqlx::query("INSERT INTO inquiry_messages (id, inquiry_id, author, body) VALUES (?, ?, ?, ?)")
        .bind(new_id())
        .bind(inquiry_id)
        .bind(author)
        .bind(body)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE inquiries SET status = ?, last_message_at = NOW() WHERE id = ?").bind(status).bind(inquiry_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

fn ops_notice(s: &AppState, kind: &str, subject: &str) {
    let what = match kind {
        "partner" => "파트너 문의",
        "report" => "신고",
        _ => "고객 문의",
    };
    s.mailer.send_all(
        &s.cfg.ops_recipients(),
        &format!("[가게요 운영] 새 {what}: {subject}"),
        &format!("운영자 화면의 문의·신고에서 확인해 주세요.\n{}", s.cfg.admin_url),
    );
}

#[derive(Debug, Deserialize)]
pub struct NewInquiry {
    pub name: Option<String>,
    pub email: Option<String>,
    pub subject: String,
    pub body: String,
    /// 봇 막기용 숨은 칸 (사람은 비워 둠)
    pub website: Option<String>,
}

async fn create_customer_inquiry(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    headers: HeaderMap,
    Json(b): Json<NewInquiry>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if b.website.as_deref().is_some_and(|w| !w.trim().is_empty()) {
        // 봇: 성공처럼 보이게 하고 저장하지 않음
        return Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": new_id() }))));
    }
    let subject = req_line(&b.subject, "제목", 200)?;
    let body = req_text(&b.body, "내용", 5000)?;
    let (name, email) = match &user {
        Some(u) => (u.display_name.clone(), Some(u.email.clone())),
        None => (
            req_line(b.name.as_deref().unwrap_or(""), "이름", 100)?,
            Some(normalize_email(b.email.as_deref().unwrap_or("")).map_err(|_| AppError::bad("답변 받을 이메일을 입력해 주세요"))?),
        ),
    };
    let ip = client_ip(&headers);
    if !ratelimit::hit(&s.db, &format!("inquiry:{ip}"), 3600, 10).await? {
        return Err(AppError::TooMany("문의를 너무 많이 남겼습니다. 잠시 후 다시 시도해 주세요".into()));
    }
    let id = new_id();
    // 비회원은 비밀 링크로만 다시 볼 수 있습니다 (48자)
    let token = user.is_none().then(|| auth::random_token()[..48].to_string());
    sqlx::query("INSERT INTO inquiries (id, kind, user_id, name, email, subject, token_hash, ip) VALUES (?, 'customer', ?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(user.as_ref().map(|u| u.id.clone()))
        .bind(&name)
        .bind(&email)
        .bind(&subject)
        .bind(token.as_deref().map(auth::token_hash))
        .bind(&ip)
        .execute(&s.db)
        .await?;
    add_message(&s, &id, "user", &body, "open").await?;
    ops_notice(&s, "customer", &subject);
    if let (Some(t), Some(e)) = (&token, &email) {
        s.mailer.send(Mail::new(
            e.clone(),
            format!("[가게요] 문의가 접수되었습니다: {subject}"),
            format!("답변이 등록되면 메일로 알려 드립니다. 아래 링크에서 답변을 확인하고 이어서 물을 수 있습니다.\n{}/?inquiry={id}&token={t}", s.cfg.app_url),
        ));
    }
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id, "token": token }))))
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

fn customer_can_see(inq: &Inquiry, user: Option<&SessionUser>, token: Option<&str>) -> bool {
    if inq.kind == "partner" {
        return false;
    }
    let owner = matches!((&inq.user_id, user), (Some(o), Some(u)) if o == &u.id);
    let by_token = match (&inq.token_hash, token) {
        (Some(h), Some(t)) if !t.is_empty() => auth::constant_eq(h, &auth::token_hash(t)),
        _ => false,
    };
    owner || by_token
}

async fn customer_thread(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Query(q): Query<TokenQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let inq = load(&s, &id).await?;
    if !customer_can_see(&inq, user.as_ref(), q.token.as_deref()) {
        return Err(AppError::not_found("문의를 찾을 수 없습니다"));
    }
    Ok(Json(thread_json(&s, &inq).await?))
}

#[derive(Debug, Deserialize)]
pub struct ReplyInput {
    pub body: String,
    pub token: Option<String>,
}

async fn customer_reply(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    Path(id): Path<String>,
    Json(b): Json<ReplyInput>,
) -> AppResult<Json<serde_json::Value>> {
    let inq = load(&s, &id).await?;
    if !customer_can_see(&inq, user.as_ref(), b.token.as_deref()) {
        return Err(AppError::not_found("문의를 찾을 수 없습니다"));
    }
    if inq.status == "closed" {
        return Err(AppError::Conflict("종료된 문의입니다. 새 문의를 남겨 주세요".into()));
    }
    let body = req_text(&b.body, "내용", 5000)?;
    add_message(&s, &id, "user", &body, "open").await?;
    ops_notice(&s, &inq.kind, &inq.subject);
    Ok(Json(thread_json(&s, &load(&s, &id).await?).await?))
}

async fn my_inquiries(State(s): State<AppState>, Customer(u): Customer) -> AppResult<Json<Vec<Inquiry>>> {
    Ok(Json(
        sqlx::query_as::<_, Inquiry>(&format!("SELECT {COLS} FROM inquiries WHERE user_id = ? AND kind <> 'partner' ORDER BY last_message_at DESC LIMIT 100"))
            .bind(&u.id)
            .fetch_all(&s.db)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct ReportInput {
    pub target_type: String,
    pub target_id: String,
    pub reason: String,
    pub email: Option<String>,
}

async fn create_report(
    State(s): State<AppState>,
    MaybeCustomer(user): MaybeCustomer,
    headers: HeaderMap,
    Json(b): Json<ReportInput>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let (table, label) = match b.target_type.as_str() {
        "workshop" => ("workshops", "업체"),
        "review" => ("reviews", "리뷰"),
        "product" => ("products", "상품"),
        _ => return Err(AppError::bad("신고 대상을 확인해 주세요")),
    };
    let target_name: Option<String> = match table {
        "workshops" => sqlx::query_scalar("SELECT name FROM workshops WHERE id = ?").bind(&b.target_id).fetch_optional(&s.db).await?,
        "reviews" => sqlx::query_scalar("SELECT LEFT(content, 40) FROM reviews WHERE id = ?").bind(&b.target_id).fetch_optional(&s.db).await?,
        _ => sqlx::query_scalar("SELECT name FROM products WHERE id = ?").bind(&b.target_id).fetch_optional(&s.db).await?,
    };
    let target_name = target_name.ok_or_else(|| AppError::not_found("신고할 대상을 찾을 수 없습니다"))?;
    let reason = req_text(&b.reason, "신고 사유", 2000)?;
    let email = match (&user, b.email.as_deref().map(str::trim).filter(|e| !e.is_empty())) {
        (Some(u), _) => Some(u.email.clone()),
        (None, Some(e)) => Some(normalize_email(e)?),
        (None, None) => None,
    };
    let ip = client_ip(&headers);
    if !ratelimit::hit(&s.db, &format!("report:{ip}"), 3600, 10).await? {
        return Err(AppError::TooMany("신고를 너무 많이 했습니다. 잠시 후 다시 시도해 주세요".into()));
    }
    let id = new_id();
    let subject: String = format!("{label} 신고: {target_name}").chars().take(200).collect();
    sqlx::query(
        "INSERT INTO inquiries (id, kind, user_id, name, email, subject, target_type, target_id, ip) VALUES (?, 'report', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user.as_ref().map(|u| u.id.clone()))
    .bind(user.as_ref().map(|u| u.display_name.clone()).unwrap_or_else(|| "비회원".into()))
    .bind(&email)
    .bind(&subject)
    .bind(&b.target_type)
    .bind(&b.target_id)
    .bind(&ip)
    .execute(&s.db)
    .await?;
    add_message(&s, &id, "user", &reason, "open").await?;
    ops_notice(&s, "report", &subject);
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[derive(Debug, Deserialize)]
pub struct PartnerInquiryInput {
    pub subject: String,
    pub body: String,
}

async fn create_partner_inquiry(State(s): State<AppState>, p: Partner, Json(b): Json<PartnerInquiryInput>) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let subject = req_line(&b.subject, "제목", 200)?;
    let body = req_text(&b.body, "내용", 5000)?;
    let id = new_id();
    sqlx::query("INSERT INTO inquiries (id, kind, user_id, workshop_id, name, email, subject) VALUES (?, 'partner', ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(p.user.as_ref().map(|u| u.id.clone()))
        .bind(&p.workshop_id)
        .bind(&p.workshop_name)
        .bind(p.user.as_ref().map(|u| u.email.clone()))
        .bind(&subject)
        .execute(&s.db)
        .await?;
    add_message(&s, &id, "user", &body, "open").await?;
    ops_notice(&s, "partner", &subject);
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn partner_inquiries(State(s): State<AppState>, p: Partner) -> AppResult<Json<Vec<Inquiry>>> {
    Ok(Json(
        sqlx::query_as::<_, Inquiry>(&format!("SELECT {COLS} FROM inquiries WHERE kind = 'partner' AND workshop_id = ? ORDER BY last_message_at DESC LIMIT 100"))
            .bind(&p.workshop_id)
            .fetch_all(&s.db)
            .await?,
    ))
}

async fn partner_load(s: &AppState, p: &Partner, id: &str) -> AppResult<Inquiry> {
    let inq = load(s, id).await?;
    if inq.kind != "partner" || inq.workshop_id.as_deref() != Some(p.workshop_id.as_str()) {
        return Err(AppError::not_found("문의를 찾을 수 없습니다"));
    }
    Ok(inq)
}

async fn partner_thread(State(s): State<AppState>, p: Partner, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let inq = partner_load(&s, &p, &id).await?;
    Ok(Json(thread_json(&s, &inq).await?))
}

async fn partner_reply(State(s): State<AppState>, p: Partner, Path(id): Path<String>, Json(b): Json<ReplyInput>) -> AppResult<Json<serde_json::Value>> {
    let inq = partner_load(&s, &p, &id).await?;
    if inq.status == "closed" {
        return Err(AppError::Conflict("종료된 문의입니다. 새 문의를 남겨 주세요".into()));
    }
    let body = req_text(&b.body, "내용", 5000)?;
    add_message(&s, &id, "user", &body, "open").await?;
    ops_notice(&s, "partner", &inq.subject);
    Ok(Json(thread_json(&s, &load(&s, &id).await?).await?))
}

#[derive(Debug, Deserialize)]
pub struct AdminQuery {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

async fn admin_inquiries(State(s): State<AppState>, _a: Admin, Query(q): Query<AdminQuery>) -> AppResult<Json<serde_json::Value>> {
    let (limit, offset) = Page { page: q.page, size: q.size }.limit_offset(50, 200);
    let rows = sqlx::query_as::<_, Inquiry>(&format!(
        "SELECT {COLS} FROM inquiries WHERE (? IS NULL OR kind = ?) AND (? IS NULL OR status = ?) \
         ORDER BY (status = 'open') DESC, last_message_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(&q.kind)
    .bind(&q.kind)
    .bind(&q.status)
    .bind(&q.status)
    .bind(limit)
    .bind(offset)
    .fetch_all(&s.db)
    .await?;
    let counts: Vec<(String, i64)> = sqlx::query_as("SELECT kind, COUNT(*) FROM inquiries WHERE status = 'open' GROUP BY kind").fetch_all(&s.db).await?;
    let open: serde_json::Map<String, serde_json::Value> = counts.into_iter().map(|(k, n)| (k, serde_json::json!(n))).collect();
    Ok(Json(serde_json::json!({ "items": rows, "open_counts": open })))
}

async fn admin_thread(State(s): State<AppState>, _a: Admin, Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let inq = load(&s, &id).await?;
    Ok(Json(thread_json(&s, &inq).await?))
}

#[derive(Debug, Deserialize)]
pub struct AdminReplyInput {
    pub body: String,
}

async fn admin_reply(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<AdminReplyInput>) -> AppResult<Json<serde_json::Value>> {
    let inq = load(&s, &id).await?;
    let body = req_text(&b.body, "답변", 5000)?;
    add_message(&s, &id, "admin", &body, "answered").await?;
    audit::log(&s, &admin, "inquiry.reply", "inquiry", &id, None).await;
    if let Some(e) = &inq.email {
        let link = match inq.kind.as_str() {
            "partner" => format!("{}/?tab=support", s.cfg.partner_url),
            _ if inq.user_id.is_some() => format!("{}/?my=inquiries", s.cfg.app_url),
            // 비회원은 접수 메일의 비밀 링크로 확인 (서버에는 링크 원문이 없음)
            _ => "문의 접수 때 받은 메일의 링크에서 확인할 수 있습니다.".into(),
        };
        s.mailer.send(Mail::new(
            e.clone(),
            format!("[가게요] 문의에 답변이 등록되었습니다: {}", inq.subject),
            format!("{body}\n\n---\n이어서 물으실 내용은 아래에서 남겨 주세요.\n{link}"),
        ));
    }
    Ok(Json(thread_json(&s, &load(&s, &id).await?).await?))
}

#[derive(Debug, Deserialize)]
pub struct StatusInput {
    pub status: String,
}

async fn admin_set_status(State(s): State<AppState>, admin: Admin, Path(id): Path<String>, Json(b): Json<StatusInput>) -> AppResult<StatusCode> {
    if !["open", "answered", "closed"].contains(&b.status.as_str()) {
        return Err(AppError::bad("문의 상태 값이 올바르지 않습니다"));
    }
    let r = sqlx::query("UPDATE inquiries SET status = ? WHERE id = ?").bind(&b.status).bind(&id).execute(&s.db).await?;
    if r.rows_affected() == 0 {
        load(&s, &id).await?;
    }
    audit::log(&s, &admin, &format!("inquiry.{}", b.status), "inquiry", &id, None).await;
    Ok(StatusCode::NO_CONTENT)
}
