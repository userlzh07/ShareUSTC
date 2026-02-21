use actix_web::{delete, web, HttpRequest, HttpResponse, Responder};
use uuid::Uuid;

use crate::db::AppState;
use crate::models::{CurrentUser, UserRole};
use crate::services::{AuditLogService, CommentService, ResourceError};
use crate::utils::{forbidden, internal_error, not_found};

/// 删除评论
#[delete("/comments/{comment_id}")]
pub async fn delete_comment(
    state: web::Data<AppState>,
    user: web::ReqData<CurrentUser>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> impl Responder {
    let comment_id = path.into_inner();
    let is_admin = user.role == UserRole::Admin;

    match CommentService::delete_comment(&state.pool, comment_id, user.id, is_admin).await {
        Ok(true) => {
            // 记录审计日志
            let ip_address = req.peer_addr().map(|addr| addr.ip().to_string());
            if let Err(e) = AuditLogService::log_delete_comment(
                &state.pool,
                user.id,
                comment_id,
                is_admin,
                ip_address.as_deref(),
            )
            .await
            {
                log::warn!(
                    "[Audit] 记录删除评论日志失败 | comment_id={}, error={}",
                    comment_id,
                    e
                );
            }

            HttpResponse::NoContent().finish()
        }
        Ok(false) => forbidden("无权删除该评论"),
        Err(ResourceError::NotFound(msg)) => not_found(&msg),
        Err(e) => {
            log::warn!("删除评论失败: {}", e);
            internal_error("删除失败")
        }
    }
}

/// 配置评论路由
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(delete_comment);
}
