use axum::{Json, extract::{Path, State}, http::StatusCode, response::{IntoResponse, Response}};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthClaims;
use crate::models::{Collaborator, CreateCollaboratorRequest};

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

/// List collaborators for a project. Auth required; clients restricted to own projects.
pub(crate) async fn list_collaborators(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    claims: AuthClaims,
) -> Response {
    if let Err(r) = project_accessible(&state.pool, &project_id, &claims).await {
        return r;
    }

    match sqlx::query_as::<_, Collaborator>(
        "SELECT id, project_id, name, role, avatar_url, created_at \
         FROM collaborators WHERE project_id = ? ORDER BY created_at ASC",
    )
    .bind(&project_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(collaborators) => Json(collaborators).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_LIST_COLLABORATORS_FAILED",
            "failed to list collaborators",
            None,
        )
        .into_response(),
    }
}

/// Add a collaborator to a project. Admin only (enforced at router layer).
pub(crate) async fn create_collaborator(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(payload): Json<CreateCollaboratorRequest>,
) -> Response {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_NAME_REQUIRED",
            "collaborator name is required",
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
    let role = payload.role.as_deref().unwrap_or("contributor").to_string();
    let avatar_url = payload.avatar_url.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);

    let insert = sqlx::query(
        "INSERT INTO collaborators (id, project_id, name, role, avatar_url) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&project_id)
    .bind(&name)
    .bind(&role)
    .bind(&avatar_url)
    .execute(&state.pool)
    .await;

    if insert.is_err() {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_CREATE_COLLABORATOR_FAILED",
            "failed to create collaborator",
            None,
        )
        .into_response();
    }

    match sqlx::query_as::<_, Collaborator>(
        "SELECT id, project_id, name, role, avatar_url, created_at FROM collaborators WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(c) => (StatusCode::CREATED, Json(c)).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_FETCH_CREATED_COLLABORATOR_FAILED",
            "failed to load created collaborator",
            None,
        )
        .into_response(),
    }
}
