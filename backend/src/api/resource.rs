use actix_web::{get, HttpResponse, Responder};

#[get("/api/resources/test")]
pub async fn resource_test() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Resource module initialized"
    }))
}
