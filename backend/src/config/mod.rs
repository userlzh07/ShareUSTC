use std::env;

/// 应用配置结构体
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub server_host: String,
    pub server_port: u16,
    pub log_level: String,
    pub image_upload_path: String,
    pub resource_upload_path: String,
    pub cors_allowed_origins: Vec<String>,
    pub admin_usernames: Vec<String>,
    pub cookie_secure: bool,
    pub image_base_url: String,
    pub file_upload_path: String,
    pub storage_backend: String,
    pub oss_access_key_id: Option<String>,
    pub oss_access_key_secret: Option<String>,
    pub oss_endpoint: Option<String>,
    pub oss_bucket: Option<String>,
    pub oss_region: Option<String>,
    pub oss_sts_role_arn: Option<String>,
    pub oss_sts_session_duration: u64,
    pub oss_key_prefix: String,
    pub oss_signed_url_expiry: u64,
}

impl Config {
    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        let optional_env = |name: &str| {
            env::var(name).ok().and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
        };

        // 解析 CORS 允许的域名列表
        let cors_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // 解析管理员用户名列表（逗号分隔）
        let admin_usernames = env::var("ADMIN_USERNAMES")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let storage_backend = match env::var("STORAGE_BACKEND")
            .unwrap_or_else(|_| "local".to_string())
            .to_lowercase()
            .as_str()
        {
            "oss" => "oss".to_string(),
            _ => "local".to_string(),
        };

        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/shareustc".to_string()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key".to_string()),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            log_level: env::var("RUST_LOG")
                .unwrap_or_else(|_| "backend=debug,actix_web=info,sqlx=warn".to_string()),
            image_upload_path: env::var("IMAGE_UPLOAD_PATH")
                .unwrap_or_else(|_| "./uploads/images".to_string()),
            resource_upload_path: env::var("RESOURCE_UPLOAD_PATH")
                .unwrap_or_else(|_| "./uploads/resources".to_string()),
            cors_allowed_origins: cors_origins,
            admin_usernames,
            cookie_secure: env::var("COOKIE_SECURE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            image_base_url: env::var("IMAGE_BASE_URL").unwrap_or_else(|_| {
                format!(
                    "http://{}:{}",
                    env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                    env::var("SERVER_PORT")
                        .ok()
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(8080)
                )
            }),
            file_upload_path: env::var("FILE_UPLOAD_PATH")
                .unwrap_or_else(|_| "./uploads".to_string()),
            storage_backend,
            oss_access_key_id: optional_env("OSS_ACCESS_KEY_ID"),
            oss_access_key_secret: optional_env("OSS_ACCESS_KEY_SECRET"),
            oss_endpoint: optional_env("OSS_ENDPOINT"),
            oss_bucket: optional_env("OSS_BUCKET"),
            oss_region: optional_env("OSS_REGION"),
            oss_sts_role_arn: optional_env("OSS_STS_ROLE_ARN"),
            oss_sts_session_duration: env::var("OSS_STS_SESSION_DURATION")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(900),
            oss_key_prefix: env::var("OSS_KEY_PREFIX").unwrap_or_default(),
            oss_signed_url_expiry: env::var("OSS_SIGNED_URL_EXPIRY")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(600),
        }
    }
}
