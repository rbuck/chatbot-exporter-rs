pub fn default_output_name(title: Option<&str>) -> String {
    let mut slug = String::new();
    for character in title.unwrap_or_default().to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    format!(
        "{}.md",
        if slug.is_empty() {
            "conversation"
        } else {
            slug
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_ascii_slugs_and_safe_fallbacks() {
        assert_eq!(default_output_name(Some("Hello, World!")), "hello-world.md");
        assert_eq!(default_output_name(Some("A / B: C")), "a-b-c.md");
        assert_eq!(default_output_name(Some("日本語")), "conversation.md");
        assert_eq!(default_output_name(None), "conversation.md");
    }
}
