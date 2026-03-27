use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthClaims;
use crate::models::{CreateMilestoneRequest, Milestone};

use super::error_response;

async fn assert_project_access(
    pool: &sqlx::SqlitePool,
    project_id: &str,
    claims: &AuthClaims,
) -> Result<(), Response> {
    if claims.is_admin() {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?)")
                .bind(project_id)
                .fetch_one(pool)
                .await
                .unwrap_or(false);
        if !exists {
            return Err(error_response(StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "project not found", None));
        }
    } else {
        let accessible: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ? AND client_user_id = ?)",
        )
        .bind(project_id)
        .bind(&claims.sub)
        .fetch_one(pool)
        .await
        .unwrap_or(false);
        if !accessible {
            return Err(error_response(StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "project not found", None));
        }
    }
    Ok(())
}

pub(crate) async fn list_milestones(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
    claims: AuthClaims,
) -> Response {
    if let Err(e) = assert_project_access(&state.pool, &project_id, &claims).await {
        return e;
    }

    match sqlx::query_as::<_, Milestone>(
        "SELECT id, project_id, name, description, due_date, status, sort_order, created_at, updated_at \
         FROM milestones WHERE project_id = ? ORDER BY sort_order ASC, created_at ASC",
    )
    .bind(&project_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(milestones) => Json(milestones).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_LIST_MILESTONES_FAILED",
            "failed to list milestones",
            None,
        ),
    }
}

pub(crate) async fn create_milestone(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<CreateMilestoneRequest>,
) -> Response {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?)")
            .bind(&project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(false);
    if !exists {
        return error_response(StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "project not found", None);
    }

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_NAME_REQUIRED",
            "milestone name is required",
            None,
        );
    }

    let id = Uuid::new_v4().to_string();
    let status = payload.status.as_deref().unwrap_or("pending").to_string();
    let sort_order = payload.sort_order.unwrap_or(0);
    let description = payload.description.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);
    let due_date = payload.due_date.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);

    if sqlx::query(
        "INSERT INTO milestones (id, project_id, name, description, due_date, status, sort_order) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&project_id)
    .bind(&name)
    .bind(&description)
    .bind(&due_date)
    .bind(&status)
    .bind(sort_order)
    .execute(&state.pool)
    .await
    .is_err()
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_CREATE_MILESTONE_FAILED",
            "failed to create milestone",
            None,
        );
    }

    match sqlx::query_as::<_, Milestone>(
        "SELECT id, project_id, name, description, due_date, status, sort_order, created_at, updated_at \
         FROM milestones WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(milestone) => (StatusCode::CREATED, Json(milestone)).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_FETCH_CREATED_MILESTONE_FAILED",
            "failed to load created milestone",
            None,
        ),
    }
}
