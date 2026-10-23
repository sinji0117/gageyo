//! 서비스 카테고리 (고객 화면의 분야 목록, 견적 요청·업체 분야에 씀).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use gageyo_core::{audit, auth::Admin, util::new_id, AppError, AppResult, AppState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ServiceCategory {
    pub id: String,
    pub group_name: String,
    pub name: String,
    pub slug: String,
    pub sort_order: i32,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CategoryInput {
    pub group_name: String,
    pub name: String,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/categories", get(public_categories))
        .route("/admin/categories", get(admin_categories).post(create_category))
        .route("/admin/categories/{id}", put(update_category).delete(delete_category))
}

const COLS: &str = "id, group_name, name, slug, sort_order, is_active";

async fn public_categories(State(s): State<AppState>) -> AppResult<Json<Vec<ServiceCategory>>> {
    let rows = sqlx::query_as::<_, ServiceCategory>(&format!(
        "SELECT {COLS} FROM service_categories WHERE is_active = 1 ORDER BY group_sort, sort_order, id"
    ))
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn admin_categories(State(s): State<AppState>, _a: Admin) -> AppResult<Json<Vec<serde_json::Value>>> {
    let rows: Vec<(String, String, String, String, i32, bool, i64)> = sqlx::query_as(
        "SELECT c.id, c.group_name, c.name, c.slug, c.sort_order, c.is_active, \
           (SELECT COUNT(*) FROM workshop_categories wc WHERE wc.category = c.name) AS workshops \
         FROM service_categories c ORDER BY c.group_sort, c.sort_order, c.id",
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, group_name, name, slug, sort_order, is_active, workshops)| {
                serde_json::json!({"id": id, "group_name": group_name, "name": name, "slug": slug,
                    "sort_order": sort_order, "is_active": is_active, "workshop_count": workshops})
            })
            .collect(),
    ))
}

/// 카테고리 이름이 있고 켜져 있는지 (견적 요청·업체 분야 검사)
pub async fn is_active_category(s: &AppState, name: &str) -> AppResult<bool> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM service_categories WHERE name = ? AND is_active = 1")
        .bind(name)
        .fetch_one(&s.db)
        .await?;
    Ok(n > 0)
}

fn validate(input: &CategoryInput) -> AppResult<(String, String)> {
    let group = input.group_name.trim();
    let name = input.name.trim();
    if group.is_empty() || group.chars().count() > 60 {
        return Err(AppError::bad("카테고리 그룹명을 확인해 주세요"));
    }
    if name.is_empty() || name.chars().count() > 80 || name.contains(['\n', '\r']) {
        return Err(AppError::bad("카테고리 이름은 1~80자로 입력해 주세요"));
    }
    if input.sort_order.unwrap_or(0) < 0 {
        return Err(AppError::bad("정렬 순서는 0 이상이어야 합니다"));
    }
    Ok((group.to_string(), name.to_string()))
}

fn group_sort(group: &str) -> i32 {
    match group.trim() {
        "차량 관리" => 0,
        "자동차 매매" => 1,
        _ => 2,
    }
}

async fn category_by_id(s: &AppState, id: &str) -> AppResult<ServiceCategory> {
    sqlx::query_as::<_, ServiceCategory>(&format!("SELECT {COLS} FROM service_categories WHERE id = ?"))
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| AppError::not_found("카테고리를 찾을 수 없습니다"))
}

async fn create_category(
    State(s): State<AppState>,
    admin: Admin,
    Json(input): Json<CategoryInput>,
) -> AppResult<(StatusCode, Json<ServiceCategory>)> {
    let (group, name) = validate(&input)?;
    let id = new_id();
    let slug = format!("category-{}", id.replace('-', ""));
    let r = sqlx::query(
        "INSERT INTO service_categories (id, group_name, name, slug, sort_order, is_active, group_sort) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&group)
    .bind(&name)
    .bind(slug)
    .bind(input.sort_order.unwrap_or(0))
    .bind(input.is_active.unwrap_or(true))
    .bind(group_sort(&group))
    .execute(&s.db)
    .await;
    if let Err(e) = r {
        if gageyo_core::error::is_duplicate(&e) {
            return Err(AppError::Conflict("같은 그룹에 같은 이름의 카테고리가 있습니다".into()));
        }
        return Err(e.into());
    }
    audit::log(&s, &admin, "category.create", "category", &id, Some(format!("{group} / {name}"))).await;
    Ok((StatusCode::CREATED, Json(category_by_id(&s, &id).await?)))
}

/// 이름을 바꾸면 그 이름을 쓰던 업체 분야도 함께 바꿉니다 (분야는 이름으로 연결됨).
async fn update_category(
    State(s): State<AppState>,
    admin: Admin,
    Path(id): Path<String>,
    Json(input): Json<CategoryInput>,
) -> AppResult<Json<ServiceCategory>> {
    let (group, name) = validate(&input)?;
    let old = category_by_id(&s, &id).await?;
    let mut tx = s.db.begin().await?;
    let r = sqlx::query("UPDATE service_categories SET group_name = ?, name = ?, sort_order = ?, is_active = ?, group_sort = ? WHERE id = ?")
        .bind(&group)
        .bind(&name)
        .bind(input.sort_order.unwrap_or(0))
        .bind(input.is_active.unwrap_or(true))
        .bind(group_sort(&group))
        .bind(&id)
        .execute(&mut *tx)
        .await;
    if let Err(e) = r {
        if gageyo_core::error::is_duplicate(&e) {
            return Err(AppError::Conflict("같은 그룹에 같은 이름의 카테고리가 있습니다".into()));
        }
        return Err(e.into());
    }
    if old.name != name {
        sqlx::query("UPDATE IGNORE workshop_categories SET category = ? WHERE category = ?")
            .bind(&name)
            .bind(&old.name)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE workshops SET category = ? WHERE category = ?").bind(&name).bind(&old.name).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    audit::log(&s, &admin, "category.update", "category", &id, Some(format!("{} → {group} / {name}", old.name))).await;
    Ok(Json(category_by_id(&s, &id).await?))
}

/// 업체가 쓰고 있는 카테고리는 지우지 않고 숨김을 권합니다.
async fn delete_category(State(s): State<AppState>, admin: Admin, Path(id): Path<String>) -> AppResult<StatusCode> {
    let c = category_by_id(&s, &id).await?;
    let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workshop_categories WHERE category = ?")
        .bind(&c.name)
        .fetch_one(&s.db)
        .await?;
    if used > 0 {
        return Err(AppError::Conflict(format!(
            "업체 {used}곳이 이 카테고리를 쓰고 있어 지울 수 없습니다. '고객 화면에 표시'를 끄면 숨길 수 있습니다"
        )));
    }
    sqlx::query("DELETE FROM service_categories WHERE id = ?").bind(&id).execute(&s.db).await?;
    audit::log(&s, &admin, "category.delete", "category", &id, Some(c.name)).await;
    Ok(StatusCode::NO_CONTENT)
}
