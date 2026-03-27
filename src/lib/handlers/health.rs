use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::app_state::AppState;
use crate::models::HealthResponse;

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub(crate) async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => (StatusCode::OK, Json(HealthResponse { status: "ready" })).into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse { status: "not ready" }),
        )
            .into_response(),
    }
}
