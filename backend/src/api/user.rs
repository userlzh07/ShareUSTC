use actix_web::{get, put, post, web, HttpResponse, Responder};
use crate::db::AppState;
use crate::models::{CurrentUser, UpdateProfileRequest, VerificationRequest};
use crate::services::{UserError, UserService};
use uuid::Uuid;

/// 获取当前用户信息
#[get("/api/users/me")]
pub async fn get_current_user(
    state: web::Data<AppState>,
    user: web::ReqData<CurrentUser>,
) -> impl Responder {
    match UserService::get_current_user(&state.pool, user.id).await {
        Ok(user_info) => HttpResponse::Ok().json(serde_json::json!({
            "code": 200,
            "message": "获取成功",
            "data": user_info
        })),
        Err(e) => {
            log::warn!("获取当前用户失败: {}", e);
            let (code, message) = match e {
                UserError::UserNotFound(_) => (404, e.to_string()),
                _ => (500, "获取用户信息失败".to_string()),
            };
            HttpResponse::Ok().json(serde_json::json!({
                "code": code,
                "message": message,
                "data": null
            }))
        }
    }
}

/// 更新当前用户资料
#[put("/api/users/me")]
pub async fn update_profile(
    state: web::Data<AppState>,
    user: web::ReqData<CurrentUser>,
    req: web::Json<UpdateProfileRequest>,
) -> impl Responder {
    // 检查是否为实名用户（只有实名用户可以修改资料）
    if user.role != crate::models::UserRole::Verified
        && user.role != crate::models::UserRole::Admin
    {
        return HttpResponse::Ok().json(serde_json::json!({
            "code": 403,
            "message": "只有实名用户可以修改资料",
            "data": null
        }));
    }

    match UserService::update_profile(&state.pool, user.id, req.into_inner()).await {
        Ok(user_info) => HttpResponse::Ok().json(serde_json::json!({
            "code": 200,
            "message": "更新成功",
            "data": user_info
        })),
        Err(e) => {
            log::warn!("更新用户资料失败: {}", e);
            let (code, message) = match e {
                UserError::UserNotFound(_) => (404, e.to_string()),
                UserError::UserExists(_) => (409, e.to_string()),
                UserError::ValidationError(_) => (400, e.to_string()),
                _ => (500, "更新失败".to_string()),
            };
            HttpResponse::Ok().json(serde_json::json!({
                "code": code,
                "message": message,
                "data": null
            }))
        }
    }
}

/// 实名认证
#[post("/api/users/verify")]
pub async fn verify_user(
    state: web::Data<AppState>,
    user: web::ReqData<CurrentUser>,
    req: web::Json<VerificationRequest>,
) -> impl Responder {
    // 检查是否为注册用户（只有非实名用户可以认证）
    if user.role == crate::models::UserRole::Verified
        || user.role == crate::models::UserRole::Admin
    {
        return HttpResponse::Ok().json(serde_json::json!({
            "code": 400,
            "message": "用户已完成实名认证",
            "data": null
        }));
    }

    match UserService::verify_user(&state.pool, user.id, req.into_inner()).await {
        Ok(user_info) => HttpResponse::Ok().json(serde_json::json!({
            "code": 200,
            "message": "实名认证成功",
            "data": user_info
        })),
        Err(e) => {
            log::warn!("实名认证失败: {}", e);
            let (code, message) = match e {
                UserError::UserNotFound(_) => (404, e.to_string()),
                UserError::ValidationError(_) => (400, e.to_string()),
                _ => (500, "认证失败".to_string()),
            };
            HttpResponse::Ok().json(serde_json::json!({
                "code": code,
                "message": message,
                "data": null
            }))
        }
    }
}

/// 获取用户公开资料
#[get("/api/users/{user_id}")]
pub async fn get_user_profile(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    _user: web::ReqData<CurrentUser>, // 需要登录才能查看他人资料
) -> impl Responder {
    let user_id = path.into_inner();

    match UserService::get_user_profile(&state.pool, user_id).await {
        Ok(profile) => HttpResponse::Ok().json(serde_json::json!({
            "code": 200,
            "message": "获取成功",
            "data": profile
        })),
        Err(e) => {
            log::warn!("获取用户资料失败: {}", e);
            let (code, message) = match e {
                UserError::UserNotFound(_) => (404, e.to_string()),
                _ => (500, "获取用户资料失败".to_string()),
            };
            HttpResponse::Ok().json(serde_json::json!({
                "code": code,
                "message": message,
                "data": null
            }))
        }
    }
}

/// 配置用户路由
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_current_user)
        .service(update_profile)
        .service(verify_user)
        .service(get_user_profile);
}
