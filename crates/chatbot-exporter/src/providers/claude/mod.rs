use crate::{
    ExportOptions, canonical_source_url, fetch::fetch_page, providers::ConversationProvider,
};
use async_trait::async_trait;
use serde_json::Value;
use url::Url;

pub struct ClaudeProvider;

impl ClaudeProvider {
    fn share_id(url: &Url) -> Result<&str, String> {
        let id = url
            .path()
            .strip_prefix("/share/")
            .and_then(|path| path.strip_suffix('/').or(Some(path)))
            .ok_or_else(|| "Only Claude share URLs are supported.".to_owned())?;
        if !is_uuid(id) {
            return Err("Only Claude share URLs are supported.".to_owned());
        }
        Ok(id)
    }

    fn validate_provider_url(url: &Url) -> Result<(), String> {
        if url.scheme() != "https"
            || url.host_str() != Some("claude.ai")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("Only HTTPS Claude share URLs are supported.".to_owned());
        }

        if url.path().starts_with("/api/chat_snapshots/") {
            let id = url.path().trim_start_matches("/api/chat_snapshots/");
            if is_uuid(id) {
                return Ok(());
            }
        }

        Self::share_id(url).map(|_| ())
    }

    fn snapshot_url(url: &Url) -> Result<Url, String> {
        Self::validate_provider_url(url)?;
        let id = Self::share_id(url)?;
        let mut snapshot = url.clone();
        snapshot.set_path(&format!("/api/chat_snapshots/{id}"));
        snapshot.set_query(Some("rendering_mode=messages&render_all_tools=true"));
        snapshot.set_fragment(None);
        Ok(snapshot)
    }
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[async_trait]
impl ConversationProvider for ClaudeProvider {
    fn base_urls(&self) -> &'static [&'static str] {
        &["claude.ai"]
    }

    fn validate_url(&self, url: &Url) -> Result<(), String> {
        Self::validate_provider_url(url)?;
        Self::share_id(url).map(|_| ())
    }

    async fn download(&self, url: &Url, options: &ExportOptions) -> Result<String, String> {
        let snapshot_url = Self::snapshot_url(url)?;
        fetch_page(&snapshot_url, Self::validate_provider_url, options).await
    }

    fn decode(&self, downloaded_page: &str) -> Result<Value, String> {
        let snapshot: Value = serde_json::from_str(downloaded_page)
            .map_err(|error| format!("Could not decode the Claude share response: {error}"))?;
        if !snapshot.get("chat_messages").is_some_and(Value::is_array) {
            return Err(
                "Claude did not return a public conversation. Check that the link is shared with anyone who has the link."
                    .to_owned(),
            );
        }
        Ok(snapshot)
    }

    fn format(&self, snapshot: &Value, source_url: &Url) -> Result<String, String> {
        let messages = snapshot
            .get("chat_messages")
            .and_then(Value::as_array)
            .ok_or_else(|| "The Claude share response did not contain messages.".to_owned())?;
        let title = snapshot
            .get("snapshot_name")
            .and_then(Value::as_str)
            .filter(|title| !title.trim().is_empty())
            .unwrap_or("Claude conversation")
            .trim()
            .replace(['\r', '\n'], " ");
        let mut output = format!(
            "# {title}\n\nSource: {}\n\n",
            canonical_source_url(source_url)
        );
        let mut exported_messages = 0;

        for message in messages {
            let (Some(sender), Some(blocks)) = (
                message.get("sender").and_then(Value::as_str),
                message.get("content").and_then(Value::as_array),
            ) else {
                continue;
            };
            let label = match sender {
                "human" => "You",
                "assistant" => "Claude",
                _ => continue,
            };
            let text = blocks
                .iter()
                .filter(|block| {
                    block.get("type").and_then(Value::as_str) == Some("text")
                        && block.get("hidden_in_chat").and_then(Value::as_bool) != Some(true)
                })
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
                .trim()
                .to_owned();
            if text.is_empty() {
                continue;
            }
            output.push_str(&format!("## {label}\n\n{text}\n\n"));
            exported_messages += 1;
        }

        if exported_messages == 0 {
            return Err("No readable user or assistant messages were found.".to_owned());
        }
        while output.ends_with('\n') {
            output.pop();
        }
        output.push('\n');
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHARE_URL: &str = "https://claude.ai/share/53b4b26d-3a49-4428-b6d8-a861867fe98b";

    #[test]
    fn validates_only_https_claude_share_links() {
        let valid = Url::parse(SHARE_URL).unwrap();
        assert!(ClaudeProvider.validate_url(&valid).is_ok());
        assert!(
            ClaudeProvider
                .validate_url(
                    &Url::parse("http://claude.ai/share/53b4b26d-3a49-4428-b6d8-a861867fe98b")
                        .unwrap()
                )
                .is_err()
        );
        assert!(
            ClaudeProvider
                .validate_url(&Url::parse("https://claude.ai/share/not-a-uuid").unwrap())
                .is_err()
        );
        assert!(
            ClaudeProvider
                .validate_url(
                    &Url::parse(
                        "https://claude.ai.evil.example/share/53b4b26d-3a49-4428-b6d8-a861867fe98b"
                    )
                    .unwrap()
                )
                .is_err()
        );
    }

    #[test]
    fn builds_public_snapshot_api_url() {
        let share = Url::parse(SHARE_URL).unwrap();
        let snapshot = ClaudeProvider::snapshot_url(&share).unwrap();
        assert_eq!(
            snapshot.as_str(),
            "https://claude.ai/api/chat_snapshots/53b4b26d-3a49-4428-b6d8-a861867fe98b?rendering_mode=messages&render_all_tools=true"
        );
    }

    #[test]
    fn decodes_and_formats_visible_messages() {
        let snapshot = ClaudeProvider
            .decode(
                r#"{
                    "snapshot_name": "A Claude chat",
                    "chat_messages": [
                        {"sender": "human", "content": [{"type": "text", "text": "Question"}]},
                        {"sender": "assistant", "content": [
                            {"type": "tool_use", "name": "search"},
                            {"type": "text", "text": "Answer"},
                            {"type": "text", "text": "More answer"}
                        ]},
                        {"sender": "assistant", "content": [
                            {"type": "text", "text": "Hidden", "hidden_in_chat": true}
                        ]}
                    ]
                }"#,
            )
            .unwrap();
        let share_url = Url::parse(SHARE_URL).unwrap();
        assert_eq!(
            ClaudeProvider.format(&snapshot, &share_url).unwrap(),
            "# A Claude chat\n\nSource: https://claude.ai/share/53b4b26d-3a49-4428-b6d8-a861867fe98b\n\n## You\n\nQuestion\n\n## Claude\n\nAnswer\n\nMore answer\n"
        );
    }

    #[test]
    fn rejects_private_or_unrecognized_snapshot_responses() {
        assert!(
            ClaudeProvider
                .decode(r#"{"type":"error","error":{"message":"not public"}}"#)
                .unwrap_err()
                .contains("anyone who has the link")
        );
        assert!(
            ClaudeProvider
                .decode("not JSON")
                .unwrap_err()
                .contains("Could not decode")
        );
    }
}
