use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fs;

#[derive(Deserialize, Debug)]
pub struct AiResponse {
    pub description: String,
    pub license_plate: Option<String>,
}

pub async fn analyze_image_with_gemini(image_path: &str) -> anyhow::Result<AiResponse> {
    let api_key = env::var("GEMINI_API_KEY").unwrap_or_default();
    if api_key.is_empty() || api_key == "your_gemini_api_key" {
        // Return dummy response if API key is not configured
        return Ok(AiResponse {
            description: "Dummy AI Description (API Key not set)".to_string(),
            license_plate: Some("ঢাকা মেট্রো-গ ১২-৩৪৫৬".to_string()),
        });
    }

    let client = Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
        api_key
    );

    let image_bytes = fs::read(image_path)?;
    let base64_image = general_purpose::STANDARD.encode(&image_bytes);

    let prompt = "Analyze this image from a CCTV camera. Provide a detailed description of the context (e.g. what is happening, vehicles present). Additionally, if you see a license plate, extract the exact text on the license plate. Return your response ONLY as a JSON object with two fields: 'description' (string) and 'license_plate' (string or null). Do not wrap the JSON in markdown blocks.";

    let payload = json!({
        "contents": [{
            "parts": [
                {"text": prompt},
                {
                    "inline_data": {
                        "mime_type": "image/jpeg",
                        "data": base64_image
                    }
                }
            ]
        }],
        "generationConfig": {
            "responseMimeType": "application/json"
        }
    });

    let response = client.post(&url)
        .json(&payload)
        .send()
        .await?;

    let response_text = response.text().await?;

    // Attempt to parse the response structure from Gemini
    #[derive(Deserialize)]
    struct GeminiResponse {
        candidates: Option<Vec<Candidate>>,
    }
    #[derive(Deserialize)]
    struct Candidate {
        content: Option<Content>,
    }
    #[derive(Deserialize)]
    struct Content {
        parts: Option<Vec<Part>>,
    }
    #[derive(Deserialize)]
    struct Part {
        text: Option<String>,
    }

    let gemini_resp: GeminiResponse = serde_json::from_str(&response_text)?;

    let ai_text = gemini_resp.candidates
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.content)
        .and_then(|c| c.parts)
        .and_then(|p| p.into_iter().next())
        .and_then(|p| p.text)
        .unwrap_or_else(|| "{}".to_string());

    let result: AiResponse = serde_json::from_str(&ai_text)?;

    Ok(result)
}
