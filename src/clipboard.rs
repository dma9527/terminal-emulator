/// Clipboard integration: copy/paste + OSC 52 support.

use std::process::Command;

/// Copy text to system clipboard.
pub fn copy(text: &str) -> bool {
    Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait()
        })
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Read text from system clipboard.
pub fn paste() -> Option<String> {
    Command::new("pbpaste")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Wrap text in bracketed paste escape sequences.
pub fn bracketed_paste_wrap(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 12);
    out.extend_from_slice(b"\x1b[200~");
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}

/// Decode OSC 52 clipboard set: `52;c;BASE64_DATA`
pub fn decode_osc52_set(data: &str) -> Option<String> {
    // Format: "52;c;BASE64" or "52;;BASE64"
    let rest = data.strip_prefix("52;")?;
    let (_target, b64) = rest.split_once(';')?;
    if b64 == "?" {
        return None; // query, not set
    }
    // Decode base64
    base64_decode(b64)
}

fn base64_decode(input: &str) -> Option<String> {
    // Minimal base64 decoder — no external dep
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buf = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for &b in input.as_bytes() {
        if b == b'=' || b == b'\n' || b == b'\r' { continue; }
        let val = table.iter().position(|&t| t == b)? as u32;
        acc = (acc << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            buf.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    String::from_utf8(buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bracketed_paste_wrap() {
        let wrapped = bracketed_paste_wrap("hello");
        assert_eq!(wrapped, b"\x1b[200~hello\x1b[201~");
    }

    #[test]
    fn test_bracketed_paste_empty() {
        let wrapped = bracketed_paste_wrap("");
        assert_eq!(wrapped, b"\x1b[200~\x1b[201~");
    }

    #[test]
    fn test_base64_decode() {
        assert_eq!(base64_decode("aGVsbG8="), Some("hello".into()));
        assert_eq!(base64_decode("d29ybGQ="), Some("world".into()));
        assert_eq!(base64_decode("YQ=="), Some("a".into()));
    }

    #[test]
    fn test_osc52_decode() {
        // "52;c;aGVsbG8=" → "hello"
        assert_eq!(decode_osc52_set("52;c;aGVsbG8="), Some("hello".into()));
        assert_eq!(decode_osc52_set("52;;d29ybGQ="), Some("world".into()));
    }

    #[test]
    fn test_osc52_query() {
        assert_eq!(decode_osc52_set("52;c;?"), None);
    }

    #[test]
    fn test_osc52_invalid() {
        assert_eq!(decode_osc52_set("not_osc52"), None);
    }

    #[test]
    fn test_copy_paste_roundtrip() {
        // This test actually uses the system clipboard
        let test_str = "term_test_clipboard_12345";
        assert!(copy(test_str));
        let pasted = paste();
        assert_eq!(pasted, Some(test_str.into()));
    }
}
