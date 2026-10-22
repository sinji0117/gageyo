use axum::{http::Method, routing::get, Json, Router};
use sqlx::mysql::MySqlPoolOptions;
use tower_http::{compression::CompressionLayer, cors::{Any, CorsLayer}, trace::TraceLayer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let database_url=std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let db=MySqlPoolOptions::new().max_connections(20).connect(&database_url).await?;
    sqlx::migrate!("../../migrations").run(&db).await?;

    let admin_api_key=std::env::var("ADMIN_API_KEY").ok().filter(|v|!v.trim().is_empty());
    let workshop_state=gageyo_workshop::WorkshopState{db:db.clone(),admin_api_key:admin_api_key.clone()};
    let commerce_state=gageyo_commerce::CommerceState{db,admin_api_key};
    let app=Router::new().route("/api/health",get(health))
        .nest("/api",gageyo_workshop::routes().with_state(workshop_state))
        .nest("/api",gageyo_commerce::routes().with_state(commerce_state))
        .fallback(||async{(axum::http::StatusCode::NOT_FOUND,Json(serde_json::json!({"error":"요청한 주소를 찾을 수 없습니다"})))} )
        .layer(CorsLayer::new().allow_origin(Any).allow_methods([Method::GET,Method::POST,Method::PUT,Method::PATCH,Method::DELETE]).allow_headers(Any))
        .layer(CompressionLayer::new()).layer(TraceLayer::new_for_http());
    let port=std::env::var("PORT").unwrap_or_else(|_|"8081".into());
    let listener=tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!(%port,"gageyo API is listening");
    axum::serve(listener,app).await?;
    Ok(())
}
async fn health()->Json<serde_json::Value>{Json(serde_json::json!({"status":"ok","service":"gageyo-api"}))}

