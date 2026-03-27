use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::{
    Json,
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::models::ApiError;

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

static MAX_REQUESTS: LazyLock<usize> = LazyLock::new(|| env_or("RATE_LIMIT_MAX_REQUESTS", 60));
static WINDOW_SECS: LazyLock<u64> = LazyLock::new(|| env_or("RATE_LIMIT_WINDOW_SECONDS", 60));

struct Bucket {
    timestamps: Vec<Instant>,
}

static BUCKETS: LazyLock<Mutex<HashMap<String, Bucket>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn check_bucket(map: &mut HashMap<String, Bucket>, key: &str, max: usize, window: Duration) -> bool {
    let now = Instant::now();
    let cutoff = now - window;
    let bucket = map.entry(key.to_owned()).or_insert_with(|| Bucket { timestamps: Vec::new() });
    bucket.timestamps.retain(|t| *t > cutoff);
    if bucket.timestamps.len() >= max {
        return false;
    }
    bucket.timestamps.push(now);
    true
}

fn is_allowed(key: &str) -> bool {
    let mut map = BUCKETS.lock().unwrap_or_else(|e| e.into_inner());
    check_bucket(&mut map, key, *MAX_REQUESTS, Duration::from_secs(*WINDOW_SECS))
}

pub fn client_ip(request: &Request) -> String {
    if let Some(forwarded) = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_owned())
    {
        return forwarded;
    }
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

pub async fn rate_limit_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if path == "/" || path == "/health" || path == "/ready" {
        return next.run(request).await;
    }
    let ip = client_ip(&request);
    if !is_allowed(&ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiError {
                code: "RATE_LIMIT_EXCEEDED".to_string(),
                message: "too many requests — try again later".to_string(),
                details: None,
            }),
        )
            .into_response();
    }
    next.run(request).await
}
