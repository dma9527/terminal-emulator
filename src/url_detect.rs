/// Clickable URL detection in terminal grid.

use crate::core::Grid;
use regex::Regex;
use std::sync::LazyLock;

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://[^\s<>\[\]{}|\\^`\x00-\x1f]+").unwrap()
});

#[derive(Debug, Clone, PartialEq)]
pub struct UrlMatch {
    pub row: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub url: String,
}

/// Detect URLs in a single grid row.
fn detect_row(grid: &Grid, row: usize) -> Vec<UrlMatch> {
    let cols = grid.cols();
    let mut text = String::with_capacity(cols);
    for c in 0..cols {
        let ch = grid.cell(row, c).ch;
        text.push(if ch == '\0' { ' ' } else { ch });
    }

    URL_RE.find_iter(&text).map(|m| {
        let url = m.as_str().trim_end_matches(|c: char| ".,;:!?)\"'".contains(c));
        UrlMatch {
            row,
            col_start: m.start(),
            col_end: m.start() + url.len(),
            url: url.to_string(),
        }
    }).collect()
}

/// Detect all URLs in the visible grid.
pub fn detect_urls(grid: &Grid) -> Vec<UrlMatch> {
    (0..grid.rows()).flat_map(|r| detect_row(grid, r)).collect()
}

/// Check if a position (row, col) is inside a URL. Returns the URL if so.
pub fn url_at(grid: &Grid, row: usize, col: usize) -> Option<String> {
    detect_row(grid, row).into_iter()
        .find(|m| col >= m.col_start && col < m.col_end)
        .map(|m| m.url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Terminal, VtParser};

    #[test]
    fn test_detect_url() {
        let mut t = Terminal::new(60, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"visit https://example.com for info");
        let urls = detect_urls(&t.grid);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[0].col_start, 6);
    }

    #[test]
    fn test_detect_multiple_urls() {
        let mut t = Terminal::new(80, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"see https://a.com and http://b.org/path?q=1");
        let urls = detect_urls(&t.grid);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].url, "https://a.com");
        assert_eq!(urls[1].url, "http://b.org/path?q=1");
    }

    #[test]
    fn test_url_strips_trailing_punct() {
        let mut t = Terminal::new(60, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"(see https://example.com).");
        let urls = detect_urls(&t.grid);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn test_no_urls() {
        let mut t = Terminal::new(40, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"no urls here just text");
        assert!(detect_urls(&t.grid).is_empty());
    }

    #[test]
    fn test_url_at_position() {
        let mut t = Terminal::new(60, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"click https://example.com here");
        assert_eq!(url_at(&t.grid, 0, 10), Some("https://example.com".into()));
        assert_eq!(url_at(&t.grid, 0, 0), None);
    }
}
