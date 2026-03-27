use std::{env, time::Instant};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderValue, Method},
    middleware::{Next, from_fn, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use tower_http::{cors::{Any, CorsLayer}, trace::TraceLayer};

use crate::app_state::AppState;
use crate::auth::{AUTH_HEADER, auth_enforced, validate_authorization_header};
use crate::handlers::{
    create_deliverable, create_message, create_milestone, create_project,
    health, list_deliverables, list_messages, list_milestones, list_projects, ready,
};
use crate::models::ApiError;
use crate::rate_limit::rate_limit_middleware;

pub fn build_router(state: AppState) -> Router {
    // Admin-only write routes
    let admin_routes = Router::new()
        .route("/api/v1/projects", post(create_project))
        .route("/api/v1/projects/{id}/milestones", post(create_milestone))
        .route("/api/v1/milestones/{id}/deliverables", post(create_deliverable))
        .layer(from_fn(require_admin));

    // Auth-required routes (client + admin reads, message sends)
    let protected_routes = Router::new()
        .route("/api/v1/projects", get(list_projects))
        .route("/api/v1/projects/{id}/milestones", get(list_milestones))
        .route("/api/v1/milestones/{id}/deliverables", get(list_deliverables))
        .route("/api/v1/projects/{id}/messages", get(list_messages).post(create_message))
        .merge(admin_routes)
        .layer(from_fn(require_auth));

    Router::new()
        .route("/", get(health))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .merge(protected_routes)
        .layer(from_fn(rate_limit_middleware))
        .layer(from_fn_with_state(state.clone(), audit_request))
        .with_state(state)
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
}

async fn require_auth(request: Request, next: Next) -> Response {
    if !auth_enforced() {
        return next.run(request).await;
    }
    let header = request.headers().get(AUTH_HEADER).and_then(|v| v.to_str().ok());
    match validate_authorization_header(header) {
        Ok(claims) if !claims.sub.trim().is_empty() => next.run(request).await,
        Ok(_) => (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(ApiError {
                code: "AUTH_INVALID_TOKEN".to_string(),
                message: "token subject is missing".to_string(),
                details: None,
            }),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(ApiError {
                code: e.code().to_string(),
                message: e.message().to_string(),
                details: Some(json!({ "required": "Authorization: Bearer <token>" })),
            }),
        )
            .into_response(),
    }
}

async fn require_admin(request: Request, next: Next) -> Response {
    if !auth_enforced() {
        return next.run(request).await;
    }
    let header = request.headers().get(AUTH_HEADER).and_then(|v| v.to_str().ok());
    match validate_authorization_header(header) {
        Ok(claims) if claims.is_admin() => next.run(request).await,
        Ok(_) => (
            axum::http::StatusCode::FORBIDDEN,
            Json(ApiError {
                code: "AUTH_ADMIN_REQUIRED".to_string(),
                message: "admin role is required".to_string(),
                details: None,
            }),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(ApiError {
                code: e.code().to_string(),
                message: e.message().to_string(),
                details: None,
            }),
        )
            .into_response(),
    }
}

async fn audit_request(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();
    let user_agent = request
        .headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);
    let subject = request
        .headers()
        .get(AUTH_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| validate_authorization_header(Some(h)).ok())
        .map(|c| c.sub);
    let started_at = Instant::now();

    let response = next.run(request).await;

    if path.starts_with("/api/") {
        let duration_ms = started_at.elapsed().as_millis() as i64;
        let status_code = i64::from(response.status().as_u16());
        let _ = sqlx::query(
            "INSERT INTO api_request_logs (subject, method, path, status_code, duration_ms, user_agent) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(subject)
        .bind(method)
        .bind(path)
        .bind(status_code)
        .bind(duration_ms)
        .bind(user_agent)
        .execute(&state.pool)
        .await;
    }

    response
}

fn build_cors_layer() -> CorsLayer {
    let configured = env::var("ALLOWED_ORIGINS").ok().unwrap_or_default();

    if configured.trim() == "*" {
        return CorsLayer::permissive();
    }

    let origins: Vec<HeaderValue> = configured
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .filter_map(|v| HeaderValue::from_str(v).ok())
        .collect();

    if origins.is_empty() {
        return CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
            .allow_headers(Any);
    }

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
}
