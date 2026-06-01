use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Pool, Postgres};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessedEvent {
    pub id: Uuid,
    pub camera_id: Option<String>,
    pub image_path: String,
    pub raw_ai_text: Option<String>,
    pub is_plate_valid: Option<bool>,
    pub detected_plate_text: Option<String>,
    pub vehicle_class: Option<String>,
    pub district: Option<String>,
    pub metro_prefix: Option<bool>,
    pub plate_number: Option<String>,
    pub analytics_data: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Camera {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
    pub enabled_modules: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiConfig {
    pub id: uuid::Uuid,
    pub name: String,
    pub provider_type: String,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub is_active: bool,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: String, // "admin" or "user"
    pub is_blocked: bool,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserSession {
    pub token: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

/// Initializes database connection pool and creates schema if missing
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    tracing::info!("Connecting to PostgreSQL...");
    let pool = Pool::<Postgres>::connect(database_url).await?;

    tracing::info!("Running embedded database migrations...");
    // 1. Create cameras table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cameras (
            id VARCHAR(255) PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            location VARCHAR(255),
            enabled_modules JSONB DEFAULT '[]'::jsonb,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
        );"
    )
    .execute(&pool)
    .await?;

    // Migrate existing cameras table if needed
    let _ = sqlx::query("ALTER TABLE cameras ADD COLUMN IF NOT EXISTS enabled_modules JSONB DEFAULT '[]'::jsonb;")
        .execute(&pool)
        .await;

    // 2. Create processed_events table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS processed_events (
            id UUID PRIMARY KEY,
            camera_id VARCHAR(255) REFERENCES cameras(id) ON DELETE SET NULL,
            image_path VARCHAR(512) NOT NULL,
            raw_ai_text TEXT,
            is_plate_valid BOOLEAN DEFAULT FALSE,
            detected_plate_text VARCHAR(100),
            vehicle_class VARCHAR(10),
            district VARCHAR(100),
            metro_prefix BOOLEAN DEFAULT FALSE,
            plate_number VARCHAR(20),
            analytics_data JSONB,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
        );"
    )
    .execute(&pool)
    .await?;

    // Migrate existing processed_events table if needed
    let _ = sqlx::query("ALTER TABLE processed_events ADD COLUMN IF NOT EXISTS analytics_data JSONB;")
        .execute(&pool)
        .await;

    // 3. Create ai_configs table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_configs (
            id UUID PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            provider_type VARCHAR(100) NOT NULL,
            api_key TEXT,
            model_name VARCHAR(100),
            is_active BOOLEAN DEFAULT FALSE,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
        );"
    )
    .execute(&pool)
    .await?;

    // 4. Create users table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id UUID PRIMARY KEY,
            username VARCHAR(100) UNIQUE NOT NULL,
            password_hash VARCHAR(255) NOT NULL,
            role VARCHAR(50) NOT NULL,
            is_blocked BOOLEAN DEFAULT FALSE,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
        );"
    )
    .execute(&pool)
    .await?;

    // 5. Create user_sessions table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_sessions (
            token UUID PRIMARY KEY,
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            expires_at TIMESTAMPTZ NOT NULL
        );"
    )
    .execute(&pool)
    .await?;

    tracing::info!("Database initialized successfully.");
    Ok(pool)
}

/// Inserts a new processed event record into the database
pub async fn save_event(
    pool: &PgPool,
    event: &ProcessedEvent,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO processed_events (
            id, camera_id, image_path, raw_ai_text, is_plate_valid, 
            detected_plate_text, vehicle_class, district, metro_prefix, plate_number, analytics_data, created_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
    )
    .bind(event.id)
    .bind(&event.camera_id)
    .bind(&event.image_path)
    .bind(&event.raw_ai_text)
    .bind(event.is_plate_valid)
    .bind(&event.detected_plate_text)
    .bind(&event.vehicle_class)
    .bind(&event.district)
    .bind(event.metro_prefix)
    .bind(&event.plate_number)
    .bind(&event.analytics_data)
    .bind(event.created_at.unwrap_or_else(Utc::now))
    .execute(pool)
    .await?;

    Ok(())
}

/// Fetches events with optional limits, ordered by newest first
pub async fn get_events(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<ProcessedEvent>, sqlx::Error> {
    let events = sqlx::query_as::<_, ProcessedEvent>(
        "SELECT id, camera_id, image_path, raw_ai_text, is_plate_valid, 
                detected_plate_text, vehicle_class, district, metro_prefix, plate_number, analytics_data, created_at
         FROM processed_events
         ORDER BY created_at DESC
         LIMIT $1"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(events)
}

/// Inserts or updates a camera record in the database
pub async fn save_camera(
    pool: &PgPool,
    camera: &Camera,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO cameras (id, name, location, enabled_modules, created_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (id) DO UPDATE 
         SET name = EXCLUDED.name, location = EXCLUDED.location, enabled_modules = EXCLUDED.enabled_modules"
    )
    .bind(&camera.id)
    .bind(&camera.name)
    .bind(&camera.location)
    .bind(&camera.enabled_modules)
    .bind(camera.created_at.unwrap_or_else(Utc::now))
    .execute(pool)
    .await?;

    Ok(())
}

/// Fetches all registered cameras
pub async fn get_cameras(
    pool: &PgPool,
) -> Result<Vec<Camera>, sqlx::Error> {
    let cameras = sqlx::query_as::<_, Camera>(
        "SELECT id, name, location, enabled_modules, created_at FROM cameras ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await?;

    Ok(cameras)
}

/// Deletes a camera record from the database
pub async fn delete_camera(
    pool: &PgPool,
    id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cameras WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Looks up a camera record by ID
pub async fn get_camera_by_id(
    pool: &PgPool,
    id: &str,
) -> Result<Option<Camera>, sqlx::Error> {
    let camera = sqlx::query_as::<_, Camera>(
        "SELECT id, name, location, enabled_modules, created_at FROM cameras WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(camera)
}

/// Inserts or updates an AI configuration in the database
pub async fn save_ai_config(
    pool: &PgPool,
    config: &AiConfig,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ai_configs (id, name, provider_type, api_key, model_name, is_active, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (id) DO UPDATE 
         SET name = EXCLUDED.name, provider_type = EXCLUDED.provider_type, 
             api_key = EXCLUDED.api_key, model_name = EXCLUDED.model_name, 
             is_active = EXCLUDED.is_active"
    )
    .bind(config.id)
    .bind(&config.name)
    .bind(&config.provider_type)
    .bind(&config.api_key)
    .bind(&config.model_name)
    .bind(config.is_active)
    .bind(config.created_at.unwrap_or_else(Utc::now))
    .execute(pool)
    .await?;

    Ok(())
}

/// Fetches all AI configurations
pub async fn get_ai_configs(
    pool: &PgPool,
) -> Result<Vec<AiConfig>, sqlx::Error> {
    let configs = sqlx::query_as::<_, AiConfig>(
        "SELECT id, name, provider_type, api_key, model_name, is_active, created_at 
         FROM ai_configs 
         ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await?;

    Ok(configs)
}

/// Deletes an AI configuration
pub async fn delete_ai_config(
    pool: &PgPool,
    id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM ai_configs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Toggles an AI configuration to active, setting all others to inactive
pub async fn set_active_ai_config(
    pool: &PgPool,
    id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("UPDATE ai_configs SET is_active = FALSE")
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE ai_configs SET is_active = TRUE WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

/// Retrieves the active AI configuration, if any
pub async fn get_active_ai_config(
    pool: &PgPool,
) -> Result<Option<AiConfig>, sqlx::Error> {
    let config = sqlx::query_as::<_, AiConfig>(
        "SELECT id, name, provider_type, api_key, model_name, is_active, created_at 
         FROM ai_configs 
         WHERE is_active = TRUE 
         LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;

    Ok(config)
}

/// Inserts or updates a user record
pub async fn save_user(
    pool: &PgPool,
    user: &User,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, is_blocked, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (id) DO UPDATE 
         SET username = EXCLUDED.username, 
             password_hash = EXCLUDED.password_hash, 
             role = EXCLUDED.role, 
             is_blocked = EXCLUDED.is_blocked"
    )
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.password_hash)
    .bind(&user.role)
    .bind(user.is_blocked)
    .bind(user.created_at.unwrap_or_else(Utc::now))
    .execute(pool)
    .await?;

    Ok(())
}

/// Retrieves a user by username
pub async fn get_user_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, is_blocked, created_at 
         FROM users 
         WHERE username = $1"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

/// Retrieves a user by ID
pub async fn get_user_by_id(
    pool: &PgPool,
    id: uuid::Uuid,
) -> Result<Option<User>, sqlx::Error> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, is_blocked, created_at 
         FROM users 
         WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

/// Fetches all users
pub async fn get_all_users(
    pool: &PgPool,
) -> Result<Vec<User>, sqlx::Error> {
    let users = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, is_blocked, created_at 
         FROM users 
         ORDER BY username ASC"
    )
    .fetch_all(pool)
    .await?;

    Ok(users)
}

/// Inserts a session token
pub async fn save_session(
    pool: &PgPool,
    session: &UserSession,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_sessions (token, user_id, expires_at)
         VALUES ($1, $2, $3)"
    )
    .bind(session.token)
    .bind(session.user_id)
    .bind(session.expires_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Retrieves a user associated with a valid, non-expired session token
pub async fn get_session_user(
    pool: &PgPool,
    token: uuid::Uuid,
) -> Result<Option<User>, sqlx::Error> {
    let user = sqlx::query_as::<_, User>(
        "SELECT u.id, u.username, u.password_hash, u.role, u.is_blocked, u.created_at 
         FROM users u
         JOIN user_sessions s ON u.id = s.user_id
         WHERE s.token = $1 AND s.expires_at > $2"
    )
    .bind(token)
    .bind(Utc::now())
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

/// Deletes a session token (logout)
pub async fn delete_session(
    pool: &PgPool,
    token: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM user_sessions WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;

    Ok(())
}
