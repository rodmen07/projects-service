use axum::{Json, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthClaims;
use crate::models::{CreateProjectRequest, Project};

use super::error_response;

/// List projects. Admins see all; clients see only their own.
pub(crate) async fn list_projects(
    State(state): State<AppState>,
    claims: AuthClaims,
) -> Response {
    let result = if claims.is_admin() {
        sqlx::query_as::<_, Project>(
            "SELECT id, account_id, client_user_id, name, description, status, \
             start_date, target_end_date, created_at, updated_at \
             FROM projects ORDER BY created_at DESC",
        )
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as::<_, Project>(
            "SELECT id, account_id, client_user_id, name, description, status, \
             start_date, target_end_date, created_at, updated_at \
             FROM projects WHERE client_user_id = ? ORDER BY created_at DESC",
        )
        .bind(&claims.sub)
        .fetch_all(&state.pool)
        .await
    };

    match result {
        Ok(projects) => Json(projects).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_LIST_PROJECTS_FAILED",
            "failed to list projects",
            None,
        )
        .into_response(),
    }
}

/// Create a new project. Admin only (enforced at router layer).
pub(crate) async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<CreateProjectRequest>,
) -> Response {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_NAME_REQUIRED",
            "project name is required",
            None,
        )
        .into_response();
    }

    let account_id = payload.account_id.trim().to_string();
    if account_id.is_empty() {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ACCOUNT_ID_REQUIRED",
            "account_id is required",
            None,
        )
        .into_response();
    }

    let id = Uuid::new_v4().to_string();
    let status = payload.status.as_deref().unwrap_or("planning").to_string();
    let description = payload.description.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);
    let client_user_id = payload.client_user_id.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);
    let start_date = payload.start_date.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);
    let target_end_date = payload.target_end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);

    let insert = sqlx::query(
        "INSERT INTO projects (id, account_id, client_user_id, name, description, status, start_date, target_end_date) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&account_id)
    .bind(&client_user_id)
    .bind(&name)
    .bind(&description)
    .bind(&status)
    .bind(&start_date)
    .bind(&target_end_date)
    .execute(&state.pool)
    .await;

    if insert.is_err() {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_CREATE_PROJECT_FAILED",
            "failed to create project",
            None,
        )
        .into_response();
    }

    match sqlx::query_as::<_, Project>(
        "SELECT id, account_id, client_user_id, name, description, status, \
         start_date, target_end_date, created_at, updated_at FROM projects WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(project) => (StatusCode::CREATED, Json(project)).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_FETCH_CREATED_PROJECT_FAILED",
            "failed to load created project",
            None,
        )
        .into_response(),
    }
}
