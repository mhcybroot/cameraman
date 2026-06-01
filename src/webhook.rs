use crate::db::ProcessedEvent;
use crate::error::AppError;
use crate::state::AppState;
use axum::{
    extract::{FromRequest, State},
    http::{header::CONTENT_TYPE, Request, StatusCode},
    response::IntoResponse,
    Json,
};
use axum::body::Body;
use base64::Engine;
use base64::engine::{general_purpose::GeneralPurpose, DecodePaddingMode, GeneralPurposeConfig};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CameraJsonPayload {
    pub image: String, // Base64 encoded image
    pub camera_id: Option<String>,
}

pub async fn handle_webhook(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<impl IntoResponse, AppError> {
    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    tracing::info!("Received webhook request with content-type: {}", content_type);

    if content_type.starts_with("application/json") {
        // Limit body size to 15MB to prevent excessive memory usage
        let bytes = axum::body::to_bytes(req.into_body(), 15 * 1024 * 1024)
            .await
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let payload: CameraJsonPayload = serde_json::from_slice(&bytes)?;
        
        // Strip data URI scheme prefix (e.g., "data:image/jpeg;base64,") if present
        let base64_data = if let Some(index) = payload.image.find("base64,") {
            &payload.image[index + 7..]
        } else {
            &payload.image
        };

        // Decode base64 image supporting both padded/unpadded format and loose trailing bits
        let config = GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true);
        let engine = GeneralPurpose::new(&base64::alphabet::STANDARD, config);
        let image_bytes = engine
            .decode(base64_data.trim())
            .map_err(|e| AppError::InvalidPayload(format!("Failed to decode base64 image: {}", e)))?;

        process_incoming_image(state, image_bytes, payload.camera_id).await?;

    } else if content_type.starts_with("multipart/form-data") {
        let mut multipart = axum::extract::Multipart::from_request(req, &state)
            .await
            .map_err(|e| AppError::InvalidPayload(e.to_string()))?;

        let mut image_bytes = None;
        let mut camera_id = None;

        while let Some(field) = multipart.next_field().await? {
            let name = field.name().unwrap_or("").to_string();
            if name == "image" || name == "file" {
                image_bytes = Some(field.bytes().await?.to_vec());
            } else if name == "camera_id" {
                camera_id = Some(field.text().await?);
            }
        }

        let bytes = image_bytes.ok_or_else(|| {
            AppError::InvalidPayload("Missing 'image' or 'file' field in multipart data".to_string())
        })?;

        process_incoming_image(state, bytes, camera_id).await?;

    } else {
        return Err(AppError::InvalidPayload(
            "Unsupported Content-Type. Must be application/json or multipart/form-data".to_string(),
        ));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "status": "success",
            "message": "Payload received and queued for AI analysis"
        })),
    ))
}

async fn process_incoming_image(
    state: AppState,
    image_bytes: Vec<u8>,
    camera_id: Option<String>,
) -> Result<(), AppError> {
    // Ensure upload directory exists
    tokio::fs::create_dir_all(&state.config.upload_dir).await?;

    let resolved_camera_id = camera_id.unwrap_or_else(|| "default-cam".to_string());

    // Resolve or auto-register camera in background
    let state_clone = state.clone();
    let resolved_camera_id_clone = resolved_camera_id.clone();
    tokio::spawn(async move {
        if let Some(ref pool) = state_clone.db_pool {
            match crate::db::get_camera_by_id(pool, &resolved_camera_id_clone).await {
                Ok(None) => {
                    let new_cam = crate::db::Camera {
                        id: resolved_camera_id_clone.clone(),
                        name: format!("Auto Cam: {}", resolved_camera_id_clone),
                        location: None,
                        created_at: Some(chrono::Utc::now()),
                    };
                    if let Err(e) = crate::db::save_camera(pool, &new_cam).await {
                        tracing::error!("Failed to auto-register camera {}: {}", resolved_camera_id_clone, e);
                    } else {
                        tracing::info!("Auto-registered camera {} in DB", resolved_camera_id_clone);
                    }
                }
                Err(e) => {
                    tracing::error!("Database error looking up camera {}: {}", resolved_camera_id_clone, e);
                }
                _ => {}
            }
        } else {
            let mut cache = state_clone.cameras_cache.write().await;
            if !cache.iter().any(|c| c.id == resolved_camera_id_clone) {
                let new_cam = crate::db::Camera {
                    id: resolved_camera_id_clone.clone(),
                    name: format!("Auto Cam: {}", resolved_camera_id_clone),
                    location: None,
                    created_at: Some(chrono::Utc::now()),
                };
                cache.push(new_cam);
                tracing::info!("Auto-registered camera {} in memory cache", resolved_camera_id_clone);
            }
        }
    });

    // Generate unique file path
    let file_id = uuid::Uuid::new_v4();
    let file_name = format!("{}.jpg", file_id);
    let file_path = state.config.upload_dir.join(&file_name);

    // Save image to disk asynchronously
    tokio::fs::write(&file_path, &image_bytes).await?;
    tracing::info!(
        "Saved incoming camera payload (ID: {}) to file: {}",
        resolved_camera_id,
        file_path.display()
    );

    // Spawn background task to call the pluggable AI provider
    tokio::spawn(async move {
        tracing::info!("Triggering background AI processing for image ID: {}", file_id);
        
        // 1. Resolve dynamic active AI Config
        let active_config = if let Some(ref pool) = state.db_pool {
            crate::db::get_active_ai_config(pool).await.unwrap_or(None)
        } else {
            let cache = state.ai_configs_cache.read().await;
            cache.iter().find(|c| c.is_active).cloned()
        };

        // 2. Build AI provider client dynamically or fallback to static env provider
        let provider: std::sync::Arc<dyn crate::ai::AiProvider> = match active_config {
            Some(ref config) => {
                match config.provider_type.to_lowercase().as_str() {
                    "gemini" => {
                        let key = config.api_key.clone().unwrap_or_default();
                        std::sync::Arc::new(crate::ai::gemini::GeminiProvider::new(key))
                    }
                    "minimax" => {
                        let key = config.api_key.clone().unwrap_or_default();
                        let model = config.model_name.clone().unwrap_or_else(|| "MiniMax-M3".to_string());
                        std::sync::Arc::new(crate::ai::minimax::MinimaxProvider::new(key, model))
                    }
                    "mock" => {
                        std::sync::Arc::new(crate::ai::MockProvider::new())
                    }
                    _ => state.ai_provider.clone(),
                }
            }
            None => state.ai_provider.clone(),
        };

        match provider.analyze_image(&image_bytes).await {
            Ok(ocr_result) => {
                tracing::info!(
                    "AI processing completed for image ID {}. Result:\n{}",
                    file_id,
                    ocr_result
                );

                // Run Bangla License Plate Validation
                let (is_valid, detected_plate, class_letter, district, metro, plate_num) = 
                    match crate::validation::validate_plate(&ocr_result) {
                        Ok(plate) => {
                            tracing::info!("Bangla License Plate Validated successfully: {:?}", plate);
                            (
                                Some(true),
                                Some(format!("{}\n{}", plate.top_line, plate.bottom_line)),
                                Some(plate.class_letter.to_string()),
                                Some(plate.district),
                                Some(plate.metro),
                                Some(plate.plate_number),
                            )
                        }
                        Err(err) => {
                            tracing::warn!("Bangla License Plate validation failed: {}", err);
                            (Some(false), None, None, None, None, None)
                        }
                    };

                // Construct DB representation
                let db_event = ProcessedEvent {
                    id: file_id,
                    camera_id: Some(resolved_camera_id.clone()),
                    image_path: file_name.clone(), // Save relative filename for easier frontend serving
                    raw_ai_text: Some(ocr_result),
                    is_plate_valid: is_valid,
                    detected_plate_text: detected_plate,
                    vehicle_class: class_letter,
                    district,
                    metro_prefix: metro,
                    plate_number: plate_num,
                    created_at: Some(chrono::Utc::now()),
                };

                // Save to Postgres if database is configured
                if let Some(ref pool) = state.db_pool {
                    if let Err(db_err) = crate::db::save_event(pool, &db_event).await {
                        tracing::error!("Failed to save event {} to Postgres database: {}", file_id, db_err);
                    } else {
                        tracing::info!("Saved event {} to database successfully", file_id);
                    }
                } else {
                    tracing::warn!("Postgres database not connected. Event {} saved to memory cache.", file_id);
                    let mut cache = state.events_cache.write().await;
                    cache.push(db_event.clone());
                    if cache.len() > 50 {
                        cache.remove(0);
                    }
                }
            }
            Err(err) => {
                tracing::error!(
                    "Error during background AI analysis for image ID {}: {}",
                    file_id,
                    err
                );
            }
        }
    });

    Ok(())
}
