use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use tokio::fs;

use crate::config::Config;

use super::oss_service::OssStorage;

pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, StorageError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackendType {
    Local,
    Oss,
}

impl StorageBackendType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Oss => "oss",
        }
    }
}

#[derive(Debug)]
pub enum StorageError {
    Validation(String),
    Config(String),
    NotFound(String),
    Io(String),
    Backend(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Validation(msg) => write!(f, "参数错误: {}", msg),
            StorageError::Config(msg) => write!(f, "配置错误: {}", msg),
            StorageError::NotFound(msg) => write!(f, "未找到: {}", msg),
            StorageError::Io(msg) => write!(f, "IO 错误: {}", msg),
            StorageError::Backend(msg) => write!(f, "后端错误: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {}

#[derive(Debug, Clone, Default)]
pub struct StorageFileMetadata {
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StorageStsCredentials {
    pub access_key_id: String,
    pub access_key_secret: String,
    pub security_token: String,
    pub expiration: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub upload_key: String,
    pub expires_in: u64,
}

pub trait StorageBackend: Send + Sync {
    fn save_file<'a>(
        &'a self,
        key: &'a str,
        data: Vec<u8>,
        content_type: Option<&'a str>,
    ) -> StorageFuture<'a, String>;

    fn read_file<'a>(&'a self, key: &'a str) -> StorageFuture<'a, Vec<u8>>;

    fn write_file<'a>(
        &'a self,
        key: &'a str,
        data: Vec<u8>,
        content_type: Option<&'a str>,
    ) -> StorageFuture<'a, ()>;

    fn delete_file<'a>(&'a self, key: &'a str) -> StorageFuture<'a, ()>;

    fn get_file_url<'a>(&'a self, key: &'a str, expires_secs: u64) -> StorageFuture<'a, String>;

    fn get_download_url<'a>(
        &'a self,
        key: &'a str,
        filename: &'a str,
        expires_secs: u64,
    ) -> StorageFuture<'a, String>;

    fn get_upload_url<'a>(
        &'a self,
        key: &'a str,
        expires_secs: u64,
        content_type: Option<&'a str>,
    ) -> StorageFuture<'a, String>;

    fn head_file<'a>(&'a self, key: &'a str) -> StorageFuture<'a, StorageFileMetadata>;

    fn file_exists<'a>(&'a self, key: &'a str) -> StorageFuture<'a, bool>;

    fn get_sts_token<'a>(
        &'a self,
        _key: &'a str,
        _duration_secs: u64,
    ) -> StorageFuture<'a, StorageStsCredentials> {
        Box::pin(async move {
            Err(StorageError::Backend(
                "当前存储后端不支持 STS 临时凭证".to_string(),
            ))
        })
    }

    fn backend_type(&self) -> StorageBackendType;

    fn supports_sts(&self) -> bool {
        false
    }

    fn default_signed_url_expiry(&self) -> u64 {
        600
    }
}

#[derive(Debug, Clone)]
pub struct LocalStorage {
    base_path: PathBuf,
    base_url: String,
}

impl LocalStorage {
    pub fn new(base_path: String, base_url: String) -> Self {
        Self {
            base_path: PathBuf::from(base_path),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn resolve_local_path(&self, key_or_path: &str) -> Result<PathBuf, StorageError> {
        if key_or_path.trim().is_empty() {
            return Err(StorageError::Validation("文件 key 不能为空".to_string()));
        }

        let path = Path::new(key_or_path);

        if path.is_absolute() || path.starts_with(&self.base_path) {
            return Ok(path.to_path_buf());
        }

        let key = key_or_path.trim_start_matches('/');
        Ok(self.base_path.join(key))
    }

    fn relative_key(&self, key_or_path: &str) -> String {
        let path = Path::new(key_or_path);

        if let Ok(relative) = path.strip_prefix(&self.base_path) {
            return relative.to_string_lossy().replace('\\', "/");
        }

        key_or_path.trim_start_matches('/').to_string()
    }
}

impl StorageBackend for LocalStorage {
    fn save_file<'a>(
        &'a self,
        key: &'a str,
        data: Vec<u8>,
        _content_type: Option<&'a str>,
    ) -> StorageFuture<'a, String> {
        Box::pin(async move {
            let full_path = self.resolve_local_path(key)?;

            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| StorageError::Io(format!("创建目录失败: {}", e)))?;
            }

            fs::write(&full_path, &data)
                .await
                .map_err(|e| StorageError::Io(format!("写入文件失败: {}", e)))?;

            Ok(full_path.to_string_lossy().to_string())
        })
    }

    fn read_file<'a>(&'a self, key: &'a str) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let full_path = self.resolve_local_path(key)?;

            match fs::read(&full_path).await {
                Ok(data) => Ok(data),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(StorageError::NotFound(
                    format!("文件不存在: {}", full_path.to_string_lossy()),
                )),
                Err(e) => Err(StorageError::Io(format!("读取文件失败: {}", e))),
            }
        })
    }

    fn write_file<'a>(
        &'a self,
        key: &'a str,
        data: Vec<u8>,
        _content_type: Option<&'a str>,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let full_path = self.resolve_local_path(key)?;

            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| StorageError::Io(format!("创建目录失败: {}", e)))?;
            }

            fs::write(&full_path, &data)
                .await
                .map_err(|e| StorageError::Io(format!("写入文件失败: {}", e)))?;

            Ok(())
        })
    }

    fn delete_file<'a>(&'a self, key: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let full_path = self.resolve_local_path(key)?;

            match fs::remove_file(&full_path).await {
                Ok(_) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(StorageError::Io(format!("删除文件失败: {}", e))),
            }
        })
    }

    fn get_file_url<'a>(&'a self, key: &'a str, _expires_secs: u64) -> StorageFuture<'a, String> {
        Box::pin(async move {
            let relative = self.relative_key(key);
            if relative.is_empty() {
                return Err(StorageError::Validation("文件 key 不能为空".to_string()));
            }

            Ok(format!("{}/{}", self.base_url, relative))
        })
    }

    fn get_download_url<'a>(
        &'a self,
        key: &'a str,
        _filename: &'a str,
        expires_secs: u64,
    ) -> StorageFuture<'a, String> {
        self.get_file_url(key, expires_secs)
    }

    fn file_exists<'a>(&'a self, key: &'a str) -> StorageFuture<'a, bool> {
        Box::pin(async move {
            let full_path = self.resolve_local_path(key)?;
            Ok(fs::metadata(full_path).await.is_ok())
        })
    }

    fn get_upload_url<'a>(
        &'a self,
        _key: &'a str,
        _expires_secs: u64,
        _content_type: Option<&'a str>,
    ) -> StorageFuture<'a, String> {
        Box::pin(async move {
            Err(StorageError::Backend(
                "当前存储后端不支持直传 URL".to_string(),
            ))
        })
    }

    fn head_file<'a>(&'a self, key: &'a str) -> StorageFuture<'a, StorageFileMetadata> {
        Box::pin(async move {
            let full_path = self.resolve_local_path(key)?;
            let metadata = fs::metadata(&full_path).await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StorageError::NotFound(format!("文件不存在: {}", full_path.to_string_lossy()))
                } else {
                    StorageError::Io(format!("读取文件元信息失败: {}", e))
                }
            })?;

            Ok(StorageFileMetadata {
                content_length: Some(metadata.len()),
                content_type: None,
                etag: None,
            })
        })
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::Local
    }
}

pub fn create_storage_backend(config: &Config) -> Result<Arc<dyn StorageBackend>, StorageError> {
    if config.storage_backend == "oss" {
        let storage = OssStorage::from_config(config)?;
        return Ok(Arc::new(storage));
    }

    Ok(Arc::new(LocalStorage::new(
        config.file_upload_path.clone(),
        config.image_base_url.clone(),
    )))
}
