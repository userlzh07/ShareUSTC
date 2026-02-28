use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// 点赞实体
/// 注意：当前实现使用原子操作，此结构体暂未使用，保留以备将来查询点赞列表
#[allow(dead_code)]
#[derive(Debug, FromRow, Serialize)]
pub struct Like {
    pub resource_id: Uuid,
    pub user_id: Uuid,
    pub created_at: NaiveDateTime,
}

/// 点赞状态响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LikeStatusResponse {
    pub is_liked: bool,
    pub like_count: i64,
}

/// 点赞操作响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LikeToggleResponse {
    pub is_liked: bool,
    pub like_count: i64,
    pub message: String,
}
