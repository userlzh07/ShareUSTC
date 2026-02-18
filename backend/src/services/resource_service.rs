use crate::models::{resource::*, CurrentUser};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

use super::{AiService, FileService};

#[derive(Debug)]
pub enum ResourceError {
    DatabaseError(String),
    FileError(String),
    NotFound(String),
    ValidationError(String),
    Unauthorized(String),
    AiError(String),
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceError::DatabaseError(msg) => write!(f, "数据库错误: {}", msg),
            ResourceError::FileError(msg) => write!(f, "文件错误: {}", msg),
            ResourceError::NotFound(msg) => write!(f, "未找到: {}", msg),
            ResourceError::ValidationError(msg) => write!(f, "验证错误: {}", msg),
            ResourceError::Unauthorized(msg) => write!(f, "未授权: {}", msg),
            ResourceError::AiError(msg) => write!(f, "AI 错误: {}", msg),
        }
    }
}

impl std::error::Error for ResourceError {}

impl From<super::file_service::FileError> for ResourceError {
    fn from(err: super::file_service::FileError) -> Self {
        match err {
            super::file_service::FileError::ValidationError(msg) => {
                ResourceError::ValidationError(msg)
            }
            super::file_service::FileError::FileSystemError(msg) => ResourceError::FileError(msg),
            super::file_service::FileError::NotFound(msg) => ResourceError::NotFound(msg),
        }
    }
}

impl From<super::storage_service::StorageError> for ResourceError {
    fn from(err: super::storage_service::StorageError) -> Self {
        match err {
            super::storage_service::StorageError::Validation(msg) => {
                ResourceError::ValidationError(msg)
            }
            super::storage_service::StorageError::Config(msg) => ResourceError::FileError(msg),
            super::storage_service::StorageError::NotFound(msg) => ResourceError::NotFound(msg),
            super::storage_service::StorageError::Io(msg) => ResourceError::FileError(msg),
            super::storage_service::StorageError::Backend(msg) => ResourceError::FileError(msg),
        }
    }
}

impl From<sqlx::Error> for ResourceError {
    fn from(err: sqlx::Error) -> Self {
        ResourceError::DatabaseError(err.to_string())
    }
}

pub struct ResourceService;

impl ResourceService {
    fn infer_resource_type(file_name: &str, mime_type: Option<&str>) -> Option<ResourceType> {
        let extension = std::path::Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        if let Some(ext) = extension {
            let resource_type = ResourceType::from_extension(&ext);
            if resource_type != ResourceType::Other {
                return Some(resource_type);
            }
        }

        mime_type.map(|mime| match mime {
            "application/pdf" => ResourceType::Pdf,
            "text/plain" => ResourceType::Txt,
            "text/markdown" => ResourceType::WebMarkdown,
            "image/jpeg" => ResourceType::Jpeg,
            "image/png" => ResourceType::Png,
            "application/zip" => ResourceType::Zip,
            "application/msword" => ResourceType::Doc,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                ResourceType::Docx
            }
            "application/vnd.ms-powerpoint" => ResourceType::Ppt,
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
                ResourceType::Pptx
            }
            _ => ResourceType::Other,
        })
    }

    pub async fn create_resource_from_oss_callback(
        pool: &PgPool,
        user: &CurrentUser,
        storage: &Arc<dyn super::StorageBackend>,
        request: UploadResourceRequest,
        oss_key: &str,
        metadata: super::StorageFileMetadata,
    ) -> Result<UploadResourceResponse, ResourceError> {
        request.validate().map_err(ResourceError::ValidationError)?;

        let file_size = metadata
            .content_length
            .ok_or_else(|| ResourceError::ValidationError("无法获取文件大小".to_string()))?
            as i64;
        if file_size <= 0 {
            return Err(ResourceError::ValidationError("文件不能为空".to_string()));
        }
        if file_size as usize > FileService::MAX_FILE_SIZE {
            return Err(ResourceError::ValidationError(format!(
                "文件大小超过限制。最大允许 100MB，当前 {:.2}MB",
                file_size as f64 / 1024.0 / 1024.0
            )));
        }

        let object_name = oss_key.rsplit('/').next().unwrap_or(oss_key);
        let resource_type =
            Self::infer_resource_type(object_name, metadata.content_type.as_deref()).ok_or_else(
                || {
                    ResourceError::ValidationError(format!(
                        "不支持的文件类型。支持的类型: {}",
                        ResourceType::supported_extensions().join(", ")
                    ))
                },
            )?;
        if resource_type == ResourceType::Other {
            return Err(ResourceError::ValidationError(format!(
                "不支持的文件类型。支持的类型: {}",
                ResourceType::supported_extensions().join(", ")
            )));
        }

        let ai_result =
            AiService::audit_resource(&request.title, request.description.as_deref(), None)
                .await
                .map_err(|e| ResourceError::AiError(e.to_string()))?;
        let audit_status = if ai_result.passed {
            AuditStatus::Approved
        } else {
            AuditStatus::Pending
        };

        let resource_id = Uuid::new_v4();
        let tags_json = request
            .tags
            .as_ref()
            .map(|tags| serde_json::to_value(tags).unwrap_or(serde_json::Value::Array(vec![])));
        let storage_type = storage.backend_type().as_str().to_string();

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| ResourceError::DatabaseError(format!("开启事务失败: {}", e)))?;

        let resource: Resource = match sqlx::query_as::<_, Resource>(
            r#"
            INSERT INTO resources (
                id, title, author_id, uploader_id, course_name,
                resource_type, category, tags, file_path, source_file_path,
                file_hash, file_size, content_accuracy, audit_status, ai_reject_reason, storage_type
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING *
            "#,
        )
        .bind(resource_id)
        .bind(&request.title)
        .bind(None::<Uuid>)
        .bind(user.id)
        .bind(request.course_name.clone())
        .bind(resource_type.to_string())
        .bind(request.category.to_string())
        .bind(tags_json)
        .bind(oss_key)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(file_size)
        .bind(ai_result.accuracy_score)
        .bind(audit_status.to_string())
        .bind(if ai_result.passed {
            None
        } else {
            ai_result.reason.as_deref()
        })
        .bind(&storage_type)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                if let Err(cleanup_err) = storage.delete_file(oss_key).await {
                    log::warn!(
                        "[Resource] OSS 回调入库失败后清理文件失败 | key={}, error={}",
                        oss_key,
                        cleanup_err
                    );
                }
                return Err(ResourceError::DatabaseError(format!("插入资源失败: {}", e)));
            }
        };

        if let Err(e) = sqlx::query(
            "INSERT INTO resource_stats (resource_id, views, downloads, likes, rating_count) VALUES ($1, 0, 0, 0, 0)",
        )
        .bind(resource_id)
        .execute(&mut *tx)
        .await
        {
            if let Err(rollback_err) = tx.rollback().await {
                log::warn!("[Resource] 资源统计初始化失败后回滚失败: {}", rollback_err);
            }
            if let Err(cleanup_err) = storage.delete_file(oss_key).await {
                log::warn!(
                    "[Resource] 资源统计初始化失败后清理文件失败 | key={}, error={}",
                    oss_key,
                    cleanup_err
                );
            }
            return Err(ResourceError::DatabaseError(format!("创建统计记录失败: {}", e)));
        }

        if let Some(teacher_sns) = &request.teacher_sns {
            for teacher_sn in teacher_sns {
                if let Err(e) = sqlx::query(
                    "INSERT INTO resource_teachers (resource_id, teacher_sn) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(resource_id)
                .bind(teacher_sn)
                .execute(&mut *tx)
                .await
                {
                    log::warn!(
                        "[Resource] 回调插入教师关联失败 | resource_id={}, teacher_sn={}, error={}",
                        resource_id,
                        teacher_sn,
                        e
                    );
                }
            }
        }

        if let Some(course_sns) = &request.course_sns {
            for course_sn in course_sns {
                if let Err(e) = sqlx::query(
                    "INSERT INTO resource_courses (resource_id, course_sn) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(resource_id)
                .bind(course_sn)
                .execute(&mut *tx)
                .await
                {
                    log::warn!(
                        "[Resource] 回调插入课程关联失败 | resource_id={}, course_sn={}, error={}",
                        resource_id,
                        course_sn,
                        e
                    );
                }
            }
        }

        if let Err(e) = tx.commit().await {
            if let Err(cleanup_err) = storage.delete_file(oss_key).await {
                log::warn!(
                    "[Resource] 回调提交事务失败后清理文件失败 | key={}, error={}",
                    oss_key,
                    cleanup_err
                );
            }
            return Err(ResourceError::DatabaseError(format!("提交事务失败: {}", e)));
        }

        Ok(UploadResourceResponse {
            id: resource.id,
            title: resource.title,
            resource_type: resource.resource_type,
            audit_status: resource.audit_status,
            ai_message: if ai_result.passed {
                Some("AI 审核通过".to_string())
            } else {
                Some("AI 审核未通过，等待人工审核".to_string())
            },
            created_at: resource.created_at,
        })
    }

    /// 上传资源
    pub async fn upload_resource(
        pool: &PgPool,
        user: &CurrentUser,
        storage: &Arc<dyn super::StorageBackend>,
        request: UploadResourceRequest,
        file_name: &str,
        file_data: Vec<u8>,
        mime_type: Option<&str>,
    ) -> Result<UploadResourceResponse, ResourceError> {
        // 验证请求
        request.validate().map_err(ResourceError::ValidationError)?;

        // 验证并确定资源类型
        let resource_type = FileService::validate_resource_file(file_name, &file_data, mime_type)?;

        // AI 审核
        let ai_result = AiService::audit_resource(
            &request.title,
            request.description.as_deref(),
            Some(&file_data),
        )
        .await
        .map_err(|e| ResourceError::AiError(e.to_string()))?;

        // 生成资源 ID
        let resource_id = Uuid::new_v4();
        let resource_type_str = resource_type.to_string();
        let extension = FileService::get_extension_by_type(&resource_type_str);
        let file_key = format!("resources/{}.{}", resource_id, extension);
        let file_hash = FileService::calculate_hash(&file_data);
        let file_size = file_data.len() as i64;
        let storage_type = storage.backend_type().as_str().to_string();

        // 保存文件（统一走存储抽象）
        let file_path = storage.save_file(&file_key, file_data, mime_type).await?;

        // 确定审核状态
        let audit_status = if ai_result.passed {
            AuditStatus::Approved
        } else {
            AuditStatus::Pending
        };

        // 转换标签为 JSON
        let tags_json = request
            .tags
            .map(|tags| serde_json::to_value(tags).unwrap_or(serde_json::Value::Array(vec![])));

        // 开启事务
        let mut tx = match pool.begin().await {
            Ok(t) => t,
            Err(e) => {
                log::error!(
                    "[Resource] 开启事务失败 | resource_id={}, error={}",
                    resource_id,
                    e
                );
                // 开启事务失败时清理已保存的文件
                if let Err(cleanup_err) = storage.delete_file(&file_path).await {
                    log::error!(
                        "[Resource] 开启事务失败后清理文件出错 | path={}, error={}",
                        file_path,
                        cleanup_err
                    );
                }
                return Err(ResourceError::DatabaseError(format!("开启事务失败: {}", e)));
            }
        };

        // 插入资源记录
        log::debug!(
            "[Resource] 准备插入资源记录 | title={}, resource_type={}",
            request.title,
            resource_type.to_string()
        );

        let resource: Resource = match sqlx::query_as::<_, Resource>(
            r#"
            INSERT INTO resources (
                id, title, author_id, uploader_id, course_name,
                resource_type, category, tags, file_path, source_file_path,
                file_hash, file_size, content_accuracy, audit_status, ai_reject_reason, storage_type
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING *
            "#,
        )
        .bind(resource_id)
        .bind(&request.title)
        .bind(None::<Uuid>) // author_id 为空，等待申领
        .bind(user.id)
        .bind(request.course_name)
        .bind(resource_type.to_string())
        .bind(request.category.to_string())
        .bind(tags_json)
        .bind(&file_path)
        .bind(None::<String>) // source_file_path 暂不处理源文件
        .bind(&file_hash)
        .bind(file_size)
        .bind(ai_result.accuracy_score)
        .bind(audit_status.to_string())
        .bind(if ai_result.passed {
            None
        } else {
            ai_result.reason.as_deref()
        })
        .bind(&storage_type)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                log::error!(
                    "[Resource] 数据库插入失败 | resource_id={}, error={}",
                    resource_id,
                    e
                );
                // 数据库插入失败时清理已保存的文件
                if let Err(cleanup_err) = storage.delete_file(&file_path).await {
                    log::error!(
                        "[Resource] 数据库插入失败后清理文件出错 | path={}, error={}",
                        file_path,
                        cleanup_err
                    );
                }
                return Err(ResourceError::DatabaseError(format!("插入资源失败: {}", e)));
            }
        };

        log::debug!("[Resource] 资源记录插入成功 | resource_id={}", resource.id);

        // 创建资源统计记录
        if let Err(e) = sqlx::query(
            "INSERT INTO resource_stats (resource_id, views, downloads, likes, rating_count) VALUES ($1, 0, 0, 0, 0)"
        )
        .bind(resource_id)
        .execute(&mut *tx)
        .await {
            log::error!("[Resource] 创建统计记录失败 | resource_id={}, error={}", resource_id, e);
            // 统计记录创建失败时，回滚事务并清理文件
            if let Err(rollback_err) = tx.rollback().await {
                log::error!("[Resource] 回滚事务失败 | error={}", rollback_err);
            }
            if let Err(cleanup_err) = storage.delete_file(&file_path).await {
                log::error!("[Resource] 创建统计记录失败后清理文件出错 | path={}, error={}", file_path, cleanup_err);
            }
            return Err(ResourceError::DatabaseError(format!("创建统计记录失败: {}", e)));
        }

        // 插入教师关联记录
        if let Some(teacher_sns) = &request.teacher_sns {
            for teacher_sn in teacher_sns {
                if let Err(e) = sqlx::query(
                    "INSERT INTO resource_teachers (resource_id, teacher_sn) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                )
                .bind(resource_id)
                .bind(teacher_sn)
                .execute(&mut *tx)
                .await {
                    log::warn!("[Resource] 插入教师关联失败 | resource_id={}, teacher_sn={}, error={}", resource_id, teacher_sn, e);
                    // 非关键错误，继续处理
                }
            }
            log::debug!(
                "[Resource] 教师关联插入完成 | resource_id={}, count={}",
                resource_id,
                teacher_sns.len()
            );
        }

        // 插入课程关联记录
        if let Some(course_sns) = &request.course_sns {
            for course_sn in course_sns {
                if let Err(e) = sqlx::query(
                    "INSERT INTO resource_courses (resource_id, course_sn) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                )
                .bind(resource_id)
                .bind(course_sn)
                .execute(&mut *tx)
                .await {
                    log::warn!("[Resource] 插入课程关联失败 | resource_id={}, course_sn={}, error={}", resource_id, course_sn, e);
                    // 非关键错误，继续处理
                }
            }
            log::debug!(
                "[Resource] 课程关联插入完成 | resource_id={}, count={}",
                resource_id,
                course_sns.len()
            );
        }

        // 提交事务
        if let Err(e) = tx.commit().await {
            log::error!(
                "[Resource] 提交事务失败 | resource_id={}, error={}",
                resource_id,
                e
            );

            // 事务提交失败时尝试清理已保存的文件，避免产生孤立文件
            if let Err(cleanup_err) = storage.delete_file(&file_path).await {
                log::error!(
                    "[Resource] 事务提交失败后清理文件出错 | path={}, error={}",
                    file_path,
                    cleanup_err
                );
            }

            return Err(ResourceError::DatabaseError(format!("提交事务失败: {}", e)));
        }
        Ok(UploadResourceResponse {
            id: resource.id,
            title: resource.title,
            resource_type: resource.resource_type,
            audit_status: resource.audit_status,
            ai_message: if ai_result.passed {
                Some("AI 审核通过".to_string())
            } else {
                Some("AI 审核未通过，等待人工审核".to_string())
            },
            created_at: resource.created_at,
        })
    }

    /// 获取资源详情
    pub async fn get_resource_detail(
        pool: &PgPool,
        resource_id: Uuid,
    ) -> Result<ResourceDetailResponse, ResourceError> {
        // 获取资源信息
        let resource: Resource =
            sqlx::query_as::<_, Resource>("SELECT * FROM resources WHERE id = $1")
                .bind(resource_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ResourceError::DatabaseError(e.to_string()))?
                .ok_or_else(|| ResourceError::NotFound(format!("资源 {} 不存在", resource_id)))?;

        // 获取统计信息
        let stats: ResourceStats = sqlx::query_as::<_, ResourceStats>(
            "SELECT * FROM resource_stats WHERE resource_id = $1",
        )
        .bind(resource_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        // 获取上传者名称
        let uploader_name: Option<String> =
            sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
                .bind(resource.uploader_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        // 转换标签
        let tags: Option<Vec<String>> = resource
            .tags
            .as_ref()
            .and_then(|t| serde_json::from_value::<Vec<String>>(t.clone()).ok());

        // 获取关联的教师列表
        let teachers: Vec<super::TeacherInfo> = sqlx::query_as::<_, super::TeacherInfo>(
            r#"
            SELECT t.sn, t.name, t.department
            FROM teachers t
            INNER JOIN resource_teachers rt ON t.sn = rt.teacher_sn
            WHERE rt.resource_id = $1 AND t.is_active = true
            ORDER BY t.sn ASC
            "#,
        )
        .bind(resource_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            log::warn!(
                "[Resource] 获取关联教师失败 | resource_id={}, error={}",
                resource_id,
                e
            );
            e
        })
        .unwrap_or_default();

        // 获取关联的课程列表
        let courses: Vec<super::CourseInfo> = sqlx::query_as::<_, super::CourseInfo>(
            r#"
            SELECT c.sn, c.name, c.semester, c.credits
            FROM courses c
            INNER JOIN resource_courses rc ON c.sn = rc.course_sn
            WHERE rc.resource_id = $1 AND c.is_active = true
            ORDER BY c.sn ASC
            "#,
        )
        .bind(resource_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            log::warn!(
                "[Resource] 获取关联课程失败 | resource_id={}, error={}",
                resource_id,
                e
            );
            e
        })
        .unwrap_or_default();

        Ok(ResourceDetailResponse {
            id: resource.id,
            title: resource.title,
            author_id: resource.author_id,
            uploader_id: resource.uploader_id,
            course_name: resource.course_name,
            resource_type: resource.resource_type,
            category: resource.category,
            tags,
            description: None, // 暂不支持描述字段
            file_size: resource.file_size,
            audit_status: resource.audit_status,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
            stats: ResourceStatsResponse {
                views: stats.views,
                downloads: stats.downloads,
                likes: stats.likes,
                avg_difficulty: stats.avg_difficulty(),
                avg_overall_quality: stats.avg_overall_quality(),
                avg_answer_quality: stats.avg_answer_quality(),
                avg_format_quality: stats.avg_format_quality(),
                avg_detail_level: stats.avg_detail_level(),
                rating_count: stats.rating_count(),
            },
            uploader_name,
            teachers,
            courses,
        })
    }

    /// 获取资源列表
    /// 使用 QueryBuilder 构建动态查询，避免字符串拼接
    pub async fn get_resource_list(
        pool: &PgPool,
        query: &ResourceListQuery,
    ) -> Result<ResourceListResponse, ResourceError> {
        let page = query.get_page();
        let per_page = query.get_per_page();
        let offset = (page - 1) * per_page;

        // 构建排序
        let sort_by = match query.sort_by.as_deref() {
            Some("downloads") => "rs.downloads",
            Some("likes") => "rs.likes",
            // 按总体质量平均分排序（当 count > 0 时计算，否则视为 0）
            Some("rating") => "CASE WHEN rs.overall_quality_count > 0 THEN rs.overall_quality_total::FLOAT / rs.overall_quality_count ELSE 0 END",
            Some("title") => "r.title",
            _ => "r.created_at",
        };
        let sort_order = match query.sort_order.as_deref() {
            Some("asc") => "ASC",
            _ => "DESC",
        };

        // 使用 QueryBuilder 构建 COUNT 查询
        let mut count_builder = sqlx::QueryBuilder::new(
            "SELECT COUNT(*) FROM resources r WHERE r.audit_status = 'approved'",
        );

        // 处理资源类型筛选（支持合并类型）
        Self::add_resource_type_condition(&mut count_builder, query.resource_type.as_deref());

        // 处理分类筛选
        if let Some(ref category) = query.category {
            count_builder.push(" AND r.category = ");
            count_builder.push_bind(category);
        }

        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(pool)
            .await
            .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        // 使用 QueryBuilder 构建列表查询
        let mut list_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT r.*, rs.views, rs.downloads, rs.likes,
                   rs.difficulty_total, rs.difficulty_count,
                   rs.overall_quality_total, rs.overall_quality_count,
                   rs.answer_quality_total, rs.answer_quality_count,
                   rs.format_quality_total, rs.format_quality_count,
                   rs.detail_level_total, rs.detail_level_count,
                   u.username as uploader_name
            FROM resources r
            LEFT JOIN resource_stats rs ON r.id = rs.resource_id
            LEFT JOIN users u ON r.uploader_id = u.id
            WHERE r.audit_status = 'approved'
            "#,
        );

        // 处理资源类型筛选
        Self::add_resource_type_condition(&mut list_builder, query.resource_type.as_deref());

        // 处理分类筛选
        if let Some(ref category) = query.category {
            list_builder.push(" AND r.category = ");
            list_builder.push_bind(category);
        }

        // 添加排序和分页
        list_builder.push(format!(" ORDER BY {} {}", sort_by, sort_order));
        list_builder.push(" LIMIT ");
        list_builder.push_bind(per_page as i64);
        list_builder.push(" OFFSET ");
        list_builder.push_bind(offset as i64);

        let rows = list_builder
            .build()
            .fetch_all(pool)
            .await
            .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        let resources = Self::map_rows_to_resources(rows)?;

        Ok(ResourceListResponse {
            resources,
            total,
            page,
            per_page,
        })
    }

    /// 辅助方法：添加资源类型筛选条件到 QueryBuilder
    fn add_resource_type_condition<'a>(
        builder: &mut sqlx::QueryBuilder<'a, sqlx::Postgres>,
        resource_type: Option<&'a str>,
    ) {
        if let Some(resource_type) = resource_type {
            match resource_type {
                "ppt" => {
                    builder.push(" AND (r.resource_type = 'ppt' OR r.resource_type = 'pptx')");
                }
                "image" => {
                    builder.push(" AND (r.resource_type = 'jpeg' OR r.resource_type = 'jpg' OR r.resource_type = 'png')");
                }
                "doc" => {
                    builder.push(" AND (r.resource_type = 'doc' OR r.resource_type = 'docx')");
                }
                _ => {
                    builder.push(" AND r.resource_type = ");
                    builder.push_bind(resource_type);
                }
            }
        }
    }

    /// 辅助方法：将查询结果行映射为 ResourceListItem
    fn map_rows_to_resources(
        rows: Vec<sqlx::postgres::PgRow>,
    ) -> Result<Vec<ResourceListItem>, ResourceError> {
        let mut resources = Vec::new();
        for row in rows {
            let tags_json: Option<serde_json::Value> = row.try_get("tags").ok();
            let tags: Option<Vec<String>> =
                tags_json.and_then(|t| serde_json::from_value::<Vec<String>>(t).ok());

            // 计算各维度的平均分
            let avg_difficulty = Self::calc_avg(
                row.try_get::<i32, _>("difficulty_total").ok(),
                row.try_get::<i32, _>("difficulty_count").ok(),
            );
            let avg_overall_quality = Self::calc_avg(
                row.try_get::<i32, _>("overall_quality_total").ok(),
                row.try_get::<i32, _>("overall_quality_count").ok(),
            );
            let avg_answer_quality = Self::calc_avg(
                row.try_get::<i32, _>("answer_quality_total").ok(),
                row.try_get::<i32, _>("answer_quality_count").ok(),
            );
            let avg_format_quality = Self::calc_avg(
                row.try_get::<i32, _>("format_quality_total").ok(),
                row.try_get::<i32, _>("format_quality_count").ok(),
            );
            let avg_detail_level = Self::calc_avg(
                row.try_get::<i32, _>("detail_level_total").ok(),
                row.try_get::<i32, _>("detail_level_count").ok(),
            );

            // 评分人数取各维度中的最大值
            let rating_count: i32 = [
                row.try_get::<i32, _>("difficulty_count").unwrap_or(0),
                row.try_get::<i32, _>("overall_quality_count").unwrap_or(0),
                row.try_get::<i32, _>("answer_quality_count").unwrap_or(0),
                row.try_get::<i32, _>("format_quality_count").unwrap_or(0),
                row.try_get::<i32, _>("detail_level_count").unwrap_or(0),
            ]
            .iter()
            .max()
            .copied()
            .unwrap_or(0);

            resources.push(ResourceListItem {
                id: row
                    .try_get("id")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                title: row
                    .try_get("title")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                course_name: row.try_get("course_name").ok(),
                resource_type: row
                    .try_get("resource_type")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                category: row
                    .try_get("category")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                tags,
                audit_status: row
                    .try_get("audit_status")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                created_at: row
                    .try_get("created_at")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                stats: ResourceStatsResponse {
                    views: row.try_get::<i32, _>("views").unwrap_or(0),
                    downloads: row.try_get::<i32, _>("downloads").unwrap_or(0),
                    likes: row.try_get::<i32, _>("likes").unwrap_or(0),
                    avg_difficulty,
                    avg_overall_quality,
                    avg_answer_quality,
                    avg_format_quality,
                    avg_detail_level,
                    rating_count,
                },
                uploader_name: row.try_get("uploader_name").ok(),
            });
        }
        Ok(resources)
    }

    /// 计算平均分辅助函数
    fn calc_avg(total: Option<i32>, count: Option<i32>) -> Option<f64> {
        match (total, count) {
            (Some(t), Some(c)) if c > 0 => Some(t as f64 / c as f64),
            _ => None,
        }
    }

    /// 搜索资源
    /// 使用 QueryBuilder 构建动态查询，避免字符串拼接
    pub async fn search_resources(
        pool: &PgPool,
        query: &ResourceSearchQuery,
    ) -> Result<ResourceListResponse, ResourceError> {
        let page = query.get_page();
        let per_page = query.get_per_page();
        let offset = (page - 1) * per_page;

        let search_pattern = format!("%{}%", query.q);

        // 使用 QueryBuilder 构建 COUNT 查询
        let mut count_builder = sqlx::QueryBuilder::new(
            "SELECT COUNT(*) FROM resources r WHERE r.audit_status = 'approved' AND (r.title ILIKE "
        );
        count_builder.push_bind(&search_pattern);
        count_builder.push(" OR r.course_name ILIKE ");
        count_builder.push_bind(&search_pattern);
        count_builder.push(")");

        // 处理资源类型筛选（支持合并类型）
        Self::add_resource_type_condition(&mut count_builder, query.resource_type.as_deref());

        // 处理分类筛选
        if let Some(ref category) = query.category {
            count_builder.push(" AND r.category = ");
            count_builder.push_bind(category);
        }

        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(pool)
            .await
            .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        // 使用 QueryBuilder 构建搜索查询
        let mut search_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT r.*, rs.views, rs.downloads, rs.likes,
                   rs.difficulty_total, rs.difficulty_count,
                   rs.overall_quality_total, rs.overall_quality_count,
                   rs.answer_quality_total, rs.answer_quality_count,
                   rs.format_quality_total, rs.format_quality_count,
                   rs.detail_level_total, rs.detail_level_count,
                   u.username as uploader_name
            FROM resources r
            LEFT JOIN resource_stats rs ON r.id = rs.resource_id
            LEFT JOIN users u ON r.uploader_id = u.id
            WHERE r.audit_status = 'approved' AND (r.title ILIKE
            "#,
        );
        search_builder.push_bind(&search_pattern);
        search_builder.push(" OR r.course_name ILIKE ");
        search_builder.push_bind(&search_pattern);
        search_builder.push(")");

        // 处理资源类型筛选
        Self::add_resource_type_condition(&mut search_builder, query.resource_type.as_deref());

        // 处理分类筛选
        if let Some(ref category) = query.category {
            search_builder.push(" AND r.category = ");
            search_builder.push_bind(category);
        }

        // 添加排序和分页
        search_builder.push(" ORDER BY r.created_at DESC LIMIT ");
        search_builder.push_bind(per_page as i64);
        search_builder.push(" OFFSET ");
        search_builder.push_bind(offset as i64);

        let rows = search_builder
            .build()
            .fetch_all(pool)
            .await
            .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        let resources = Self::map_rows_to_resources(rows)?;

        Ok(ResourceListResponse {
            resources,
            total,
            page,
            per_page,
        })
    }

    /// 删除资源
    /// 返回被删除资源的标题
    pub async fn delete_resource(
        pool: &PgPool,
        user: &CurrentUser,
        storage: &Arc<dyn super::StorageBackend>,
        resource_id: Uuid,
    ) -> Result<String, ResourceError> {
        // 获取资源信息
        let resource: Resource =
            sqlx::query_as::<_, Resource>("SELECT * FROM resources WHERE id = $1")
                .bind(resource_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ResourceError::DatabaseError(e.to_string()))?
                .ok_or_else(|| ResourceError::NotFound(format!("资源 {} 不存在", resource_id)))?;

        // 检查权限（上传者或管理员）
        if resource.uploader_id != user.id && user.role != crate::models::UserRole::Admin {
            return Err(ResourceError::Unauthorized(
                "没有权限删除此资源".to_string(),
            ));
        }

        // 删除文件
        if let Err(e) = storage.delete_file(&resource.file_path).await {
            log::warn!(
                "[Resource] 删除资源文件失败 | resource_id={}, path={}, error={}",
                resource_id,
                resource.file_path,
                e
            );
            // 继续执行，即使文件删除失败也要删除数据库记录
        }

        // 删除源文件（如果存在）
        if let Some(source_path) = &resource.source_file_path {
            if let Err(e) = storage.delete_file(source_path).await {
                log::warn!(
                    "[Resource] 删除源文件失败 | resource_id={}, path={}, error={}",
                    resource_id,
                    source_path,
                    e
                );
            }
        }

        // 保存资源标题用于返回
        let title = resource.title.clone();

        // 删除数据库记录
        sqlx::query("DELETE FROM resources WHERE id = $1")
            .bind(resource_id)
            .execute(pool)
            .await
            .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        Ok(title)
    }

    /// 获取用户上传的资源列表
    pub async fn get_user_resources(
        pool: &PgPool,
        user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> Result<ResourceListResponse, ResourceError> {
        let offset = (page - 1) * per_page;

        // 获取总数
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE uploader_id = $1")
                .bind(user_id)
                .fetch_one(pool)
                .await
                .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        // 获取资源列表
        let rows = sqlx::query(
            r#"
            SELECT r.*, rs.views, rs.downloads, rs.likes,
                   rs.difficulty_total, rs.difficulty_count,
                   rs.overall_quality_total, rs.overall_quality_count,
                   rs.answer_quality_total, rs.answer_quality_count,
                   rs.format_quality_total, rs.format_quality_count,
                   rs.detail_level_total, rs.detail_level_count,
                   u.username as uploader_name
            FROM resources r
            LEFT JOIN resource_stats rs ON r.id = rs.resource_id
            LEFT JOIN users u ON r.uploader_id = u.id
            WHERE r.uploader_id = $1
            ORDER BY r.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(per_page as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        let mut resources = Vec::new();
        for (idx, row) in rows.iter().enumerate() {
            log::debug!("处理第 {} 行数据", idx);

            let tags_json: Option<serde_json::Value> = row.try_get("tags").ok();
            let tags: Option<Vec<String>> =
                tags_json.and_then(|t| serde_json::from_value::<Vec<String>>(t).ok());

            // 安全地获取每个字段
            let id: Uuid = row.try_get("id").map_err(|e| {
                log::error!("第 {} 行获取 id 失败: {:?}", idx, e);
                ResourceError::DatabaseError(format!("获取 id 失败: {}", e))
            })?;

            let title: String = row.try_get("title").map_err(|e| {
                log::error!("第 {} 行获取 title 失败: {:?}", idx, e);
                ResourceError::DatabaseError(format!("获取 title 失败: {}", e))
            })?;

            let resource_type: String = row.try_get("resource_type").map_err(|e| {
                log::error!("第 {} 行获取 resource_type 失败: {:?}", idx, e);
                ResourceError::DatabaseError(format!("获取 resource_type 失败: {}", e))
            })?;

            let category: String = row.try_get("category").map_err(|e| {
                log::error!("第 {} 行获取 category 失败: {:?}", idx, e);
                ResourceError::DatabaseError(format!("获取 category 失败: {}", e))
            })?;

            let audit_status: String = row.try_get("audit_status").map_err(|e| {
                log::error!("第 {} 行获取 audit_status 失败: {:?}", idx, e);
                ResourceError::DatabaseError(format!("获取 audit_status 失败: {}", e))
            })?;

            let created_at: chrono::NaiveDateTime = row.try_get("created_at").map_err(|e| {
                log::error!("第 {} 行获取 created_at 失败: {:?}", idx, e);
                ResourceError::DatabaseError(format!("获取 created_at 失败: {}", e))
            })?;

            // 处理 stats 字段（可能为 NULL 因为是 LEFT JOIN）
            let views: i32 = row.try_get::<i32, _>("views").unwrap_or(0);
            let downloads: i32 = row.try_get::<i32, _>("downloads").unwrap_or(0);
            let likes: i32 = row.try_get::<i32, _>("likes").unwrap_or(0);

            // 计算各维度的平均分
            let avg_difficulty = Self::calc_avg(
                row.try_get::<i32, _>("difficulty_total").ok(),
                row.try_get::<i32, _>("difficulty_count").ok(),
            );
            let avg_overall_quality = Self::calc_avg(
                row.try_get::<i32, _>("overall_quality_total").ok(),
                row.try_get::<i32, _>("overall_quality_count").ok(),
            );
            let avg_answer_quality = Self::calc_avg(
                row.try_get::<i32, _>("answer_quality_total").ok(),
                row.try_get::<i32, _>("answer_quality_count").ok(),
            );
            let avg_format_quality = Self::calc_avg(
                row.try_get::<i32, _>("format_quality_total").ok(),
                row.try_get::<i32, _>("format_quality_count").ok(),
            );
            let avg_detail_level = Self::calc_avg(
                row.try_get::<i32, _>("detail_level_total").ok(),
                row.try_get::<i32, _>("detail_level_count").ok(),
            );

            // 评分人数取各维度中的最大值
            let rating_count: i32 = [
                row.try_get::<i32, _>("difficulty_count").unwrap_or(0),
                row.try_get::<i32, _>("overall_quality_count").unwrap_or(0),
                row.try_get::<i32, _>("answer_quality_count").unwrap_or(0),
                row.try_get::<i32, _>("format_quality_count").unwrap_or(0),
                row.try_get::<i32, _>("detail_level_count").unwrap_or(0),
            ]
            .iter()
            .max()
            .copied()
            .unwrap_or(0);

            log::debug!(
                "资源 {} stats: views={}, downloads={}, likes={}",
                id,
                views,
                downloads,
                likes
            );

            resources.push(ResourceListItem {
                id,
                title,
                course_name: row.try_get("course_name").ok(),
                resource_type,
                category,
                tags,
                audit_status,
                created_at,
                stats: ResourceStatsResponse {
                    views,
                    downloads,
                    likes,
                    avg_difficulty,
                    avg_overall_quality,
                    avg_answer_quality,
                    avg_format_quality,
                    avg_detail_level,
                    rating_count,
                },
                uploader_name: row.try_get("uploader_name").ok(),
            });
        }

        log::debug!("成功构建 {} 个资源列表项", resources.len());

        Ok(ResourceListResponse {
            resources,
            total,
            page,
            per_page,
        })
    }

    /// 增加下载次数
    pub async fn increment_downloads(
        pool: &PgPool,
        resource_id: Uuid,
    ) -> Result<(), ResourceError> {
        sqlx::query("UPDATE resource_stats SET downloads = downloads + 1 WHERE resource_id = $1")
            .bind(resource_id)
            .execute(pool)
            .await
            .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 增加访问次数（预留接口）
    #[allow(dead_code)]
    pub async fn increment_views(pool: &PgPool, resource_id: Uuid) -> Result<(), ResourceError> {
        sqlx::query("UPDATE resource_stats SET views = views + 1 WHERE resource_id = $1")
            .bind(resource_id)
            .execute(pool)
            .await
            .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 获取资源文件路径（检查审核状态，用于下载）
    /// 返回：(file_path, resource_type, title)
    pub async fn get_resource_file_path(
        pool: &PgPool,
        resource_id: Uuid,
    ) -> Result<(String, String, String), ResourceError> {
        let row: (String, String, String) = sqlx::query_as(
            "SELECT file_path, resource_type, title FROM resources WHERE id = $1 AND audit_status = 'approved'"
        )
        .bind(resource_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ResourceError::DatabaseError(e.to_string()))?
        .ok_or_else(|| ResourceError::NotFound(format!("资源 {} 不存在或未通过审核", resource_id)))?;

        Ok(row)
    }

    /// 获取资源文件路径（不检查审核状态，用于预览）
    pub async fn get_resource_file_path_for_preview(
        pool: &PgPool,
        resource_id: Uuid,
    ) -> Result<(String, String), ResourceError> {
        let row: (String, String) =
            sqlx::query_as("SELECT file_path, resource_type FROM resources WHERE id = $1")
                .bind(resource_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ResourceError::DatabaseError(e.to_string()))?
                .ok_or_else(|| ResourceError::NotFound(format!("资源 {} 不存在", resource_id)))?;

        Ok(row)
    }

    /// 记录下载日志
    /// 将下载记录写入数据库，用于统计和审计
    pub async fn record_download(
        pool: &PgPool,
        resource_id: Uuid,
        user_id: Option<Uuid>,
        ip_address: &str,
    ) -> Result<(), ResourceError> {
        sqlx::query(
            "INSERT INTO download_logs (resource_id, user_id, ip_address) VALUES ($1, $2, $3::inet)"
        )
        .bind(resource_id)
        .bind(user_id)
        .bind(ip_address)
        .execute(pool)
        .await
        .map_err(|e| {
            log::warn!("记录下载日志失败: {}", e);
            ResourceError::DatabaseError(e.to_string())
        })?;

        Ok(())
    }

    /// 更新资源内容（用于Markdown在线编辑）
    /// 更新后会进行AI审核，并更新 file_hash、file_size、updated_at 字段
    pub async fn update_resource_content(
        pool: &PgPool,
        user: &CurrentUser,
        storage: &Arc<dyn super::StorageBackend>,
        resource_id: Uuid,
        content: String,
    ) -> Result<crate::models::UpdateResourceContentResponse, ResourceError> {
        // 验证内容长度
        if content.len() > 10 * 1024 * 1024 {
            return Err(ResourceError::ValidationError(
                "内容大小超过10MB限制".to_string(),
            ));
        }

        // 获取资源信息
        let resource: crate::models::Resource =
            sqlx::query_as::<_, crate::models::Resource>("SELECT * FROM resources WHERE id = $1")
                .bind(resource_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ResourceError::DatabaseError(e.to_string()))?
                .ok_or_else(|| ResourceError::NotFound(format!("资源 {} 不存在", resource_id)))?;

        // 检查权限（上传者或管理员）
        if resource.uploader_id != user.id && user.role != crate::models::UserRole::Admin {
            return Err(ResourceError::Unauthorized(
                "没有权限编辑此资源".to_string(),
            ));
        }

        // 检查资源类型是否为web_markdown
        if resource.resource_type != "web_markdown" {
            return Err(ResourceError::ValidationError(
                "只有Markdown类型资源可以在线编辑".to_string(),
            ));
        }

        // AI 审核更新后的内容
        let ai_result =
            AiService::audit_resource(&resource.title, Some(&content), Some(content.as_bytes()))
                .await
                .map_err(|e| ResourceError::AiError(e.to_string()))?;

        // 更新文件内容
        storage
            .write_file(
                &resource.file_path,
                content.as_bytes().to_vec(),
                Some("text/markdown"),
            )
            .await?;

        // 计算新的文件哈希和大小（使用字节长度而非字符长度）
        let file_hash = crate::services::FileService::calculate_hash(content.as_bytes());
        let file_size = content.as_bytes().len() as i64;

        // 确定审核状态
        let audit_status = if ai_result.passed {
            AuditStatus::Approved
        } else {
            AuditStatus::Pending
        };

        // 更新数据库中的 updated_at、file_hash、file_size、audit_status、content_accuracy
        let updated_at = sqlx::query_scalar::<_, chrono::NaiveDateTime>(
            r#"
            UPDATE resources
            SET
                updated_at = CURRENT_TIMESTAMP,
                file_hash = $1,
                file_size = $2,
                audit_status = $3,
                content_accuracy = $4,
                ai_reject_reason = $5
            WHERE id = $6
            RETURNING updated_at
            "#,
        )
        .bind(file_hash)
        .bind(file_size)
        .bind(audit_status.to_string())
        .bind(ai_result.accuracy_score)
        .bind(if ai_result.passed {
            None
        } else {
            ai_result.reason
        })
        .bind(resource_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ResourceError::DatabaseError(e.to_string()))?;

        Ok(crate::models::UpdateResourceContentResponse {
            id: resource_id,
            updated_at,
        })
    }

    /// 获取资源原始内容（用于编辑）
    pub async fn get_resource_content_raw(
        pool: &PgPool,
        storage: &Arc<dyn super::StorageBackend>,
        user: &CurrentUser,
        resource_id: Uuid,
    ) -> Result<String, ResourceError> {
        // 获取资源信息
        let resource: crate::models::Resource =
            sqlx::query_as::<_, crate::models::Resource>("SELECT * FROM resources WHERE id = $1")
                .bind(resource_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ResourceError::DatabaseError(e.to_string()))?
                .ok_or_else(|| ResourceError::NotFound(format!("资源 {} 不存在", resource_id)))?;

        // 检查权限（上传者或管理员）
        if resource.uploader_id != user.id && user.role != crate::models::UserRole::Admin {
            return Err(ResourceError::Unauthorized(
                "没有权限查看此资源的原始内容".to_string(),
            ));
        }

        // 读取文件内容（统一走存储抽象，兼容 local/oss）
        let content_bytes = storage.read_file(&resource.file_path).await?;
        let content = String::from_utf8(content_bytes)
            .map_err(|e| ResourceError::FileError(format!("文件内容不是有效 UTF-8: {}", e)))?;

        Ok(content)
    }

    /// 获取热门资源列表
    /// 按浏览量降序排序（主要），下载量次之
    /// 返回所有资源（包括待审核的），只要浏览量>0或按创建时间排序
    pub async fn get_hot_resources(
        pool: &PgPool,
        limit: i32,
    ) -> Result<Vec<crate::models::HotResourceItem>, ResourceError> {
        let limit = limit.max(1).min(20);

        log::info!("获取热门资源，限制数量: {}", limit);

        // 先检查资源总数
        let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resources")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
        log::info!("数据库中共有 {} 条资源", total_count);

        // 先尝试获取有浏览量的资源
        let rows = sqlx::query(
            r#"
            SELECT 
                r.id,
                r.title,
                r.course_name,
                r.resource_type,
                COALESCE(rs.downloads, 0) as downloads,
                COALESCE(rs.views, 0) as views,
                COALESCE(rs.likes, 0) as likes
            FROM resources r
            LEFT JOIN resource_stats rs ON r.id = rs.resource_id
            ORDER BY COALESCE(rs.views, 0) DESC, COALESCE(rs.downloads, 0) DESC, r.created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            log::error!("获取热门资源查询失败: {}", e);
            ResourceError::DatabaseError(e.to_string())
        })?;

        log::info!("获取到 {} 条热门资源", rows.len());

        let mut resources = Vec::new();
        for row in rows {
            resources.push(crate::models::HotResourceItem {
                id: row
                    .try_get("id")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                title: row
                    .try_get("title")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                course_name: row.try_get("course_name").ok(),
                resource_type: row
                    .try_get("resource_type")
                    .map_err(|e| ResourceError::DatabaseError(e.to_string()))?,
                downloads: row.try_get::<i32, _>("downloads").unwrap_or(0),
                views: row.try_get::<i32, _>("views").unwrap_or(0),
                likes: row.try_get::<i32, _>("likes").unwrap_or(0),
            });
        }

        Ok(resources)
    }
}
