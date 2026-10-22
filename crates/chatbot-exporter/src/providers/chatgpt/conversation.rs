use super::decode::decode_share_payload;
use serde_json::Value;

pub fn extract_conversation(html: &str) -> Result<Value, String> {
    let page_data = decode_share_payload(html)?;
    let loader_data = page_data
        .get("loaderData")
        .and_then(Value::as_object)
        .ok_or_else(|| "The share page did not contain a conversation.".to_owned())?;

    for route_data in loader_data.values() {
        let Some(response) = route_data.get("serverResponse") else {
            continue;
        };
        let Some(conversation) = response.get("data") else {
            continue;
        };
        if conversation
            .get("mapping")
            .and_then(Value::as_object)
            .is_some()
        {
            return Ok(conversation.clone());
        }
    }

    Err("The share page did not contain a readable conversation.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    fn encode_flat(value: &Value) -> Vec<Value> {
        fn insert(value: &Value, flat: &mut Vec<Value>) -> usize {
            let index = flat.len();
            flat.push(Value::Null);
            let encoded = match value {
                Value::Object(object) => {
                    let mut result = Map::new();
                    for (key, value) in object {
                        let key_index = insert(&Value::String(key.clone()), flat);
                        let value_index = insert(value, flat);
                        result.insert(format!("_{key_index}"), Value::from(value_index));
                    }
                    Value::Object(result)
                }
                Value::Array(values) => {
                    let indices = values
                        .iter()
                        .map(|value| Value::from(insert(value, flat)))
                        .collect();
                    Value::Array(indices)
                }
                value => value.clone(),
            };
            flat[index] = encoded;
            index
        }

        let mut flat = Vec::new();
        insert(value, &mut flat);
        flat
    }

    fn share_html(conversation: Value) -> String {
        let page_data = serde_json::json!({
            "loaderData": {
                "share": {
                    "serverResponse": {
                        "data": conversation
                    }
                }
            }
        });
        let flat = encode_flat(&page_data);
        let chunk = serde_json::to_string(&serde_json::to_string(&flat).unwrap()).unwrap();
        format!("<script>streamController.enqueue({chunk});</script>")
    }

    #[test]
    fn extracts_conversation_from_loader_data() {
        let conversation = serde_json::json!({"mapping": {}, "title": "Sample"});
        assert_eq!(
            extract_conversation(&share_html(conversation.clone())).unwrap(),
            conversation
        );
    }

    #[test]
    fn distinguishes_missing_and_unreadable_conversations() {
        let page_without_loader = format!(
            "<script>streamController.enqueue({});</script>",
            serde_json::to_string(
                &serde_json::to_string(&encode_flat(&serde_json::json!({"other": true}))).unwrap()
            )
            .unwrap()
        );
        assert!(
            extract_conversation(&page_without_loader)
                .unwrap_err()
                .contains("did not contain a conversation")
        );

        let unreadable = serde_json::json!({"title": "No mapping"});
        assert!(
            extract_conversation(&share_html(unreadable))
                .unwrap_err()
                .contains("did not contain a readable conversation")
        );
    }
}
