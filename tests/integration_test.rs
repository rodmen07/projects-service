//! Integration tests: crew-assist client portal — projects-service API
//!
//! Scenario: admin creates the crew-assist project for client Ryan, adds collaborators
//! and a progress update, then Ryan (as a client) views his project and sends a message.
//!
//! Auth is validated with real JWTs signed by the default dev secret so the full
//! auth middleware stack runs. Run with: `cargo test --test integration_test`

use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value, json};
use tower::ServiceExt;

use projects_service::{AppState, build_router};

// ── JWT helpers ──────────────────────────────────────────────────────────────

const DEV_SECRET: &[u8] = b"dev-insecure-secret-change-me";
const DEV_ISSUER: &str = "auth-service";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn make_jwt(sub: &str, roles: &[&str]) -> String {
    let claims = json!({
        "sub": sub,
        "roles": roles,
        "iat": now_secs(),
        "exp": now_secs() + 3600,
        "iss": DEV_ISSUER,
        "jti": uuid::Uuid::new_v4().to_string(),
    });
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(DEV_SECRET),
    )
    .expect("JWT encoding failed")
}

// ── Request helpers ──────────────────────────────────────────────────────────

fn json_post(uri: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn json_patch(uri: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method("PATCH")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn get_req(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

/// Creates an isolated AppState with a temporary SQLite database.
async fn make_state() -> AppState {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.keep().join("test.db");
    // SAFETY: tests run single-threaded per tokio runtime; env var is local to test.
    unsafe { std::env::set_var("AUTH_ENFORCED", "true") };
    AppState::from_database_url(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await
        .expect("AppState init")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_health_endpoint() {
    let state = make_state().await;
    let app = build_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_crewassist_full_project_lifecycle() {
    let state = make_state().await;
    let app = build_router(state);

    // Stable subject IDs for the test
    let admin_id = "admin-e2e-00000000";
    let ryan_id = "ryan-crewassist-00000000";

    let admin_jwt = make_jwt(admin_id, &["admin"]);
    let ryan_jwt = make_jwt(ryan_id, &["client"]);

    // ── 1. Unauthenticated list → 401 ────────────────────────────────────
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // ── 2. Admin creates the crew-assist project ──────────────────────────
    let create_body = json!({
        "account_id": admin_id,
        "client_user_id": ryan_id,
        "name": "Crew Assist — UA Flight Crew PWA",
        "description": "Progressive web app for United Airlines flight crew. Schedule parsing, layover matching, contract Q&A, and crew coordination.",
        "status": "active",
        "budget": 4500.0,
        "start_date": "2025-11-01",
        "target_end_date": "2026-06-30"
    });
    let resp = app
        .clone()
        .oneshot(json_post("/api/v1/projects", &admin_jwt, create_body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "create project failed");
    let project = body_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();
    assert_eq!(project["name"], "Crew Assist — UA Flight Crew PWA");
    assert_eq!(project["budget"], 4500.0);
    assert_eq!(project["status"], "active");

    // ── 3. Client (Ryan) can only see creates returns his project ─────────
    let resp = app
        .clone()
        .oneshot(get_req("/api/v1/projects", &ryan_jwt))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let projects: Value = body_json(resp).await;
    let list = projects.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], project_id);

    // ── 4. Admin sees all projects ────────────────────────────────────────
    let resp = app
        .clone()
        .oneshot(get_req("/api/v1/projects", &admin_jwt))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let all = body_json(resp).await;
    assert!(!all.as_array().unwrap().is_empty());

    // ── 5. Get single project ─────────────────────────────────────────────
    let resp = app
        .clone()
        .oneshot(get_req(&format!("/api/v1/projects/{project_id}"), &ryan_jwt))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let single = body_json(resp).await;
    assert_eq!(single["id"], project_id);
    assert_eq!(single["client_user_id"], ryan_id);

    // ── 6. Admin patches project — milestone 1 delivered ─────────────────
    let patch_body = json!({ "status": "active", "budget": 5200.0 });
    let resp = app
        .clone()
        .oneshot(json_patch(
            &format!("/api/v1/projects/{project_id}"),
            &admin_jwt,
            patch_body,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let patched = body_json(resp).await;
    assert_eq!(patched["budget"], 5200.0);

    // ── 7. Non-owner client cannot access project ─────────────────────────
    let other_jwt = make_jwt("other-client-99999", &["client"]);
    let resp = app
        .clone()
        .oneshot(get_req(&format!("/api/v1/projects/{project_id}"), &other_jwt))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // ── 8. Admin adds a collaborator ──────────────────────────────────────
    let collab_body = json!({
        "name": "Ryan Chyler Thomas",
        "role": "Product Owner",
        "avatar_url": null
    });
    let resp = app
        .clone()
        .oneshot(json_post(
            &format!("/api/v1/projects/{project_id}/collaborators"),
            &admin_jwt,
            collab_body,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "add collaborator failed");
    let collab = body_json(resp).await;
    assert_eq!(collab["name"], "Ryan Chyler Thomas");
    assert_eq!(collab["role"], "Product Owner");

    // ── 9. Ryan lists collaborators ───────────────────────────────────────
    let resp = app
        .clone()
        .oneshot(get_req(
            &format!("/api/v1/projects/{project_id}/collaborators"),
            &ryan_jwt,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let collabs = body_json(resp).await;
    assert_eq!(collabs.as_array().unwrap().len(), 1);

    // ── 10. Admin posts a progress update ─────────────────────────────────
    let update_body = json!({
        "content": "Completed schedule parsing MVP — pairing/schedule screenshots now extract trips reliably. Contract Q&A shipped to production. Layover matching algorithm complete."
    });
    let resp = app
        .clone()
        .oneshot(json_post(
            &format!("/api/v1/projects/{project_id}/progress-updates"),
            &admin_jwt,
            update_body,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "add progress update failed");
    let update = body_json(resp).await;
    assert!(update["content"]
        .as_str()
        .unwrap()
        .contains("Completed schedule parsing"));

    // ── 11. Ryan reads progress updates ───────────────────────────────────
    let resp = app
        .clone()
        .oneshot(get_req(
            &format!("/api/v1/projects/{project_id}/progress-updates"),
            &ryan_jwt,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let updates = body_json(resp).await;
    assert_eq!(updates.as_array().unwrap().len(), 1);

    // ── 12. Ryan sends a message ──────────────────────────────────────────
    let msg_body = json!({ "body": "Hey! When do you think push notifications will be fully live? The crew is asking." });
    let resp = app
        .clone()
        .oneshot(json_post(
            &format!("/api/v1/projects/{project_id}/messages"),
            &ryan_jwt,
            msg_body,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "send message failed");
    let msg = body_json(resp).await;
    assert_eq!(msg["author_id"], ryan_id);
    assert!(msg["body"]
        .as_str()
        .unwrap()
        .contains("push notifications"));

    // ── 13. Message appears in thread ─────────────────────────────────────
    let resp = app
        .clone()
        .oneshot(get_req(
            &format!("/api/v1/projects/{project_id}/messages"),
            &ryan_jwt,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let thread = body_json(resp).await;
    assert!(!thread.as_array().unwrap().is_empty());

    // ── 14. Admin cannot patch non-existent project ───────────────────────
    let resp = app
        .clone()
        .oneshot(json_patch(
            "/api/v1/projects/does-not-exist",
            &admin_jwt,
            json!({ "status": "completed" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // ── 15. Client cannot create a project (admin-only) ───────────────────
    let resp = app
        .clone()
        .oneshot(json_post(
            "/api/v1/projects",
            &ryan_jwt,
            json!({ "account_id": ryan_id, "client_user_id": ryan_id, "name": "Sneaky project" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
