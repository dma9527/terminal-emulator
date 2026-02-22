/// Security utilities: input sanitization, dependency audit helpers.

/// Sanitize OSC data to prevent injection attacks.
pub fn sanitize_osc(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data);
    s.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .take(4096) // limit title length
        .collect()
}

/// Sanitize paste input — strip dangerous control sequences.
pub fn sanitize_paste(input: &str) -> String {
    input.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}

/// Check if a URL is safe to open (no javascript:, data:, etc).
pub fn is_safe_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_osc_strips_control() {
        let data = b"Hello\x07World\x1bBad";
        let result = sanitize_osc(data);
        assert_eq!(result, "HelloWorldBad");
    }

    #[test]
    fn test_sanitize_osc_length_limit() {
        let data = vec![b'A'; 10_000];
        let result = sanitize_osc(&data);
        assert_eq!(result.len(), 4096);
    }

    #[test]
    fn test_sanitize_paste() {
        let input = "hello\nworld\x1b[31mred";
        let result = sanitize_paste(input);
        assert_eq!(result, "hello\nworld[31mred");
    }

    #[test]
    fn test_safe_url() {
        assert!(is_safe_url("https://example.com"));
        assert!(is_safe_url("http://example.com"));
        assert!(is_safe_url("HTTPS://EXAMPLE.COM"));
        assert!(!is_safe_url("javascript:alert(1)"));
        assert!(!is_safe_url("data:text/html,<h1>hi</h1>"));
        assert!(!is_safe_url("file:///etc/passwd"));
    }
}
