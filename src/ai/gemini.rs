use crate::ai::AiProvider;
use crate::error::AppError;
use async_trait::async_trait;
use base64::{prelude::BASE64_STANDARD, Engine};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

pub struct GeminiProvider {
    api_key: String,
    client: Client,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { api_key, client }
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn analyze_image(&self, image_bytes: &[u8]) -> Result<String, AppError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
            self.api_key
        );

        let image_b64 = BASE64_STANDARD.encode(image_bytes);

        // System prompt instructs the model to extract Bangladeshi license plates strictly.
        let prompt = "Extract the Bangladeshi vehicle license plate from this image. \
                      Bangladeshi license plates have two lines:\
                      - Top Line (e.g. 'ঢাকা মেট্রো ঘ' or 'সিলেট হ') containing the district, optionally the word 'মেট্রো', and a single vehicle class letter (ক to হ).\
                      - Bottom Line (e.g. '১২-৩৪৫৬') containing exactly 6 digits formatted as XX-XXXX.\
                      Provide your response in the following format:\
                      LICENSE_PLATE:\
                      [Top Line]\
                      [Bottom Line]\
                      \
                      CONTEXT:\
                      [Brief description of the vehicle type, color, and location context]";

        let payload = json!({
            "contents": [
                {
                    "parts": [
                        {
                            "text": prompt
                        },
                        {
                            "inlineData": {
                                "mimeType": "image/jpeg",
                                "data": image_b64
                            }
                        }
                    ]
                }
            ]
        });

        tracing::info!("Sending image to Gemini 2.5 Flash API...");
        
        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Gemini API error (Status {}): {}", status, error_text);
            return Err(AppError::AiProvider(format!(
                "Gemini API returned error status {}: {}",
                status, error_text
            )));
        }

        let res_json: serde_json::Value = response.json().await?;
        
        // Extract text from the candidate response structure
        let extracted_text = res_json
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::AiProvider("Failed to parse response text from Gemini API response".to_string())
            })?;

        tracing::info!("Received response from Gemini API");
        Ok(extracted_text.to_string())
    }
}
