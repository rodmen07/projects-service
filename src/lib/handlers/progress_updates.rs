use axum::{Json, extract::{Path, State}, http::StatusCode, response::{IntoResponse, Response}};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthClaims;
use crate::models::{CreateProgressUpdateRequest, ProgressUpdate};

use super::error_response;

async fn project_accessible(pool: &sqlx::SqlitePool, project_id: &str, claims: &AuthClaims) -> Result<(), Response> {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT id, client_user_id FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some((_, client_user_id))) => {
            if !claims.is_admin() && client_user_id.as_deref() != Some(&claims.sub) {
                Err(error_response(StatusCode::FORBIDDEN, "ACCESS_DENIED", "access denied", None).into_response())
            } else {
                Ok(())
            }
        }
        Ok(None) => Err(
            error_response(StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "project not found", None).into_response()
        ),
        Err(_) => Err(
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", "database error", None).into_response()
        ),
    }
}

/// List progress updates for a project. Auth required; clients restricted to own projects.
pub(crate) async fn list_progress_updates(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    claims: AuthClaims,
) -> Response {
    if let Err(r) = project_accessible(&state.pool, &project_id, &claims).await {
        return r;
    }

    match sqlx::query_as::<_, ProgressUpdate>(
        "SELECT id, project_id, content, created_at \
         FROM progress_updates WHERE project_id = ? ORDER BY created_at DESC",
    )
    .bind(&project_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(updates) => Json(updates).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_LIST_PROGRESS_UPDATES_FAILED",
            "failed to list progress updates",
            None,
        )
        .into_response(),
    }
}

/// Post a progress update on a project. Admin only (enforced at router layer).
pub(crate) async fn create_progress_update(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(payload): Json<CreateProgressUpdateRequest>,
) -> Response {
    let content = payload.content.trim().to_string();
    if content.is_empty() {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_CONTENT_REQUIRED",
            "content is required",
            None,
        )
        .into_response();
    }

    // Verify project exists
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
        .bind(&project_id)
        .fetch_one(&state.pool)
        .await;
    if matches!(exists, Ok(0) | Err(_)) {
        return error_response(StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "project not found", None)
            .into_response();
    }

    let id = Uuid::new_v4().to_string();

    let insert = sqlx::query(
        "INSERT INTO progress_updates (id, project_id, content) VALUES (?, ?, ?)",
    )
    .bind(&id)
    .bind(&project_id)
    .bind(&content)
    .execute(&state.pool)
    .await;

    if insert.is_err() {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_CREATE_PROGRESS_UPDATE_FAILED",
            "failed to create progress update",
            None,
        )
        .into_response();
    }

    match sqlx::query_as::<_, ProgressUpdate>(
        "SELECT id, project_id, content, created_at FROM progress_updates WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(u) => (StatusCode::CREATED, Json(u)).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_FETCH_CREATED_PROGRESS_UPDATE_FAILED",
            "failed to load created progress update",
            None,
        )
        .into_response(),
    }
}
