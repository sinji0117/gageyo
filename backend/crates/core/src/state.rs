use std::sync::Arc;

use sqlx::MySqlPool;

use crate::{config::Config, mail::Mailer, secret::SecretBox, toss::TossClient};

/// 모든 모듈이 함께 쓰는 서버 상태. 복사 비용이 작도록 내부는 `Arc`·풀 핸들입니다.
#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub cfg: Arc<Config>,
    pub mailer: Arc<Mailer>,
    pub secrets: SecretBox,
    pub toss: Arc<TossClient>,
}

impl AppState {
    pub fn new(db: MySqlPool, cfg: Config) -> Result<Self, String> {
        if cfg.data_key.is_none() {
            tracing::warn!("DATA_KEY 가 없어 DATABASE_URL 에서 암호화 키를 만듭니다 — 운영에서는 DATA_KEY 를 설정하세요");
        }
        let secrets = SecretBox::from_config(cfg.data_key.as_deref(), &cfg.database_url)?;
        let mailer = Mailer::from_config(cfg.smtp_url.as_deref(), cfg.mail_from.as_deref());
        let toss = TossClient::new(&cfg.toss_api_base);
        Ok(Self { db, cfg: Arc::new(cfg), mailer: Arc::new(mailer), secrets, toss: Arc::new(toss) })
    }
}
