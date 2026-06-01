mod ai;
mod config;
mod db;
mod error;
mod state;
mod validation;
mod webhook;

use axum::{
    extract::{State, FromRequestParts, FromRef},
    http::{StatusCode, request::Parts},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use config::Config;
use state::AppState;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AuthenticatedUser(pub db::User);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = error::AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| error::AppError::Authentication("Missing Authorization header".to_string()))?;

        if !auth_header.starts_with("Bearer ") {
            return Err(error::AppError::Authentication("Authorization header must start with 'Bearer '".to_string()));
        }

        let token_str = &auth_header[7..];
        let token = uuid::Uuid::parse_str(token_str)
            .map_err(|_| error::AppError::Authentication("Invalid token format".to_string()))?;

        let user = if let Some(ref pool) = app_state.db_pool {
            db::get_session_user(pool, token)
                .await
                .map_err(|e| error::AppError::Database(e.to_string()))?
        } else {
            let sessions = app_state.sessions_cache.read().await;
            let session = sessions.iter().find(|s| s.token == token && s.expires_at > chrono::Utc::now());
            if let Some(s) = session {
                let users = app_state.users_cache.read().await;
                users.iter().find(|u| u.id == s.user_id).cloned()
            } else {
                None
            }
        };

        match user {
            Some(u) => {
                if u.is_blocked {
                    return Err(error::AppError::Authentication("User account is temporarily blocked".to_string()));
                }
                Ok(AuthenticatedUser(u))
            }
            None => Err(error::AppError::Authentication("Invalid or expired session token".to_string())),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (logging)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cameraman=info,axum=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Cameraman CCTV AI processing backend...");

    // Load configuration
    let config = Config::load();
    tracing::info!("Configuration loaded. Active AI Provider: {}", config.ai_provider);

    // Initialize database pool (gracefully fall back to None if DB is offline or missing)
    let db_pool = if let Some(ref db_url) = config.database_url {
        match db::connect_and_migrate(db_url).await {
            Ok(pool) => Some(pool),
            Err(err) => {
                tracing::error!(
                    "Failed to connect to database at {}: {}. Starting in dry-run/logging-only mode.",
                    db_url,
                    err
                );
                None
            }
        }
    } else {
        tracing::warn!("DATABASE_URL is not set. Starting in dry-run/logging-only mode.");
        None
    };

    // Build the AI provider
    let ai_provider = match ai::build_provider(&config) {
        Ok(provider) => provider,
        Err(err) => {
            tracing::error!("Initialization failed to build AI provider: {}", err);
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    let cameras_cache = std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new()));

    // Seed default camera
    let default_cam = db::Camera {
        id: "default-cam".to_string(),
        name: "Default Camera".to_string(),
        location: Some("Entrance".to_string()),
        enabled_modules: Some(serde_json::json!(["anpr"])),
        created_at: Some(chrono::Utc::now()),
    };

    if let Some(ref pool) = db_pool {
        if let Err(e) = db::save_camera(pool, &default_cam).await {
            tracing::error!("Failed to save default camera to DB: {}", e);
        }
    } else {
        cameras_cache.write().await.push(default_cam);
    }

    let events_cache = std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let ai_configs_cache = std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let users_cache = std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let sessions_cache = std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new()));

    // Seed default admin user
    let default_admin_password_hash = bcrypt::hash("admin123", bcrypt::DEFAULT_COST)
        .expect("Failed to hash default admin password");
    let default_admin = db::User {
        id: uuid::Uuid::new_v4(),
        username: "admin".to_string(),
        password_hash: default_admin_password_hash,
        role: "admin".to_string(),
        is_blocked: false,
        created_at: Some(chrono::Utc::now()),
    };

    if let Some(ref pool) = db_pool {
        match db::get_user_by_username(pool, "admin").await {
            Ok(None) => {
                if let Err(e) = db::save_user(pool, &default_admin).await {
                    tracing::error!("Failed to seed initial admin user in DB: {}", e);
                } else {
                    tracing::info!("Seeded default admin user in database");
                }
            }
            Err(e) => {
                tracing::error!("Failed to check existing users in DB: {}", e);
            }
            _ => {}
        }
    } else {
        users_cache.write().await.push(default_admin);
    }

    // Seed default AI config
    let env_provider = config.ai_provider.clone();
    let env_key = config.gemini_api_key.clone();
    let default_ai_config = db::AiConfig {
        id: uuid::Uuid::new_v4(),
        name: format!("Env Config ({})", env_provider),
        provider_type: env_provider,
        api_key: env_key,
        model_name: Some("default-model".to_string()),
        is_active: true,
        created_at: Some(chrono::Utc::now()),
    };

    if let Some(ref pool) = db_pool {
        match db::get_ai_configs(pool).await {
            Ok(ref configs) if configs.is_empty() => {
                if let Err(e) = db::save_ai_config(pool, &default_ai_config).await {
                    tracing::error!("Failed to seed initial AI config in DB: {}", e);
                } else {
                    tracing::info!("Seeded default AI config from environment in database");
                }
            }
            Err(e) => {
                tracing::error!("Failed to check existing AI configs in DB: {}", e);
            }
            _ => {}
        }
    } else {
        ai_configs_cache.write().await.push(default_ai_config);
    }

    // Shared application state
    let state = AppState {
        config: config.clone(),
        ai_provider,
        db_pool,
        cameras_cache,
        events_cache,
        ai_configs_cache,
        users_cache,
        sessions_cache,
    };

    // Build routing
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/webhooks/camera", post(webhook::handle_webhook))
        
        // Auth API
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/me", get(me_handler))
        
        // Users API
        .route("/api/users", get(get_users_handler).post(create_user_handler))
        .route("/api/users/:id/reset-password", post(reset_password_handler))
        .route("/api/users/:id/toggle-block", post(toggle_block_handler))

        // Business API
        .route("/api/events", get(get_events_handler))
        .route("/api/cameras", get(get_cameras_handler).post(create_camera_handler))
        .route("/api/cameras/:id", delete(delete_camera_handler))
        .route("/api/ai-configs", get(get_ai_configs_handler).post(create_ai_config_handler))
        .route("/api/ai-configs/:id", delete(delete_ai_config_handler))
        .route("/api/ai-configs/:id/activate", post(activate_ai_config_handler))
        
        .nest_service("/uploads", tower_http::services::ServeDir::new(&config.upload_dir))
        .fallback_service(tower_http::services::ServeDir::new("frontend/dist"))
        .with_state(state);

    // Run server
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Failed to parse address '{}:{}': {}", config.host, config.port, e),
            )
        })?;

    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "message": "cameraman service is healthy"
        })),
    )
}

// ==================== AUTH HANDLERS ====================

#[derive(serde::Deserialize)]
struct LoginInput {
    username: String,
    password: String,
}

#[derive(serde::Serialize)]
struct LoginResponse {
    token: uuid::Uuid,
    username: String,
    role: String,
}

async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginInput>,
) -> Result<impl IntoResponse, error::AppError> {
    let user = if let Some(ref pool) = state.db_pool {
        db::get_user_by_username(pool, &payload.username).await?
    } else {
        let users = state.users_cache.read().await;
        users.iter().find(|u| u.username == payload.username).cloned()
    };

    let user = user.ok_or_else(|| error::AppError::Authentication("Invalid username or password".to_string()))?;

    if user.is_blocked {
        return Err(error::AppError::Authentication("User account is temporarily blocked".to_string()));
    }

    // Verify password
    let matches = bcrypt::verify(&payload.password, &user.password_hash)
        .map_err(|e| error::AppError::Authentication(format!("Password verification failed: {}", e)))?;

    if !matches {
        return Err(error::AppError::Authentication("Invalid username or password".to_string()));
    }

    // Create session token
    let token = uuid::Uuid::new_v4();
    let session = db::UserSession {
        token,
        user_id: user.id,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    if let Some(ref pool) = state.db_pool {
        db::save_session(pool, &session).await?;
    } else {
        let mut sessions = state.sessions_cache.write().await;
        sessions.push(session);
    }

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            token,
            username: user.username,
            role: user.role,
        }),
    ))
}

async fn logout_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    State(state): State<AppState>,
    parts: Parts,
) -> Result<impl IntoResponse, error::AppError> {
    let auth_header = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| error::AppError::Authentication("Missing Authorization header".to_string()))?;

    let token_str = &auth_header[7..];
    let token = uuid::Uuid::parse_str(token_str)
        .map_err(|_| error::AppError::Authentication("Invalid token format".to_string()))?;

    if let Some(ref pool) = state.db_pool {
        db::delete_session(pool, token).await?;
    } else {
        let mut sessions = state.sessions_cache.write().await;
        sessions.retain(|s| s.token != token);
    }

    tracing::info!("User {} logged out successfully", user.username);
    Ok(StatusCode::OK)
}

#[derive(serde::Serialize)]
struct MeResponse {
    username: String,
    role: String,
}

async fn me_handler(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, error::AppError> {
    Ok((
        StatusCode::OK,
        Json(MeResponse {
            username: user.username,
            role: user.role,
        }),
    ))
}

// ==================== USER MANAGEMENT HANDLERS ====================

#[derive(serde::Serialize)]
struct UserResponse {
    id: uuid::Uuid,
    username: String,
    role: String,
    is_blocked: bool,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn get_users_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, error::AppError> {
    if user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can list users".to_string()));
    }

    let users = if let Some(ref pool) = state.db_pool {
        db::get_all_users(pool).await?
    } else {
        state.users_cache.read().await.clone()
    };

    let response_users: Vec<UserResponse> = users
        .into_iter()
        .map(|u| UserResponse {
            id: u.id,
            username: u.username,
            role: u.role,
            is_blocked: u.is_blocked,
            created_at: u.created_at,
        })
        .collect();

    Ok((StatusCode::OK, Json(response_users)))
}

#[derive(serde::Deserialize)]
struct CreateUserInput {
    username: String,
    password: Option<String>,
    role: String,
}

async fn create_user_handler(
    AuthenticatedUser(admin_user): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateUserInput>,
) -> Result<impl IntoResponse, error::AppError> {
    if admin_user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can create users".to_string()));
    }

    let raw_password = payload.password.unwrap_or_else(|| "123456".to_string());
    let password_hash = bcrypt::hash(&raw_password, bcrypt::DEFAULT_COST)
        .map_err(|e| error::AppError::Config(format!("Failed to hash password: {}", e)))?;

    let new_user = db::User {
        id: uuid::Uuid::new_v4(),
        username: payload.username.trim().to_string(),
        password_hash,
        role: payload.role.trim().to_lowercase(),
        is_blocked: false,
        created_at: Some(chrono::Utc::now()),
    };

    if new_user.role != "admin" && new_user.role != "user" {
        return Err(error::AppError::InvalidPayload("Role must be either 'admin' or 'user'".to_string()));
    }

    if let Some(ref pool) = state.db_pool {
        if db::get_user_by_username(pool, &new_user.username).await?.is_some() {
            return Err(error::AppError::InvalidPayload("Username already exists".to_string()));
        }
        db::save_user(pool, &new_user).await?;
    } else {
        let mut cache = state.users_cache.write().await;
        if cache.iter().any(|u| u.username == new_user.username) {
            return Err(error::AppError::InvalidPayload("Username already exists".to_string()));
        }
        cache.push(new_user.clone());
    }

    Ok((
        StatusCode::CREATED,
        Json(UserResponse {
            id: new_user.id,
            username: new_user.username,
            role: new_user.role,
            is_blocked: new_user.is_blocked,
            created_at: new_user.created_at,
        }),
    ))
}

#[derive(serde::Deserialize)]
struct ResetPasswordInput {
    new_password: String,
}

async fn reset_password_handler(
    AuthenticatedUser(admin_user): AuthenticatedUser,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<ResetPasswordInput>,
) -> Result<impl IntoResponse, error::AppError> {
    if admin_user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can reset passwords".to_string()));
    }

    let password_hash = bcrypt::hash(&payload.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| error::AppError::Config(format!("Failed to hash password: {}", e)))?;

    if let Some(ref pool) = state.db_pool {
        let mut user = db::get_user_by_id(pool, id)
            .await?
            .ok_or_else(|| error::AppError::InvalidPayload("User not found".to_string()))?;
        user.password_hash = password_hash;
        db::save_user(pool, &user).await?;
    } else {
        let mut cache = state.users_cache.write().await;
        let user = cache
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or_else(|| error::AppError::InvalidPayload("User not found".to_string()))?;
        user.password_hash = password_hash;
    }

    Ok(StatusCode::OK)
}

async fn toggle_block_handler(
    AuthenticatedUser(admin_user): AuthenticatedUser,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, error::AppError> {
    if admin_user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can block users".to_string()));
    }

    if admin_user.id == id {
        return Err(error::AppError::InvalidPayload("You cannot block your own admin account".to_string()));
    }

    let is_blocked;

    if let Some(ref pool) = state.db_pool {
        let mut user = db::get_user_by_id(pool, id)
            .await?
            .ok_or_else(|| error::AppError::InvalidPayload("User not found".to_string()))?;
        user.is_blocked = !user.is_blocked;
        is_blocked = user.is_blocked;
        db::save_user(pool, &user).await?;
        
        // If blocked, immediately delete all active sessions for this user
        if user.is_blocked {
            sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
    } else {
        let mut cache = state.users_cache.write().await;
        let user = cache
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or_else(|| error::AppError::InvalidPayload("User not found".to_string()))?;
        user.is_blocked = !user.is_blocked;
        is_blocked = user.is_blocked;

        // Invalidate session cache
        if user.is_blocked {
            let mut sessions = state.sessions_cache.write().await;
            sessions.retain(|s| s.user_id != id);
        }
    }

    tracing::info!("Admin {} toggled block on user {}. Blocked: {}", admin_user.username, id, is_blocked);
    Ok(StatusCode::OK)
}

// ==================== BUSINESS LOGIC HANDLERS (PROTECTED) ====================

async fn get_events_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, error::AppError> {
    if let Some(ref pool) = state.db_pool {
        let events = db::get_events(pool, 50).await?;
        Ok((StatusCode::OK, Json(events)))
    } else {
        let cache = state.events_cache.read().await;
        let mut events = cache.clone();
        events.reverse();
        Ok((StatusCode::OK, Json(events)))
    }
}

async fn get_cameras_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, error::AppError> {
    if let Some(ref pool) = state.db_pool {
        let cameras = db::get_cameras(pool).await?;
        Ok((StatusCode::OK, Json(cameras)))
    } else {
        let cameras = state.cameras_cache.read().await.clone();
        Ok((StatusCode::OK, Json(cameras)))
    }
}

#[derive(serde::Deserialize)]
struct CreateCameraInput {
    id: Option<String>,
    name: String,
    location: Option<String>,
    enabled_modules: Option<serde_json::Value>,
}

async fn create_camera_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateCameraInput>,
) -> Result<impl IntoResponse, error::AppError> {
    if user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can register cameras".to_string()));
    }

    let camera_id = payload.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let new_camera = db::Camera {
        id: camera_id,
        name: payload.name,
        location: payload.location,
        enabled_modules: payload.enabled_modules.or_else(|| Some(serde_json::json!(["anpr"]))),
        created_at: Some(chrono::Utc::now()),
    };

    if let Some(ref pool) = state.db_pool {
        db::save_camera(pool, &new_camera).await?;
    } else {
        let mut cache = state.cameras_cache.write().await;
        if let Some(existing) = cache.iter_mut().find(|c| c.id == new_camera.id) {
            existing.name = new_camera.name.clone();
            existing.location = new_camera.location.clone();
            existing.enabled_modules = new_camera.enabled_modules.clone();
        } else {
            cache.push(new_camera.clone());
        }
    }

    Ok((StatusCode::CREATED, Json(new_camera)))
}

async fn delete_camera_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, error::AppError> {
    if user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can unregister cameras".to_string()));
    }

    if let Some(ref pool) = state.db_pool {
        db::delete_camera(pool, &id).await?;
    } else {
        let mut cache = state.cameras_cache.write().await;
        cache.retain(|c| c.id != id);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_ai_configs_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, error::AppError> {
    let mut configs = if let Some(ref pool) = state.db_pool {
        db::get_ai_configs(pool).await?
    } else {
        state.ai_configs_cache.read().await.clone()
    };

    for config in &mut configs {
        if let Some(ref key) = config.api_key {
            if key.len() > 6 {
                config.api_key = Some(format!("{}...[masked]", &key[..6]));
            } else {
                config.api_key = Some("...[masked]".to_string());
            }
        }
    }

    Ok((StatusCode::OK, Json(configs)))
}

#[derive(serde::Deserialize)]
struct CreateAiConfigInput {
    id: Option<uuid::Uuid>,
    name: String,
    provider_type: String,
    api_key: Option<String>,
    model_name: Option<String>,
    is_active: bool,
}

async fn create_ai_config_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateAiConfigInput>,
) -> Result<impl IntoResponse, error::AppError> {
    if user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can manage AI configs".to_string()));
    }

    let config_id = payload.id.unwrap_or_else(uuid::Uuid::new_v4);
    let mut resolved_api_key = payload.api_key.clone();
    
    let is_masked = |key: &Option<String>| -> bool {
        if let Some(ref k) = key {
            k.contains("[masked]") || k.contains('*')
        } else {
            false
        }
    };

    if is_masked(&payload.api_key) {
        if let Some(ref pool) = state.db_pool {
            let configs = db::get_ai_configs(pool).await?;
            if let Some(existing) = configs.iter().find(|c| c.id == config_id) {
                resolved_api_key = existing.api_key.clone();
            }
        } else {
            let cache = state.ai_configs_cache.read().await;
            if let Some(existing) = cache.iter().find(|c| c.id == config_id) {
                resolved_api_key = existing.api_key.clone();
            }
        }
    }

    let new_config = db::AiConfig {
        id: config_id,
        name: payload.name,
        provider_type: payload.provider_type,
        api_key: resolved_api_key,
        model_name: payload.model_name,
        is_active: payload.is_active,
        created_at: Some(chrono::Utc::now()),
    };

    if let Some(ref pool) = state.db_pool {
        db::save_ai_config(pool, &new_config).await?;
        if new_config.is_active {
            db::set_active_ai_config(pool, new_config.id).await?;
        }
    } else {
        let mut cache = state.ai_configs_cache.write().await;
        if new_config.is_active {
            for c in cache.iter_mut() {
                c.is_active = false;
            }
        }
        
        if let Some(existing) = cache.iter_mut().find(|c| c.id == new_config.id) {
            existing.name = new_config.name.clone();
            existing.provider_type = new_config.provider_type.clone();
            existing.api_key = new_config.api_key.clone();
            existing.model_name = new_config.model_name.clone();
            existing.is_active = new_config.is_active;
        } else {
            cache.push(new_config.clone());
        }
    }

    let mut response_config = new_config;
    if let Some(ref key) = response_config.api_key {
        if key.len() > 6 {
            response_config.api_key = Some(format!("{}...[masked]", &key[..6]));
        } else {
            response_config.api_key = Some("...[masked]".to_string());
        }
    }

    Ok((StatusCode::CREATED, Json(response_config)))
}

async fn delete_ai_config_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, error::AppError> {
    if user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can manage AI configs".to_string()));
    }

    if let Some(ref pool) = state.db_pool {
        db::delete_ai_config(pool, id).await?;
    } else {
        let mut cache = state.ai_configs_cache.write().await;
        cache.retain(|c| c.id != id);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn activate_ai_config_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, error::AppError> {
    if user.role != "admin" {
        return Err(error::AppError::Authorization("Only administrators can manage AI configs".to_string()));
    }

    if let Some(ref pool) = state.db_pool {
        db::set_active_ai_config(pool, id).await?;
    } else {
        let mut cache = state.ai_configs_cache.write().await;
        for c in cache.iter_mut() {
            c.is_active = c.id == id;
        }
    }

    Ok(StatusCode::OK)
}
