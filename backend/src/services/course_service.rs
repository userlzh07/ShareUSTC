use sqlx::PgPool;

use crate::models::{
    Course, CourseListQuery, CourseListResponse, CreateCourseRequest, UpdateCourseRequest,
    UpdateCourseStatusRequest,
};

/// 课程服务错误类型
#[derive(Debug)]
pub enum CourseError {
    DatabaseError(String),
    NotFound(String),
    ValidationError(String),
}

impl std::fmt::Display for CourseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CourseError::DatabaseError(msg) => write!(f, "数据库错误: {}", msg),
            CourseError::NotFound(msg) => write!(f, "未找到: {}", msg),
            CourseError::ValidationError(msg) => write!(f, "验证错误: {}", msg),
        }
    }
}

impl std::error::Error for CourseError {}

/// 课程服务
pub struct CourseService;

impl CourseService {
    /// 创建课程
    pub async fn create_course(
        pool: &PgPool,
        req: CreateCourseRequest,
    ) -> Result<Course, CourseError> {
        // 验证请求
        if let Err(e) = req.validate() {
            return Err(CourseError::ValidationError(e));
        }

        let course = sqlx::query_as::<_, Course>(
            r#"
            INSERT INTO courses (name, semester, credits, is_active)
            VALUES ($1, $2, $3, true)
            RETURNING id, sn, name, semester, credits, is_active, created_at, updated_at
            "#,
        )
        .bind(&req.name)
        .bind(&req.semester)
        .bind(req.credits)
        .fetch_one(pool)
        .await
        .map_err(|e| CourseError::DatabaseError(e.to_string()))?;

        Ok(course)
    }

    /// 获取课程列表（管理员）
    pub async fn get_course_list(
        pool: &PgPool,
        query: CourseListQuery,
    ) -> Result<CourseListResponse, CourseError> {
        let page = query.get_page();
        let per_page = query.get_per_page();
        let offset = (page - 1) * per_page;

        // 构建查询条件
        let mut conditions = vec!["1=1".to_string()];
        if let Some(semester) = &query.semester {
            conditions.push(format!("semester = '{}'", semester));
        }
        if let Some(is_active) = query.is_active {
            conditions.push(format!("is_active = {}", is_active));
        }
        let where_clause = conditions.join(" AND ");

        // 查询总数
        let count_sql = format!("SELECT COUNT(*) FROM courses WHERE {}", where_clause);
        let total: i64 = sqlx::query_scalar(&count_sql)
            .fetch_one(pool)
            .await
            .map_err(|e| CourseError::DatabaseError(e.to_string()))?;

        // 查询列表
        let list_sql = format!(
            r#"
            SELECT id, sn, name, semester, credits, is_active, created_at, updated_at
            FROM courses
            WHERE {}
            ORDER BY sn ASC
            LIMIT {} OFFSET {}
            "#,
            where_clause, per_page, offset
        );

        let courses = sqlx::query_as::<_, Course>(&list_sql)
            .fetch_all(pool)
            .await
            .map_err(|e| CourseError::DatabaseError(e.to_string()))?;

        Ok(CourseListResponse {
            courses,
            total,
            page,
            per_page,
        })
    }

    /// 根据编号获取课程
    pub async fn get_course_by_sn(pool: &PgPool, sn: i64) -> Result<Course, CourseError> {
        let course = sqlx::query_as::<_, Course>(
            r#"
            SELECT id, sn, name, semester, credits, is_active, created_at, updated_at
            FROM courses
            WHERE sn = $1
            "#,
        )
        .bind(sn)
        .fetch_optional(pool)
        .await
        .map_err(|e| CourseError::DatabaseError(e.to_string()))?;

        course.ok_or_else(|| CourseError::NotFound(format!("课程编号 {} 不存在", sn)))
    }

    /// 获取有效课程列表（公开）
    pub async fn get_active_courses(pool: &PgPool) -> Result<Vec<Course>, CourseError> {
        let courses = sqlx::query_as::<_, Course>(
            r#"
            SELECT id, sn, name, semester, credits, is_active, created_at, updated_at
            FROM courses
            WHERE is_active = true
            ORDER BY sn ASC
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| CourseError::DatabaseError(e.to_string()))?;

        Ok(courses)
    }

    /// 更新课程信息
    pub async fn update_course(
        pool: &PgPool,
        sn: i64,
        req: UpdateCourseRequest,
    ) -> Result<Course, CourseError> {
        // 验证请求
        if let Err(e) = req.validate() {
            return Err(CourseError::ValidationError(e));
        }

        // 检查课程是否存在
        let existing = Self::get_course_by_sn(pool, sn).await?;

        // 构建更新语句
        let mut updates = vec![];
        if let Some(name) = req.name {
            updates.push(format!("name = '{}'", name));
        }
        if let Some(semester) = req.semester {
            updates.push(format!("semester = '{}'", semester));
        }
        if let Some(credits) = req.credits {
            updates.push(format!("credits = {}", credits));
        }

        if updates.is_empty() {
            return Ok(existing);
        }

        let update_sql = format!(
            "UPDATE courses SET {}, updated_at = CURRENT_TIMESTAMP WHERE sn = $1 RETURNING id, sn, name, semester, credits, is_active, created_at, updated_at",
            updates.join(", ")
        );

        let course = sqlx::query_as::<_, Course>(&update_sql)
            .bind(sn)
            .fetch_one(pool)
            .await
            .map_err(|e| CourseError::DatabaseError(e.to_string()))?;

        Ok(course)
    }

    /// 更新课程状态
    pub async fn update_course_status(
        pool: &PgPool,
        sn: i64,
        req: UpdateCourseStatusRequest,
    ) -> Result<Course, CourseError> {
        let course = sqlx::query_as::<_, Course>(
            r#"
            UPDATE courses
            SET is_active = $1, updated_at = CURRENT_TIMESTAMP
            WHERE sn = $2
            RETURNING id, sn, name, semester, credits, is_active, created_at, updated_at
            "#,
        )
        .bind(req.is_active)
        .bind(sn)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("no rows") {
                CourseError::NotFound(format!("课程编号 {} 不存在", sn))
            } else {
                CourseError::DatabaseError(e.to_string())
            }
        })?;

        Ok(course)
    }

    /// 删除课程
    pub async fn delete_course(pool: &PgPool, sn: i64) -> Result<(), CourseError> {
        let result = sqlx::query("DELETE FROM courses WHERE sn = $1")
            .bind(sn)
            .execute(pool)
            .await
            .map_err(|e| CourseError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(CourseError::NotFound(format!("课程编号 {} 不存在", sn)));
        }

        Ok(())
    }
}
