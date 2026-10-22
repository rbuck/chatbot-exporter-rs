use crate::canonical_source_url;
use serde_json::Value;
use std::collections::HashSet;
use url::Url;

const HIDDEN_CHANNELS: [&str; 5] = [
    "analysis",
    "justify",
    "confidence",
    "delegate_summary",
    "juicepred",
];

pub fn conversation_markdown(conversation: &Value, source_url: &Url) -> Result<String, String> {
    let mapping = conversation
        .get("mapping")
        .and_then(Value::as_object)
        .ok_or_else(|| "The share page did not contain a complete conversation.".to_owned())?;
    let mut node_id = conversation
        .get("current_node")
        .and_then(Value::as_str)
        .ok_or_else(|| "The share page did not contain a complete conversation.".to_owned())?;
    let mut path = Vec::new();
    let mut visited = HashSet::new();

    loop {
        if !visited.insert(node_id.to_owned()) {
            return Err("The conversation contains a circular message history.".to_owned());
        }
        let node = mapping
            .get(node_id)
            .and_then(Value::as_object)
            .ok_or_else(|| "The conversation contains a missing message.".to_owned())?;
        path.push(node);
        match node.get("parent").and_then(Value::as_str) {
            Some(parent) => node_id = parent,
            None => break,
        }
    }
    path.reverse();

    let title = conversation
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .unwrap_or("ChatGPT conversation")
        .trim()
        .replace(['\r', '\n'], " ");
    let mut output = format!(
        "# {title}\n\nSource: {}\n\n",
        canonical_source_url(source_url)
    );
    let mut exported_messages = 0usize;

    for node in path {
        let Some(message) = node.get("message").and_then(Value::as_object) else {
            continue;
        };
        let Some(author) = message.get("author").and_then(Value::as_object) else {
            continue;
        };
        let Some(role) = author.get("role").and_then(Value::as_str) else {
            continue;
        };
        let label = match role {
            "user" => "You",
            "assistant" => "ChatGPT",
            _ => continue,
        };

        let metadata = message.get("metadata").and_then(Value::as_object);
        if metadata
            .and_then(|metadata| metadata.get("channel"))
            .and_then(Value::as_str)
            .is_some_and(|channel| HIDDEN_CHANNELS.contains(&channel))
        {
            continue;
        }

        let text = message_text(message.get("content"));
        let text = text.trim();
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

fn message_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(object)) => {
            if let Some(parts) = object.get("parts").and_then(Value::as_array) {
                return parts.iter().map(message_part_text).collect();
            }
            object
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        }
        _ => String::new(),
    }
}

fn message_part_text(part: &Value) -> String {
    if let Some(text) = part.as_str() {
        return text.to_owned();
    }
    let Some(object) = part.as_object() else {
        return String::new();
    };
    if let Some(text) = object.get("text").and_then(Value::as_str) {
        return text.to_owned();
    }
    let content_type = object
        .get("content_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if content_type.contains("image") {
        "\n[Image attachment]\n".to_owned()
    } else if content_type.contains("audio") {
        "\n[Audio attachment]\n".to_owned()
    } else if content_type.contains("video") {
        "\n[Video attachment]\n".to_owned()
    } else {
        "\n[Attachment omitted]\n".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    use url::Url;

    fn share() -> Url {
        Url::parse("https://chatgpt.com/share/test-id").unwrap()
    }

    fn conversation(messages: Vec<Value>) -> Value {
        let mut mapping = Map::new();
        let mut parent: Option<String> = None;
        let mut current_node = String::new();
        for (index, message) in messages.into_iter().enumerate() {
            let id = format!("node-{index}");
            let mut node = Map::new();
            if let Some(parent_id) = parent {
                node.insert("parent".to_owned(), Value::String(parent_id));
            }
            node.insert("message".to_owned(), message);
            mapping.insert(id.clone(), Value::Object(node));
            parent = Some(id.clone());
            current_node = id;
        }
        serde_json::json!({
            "title": "  A useful title\nwith lines ",
            "mapping": mapping,
            "current_node": current_node
        })
    }

    fn message(role: &str, content: Value) -> Value {
        serde_json::json!({
            "author": {"role": role},
            "content": content
        })
    }

    #[test]
    fn renders_ordered_visible_messages_and_canonical_source() {
        let sample = conversation(vec![
            message(
                "user",
                serde_json::json!({"parts": [" Hi ", {"content_type":"image"}]}),
            ),
            message("assistant", serde_json::json!({"text": "Hello"})),
            serde_json::json!({
                "author": {"role": "assistant"},
                "metadata": {"channel": "analysis"},
                "content": {"parts": ["hidden"]}
            }),
            message("system", serde_json::json!("not exported")),
        ]);
        let output = conversation_markdown(&sample, &share()).unwrap();
        assert_eq!(
            output,
            "# A useful title with lines\n\nSource: https://chatgpt.com/share/test-id\n\n## You\n\nHi \n[Image attachment]\n\n## ChatGPT\n\nHello\n"
        );
    }

    #[test]
    fn handles_attachment_types_and_empty_content() {
        let content = serde_json::json!({
            "parts": [
                {"content_type":"audio"},
                {"content_type":"video"},
                {"content_type":"file"},
                42
            ]
        });
        let sample = conversation(vec![message("user", content)]);
        let markdown = conversation_markdown(&sample, &share()).unwrap();
        assert!(markdown.contains("[Audio attachment]"));
        assert!(markdown.contains("[Video attachment]"));
        assert!(markdown.contains("[Attachment omitted]"));
        assert!(
            conversation_markdown(
                &conversation(vec![message("assistant", serde_json::json!("  "))]),
                &share()
            )
            .unwrap_err()
            .contains("No readable")
        );
    }

    #[test]
    fn rejects_broken_and_circular_message_paths() {
        let missing = serde_json::json!({"mapping": {}, "current_node": "missing"});
        assert!(conversation_markdown(&missing, &share()).is_err());

        let circular = serde_json::json!({
            "mapping": {
                "a": {"parent": "b"},
                "b": {"parent": "a"}
            },
            "current_node": "a"
        });
        assert!(
            conversation_markdown(&circular, &share())
                .unwrap_err()
                .contains("circular")
        );
    }
}
