use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{
    CreateTeacherRequest, Teacher, TeacherListQuery, TeacherListResponse, UpdateTeacherRequest,
    UpdateTeacherStatusRequest,
};

/// 教师服务错误类型
#[derive(Debug)]
pub enum TeacherError {
    DatabaseError(String),
    NotFound(String),
    ValidationError(String),
}

impl std::fmt::Display for TeacherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeacherError::DatabaseError(msg) => write!(f, "数据库错误: {}", msg),
            TeacherError::NotFound(msg) => write!(f, "未找到: {}", msg),
            TeacherError::ValidationError(msg) => write!(f, "验证错误: {}", msg),
        }
    }
}

impl std::error::Error for TeacherError {}

/// 教师服务
pub struct TeacherService;

impl TeacherService {
    /// 创建教师
    pub async fn create_teacher(
        pool: &PgPool,
        req: CreateTeacherRequest,
    ) -> Result<Teacher, TeacherError> {
        // 验证请求
        if let Err(e) = req.validate() {
            return Err(TeacherError::ValidationError(e));
        }

        let teacher = sqlx::query_as::<_, Teacher>(
            r#"
            INSERT INTO teachers (name, department, is_active)
            VALUES ($1, $2, true)
            RETURNING id, sn, name, department, is_active, created_at, updated_at
            "#,
        )
        .bind(&req.name)
        .bind(&req.department)
        .fetch_one(pool)
        .await
        .map_err(|e| TeacherError::DatabaseError(e.to_string()))?;

        Ok(teacher)
    }

    /// 获取教师列表（管理员）
    pub async fn get_teacher_list(
        pool: &PgPool,
        query: TeacherListQuery,
    ) -> Result<TeacherListResponse, TeacherError> {
        let page = query.get_page();
        let per_page = query.get_per_page();
        let offset = (page - 1) * per_page;

        // 构建查询条件
        let mut conditions = vec!["1=1".to_string()];
        if let Some(dept) = &query.department {
            conditions.push(format!("department = '{}'", dept));
        }
        if let Some(is_active) = query.is_active {
            conditions.push(format!("is_active = {}", is_active));
        }
        let where_clause = conditions.join(" AND ");

        // 查询总数
        let count_sql = format!("SELECT COUNT(*) FROM teachers WHERE {}", where_clause);
        let total: i64 = sqlx::query_scalar(&count_sql)
            .fetch_one(pool)
            .await
            .map_err(|e| TeacherError::DatabaseError(e.to_string()))?;

        // 查询列表
        let list_sql = format!(
            r#"
            SELECT id, sn, name, department, is_active, created_at, updated_at
            FROM teachers
            WHERE {}
            ORDER BY sn ASC
            LIMIT {} OFFSET {}
            "#,
            where_clause, per_page, offset
        );

        let teachers = sqlx::query_as::<_, Teacher>(&list_sql)
            .fetch_all(pool)
            .await
            .map_err(|e| TeacherError::DatabaseError(e.to_string()))?;

        Ok(TeacherListResponse {
            teachers,
            total,
            page,
            per_page,
        })
    }

    /// 根据编号获取教师
    pub async fn get_teacher_by_sn(pool: &PgPool, sn: i64) -> Result<Teacher, TeacherError> {
        let teacher = sqlx::query_as::<_, Teacher>(
            r#"
            SELECT id, sn, name, department, is_active, created_at, updated_at
            FROM teachers
            WHERE sn = $1
            "#,
        )
        .bind(sn)
        .fetch_optional(pool)
        .await
        .map_err(|e| TeacherError::DatabaseError(e.to_string()))?;

        teacher.ok_or_else(|| TeacherError::NotFound(format!("教师编号 {} 不存在", sn)))
    }

    /// 获取有效教师列表（公开）
    pub async fn get_active_teachers(pool: &PgPool) -> Result<Vec<Teacher>, TeacherError> {
        let teachers = sqlx::query_as::<_, Teacher>(
            r#"
            SELECT id, sn, name, department, is_active, created_at, updated_at
            FROM teachers
            WHERE is_active = true
            ORDER BY sn ASC
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| TeacherError::DatabaseError(e.to_string()))?;

        Ok(teachers)
    }

    /// 更新教师信息
    pub async fn update_teacher(
        pool: &PgPool,
        sn: i64,
        req: UpdateTeacherRequest,
    ) -> Result<Teacher, TeacherError> {
        // 验证请求
        if let Err(e) = req.validate() {
            return Err(TeacherError::ValidationError(e));
        }

        // 检查教师是否存在
        let existing = Self::get_teacher_by_sn(pool, sn).await?;

        // 构建更新语句
        let mut updates = vec![];
        if let Some(name) = req.name {
            updates.push(format!("name = '{}'", name));
        }
        if let Some(department) = req.department {
            updates.push(format!("department = '{}'", department));
        }

        if updates.is_empty() {
            return Ok(existing);
        }

        let update_sql = format!(
            "UPDATE teachers SET {}, updated_at = CURRENT_TIMESTAMP WHERE sn = $1 RETURNING id, sn, name, department, is_active, created_at, updated_at",
            updates.join(", ")
        );

        let teacher = sqlx::query_as::<_, Teacher>(&update_sql)
            .bind(sn)
            .fetch_one(pool)
            .await
            .map_err(|e| TeacherError::DatabaseError(e.to_string()))?;

        Ok(teacher)
    }

    /// 更新教师状态
    pub async fn update_teacher_status(
        pool: &PgPool,
        sn: i64,
        req: UpdateTeacherStatusRequest,
    ) -> Result<Teacher, TeacherError> {
        let teacher = sqlx::query_as::<_, Teacher>(
            r#"
            UPDATE teachers
            SET is_active = $1, updated_at = CURRENT_TIMESTAMP
            WHERE sn = $2
            RETURNING id, sn, name, department, is_active, created_at, updated_at
            "#,
        )
        .bind(req.is_active)
        .bind(sn)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("no rows") {
                TeacherError::NotFound(format!("教师编号 {} 不存在", sn))
            } else {
                TeacherError::DatabaseError(e.to_string())
            }
        })?;

        Ok(teacher)
    }

    /// 删除教师
    pub async fn delete_teacher(pool: &PgPool, sn: i64) -> Result<(), TeacherError> {
        let result = sqlx::query("DELETE FROM teachers WHERE sn = $1")
            .bind(sn)
            .execute(pool)
            .await
            .map_err(|e| TeacherError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(TeacherError::NotFound(format!("教师编号 {} 不存在", sn)));
        }

        Ok(())
    }
}
