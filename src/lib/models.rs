use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

// --- Project ---

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Project {
    pub id: String,
    pub account_id: String,
    pub client_user_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub account_id: String,
    pub client_user_id: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
}

// --- Milestone ---

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Milestone {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub due_date: Option<String>,
    pub status: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMilestoneRequest {
    pub name: String,
    pub description: Option<String>,
    pub due_date: Option<String>,
    pub status: Option<String>,
    pub sort_order: Option<i64>,
}

// --- Deliverable ---

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Deliverable {
    pub id: String,
    pub milestone_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeliverableRequest {
    pub name: String,
    pub description: Option<String>,
    pub status: Option<String>,
}

// --- Message ---

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Message {
    pub id: String,
    pub project_id: String,
    pub author_id: String,
    pub author_role: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub body: String,
}

// --- Common ---

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}
