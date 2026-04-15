use axum::{Json, extract::{Path, State}, http::StatusCode, response::{IntoResponse, Response}};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthClaims;
use crate::models::{CreateProjectRequest, PatchProjectRequest, Project};

use super::error_response;

const PROJECT_COLS: &str =
    "id, account_id, client_user_id, name, description, status, budget, \
     start_date, target_end_date, created_at, updated_at";

/// List projects. Admins see all; clients see only their own.
pub(crate) async fn list_projects(
    State(state): State<AppState>,
    claims: AuthClaims,
) -> Response {
    let result = if claims.is_admin() {
        sqlx::query_as::<_, Project>(
            &format!("SELECT {PROJECT_COLS} FROM projects ORDER BY created_at DESC"),
        )
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as::<_, Project>(
            &format!("SELECT {PROJECT_COLS} FROM projects WHERE client_user_id = ? ORDER BY created_at DESC"),
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

/// Get a single project by ID. Admins can fetch any; clients only their own.
pub(crate) async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
    claims: AuthClaims,
) -> Response {
    let result = sqlx::query_as::<_, Project>(
        &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?"),
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(project)) => {
            if !claims.is_admin() && project.client_user_id.as_deref() != Some(&claims.sub) {
                return error_response(StatusCode::FORBIDDEN, "ACCESS_DENIED", "access denied", None)
                    .into_response();
            }
            Json(project).into_response()
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "project not found", None)
            .into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_GET_PROJECT_FAILED",
            "failed to fetch project",
            None,
        )
        .into_response(),
    }
}

/// Patch a project. Admin only (enforced at router layer).
pub(crate) async fn patch_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<PatchProjectRequest>,
) -> Response {
    // Verify project exists first
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.pool)
        .await;

    match exists {
        Ok(0) => {
            return error_response(StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "project not found", None)
                .into_response();
        }
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_PATCH_PROJECT_FAILED",
                "failed to fetch project",
                None,
            )
            .into_response();
        }
        _ => {}
    }

    // Build SET clause dynamically from provided fields
    let mut sets: Vec<&str> = Vec::new();
    if payload.name.is_some() { sets.push("name = ?"); }
    if payload.description.is_some() { sets.push("description = ?"); }
    if payload.status.is_some() { sets.push("status = ?"); }
    if payload.budget.is_some() { sets.push("budget = ?"); }
    if payload.client_user_id.is_some() { sets.push("client_user_id = ?"); }
    if payload.start_date.is_some() { sets.push("start_date = ?"); }
    if payload.target_end_date.is_some() { sets.push("target_end_date = ?"); }

    if sets.is_empty() {
        // No fields — just return the current project
        return match sqlx::query_as::<_, Project>(
            &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?"),
        )
        .bind(&id)
        .fetch_one(&state.pool)
        .await
        {
            Ok(p) => Json(p).into_response(),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_PATCH_PROJECT_FAILED",
                "failed to load project",
                None,
            )
            .into_response(),
        };
    }

    sets.push("updated_at = datetime('now')");
    let set_clause = sets.join(", ");
    let sql = format!("UPDATE projects SET {set_clause} WHERE id = ?");

    let mut q = sqlx::query(&sql);
    if let Some(v) = payload.name.as_deref() { q = q.bind(v.trim()); }
    if let Some(v) = payload.description.as_deref() { q = q.bind(v.trim()); }
    if let Some(v) = payload.status.as_deref() { q = q.bind(v.trim()); }
    if let Some(v) = payload.budget { q = q.bind(v); }
    if let Some(v) = payload.client_user_id.as_deref() { q = q.bind(v.trim()); }
    if let Some(v) = payload.start_date.as_deref() { q = q.bind(v.trim()); }
    if let Some(v) = payload.target_end_date.as_deref() { q = q.bind(v.trim()); }
    q = q.bind(&id);

    if q.execute(&state.pool).await.is_err() {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_PATCH_PROJECT_FAILED",
            "failed to update project",
            None,
        )
        .into_response();
    }

    match sqlx::query_as::<_, Project>(
        &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?"),
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(project) => Json(project).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_PATCH_PROJECT_FAILED",
            "failed to load updated project",
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
        "INSERT INTO projects \
         (id, account_id, client_user_id, name, description, status, budget, start_date, target_end_date) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&account_id)
    .bind(&client_user_id)
    .bind(&name)
    .bind(&description)
    .bind(&status)
    .bind(payload.budget)
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
        &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?"),
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
