use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header,
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
    task::{Context, Poll},
};

use crate::models::CurrentUser;
use crate::utils::{extract_current_user, verify_token};

/// JWT认证中间件
pub struct JwtAuth {
    jwt_secret: String,
}

impl JwtAuth {
    pub fn new(jwt_secret: String) -> Self {
        Self { jwt_secret }
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = JwtAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtAuthMiddleware {
            service: Rc::new(service),
            jwt_secret: self.jwt_secret.clone(),
        }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
    jwt_secret: String,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let jwt_secret = self.jwt_secret.clone();

        Box::pin(async move {
            // 从请求头中提取Authorization
            let auth_header = req
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok());

            let token = match auth_header {
                Some(header) if header.starts_with("Bearer ") => {
                    header.trim_start_matches("Bearer ").to_string()
                }
                _ => {
                    log::debug!("请求缺少Authorization头");
                    return Err(ErrorUnauthorized("缺少认证信息"));
                }
            };

            // 验证Token
            match verify_token(&token, &jwt_secret, Some("access")) {
                Ok(claims) => {
                    match extract_current_user(claims) {
                        Ok(current_user) => {
                            log::debug!("用户认证成功: {}", current_user.username);
                            // 将用户信息存入请求扩展
                            req.extensions_mut().insert(current_user);
                            // 继续处理请求
                            service.call(req).await
                        }
                        Err(e) => {
                            log::warn!("提取用户信息失败: {}", e);
                            Err(ErrorUnauthorized("无效的认证信息"))
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Token验证失败: {}", e);
                    Err(ErrorUnauthorized("认证已过期或无效"))
                }
            }
        })
    }
}

/// 从请求中提取当前用户
pub fn get_current_user(req: &ServiceRequest) -> Option<CurrentUser> {
    req.extensions().get::<CurrentUser>().cloned()
}

/// 角色检查中间件工厂
pub struct RequireRole {
    role: String,
}

impl RequireRole {
    pub fn admin() -> Self {
        Self {
            role: "admin".to_string(),
        }
    }

    pub fn verified() -> Self {
        Self {
            role: "verified".to_string(),
        }
    }
}

/// 需要认证的处理函数包装器
pub async fn auth_required<F, Fut>(
    req: ServiceRequest,
    f: F,
) -> Result<ServiceResponse<actix_web::body::BoxBody>, Error>
where
    F: FnOnce(ServiceRequest) -> Fut,
    Fut: std::future::Future<Output = Result<ServiceResponse<actix_web::body::BoxBody>, Error>>,
{
    if req.extensions().get::<CurrentUser>().is_none() {
        return Err(ErrorUnauthorized("需要登录"));
    }
    f(req).await
}
