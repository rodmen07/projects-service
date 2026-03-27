use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthClaims;
use crate::models::{CreateDeliverableRequest, Deliverable};

use super::error_response;

async fn assert_milestone_access(
    pool: &sqlx::SqlitePool,
    milestone_id: &str,
    claims: &AuthClaims,
) -> Result<(), Response> {
    if claims.is_admin() {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM milestones WHERE id = ?)")
                .bind(milestone_id)
                .fetch_one(pool)
                .await
                .unwrap_or(false);
        if !exists {
            return Err(error_response(StatusCode::NOT_FOUND, "MILESTONE_NOT_FOUND", "milestone not found", None));
        }
    } else {
        let accessible: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
               SELECT 1 FROM milestones m \
               JOIN projects p ON p.id = m.project_id \
               WHERE m.id = ? AND p.client_user_id = ? \
             )",
        )
        .bind(milestone_id)
        .bind(&claims.sub)
        .fetch_one(pool)
        .await
        .unwrap_or(false);
        if !accessible {
            return Err(error_response(StatusCode::NOT_FOUND, "MILESTONE_NOT_FOUND", "milestone not found", None));
        }
    }
    Ok(())
}

pub(crate) async fn list_deliverables(
    Path(milestone_id): Path<String>,
    State(state): State<AppState>,
    claims: AuthClaims,
) -> Response {
    if let Err(e) = assert_milestone_access(&state.pool, &milestone_id, &claims).await {
        return e;
    }

    match sqlx::query_as::<_, Deliverable>(
        "SELECT id, milestone_id, name, description, status \
         FROM deliverables WHERE milestone_id = ? ORDER BY rowid ASC",
    )
    .bind(&milestone_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(deliverables) => Json(deliverables).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_LIST_DELIVERABLES_FAILED",
            "failed to list deliverables",
            None,
        ),
    }
}

pub(crate) async fn create_deliverable(
    Path(milestone_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<CreateDeliverableRequest>,
) -> Response {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM milestones WHERE id = ?)")
            .bind(&milestone_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(false);
    if !exists {
        return error_response(StatusCode::NOT_FOUND, "MILESTONE_NOT_FOUND", "milestone not found", None);
    }

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_NAME_REQUIRED",
            "deliverable name is required",
            None,
        );
    }

    let id = Uuid::new_v4().to_string();
    let status = payload.status.as_deref().unwrap_or("pending").to_string();
    let description = payload.description.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned);

    if sqlx::query(
        "INSERT INTO deliverables (id, milestone_id, name, description, status) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&milestone_id)
    .bind(&name)
    .bind(&description)
    .bind(&status)
    .execute(&state.pool)
    .await
    .is_err()
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_CREATE_DELIVERABLE_FAILED",
            "failed to create deliverable",
            None,
        );
    }

    match sqlx::query_as::<_, Deliverable>(
        "SELECT id, milestone_id, name, description, status FROM deliverables WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(d) => (StatusCode::CREATED, Json(d)).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_FETCH_CREATED_DELIVERABLE_FAILED",
            "failed to load created deliverable",
            None,
        ),
    }
}
