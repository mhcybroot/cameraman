use crate::ai::AiProvider;
use crate::config::Config;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::db::{Camera, ProcessedEvent, AiConfig};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub ai_provider: Arc<dyn AiProvider>,
    pub db_pool: Option<sqlx::PgPool>,
    pub cameras_cache: Arc<RwLock<Vec<Camera>>>,
    pub events_cache: Arc<RwLock<Vec<ProcessedEvent>>>,
    pub ai_configs_cache: Arc<RwLock<Vec<AiConfig>>>,
    pub users_cache: Arc<RwLock<Vec<crate::db::User>>>,
    pub sessions_cache: Arc<RwLock<Vec<crate::db::UserSession>>>,
}
