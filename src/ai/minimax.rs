use crate::ai::AiProvider;
use crate::error::AppError;
use async_trait::async_trait;
use base64::{prelude::BASE64_STANDARD, Engine};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

pub struct MinimaxProvider {
    api_key: String,
    model_name: String,
    client: Client,
}

impl MinimaxProvider {
    pub fn new(api_key: String, model_name: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            api_key,
            model_name,
            client,
        }
    }
}

#[async_trait]
impl AiProvider for MinimaxProvider {
    async fn analyze_image(&self, image_bytes: &[u8], prompt: &str) -> Result<String, AppError> {
        let url = "https://api.minimax.io/v1/chat/completions";

        let image_b64 = BASE64_STANDARD.encode(image_bytes);
        let image_data_url = format!("data:image/jpeg;base64,{}", image_b64);

        let payload = json!({
            "model": self.model_name,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_data_url
                            }
                        }
                    ]
                }
            ]
        });

        tracing::info!("Sending image to Minimax API (model: {})...", self.model_name);

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Minimax API error (Status {}): {}", status, error_text);
            return Err(AppError::AiProvider(format!(
                "Minimax API returned error status {}: {}",
                status, error_text
            )));
        }

        let res_json: serde_json::Value = response.json().await?;

        let extracted_text = res_json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::AiProvider("Failed to parse response text from Minimax API response".to_string())
            })?;

        tracing::info!("Received response from Minimax API");
        Ok(extracted_text.to_string())
    }
}
