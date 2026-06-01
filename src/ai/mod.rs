pub mod gemini;
pub mod minimax;
pub mod prompts;

use crate::config::Config;
use crate::error::AppError;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Analyzes the raw image bytes and returns the raw output text from the model.
    async fn analyze_image(&self, image_bytes: &[u8], prompt: &str) -> Result<String, AppError>;
}

/// Dynamic provider constructor based on Configuration
pub fn build_provider(config: &Config) -> Result<Arc<dyn AiProvider>, AppError> {
    match config.ai_provider.to_lowercase().as_str() {
        "gemini" => {
            let api_key = config
                .gemini_api_key
                .clone()
                .ok_or_else(|| AppError::Config("GEMINI_API_KEY environment variable is not set".to_string()))?;
            Ok(Arc::new(gemini::GeminiProvider::new(api_key)))
        }
        "minimax" => {
            let api_key = config
                .minimax_api_key
                .clone()
                .ok_or_else(|| AppError::Config("MINIMAX_API_KEY environment variable is not set".to_string()))?;
            let model = std::env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M3".to_string());
            Ok(Arc::new(minimax::MinimaxProvider::new(api_key, model)))
        }
        "mock" => {
            Ok(Arc::new(MockProvider::new()))
        }
        other => Err(AppError::Config(format!(
            "Unsupported AI provider: '{}'. Valid options are 'gemini', 'minimax', 'mock'.",
            other
        ))),
    }
}

pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AiProvider for MockProvider {
    async fn analyze_image(&self, _image_bytes: &[u8], prompt: &str) -> Result<String, AppError> {
        tracing::info!("Mock AI Provider invoked");
        
        let mut response = serde_json::Map::new();

        if prompt.contains("anpr") {
            response.insert("anpr".to_string(), serde_json::json!({
                "plate_found": true,
                "top_line": "ঢাকা মেট্রো ঘ",
                "bottom_line": "১২-৩৪৫৬",
                "raw_text": "ঢাকা মেট্রো ঘ\n১২-৩৪৫৬",
                "vehicle_context": "white microbus"
            }));
        }

        if prompt.contains("face_detection") {
            response.insert("face_detection".to_string(), serde_json::json!({
                "detected": true,
                "count": 2,
                "details": [
                    {
                        "bbox": [20.5, 30.0, 45.0, 52.5],
                        "attributes": { "age": "25-35", "gender": "male", "emotion": "happy" }
                    },
                    {
                        "bbox": [18.2, 60.1, 41.5, 78.0],
                        "attributes": { "age": "30-40", "gender": "female", "emotion": "neutral" }
                    }
                ]
            }));
        }

        if prompt.contains("perimeter_protection") {
            response.insert("perimeter_protection".to_string(), serde_json::json!({
                "breach": true,
                "zone_name": "restricted_gate",
                "objects": ["person"]
            }));
        }

        if prompt.contains("face_recognition") {
            response.insert("face_recognition".to_string(), serde_json::json!({
                "matches": [
                    { "name": "Abir Hasan", "confidence": 0.95, "role": "staff" },
                    { "name": "Unknown Person", "confidence": 0.0, "role": "unknown" }
                ]
            }));
        }

        if prompt.contains("video_metadata") {
            response.insert("video_metadata".to_string(), serde_json::json!({
                "humans": [
                    { "gender": "male", "upper_clothing_color": "blue", "lower_clothing_color": "black", "backpack": true },
                    { "gender": "female", "upper_clothing_color": "red", "lower_clothing_color": "white", "backpack": false }
                ],
                "motor_vehicles": [
                    { "type": "car", "color": "white", "brand": "Toyota Noah" }
                ],
                "non_motor_vehicles": [
                    { "type": "rickshaw" }
                ]
            }));
        }

        if prompt.contains("smd_plus") {
            response.insert("smd_plus".to_string(), serde_json::json!({
                "trigger_type": "human",
                "confidence": 0.98,
                "description": "Person walked past the entrance corridor"
            }));
        }

        if prompt.contains("stereo_analysis") {
            response.insert("stereo_analysis".to_string(), serde_json::json!({
                "anomaly_detected": false,
                "anomaly_type": "none",
                "estimated_height_m": 1.78,
                "details": "Height estimations within normal bounds"
            }));
        }

        if prompt.contains("crowd_distribution") {
            response.insert("crowd_distribution".to_string(), serde_json::json!({
                "crowd_level": "medium",
                "estimated_count": 8,
                "density_percentage": 45
            }));
        }

        if prompt.contains("people_counting") {
            response.insert("people_counting".to_string(), serde_json::json!({
                "current_count": 4,
                "entered": 15,
                "exited": 11
            }));
        }

        if prompt.contains("vehicle_density") {
            response.insert("vehicle_density".to_string(), serde_json::json!({
                "congestion_level": "clear",
                "vehicle_count": 1,
                "density_percentage": 10
            }));
        }

        if prompt.contains("heat_map") {
            response.insert("heat_map".to_string(), serde_json::json!({
                "hotspots": [ [3, 4], [4, 4], [4, 5] ],
                "description": "Activity concentrated near center entrance gate"
            }));
        }

        if prompt.contains("ppe_detection") {
            response.insert("ppe_detection".to_string(), serde_json::json!({
                "violations_found": true,
                "details": [
                    {
                        "person_index": 1,
                        "has_hardhat": true,
                        "has_safety_vest": false,
                        "has_mask": true,
                        "missing_items": ["safety_vest"]
                    }
                ]
            }));
        }

        if prompt.contains("smart_object_detection") {
            response.insert("smart_object_detection".to_string(), serde_json::json!({
                "alert_triggered": true,
                "alert_type": "loitering",
                "details": "Person loitering in lobby for over 5 minutes"
            }));
        }

        if prompt.contains("smart_sound_detection") {
            response.insert("smart_sound_detection".to_string(), serde_json::json!({
                "sound_alert_triggered": false,
                "inferred_sound_type": "none",
                "confidence": 0.0,
                "details": "No screaming or impact sounds detected visually"
            }));
        }

        let json_value = serde_json::Value::Object(response);
        let res_string = serde_json::to_string(&json_value).unwrap_or_default();
        Ok(res_string)
    }
}
