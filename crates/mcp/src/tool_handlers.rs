pub fn format_tool_result(result: serde_json::Value) -> String {
    let is_error = result
        .get("isError")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let text = result
        .get("content")
        .and_then(|content| content.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(|text| text.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| result.to_string());

    if is_error && !text.starts_with("Error: ") {
        format!("Error: {text}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::format_tool_result;
    use serde_json::json;

    #[test]
    fn extracts_text_content_items() {
        let result = json!({
            "content": [
                { "type": "text", "text": "first" },
                { "type": "text", "text": "second" }
            ]
        });

        assert_eq!(format_tool_result(result), "first\nsecond");
    }

    #[test]
    fn prefixes_error_content() {
        let result = json!({
            "isError": true,
            "content": [
                { "type": "text", "text": "failed" }
            ]
        });

        assert_eq!(format_tool_result(result), "Error: failed");
    }
}
