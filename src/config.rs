use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub ai_provider: String,
    pub gemini_api_key: Option<String>,
    pub minimax_api_key: Option<String>,
    pub upload_dir: PathBuf,
    pub database_url: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        // Load .env file if present
        let _ = dotenvy::dotenv();

        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let ai_provider = env::var("AI_PROVIDER").unwrap_or_else(|_| "gemini".to_string());
        
        let gemini_api_key = env::var("GEMINI_API_KEY").ok();
        let minimax_api_key = env::var("MINIMAX_API_KEY").ok();

        let upload_dir_str = env::var("UPLOAD_DIR").unwrap_or_else(|_| "./uploads".to_string());
        let upload_dir = PathBuf::from(upload_dir_str);
        
        let database_url = env::var("DATABASE_URL").ok();

        Self {
            host,
            port,
            ai_provider,
            gemini_api_key,
            minimax_api_key,
            upload_dir,
            database_url,
        }
    }
}
