mod collaborators;
mod deliverables;
mod health;
mod messages;
mod milestones;
mod progress_updates;
mod projects;

pub(crate) use collaborators::{create_collaborator, list_collaborators};
pub(crate) use deliverables::{create_deliverable, list_deliverables};
pub(crate) use health::{health, ready};
pub(crate) use messages::{create_message, list_messages};
pub(crate) use milestones::{create_milestone, list_milestones};
pub(crate) use progress_updates::{create_progress_update, list_progress_updates};
pub(crate) use projects::{create_project, get_project, list_projects, patch_project};

use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use serde_json::Value;

use crate::models::ApiError;

pub(crate) fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    details: Option<Value>,
) -> Response {
    (
        status,
        Json(ApiError {
            code: code.to_string(),
            message: message.to_string(),
            details,
        }),
    )
        .into_response()
}
