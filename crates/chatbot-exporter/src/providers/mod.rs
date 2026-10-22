use crate::ExportOptions;
use async_trait::async_trait;
use serde_json::Value;
use url::Url;

pub mod chatgpt;

#[async_trait]
pub trait ConversationProvider: Send + Sync {
    fn base_urls(&self) -> &'static [&'static str];
    fn validate_url(&self, url: &Url) -> Result<(), String>;
    async fn download(&self, url: &Url, options: &ExportOptions) -> Result<String, String>;
    fn decode(&self, downloaded_page: &str) -> Result<Value, String>;
    fn format(&self, conversation: &Value, source_url: &Url) -> Result<String, String>;
}
