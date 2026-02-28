use crate::models::resource::ResourceType;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug)]
pub enum FileError {
    ValidationError(String),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::ValidationError(msg) => write!(f, "验证错误: {}", msg),
        }
    }
}

impl std::error::Error for FileError {}

pub struct FileService;

impl FileService {
    /// 最大文件大小 (100MB)
    pub const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

    /// 计算文件 SHA-256 哈希
    pub fn calculate_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// 验证资源文件
    pub fn validate_resource_file(
        file_name: &str,
        file_data: &[u8],
        mime_type: Option<&str>,
    ) -> Result<ResourceType, FileError> {
        // 检查文件大小
        if file_data.is_empty() {
            return Err(FileError::ValidationError("文件不能为空".to_string()));
        }

        if file_data.len() > Self::MAX_FILE_SIZE {
            return Err(FileError::ValidationError(format!(
                "文件大小超过限制。最大允许 100MB，当前 {:.2}MB",
                file_data.len() as f64 / 1024.0 / 1024.0
            )));
        }

        // 从文件名获取扩展名
        let extension = Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        let resource_type = match extension.as_deref() {
            Some(ext) => ResourceType::from_extension(ext),
            None => {
                // 尝试从 MIME 类型推断
                mime_type.map_or(ResourceType::Other, |mime| match mime {
                    "application/pdf" => ResourceType::Pdf,
                    "text/plain" => ResourceType::Txt,
                    "text/markdown" => ResourceType::WebMarkdown,
                    "image/jpeg" => ResourceType::Jpeg,
                    "image/png" => ResourceType::Png,
                    "application/zip" => ResourceType::Zip,
                    _ => ResourceType::Other,
                })
            }
        };

        // 检查文件扩展名是否受支持
        if resource_type == ResourceType::Other {
            return Err(FileError::ValidationError(format!(
                "不支持的文件类型。支持的类型: {}",
                ResourceType::supported_extensions().join(", ")
            )));
        }

        Ok(resource_type)
    }

    /// 根据资源类型获取 MIME 类型
    pub fn get_mime_type_by_type(resource_type: &str) -> String {
        let resource_type_lower = resource_type.to_lowercase();
        match resource_type_lower.as_str() {
            "web_markdown" | "md" | "markdown" => "text/markdown",
            "ppt" => "application/vnd.ms-powerpoint",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "doc" => "application/msword",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "jpeg" | "jpg" => "image/jpeg",
            "png" => "image/png",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
        .to_string()
    }

    /// 根据资源类型获取文件扩展名
    pub fn get_extension_by_type(resource_type: &str) -> String {
        let resource_type_lower = resource_type.to_lowercase();
        match resource_type_lower.as_str() {
            "web_markdown" => "md",
            "ppt" => "ppt",
            "pptx" => "pptx",
            "doc" => "doc",
            "docx" => "docx",
            "pdf" => "pdf",
            "txt" => "txt",
            "jpeg" => "jpeg",
            "jpg" => "jpg",
            "png" => "png",
            "zip" => "zip",
            _ => "bin",
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_hash() {
        let data = b"hello world";
        let hash = FileService::calculate_hash(data);
        assert_eq!(hash.len(), 64); // SHA-256 hash is 64 hex characters
    }

    #[test]
    fn test_validate_resource_file_empty() {
        let result = FileService::validate_resource_file("test.pdf", &[], Some("application/pdf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_resource_file_too_large() {
        let large_data = vec![0u8; FileService::MAX_FILE_SIZE + 1];
        let result =
            FileService::validate_resource_file("test.pdf", &large_data, Some("application/pdf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_type_from_extension() {
        assert_eq!(ResourceType::from_extension("pdf"), ResourceType::Pdf);
        assert_eq!(ResourceType::from_extension("PDF"), ResourceType::Pdf);
        assert_eq!(
            ResourceType::from_extension("md"),
            ResourceType::WebMarkdown
        );
        assert_eq!(ResourceType::from_extension("jpg"), ResourceType::Jpg);
        assert_eq!(ResourceType::from_extension("unknown"), ResourceType::Other);
    }
}
