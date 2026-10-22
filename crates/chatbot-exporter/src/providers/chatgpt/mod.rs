mod conversation;
mod decode;
mod markdown;

use crate::providers::ConversationProvider;
use crate::{ExportOptions, fetch::fetch_page};
use async_trait::async_trait;
use serde_json::Value;
use url::Url;

pub struct ChatGptProvider;

impl ChatGptProvider {
    pub(crate) fn validate_share_url(url: &Url) -> Result<(), String> {
        let path = url.path().strip_suffix('/').unwrap_or(url.path());
        let valid_path = {
            let path_without_leading_slash = path.strip_prefix('/').unwrap_or(path);
            let mut segments = path_without_leading_slash.split('/');
            segments.next() == Some("share")
                && segments.next().is_some_and(|id| {
                    !id.is_empty()
                        && id
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '-')
                })
                && segments.next().is_none()
        };
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || !["chatgpt.com", "chat.openai.com"].contains(&url.host_str().unwrap_or_default())
            || !valid_path
        {
            return Err(
                "Only HTTPS ChatGPT share URLs (chatgpt.com/share/… or chat.openai.com/share/…) are supported."
                    .to_owned(),
            );
        }
        Ok(())
    }
}

#[async_trait]
impl ConversationProvider for ChatGptProvider {
    fn base_urls(&self) -> &'static [&'static str] {
        &["chatgpt.com", "chat.openai.com"]
    }

    fn validate_url(&self, url: &Url) -> Result<(), String> {
        Self::validate_share_url(url)
    }

    async fn download(&self, url: &Url, options: &ExportOptions) -> Result<String, String> {
        fetch_page(url, Self::validate_share_url, options).await
    }

    fn decode(&self, downloaded_page: &str) -> Result<Value, String> {
        conversation::extract_conversation(downloaded_page)
    }

    fn format(&self, conversation: &Value, source_url: &Url) -> Result<String, String> {
        markdown::conversation_markdown(conversation, source_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_chatgpt_share_urls_and_rejects_other_paths_or_hosts() {
        for value in [
            "https://chatgpt.com/share/abc-123",
            "https://chatgpt.com/share/abc-123/",
            "https://chat.openai.com/share/abc-123",
        ] {
            assert!(
                ChatGptProvider
                    .validate_url(&Url::parse(value).unwrap())
                    .is_ok()
            );
        }
        for value in [
            "https://chatgpt.com/share/",
            "https://chatgpt.com/share/abc/extra",
            "https://chatgpt.com.evil.example/share/abc",
        ] {
            assert!(
                ChatGptProvider
                    .validate_url(&Url::parse(value).unwrap())
                    .is_err()
            );
        }
    }
}
