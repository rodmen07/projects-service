mod deliverables;
mod health;
mod messages;
mod milestones;
mod projects;

pub(crate) use deliverables::{create_deliverable, list_deliverables};
pub(crate) use health::{health, ready};
pub(crate) use messages::{create_message, list_messages};
pub(crate) use milestones::{create_milestone, list_milestones};
pub(crate) use projects::{create_project, list_projects};

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
