/// Scrollback search: find text/regex in terminal grid + scrollback.

use crate::core::Grid;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    /// Row index (negative = scrollback, 0+ = visible grid)
    pub row: i32,
    pub col_start: usize,
    pub col_end: usize,
}

/// Extract text content from a grid row.
fn row_text(grid: &Grid, row: usize) -> String {
    let cols = grid.cols();
    let mut s = String::with_capacity(cols);
    for c in 0..cols {
        let ch = grid.cell(row, c).ch;
        s.push(if ch == '\0' { ' ' } else { ch });
    }
    // Trim trailing spaces
    s.truncate(s.trim_end().len());
    s
}

/// Search visible grid for a pattern. Returns matches sorted top-to-bottom.
pub fn search_grid(grid: &Grid, pattern: &str, use_regex: bool) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    let re = if use_regex {
        regex::Regex::new(pattern).ok()
    } else {
        regex::Regex::new(&regex::escape(pattern)).ok()
    };
    let Some(re) = re else { return matches };

    for row in 0..grid.rows() {
        let text = row_text(grid, row);
        for m in re.find_iter(&text) {
            matches.push(SearchMatch {
                row: row as i32,
                col_start: m.start(),
                col_end: m.end(),
            });
        }
    }
    matches
}

/// Search scrollback buffer. Row indices are negative (most recent = -1).
pub fn search_scrollback(grid: &Grid, pattern: &str, use_regex: bool) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    let re = if use_regex {
        regex::Regex::new(pattern).ok()
    } else {
        regex::Regex::new(&regex::escape(pattern)).ok()
    };
    let Some(re) = re else { return matches };

    let scrollback = grid.scrollback();
    let len = scrollback.len();
    for (i, row_cells) in scrollback.iter().enumerate() {
        let mut text = String::new();
        for cell in row_cells {
            let ch = cell.ch;
            text.push(if ch == '\0' { ' ' } else { ch });
        }
        text.truncate(text.trim_end().len());
        for m in re.find_iter(&text) {
            matches.push(SearchMatch {
                row: -(len as i32 - i as i32),
                col_start: m.start(),
                col_end: m.end(),
            });
        }
    }
    matches
}

/// Search both scrollback and visible grid.
pub fn search_all(grid: &Grid, pattern: &str, use_regex: bool) -> Vec<SearchMatch> {
    let mut results = search_scrollback(grid, pattern, use_regex);
    results.extend(search_grid(grid, pattern, use_regex));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Terminal, VtParser};

    #[test]
    fn test_search_literal() {
        let mut t = Terminal::new(20, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"hello world\r\nfoo bar\r\nhello again");
        let matches = search_grid(&t.grid, "hello", false);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], SearchMatch { row: 0, col_start: 0, col_end: 5 });
        assert_eq!(matches[1], SearchMatch { row: 2, col_start: 0, col_end: 5 });
    }

    #[test]
    fn test_search_regex() {
        let mut t = Terminal::new(40, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"error: file not found\r\nwarning: deprecated\r\nerror: timeout");
        let matches = search_grid(&t.grid, r"error: \w+", true);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].row, 0);
        assert_eq!(matches[1].row, 2);
    }

    #[test]
    fn test_search_no_match() {
        let mut t = Terminal::new(20, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"hello world");
        let matches = search_grid(&t.grid, "xyz", false);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_search_invalid_regex() {
        let mut t = Terminal::new(20, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"hello");
        let matches = search_grid(&t.grid, "[invalid", true);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_search_case_sensitive() {
        let mut t = Terminal::new(20, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"Hello HELLO hello");
        let matches = search_grid(&t.grid, "hello", false);
        assert_eq!(matches.len(), 1); // only lowercase
    }

    #[test]
    fn test_search_case_insensitive_regex() {
        let mut t = Terminal::new(20, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"Hello HELLO hello");
        let matches = search_grid(&t.grid, "(?i)hello", true);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_search_all_includes_scrollback() {
        let mut t = Terminal::new(10, 3);
        let mut p = VtParser::new();
        // Fill enough lines to push into scrollback
        t.feed_bytes(&mut p, b"line1 abc\r\nline2 abc\r\nline3 abc\r\nline4 abc\r\nline5 abc");
        let matches = search_all(&t.grid, "abc", false);
        // Should find in both scrollback and visible
        assert!(matches.len() >= 3);
        // Scrollback matches have negative row
        assert!(matches.iter().any(|m| m.row < 0));
    }
}
