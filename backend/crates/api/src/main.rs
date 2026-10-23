//! gageyo API 서버.
//!
//! 모든 API 는 `/api` 아래에 있습니다. 화면(정적 앱 4개)은 Rpanel 에서 각각 올리고 `/api/` 를 이 서버로 프록시합니다.

use std::{str::FromStr, time::Duration};

use axum::{
    http::{header, HeaderName, HeaderValue, Method, StatusCode},
    Json, Router,
};
use gageyo_core::{config::Config, AppState};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

mod accounts;
mod admin;
mod admin_auth;
mod files;
mod public;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,gageyo_api=debug".into()))
        .init();
    let cfg = Config::from_env().map_err(|e| anyhow::anyhow!("설정 오류: {e}"))?;
    if cfg.admin_emails.is_empty() && cfg.admin_api_key.is_none() {
        tracing::warn!("ADMIN_EMAILS·ADMIN_API_KEY 가 모두 없어 운영자 화면을 쓸 수 없습니다");
    }

    // DB 세션 시간대를 한국 시간으로 맞춤: NOW()·기본값과 화면에서 받은 예약 시각이 같은 기준이 됩니다.
    let opts = MySqlConnectOptions::from_str(&cfg.database_url)?.timezone(Some(String::from("+09:00")));
    let db = MySqlPoolOptions::new().max_connections(20).acquire_timeout(Duration::from_secs(10)).connect_with(opts).await?;
    sqlx::migrate!("../../migrations").run(&db).await?;

    let bind = cfg.bind_addr.clone();
    let cors_origins = cfg.cors_origins.clone();
    let upload_max = cfg.upload_max_bytes;
    let state = AppState::new(db, cfg).map_err(|e| anyhow::anyhow!("설정 오류: {e}"))?;
    tokio::fs::create_dir_all(&state.cfg.upload_dir).await.ok();
    spawn_cleanup(state.clone());

    let api = Router::new()
        .merge(public::routes())
        .merge(accounts::routes())
        .merge(admin_auth::routes())
        .merge(admin::routes())
        .merge(files::routes(upload_max))
        .merge(gageyo_workshop::routes())
        .merge(gageyo_commerce::routes())
        .merge(gageyo_support::routes());

    let mut app = Router::new()
        .nest("/api", api)
        .fallback(|| async { (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "요청한 주소를 찾을 수 없습니다", "code": "not_found" }))) })
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());
    // 같은 주소(/api 프록시)로만 쓰면 CORS 가 필요 없습니다. 다른 주소에서 부를 때만 CORS_ORIGINS 에 적습니다.
    if !cors_origins.is_empty() {
        let origins: Vec<HeaderValue> = cors_origins.iter().filter_map(|o| HeaderValue::from_str(o).ok()).collect();
        app = app.layer(
            CorsLayer::new()
                .allow_origin(origins)
                .allow_credentials(true)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
                .allow_headers([header::CONTENT_TYPE, HeaderName::from_static("x-admin-key"), HeaderName::from_static("x-partner-key")]),
        );
    }

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "gageyo API is listening");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}

/// 한 시간마다 만료된 세션·인증 코드·요청 횟수 기록 정리, 결제 기한 지난 주문 자동 취소
fn spawn_cleanup(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            let db = &state.db;
            let _ = gageyo_core::ratelimit::cleanup(db).await;
            let _ = sqlx::query("DELETE FROM auth_sessions WHERE expires_at < NOW()").execute(db).await;
            let _ = sqlx::query("DELETE FROM email_codes WHERE expires_at < NOW() - INTERVAL 1 DAY").execute(db).await;
            let _ = sqlx::query("DELETE FROM password_resets WHERE expires_at < NOW() - INTERVAL 7 DAY").execute(db).await;
            if let Err(e) = gageyo_commerce::orders::expire_unpaid(&state).await {
                tracing::warn!(error = %e, "expire unpaid orders");
            }
        }
    });
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
