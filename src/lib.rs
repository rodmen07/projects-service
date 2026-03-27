#[path = "lib/app_state.rs"]
mod app_state;
#[path = "lib/auth.rs"]
mod auth;
#[path = "lib/handlers/mod.rs"]
mod handlers;
#[path = "lib/models.rs"]
mod models;
#[path = "lib/rate_limit.rs"]
mod rate_limit;
#[path = "lib/router.rs"]
mod router;

pub use app_state::AppState;
pub use router::build_router;
