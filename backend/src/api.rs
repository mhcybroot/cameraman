use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use sqlx::{PgPool, FromRow};
use uuid::Uuid;

#[derive(Serialize, FromRow)]
pub struct Event {
    pub id: Uuid,
    pub image_path: String,
    pub ai_description: Option<String>,
    pub license_plate: Option<String>,
    pub is_valid_plate: Option<bool>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn get_events(State(pool): State<PgPool>) -> impl IntoResponse {
    let result = sqlx::query_as::<_, Event>(
        "SELECT id, image_path, ai_description, license_plate, is_valid_plate, created_at FROM events ORDER BY created_at DESC"
    )
    .fetch_all(&pool)
    .await;

    match result {
        Ok(events) => (StatusCode::OK, Json(events)),
        Err(e) => {
            eprintln!("Failed to fetch events: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
        }
    }
}
