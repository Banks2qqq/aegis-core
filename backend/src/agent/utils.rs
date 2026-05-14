//! AEGIS utility functions — shared across agent modules.
//!
//! Centralizes duplicated helpers to avoid divergence between implementations.

/// Обрезает строку до `max` байт с добавлением `…` если строка длиннее.
pub fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Безопасная обрезка по границам Unicode
    let mut end = max;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Извлекает первый JSON-объект `{…}` из текста (с поддержкой markdown-fence).
pub fn extract_json_object_owned(text: &str) -> Option<String> {
    extract_json_object(text).map(|s| s.to_string())
}

/// Извлекает первый JSON-объект `{…}` из текста как &str slice.
pub fn extract_json_object(text: &str) -> Option<&str> {
    let s = strip_markdown_fence_ref(text);
    let s = s.trim();
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&s[start..=end])
}

/// Извлекает первый JSON-массив `[…]` из текста.
pub fn extract_json_array_owned(text: &str) -> Option<String> {
    let s = strip_markdown_fence(text);
    let s = s.trim().to_string();
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end <= start {
        return None;
    }
    Some(s[start..=end].to_string())
}

/// Удаляет markdown code-fence (```json ... ```) из текста (возвращает owned String).
pub fn strip_markdown_fence(text: &str) -> String {
    strip_markdown_fence_ref(text).to_string()
}

fn strip_markdown_fence_ref(text: &str) -> &str {
    let s = text.trim();
    if s.starts_with("```") {
        // Пропускаем первую строку (```json или ```)
        let after_fence = s.find('\n').map(|i| &s[i + 1..]).unwrap_or(s);
        // Убираем закрывающий ```
        if let Some(end) = after_fence.rfind("```") {
            return after_fence[..end].trim();
        }
        return after_fence.trim();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_short_string() {
        assert_eq!(clip("hello", 10), "hello");
    }

    #[test]
    fn clip_long_string() {
        let s = "a".repeat(200);
        let c = clip(&s, 100);
        assert!(c.ends_with('…'));
        assert!(c.len() <= 104); // 100 bytes + UTF-8 ellipsis (3 bytes)
    }

    #[test]
    fn extract_json_object_basic() {
        let text = r#"Some text {"key": "value"} more text"#;
        let obj = extract_json_object_owned(text).unwrap();
        assert!(obj.contains("\"key\""));
    }

    #[test]
    fn extract_json_from_markdown_fence() {
        let text = "```json\n{\"verdict\": \"ALLOW\"}\n```";
        let obj = extract_json_object_owned(text).unwrap();
        assert!(obj.contains("ALLOW"));
    }

    #[test]
    fn extract_json_array_basic() {
        let text = r#"result: [{"a": 1}, {"b": 2}]"#;
        let arr = extract_json_array_owned(text).unwrap();
        assert!(arr.starts_with('['));
        assert!(arr.ends_with(']'));
    }

    #[test]
    fn strip_markdown_fence_noop_for_plain() {
        let text = r#"{"key": "val"}"#;
        assert_eq!(strip_markdown_fence(text), text);
    }
}
