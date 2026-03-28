use axum::{
    Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

pub fn routes() -> axum::Router {
    axum::Router::new()
        .route("/hello", get(hello))
        .route("/echo", post(echo))
}

async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello from Tsukuyomi!".to_string(),
    })
}

async fn echo(Json(body): Json<EchoRequest>) -> Json<EchoResponse> {
    Json(EchoResponse {
        you_said: body.message,
    })
}

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

#[derive(Deserialize)]
struct EchoRequest {
    message: String,
}

#[derive(Serialize)]
struct EchoResponse {
    you_said: String,
}
