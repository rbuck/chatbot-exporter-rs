use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

fn script_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?is)<script\b[^>]*>(.*?)</script\s*>").unwrap())
}

fn enqueue_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r#"streamController\.enqueue\(("(?:\\.|[^"\\])*")\)"#).unwrap())
}

pub(super) fn decode_share_payload(html: &str) -> Result<Value, String> {
    let mut parsed_array = None;
    for script in script_pattern().captures_iter(html) {
        let Some(body) = script.get(1) else {
            continue;
        };
        for chunk in enqueue_pattern().captures_iter(body.as_str()) {
            let Some(encoded_chunk) = chunk.get(1) else {
                continue;
            };
            let Ok(chunk_text) = serde_json::from_str::<String>(encoded_chunk.as_str()) else {
                return Err("Could not decode conversation data from the share page.".to_owned());
            };
            let trimmed = chunk_text.trim();
            if !trimmed.starts_with('[') {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(trimmed)
                && value.is_array()
            {
                parsed_array = Some(value);
                break;
            }
        }
        if parsed_array.is_some() {
            break;
        }
    }

    let Some(flat) = parsed_array else {
        return Err(
            "Could not find conversation data in the share page. ChatGPT may have changed its page format."
                .to_owned(),
        );
    };
    let flat = flat
        .as_array()
        .ok_or_else(|| "The share page contains an invalid conversation payload.".to_owned())?;
    decode_node(
        0,
        flat,
        &mut vec![None; flat.len()],
        &mut vec![false; flat.len()],
    )
}

fn decode_node(
    index: usize,
    flat: &[Value],
    cache: &mut [Option<Value>],
    active: &mut [bool],
) -> Result<Value, String> {
    if index >= flat.len() {
        return Err("The share page contains an invalid conversation payload.".to_owned());
    }
    if let Some(value) = &cache[index] {
        return Ok(value.clone());
    }
    if active[index] {
        return Err("The share page contains a circular conversation payload.".to_owned());
    }
    active[index] = true;

    let decoded = match &flat[index] {
        Value::Array(values)
            if values.len() == 2
                && values[0] == Value::String("P".to_owned())
                && values[1].as_i64() == i64::try_from(index).ok() =>
        {
            // Pending promises are self-referential markers and are not conversation data.
            Value::Null
        }
        Value::Array(values) => {
            let mut result = Vec::with_capacity(values.len());
            for value in values {
                result.push(resolve(value, flat, cache, active)?);
            }
            Value::Array(result)
        }
        Value::Object(values) => {
            let mut result = Map::new();
            for (encoded_key, encoded_value) in values {
                let key_index = encoded_key
                    .strip_prefix('_')
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(|| {
                        "The share page contains an unsupported data format.".to_owned()
                    })?;
                let key = flat.get(key_index).and_then(Value::as_str).ok_or_else(|| {
                    "The share page contains an unsupported data format.".to_owned()
                })?;
                result.insert(key.to_owned(), resolve(encoded_value, flat, cache, active)?);
            }
            Value::Object(result)
        }
        value => value.clone(),
    };

    active[index] = false;
    cache[index] = Some(decoded.clone());
    Ok(decoded)
}

fn resolve(
    value: &Value,
    flat: &[Value],
    cache: &mut [Option<Value>],
    active: &mut [bool],
) -> Result<Value, String> {
    let Some(index) = value.as_i64() else {
        return Ok(value.clone());
    };
    if index < 0 {
        return Ok(Value::Null);
    }
    decode_node(index as usize, flat, cache, active)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_flattened_references_and_object_keys() {
        let flat = vec![
            serde_json::json!({"_1": 2, "_3": 4}),
            serde_json::json!("first"),
            serde_json::json!([5]),
            serde_json::json!("second"),
            serde_json::json!({"_6": 7}),
            serde_json::json!("value"),
            serde_json::json!("answer"),
            serde_json::json!(42),
        ];
        assert_eq!(
            decode_node(
                0,
                &flat,
                &mut vec![None; flat.len()],
                &mut vec![false; flat.len()]
            )
            .unwrap(),
            serde_json::json!({"first": ["value"], "second": {"answer": 42}})
        );
    }

    #[test]
    fn rejects_invalid_keys_and_cycles() {
        let invalid_key = vec![serde_json::json!({"bad": 1}), serde_json::json!("value")];
        assert!(
            decode_node(
                0,
                &invalid_key,
                &mut vec![None; invalid_key.len()],
                &mut vec![false; invalid_key.len()]
            )
            .is_err()
        );

        let cycle = vec![serde_json::json!([0])];
        assert!(decode_node(0, &cycle, &mut [None], &mut [false]).is_err());
    }

    #[test]
    fn ignores_self_referential_pending_promise_markers() {
        let flat = vec![
            serde_json::json!({"_1": 2}),
            serde_json::json!("statsigGateEvaluationsPromise"),
            serde_json::json!(["P", 2]),
        ];
        assert_eq!(
            decode_node(
                0,
                &flat,
                &mut vec![None; flat.len()],
                &mut vec![false; flat.len()]
            )
            .unwrap(),
            serde_json::json!({"statsigGateEvaluationsPromise": null})
        );
    }

    #[test]
    fn reads_serialized_stream_chunks_from_html() {
        let html = r#"<script>streamController.enqueue("not-data");
            streamController.enqueue("[{\"_1\":2},\"loaderData\",{}]");</script>"#;
        assert_eq!(
            decode_share_payload(html).unwrap(),
            serde_json::json!({"loaderData": {}})
        );
        assert!(decode_share_payload("<html></html>").is_err());
    }
}
