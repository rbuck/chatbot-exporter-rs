use crate::{
    ExportOptions,
    providers::{ConversationProvider, chatgpt::ChatGptProvider, claude::ClaudeProvider},
};
use serde_json::Value;
use url::Url;

pub struct ProviderRegistry {
    providers: Vec<Box<dyn ConversationProvider>>,
}

impl ProviderRegistry {
    pub fn new(providers: Vec<Box<dyn ConversationProvider>>) -> Self {
        Self { providers }
    }

    pub async fn export(
        &self,
        source_url: &Url,
        options: &ExportOptions,
    ) -> Result<String, String> {
        options.validate()?;
        let host = source_url.host_str().unwrap_or_default();
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.base_urls().contains(&host))
            .ok_or_else(|| format!("No conversation exporter is registered for host {host:?}."))?;
        provider.validate_url(source_url)?;
        let downloaded_page = provider.download(source_url, options).await?;
        let conversation: Value = provider.decode(&downloaded_page)?;
        provider.format(&conversation, source_url)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new(vec![Box::new(ChatGptProvider), Box::new(ClaudeProvider)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ConversationProvider;
    use async_trait::async_trait;

    struct FixtureProvider;

    #[async_trait]
    impl ConversationProvider for FixtureProvider {
        fn base_urls(&self) -> &'static [&'static str] {
            &["fixture.example"]
        }

        fn validate_url(&self, url: &Url) -> Result<(), String> {
            (url.path() == "/share/example")
                .then_some(())
                .ok_or_else(|| "Unexpected fixture URL path.".to_owned())
        }

        async fn download(&self, _url: &Url, _options: &ExportOptions) -> Result<String, String> {
            Ok("fixture page".to_owned())
        }

        fn decode(&self, page: &str) -> Result<Value, String> {
            Ok(serde_json::json!({"page": page}))
        }

        fn format(&self, conversation: &Value, _url: &Url) -> Result<String, String> {
            Ok(format!("Fixture provider: {}", conversation["page"]))
        }
    }

    #[tokio::test]
    async fn dispatches_by_host_and_supports_added_providers() {
        let registry = ProviderRegistry::new(vec![Box::new(FixtureProvider)]);
        let options = ExportOptions::default();
        let url = Url::parse("https://fixture.example/share/example").unwrap();
        assert_eq!(
            registry.export(&url, &options).await.unwrap(),
            "Fixture provider: \"fixture page\""
        );
        let unknown = Url::parse("https://example.com/share/example").unwrap();
        assert!(
            registry
                .export(&unknown, &options)
                .await
                .unwrap_err()
                .contains("No conversation exporter")
        );
    }
}
