//! 공개 설정·사이트맵·상태 확인.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use gageyo_core::{settings, verify, AppResult, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/config", get(public_config))
        .route("/sitemap.xml", get(sitemap))
}

/// DB 연결까지 확인 (Rpanel 상태 확인 경로)
async fn health(State(s): State<AppState>) -> Response {
    match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&s.db).await {
        Ok(_) => Json(serde_json::json!({ "status": "ok", "service": "gageyo-api", "version": env!("CARGO_PKG_VERSION") })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "health: db unreachable");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "status": "db_unavailable", "service": "gageyo-api" }))).into_response()
        }
    }
}

/// 화면들이 처음에 읽는 설정 (가입 코드 인증 여부, 결제 수단, 고객센터 정보)
async fn public_config(State(s): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let bank = settings::bank_account(&s).await?;
    Ok(Json(serde_json::json!({
        "signup_email_verify": verify::required(&s),
        "payments": {
            "toss": s.cfg.toss_enabled().then(|| serde_json::json!({ "client_key": s.cfg.toss_client_key })),
            "bank": bank.is_some(),
            "none_allowed": !s.cfg.toss_enabled() && bank.is_none(),
        },
        "support": {
            "phone": settings::get(&s, settings::SUPPORT_PHONE).await?,
            "email": settings::get(&s, settings::SUPPORT_EMAIL).await?,
            "company_info": settings::get(&s, settings::COMPANY_INFO).await?,
        },
        "urls": { "app": s.cfg.app_url, "partner": s.cfg.partner_url },
        "carriers": gageyo_core::util::carriers_json(),
    })))
}

fn xml_escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// 검색 노출용 사이트맵: 고객 화면 첫 페이지와 승인된 업체 상세(`/?shop=`)
async fn sitemap(State(s): State<AppState>) -> AppResult<Response> {
    let rows: Vec<(String, Option<chrono::NaiveDateTime>)> =
        sqlx::query_as("SELECT id, COALESCE(updated_at, approved_at, created_at) FROM workshops WHERE status = 'approved' ORDER BY approved_at DESC LIMIT 5000")
            .fetch_all(&s.db)
            .await?;
    let base = &s.cfg.app_url;
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    xml.push_str(&format!("  <url><loc>{}/</loc><changefreq>daily</changefreq></url>\n", xml_escape(base)));
    for (id, updated) in rows {
        xml.push_str(&format!("  <url><loc>{}</loc>", xml_escape(&format!("{base}/?shop={id}"))));
        if let Some(u) = updated {
            xml.push_str(&format!("<lastmod>{}</lastmod>", u.format("%Y-%m-%d")));
        }
        xml.push_str("</url>\n");
    }
    xml.push_str("</urlset>\n");
    Ok(([(header::CONTENT_TYPE, "application/xml; charset=utf-8")], xml).into_response())
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_xml() {
        assert_eq!(super::xml_escape("a&b<c>\"'"), "a&amp;b&lt;c&gt;&quot;&apos;");
    }
}
