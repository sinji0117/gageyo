use std::{env, path::PathBuf};

/// 환경변수에서 읽는 설정.
///
/// Rpanel 에서는 `PORT`·`DATABASE_URL` 을 패널이 넣어 주고, 나머지는 API 앱의 **환경 변수** 탭에 넣습니다.
/// 전체 목록과 설명은 `docs/configuration.md` 를 보세요.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    /// 운영자 화면에 로그인할 수 있는 계정 이메일 (쉼표로 구분, 소문자)
    pub admin_emails: Vec<String>,
    /// 스크립트·비상용 운영자 키 (`x-admin-key` 헤더). 비우면 키 접속을 막습니다.
    pub admin_api_key: Option<String>,
    /// 쿠키에 `Secure` 를 붙일지 (운영 true, 로컬 http 개발 false)
    pub cookie_secure: bool,
    /// 다른 주소에서 API 를 부를 때 허용할 origin (쉼표 구분). 비우면 같은 주소(`/api` 프록시)만
    pub cors_origins: Vec<String>,
    /// 화면 주소 (메일 안의 링크에 씀)
    pub app_url: String,
    pub partner_url: String,
    pub admin_url: String,
    /// 업로드 이미지 저장 폴더
    pub upload_dir: PathBuf,
    pub upload_max_bytes: usize,
    /// 메일 서버 (예: `smtps://아이디:앱비밀번호@smtp.gmail.com:465`). 없으면 메일을 보내지 않고 로그에만 남김
    pub smtp_url: Option<String>,
    /// 보내는 사람 (예: `가게요 <no-reply@gageyo.com>`)
    pub mail_from: Option<String>,
    /// 운영 알림을 받을 주소 (새 입점 신청·문의). 비우면 ADMIN_EMAILS 로 보냄
    pub ops_email: Option<String>,
    /// 비밀값 암호화 키 (16진수 64자, `openssl rand -hex 32`)
    pub data_key: Option<String>,
    /// 회원가입 이메일 코드 인증: `auto`(메일 설정이 있으면 켬, 기본) · `on` · `off`
    pub signup_email_verify: crate::verify::Mode,
    /// 토스페이먼츠 키 (없으면 카드 결제를 끔)
    pub toss_client_key: Option<String>,
    pub toss_secret_key: Option<String>,
    pub toss_api_base: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "환경변수 DATABASE_URL 이 없습니다 — Rpanel 앱의 데이터베이스 탭에서 MariaDB 를 연결하세요".to_string())?;
        let toss_client_key = non_empty("TOSS_CLIENT_KEY");
        let toss_secret_key = non_empty("TOSS_SECRET_KEY");
        if let (Some(ck), Some(sk)) = (&toss_client_key, &toss_secret_key) {
            check_toss_pair(ck, sk)?;
        }
        let data_key = non_empty("DATA_KEY");
        if let Some(k) = &data_key {
            if k.len() != 64 || hex::decode(k).is_err() {
                return Err("DATA_KEY 는 16진수 64자여야 합니다 (openssl rand -hex 32)".into());
            }
        }
        Ok(Self {
            database_url,
            bind_addr: bind_addr(env::var("BIND_ADDR").ok(), env::var("PORT").ok()),
            admin_emails: parse_list(&env::var("ADMIN_EMAILS").unwrap_or_default())
                .into_iter()
                .filter(|e| e.contains('@'))
                .collect(),
            admin_api_key: non_empty("ADMIN_API_KEY"),
            cookie_secure: env::var("COOKIE_SECURE").map(|v| v.trim() != "false").unwrap_or(true),
            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().trim_end_matches('/').to_string())
                .filter(|s| s.starts_with("http"))
                .collect(),
            app_url: url_env("APP_URL", "https://app.gageyo.com"),
            partner_url: url_env("PARTNER_URL", "https://partner.gageyo.com"),
            admin_url: url_env("ADMIN_URL", "https://admin.gageyo.com"),
            upload_dir: upload_dir(non_empty("UPLOAD_DIR"), env::current_exe().ok()),
            upload_max_bytes: mb("UPLOAD_MAX_MB", 10)? as usize * 1024 * 1024,
            smtp_url: non_empty("SMTP_URL"),
            mail_from: non_empty("MAIL_FROM"),
            ops_email: non_empty("OPS_EMAIL"),
            data_key,
            signup_email_verify: crate::verify::Mode::parse(env::var("SIGNUP_EMAIL_VERIFY").ok().as_deref()),
            toss_client_key,
            toss_secret_key,
            toss_api_base: env::var("TOSS_API_BASE")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| crate::toss::DEFAULT_API_BASE.into()),
        })
    }

    /// 운영 알림을 받을 주소들
    pub fn ops_recipients(&self) -> Vec<String> {
        match &self.ops_email {
            Some(e) => vec![e.clone()],
            None => self.admin_emails.clone(),
        }
    }

    pub fn toss_enabled(&self) -> bool {
        self.toss_client_key.is_some() && self.toss_secret_key.is_some()
    }

    /// 테스트·개발용 기본값
    pub fn for_tests() -> Self {
        Self {
            database_url: "mysql://localhost/test".into(),
            bind_addr: "127.0.0.1:0".into(),
            admin_emails: vec!["ops@gageyo.test".into()],
            admin_api_key: None,
            cookie_secure: false,
            cors_origins: vec![],
            app_url: "http://localhost:5173".into(),
            partner_url: "http://localhost:5174".into(),
            admin_url: "http://localhost:5175".into(),
            upload_dir: std::env::temp_dir().join("gageyo-test-uploads"),
            upload_max_bytes: 10 * 1024 * 1024,
            smtp_url: None,
            mail_from: None,
            ops_email: None,
            data_key: None,
            signup_email_verify: crate::verify::Mode::Off,
            toss_client_key: None,
            toss_secret_key: None,
            toss_api_base: crate::toss::DEFAULT_API_BASE.into(),
        }
    }
}

fn non_empty(key: &str) -> Option<String> {
    env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn url_env(key: &str, default: &str) -> String {
    non_empty(key).unwrap_or_else(|| default.into()).trim_end_matches('/').to_string()
}

/// `"A@x.kr, b@y.kr"` → `["a@x.kr", "b@y.kr"]`
pub fn parse_list(raw: &str) -> Vec<String> {
    raw.split([',', ' ', ';']).map(|e| e.trim().to_ascii_lowercase()).filter(|e| !e.is_empty()).collect()
}

/// 토스 키는 테스트(`test_`)·실제(`live_`) 키를 섞어 쓰면 결제가 실패합니다.
fn check_toss_pair(ck: &str, sk: &str) -> Result<(), String> {
    let ok_ck = ck.starts_with("test_ck_") || ck.starts_with("live_ck_") || ck.starts_with("test_gck_") || ck.starts_with("live_gck_");
    let ok_sk = sk.starts_with("test_sk_") || sk.starts_with("live_sk_") || sk.starts_with("test_gsk_") || sk.starts_with("live_gsk_");
    if !ok_ck {
        return Err("TOSS_CLIENT_KEY 는 test_ck_ 또는 live_ck_ 로 시작해야 합니다".into());
    }
    if !ok_sk {
        return Err("TOSS_SECRET_KEY 는 test_sk_ 또는 live_sk_ 로 시작해야 합니다".into());
    }
    if ck.starts_with("test_") != sk.starts_with("test_") {
        return Err("토스 클라이언트 키와 시크릿 키의 테스트/실제 종류가 다릅니다".into());
    }
    Ok(())
}

/// 업로드 폴더: `UPLOAD_DIR` > Rpanel 앱의 data 폴더(자동 감지) > `data/uploads`(로컬 개발)
///
/// Rpanel 은 Git 으로 배포한 Rust 앱을 `/var/lib/rpanel/apps/<slug>/releases/<배포id>/<바이너리>` 에서 실행하고,
/// 배포해도 남는 쓰기 폴더는 `/var/lib/rpanel/apps/<slug>/data` 입니다.
pub fn upload_dir(env_value: Option<String>, exe: Option<PathBuf>) -> PathBuf {
    if let Some(v) = env_value.filter(|v| !v.trim().is_empty()) {
        return v.trim().into();
    }
    if let Some(app_dir) = exe.as_deref().and_then(rpanel_app_dir) {
        return app_dir.join("data").join("uploads");
    }
    "data/uploads".into()
}

fn rpanel_app_dir(exe: &std::path::Path) -> Option<PathBuf> {
    let release = exe.parent()?;
    let releases = release.parent()?;
    if releases.file_name()? != "releases" {
        return None;
    }
    let app = releases.parent()?;
    app.join("data").is_dir().then(|| app.to_path_buf())
}

fn mb(key: &str, default: u64) -> Result<u64, String> {
    match non_empty(key) {
        Some(v) => match v.parse::<u64>() {
            Ok(n) if (1..=1000).contains(&n) => Ok(n),
            _ => Err(format!("환경변수 {key} 는 MB 단위 숫자(1~1000)여야 합니다")),
        },
        None => Ok(default),
    }
}

/// 바인드 주소: `BIND_ADDR` > `127.0.0.1:$PORT`(Rpanel) > `127.0.0.1:8081`(로컬 개발)
pub fn bind_addr(bind: Option<String>, port: Option<String>) -> String {
    if let Some(b) = bind.filter(|b| !b.trim().is_empty()) {
        return b.trim().to_string();
    }
    match port.as_deref().map(str::trim).and_then(|p| p.parse::<u16>().ok()) {
        Some(p) => format!("127.0.0.1:{p}"),
        None => "127.0.0.1:8081".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_address_priority() {
        assert_eq!(bind_addr(None, None), "127.0.0.1:8081");
        assert_eq!(bind_addr(None, Some("8101".into())), "127.0.0.1:8101");
        assert_eq!(bind_addr(Some("0.0.0.0:9000".into()), Some("8101".into())), "0.0.0.0:9000");
        assert_eq!(bind_addr(None, Some("x".into())), "127.0.0.1:8081");
    }

    #[test]
    fn list_parsing() {
        assert_eq!(parse_list(" Boss@X.kr, ops@y.kr ;"), vec!["boss@x.kr", "ops@y.kr"]);
        assert!(parse_list("").is_empty());
    }

    #[test]
    fn toss_key_pairs() {
        assert!(check_toss_pair("test_ck_a", "test_sk_b").is_ok());
        assert!(check_toss_pair("live_ck_a", "live_sk_b").is_ok());
        assert!(check_toss_pair("test_ck_a", "live_sk_b").is_err());
        assert!(check_toss_pair("ck_a", "test_sk_b").is_err());
        assert!(check_toss_pair("test_ck_a", "sk").is_err());
    }

    #[test]
    fn upload_dir_detects_rpanel_layout() {
        let tmp = std::env::temp_dir().join(format!("gy-cfg-{}", std::process::id()));
        let app = tmp.join("apps/gageyo-api");
        std::fs::create_dir_all(app.join("releases/42")).unwrap();
        std::fs::create_dir_all(app.join("data")).unwrap();
        let exe = app.join("releases/42/gageyo-api");
        assert_eq!(upload_dir(None, Some(exe.clone())), app.join("data/uploads"));
        assert_eq!(upload_dir(Some(" /srv/up ".into()), Some(exe)), PathBuf::from("/srv/up"));
        assert_eq!(upload_dir(None, Some(PathBuf::from("/usr/bin/gageyo-api"))), PathBuf::from("data/uploads"));
        std::fs::remove_dir_all(tmp).ok();
    }
}
