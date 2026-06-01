pub mod gemini;
pub mod minimax;

use crate::config::Config;
use crate::error::AppError;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Analyzes the raw image bytes and returns the raw output text from the model.
    async fn analyze_image(&self, image_bytes: &[u8]) -> Result<String, AppError>;
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
    async fn analyze_image(&self, _image_bytes: &[u8]) -> Result<String, AppError> {
        // Return a mock Bangladeshi license plate recognition response
        tracing::info!("Mock AI Provider invoked");
        Ok("ঢাকা মেট্রো ঘ\n১২-৩৪৫৬".to_string())
    }
}
