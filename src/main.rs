mod ai;
mod config;
mod db;
mod error;
mod state;
mod validation;
mod webhook;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use config::Config;
use state::AppState;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    };

    // Build routing
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/webhooks/camera", post(webhook::handle_webhook))
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

async fn get_events_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, error::AppError> {
    if let Some(ref pool) = state.db_pool {
        let events = db::get_events(pool, 50).await?;
        Ok((StatusCode::OK, Json(events)))
    } else {
        let cache = state.events_cache.read().await;
        let mut events = cache.clone();
        events.reverse(); // Newest first
        Ok((StatusCode::OK, Json(events)))
    }
}

async fn get_cameras_handler(
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
}

async fn create_camera_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateCameraInput>,
) -> Result<impl IntoResponse, error::AppError> {
    let camera_id = payload.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let new_camera = db::Camera {
        id: camera_id,
        name: payload.name,
        location: payload.location,
        created_at: Some(chrono::Utc::now()),
    };

    if let Some(ref pool) = state.db_pool {
        db::save_camera(pool, &new_camera).await?;
    } else {
        let mut cache = state.cameras_cache.write().await;
        if let Some(existing) = cache.iter_mut().find(|c| c.id == new_camera.id) {
            existing.name = new_camera.name.clone();
            existing.location = new_camera.location.clone();
        } else {
            cache.push(new_camera.clone());
        }
    }

    Ok((StatusCode::CREATED, Json(new_camera)))
}

async fn delete_camera_handler(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, error::AppError> {
    if let Some(ref pool) = state.db_pool {
        db::delete_camera(pool, &id).await?;
    } else {
        let mut cache = state.cameras_cache.write().await;
        cache.retain(|c| c.id != id);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_ai_configs_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, error::AppError> {
    let mut configs = if let Some(ref pool) = state.db_pool {
        db::get_ai_configs(pool).await?
    } else {
        state.ai_configs_cache.read().await.clone()
    };

    // Mask the API keys for security
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
    State(state): State<AppState>,
    Json(payload): Json<CreateAiConfigInput>,
) -> Result<impl IntoResponse, error::AppError> {
    let config_id = payload.id.unwrap_or_else(uuid::Uuid::new_v4);
    
    // Resolve api_key: if it is masked, we must retrieve the existing key to avoid overwriting it
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
            // Find existing config in DB
            let configs = db::get_ai_configs(pool).await?;
            if let Some(existing) = configs.iter().find(|c| c.id == config_id) {
                resolved_api_key = existing.api_key.clone();
            }
        } else {
            // Find existing config in cache
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
        
        // If this is set as active, disable all others
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

    // Mask key in response
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
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, error::AppError> {
    if let Some(ref pool) = state.db_pool {
        db::delete_ai_config(pool, id).await?;
    } else {
        let mut cache = state.ai_configs_cache.write().await;
        cache.retain(|c| c.id != id);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn activate_ai_config_handler(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, error::AppError> {
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
