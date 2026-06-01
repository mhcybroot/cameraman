use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

use crate::ai::analyze_image_with_gemini;
use crate::validation::validate_bangla_license_plate;

#[derive(Serialize)]
pub struct WebhookResponse {
    pub message: String,
    pub event_id: Option<Uuid>,
}

pub async fn handle_camera_webhook(
    State(pool): State<PgPool>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut image_path = String::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().unwrap_or("image.jpg").to_string();
        let content_type = field.content_type().unwrap_or("").to_string();

        if name == "image" || content_type.starts_with("image/") {
            let data = field.bytes().await.unwrap();
            let timestamp = Utc::now().timestamp_millis();
            let safe_file_name = format!("{}_{}", timestamp, file_name);
            let path = Path::new("uploads").join(&safe_file_name);

            let mut file = File::create(&path).unwrap();
            file.write_all(&data).unwrap();
            image_path = format!("uploads/{}", safe_file_name);
            break;
        }
    }

    if image_path.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(WebhookResponse {
                message: "No image found in payload".to_string(),
                event_id: None,
            }),
        );
    }

    // Process image with AI
    let mut ai_description = String::new();
    let mut license_plate = None;
    let mut is_valid_plate = false;

    if let Ok(ai_result) = analyze_image_with_gemini(&image_path).await {
        ai_description = ai_result.description;
        license_plate = ai_result.license_plate;

        if let Some(ref plate) = license_plate {
            is_valid_plate = validate_bangla_license_plate(plate);
        }
    }

    let result = sqlx::query(
        "INSERT INTO events (image_path, ai_description, license_plate, is_valid_plate) VALUES ($1, $2, $3, $4) RETURNING id"
    )
    .bind(&image_path)
    .bind(&ai_description)
    .bind(&license_plate)
    .bind(is_valid_plate)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(row) => {
            let id: Uuid = row.get("id");
            (
                StatusCode::OK,
                Json(WebhookResponse {
                    message: "Image received and processed".to_string(),
                    event_id: Some(id),
                }),
            )
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    message: "Failed to save event".to_string(),
                    event_id: None,
                }),
            )
        }
    }
}
