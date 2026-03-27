use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthClaims;
use crate::models::{CreateMessageRequest, Message};

use super::error_response;

async fn assert_project_access(
    pool: &sqlx::SqlitePool,
    project_id: &str,
    claims: &AuthClaims,
) -> Result<String, Response> {
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
        Ok("admin".to_string())
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
        Ok("client".to_string())
    }
}

pub(crate) async fn list_messages(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
    claims: AuthClaims,
) -> Response {
    if let Err(e) = assert_project_access(&state.pool, &project_id, &claims).await {
        return e;
    }

    match sqlx::query_as::<_, Message>(
        "SELECT id, project_id, author_id, author_role, body, created_at \
         FROM messages WHERE project_id = ? ORDER BY created_at ASC",
    )
    .bind(&project_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(messages) => Json(messages).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_LIST_MESSAGES_FAILED",
            "failed to list messages",
            None,
        ),
    }
}

pub(crate) async fn create_message(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
    claims: AuthClaims,
    Json(payload): Json<CreateMessageRequest>,
) -> Response {
    let author_role = match assert_project_access(&state.pool, &project_id, &claims).await {
        Ok(role) => role,
        Err(e) => return e,
    };

    let body = payload.body.trim().to_string();
    if body.is_empty() {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_BODY_REQUIRED",
            "message body is required",
            None,
        );
    }

    let id = Uuid::new_v4().to_string();

    if sqlx::query(
        "INSERT INTO messages (id, project_id, author_id, author_role, body) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&project_id)
    .bind(&claims.sub)
    .bind(&author_role)
    .bind(&body)
    .execute(&state.pool)
    .await
    .is_err()
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_CREATE_MESSAGE_FAILED",
            "failed to create message",
            None,
        );
    }

    match sqlx::query_as::<_, Message>(
        "SELECT id, project_id, author_id, author_role, body, created_at FROM messages WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(msg) => (StatusCode::CREATED, Json(msg)).into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_FETCH_CREATED_MESSAGE_FAILED",
            "failed to load created message",
            None,
        ),
    }
}
