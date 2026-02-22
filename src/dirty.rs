/// Dirty region tracking for incremental rendering.
/// Instead of redrawing the entire grid every frame, only redraw changed rows.

pub struct DirtyTracker {
    dirty: Vec<bool>,
    all_dirty: bool,
}

impl DirtyTracker {
    pub fn new(rows: usize) -> Self {
        Self {
            dirty: vec![true; rows],
            all_dirty: true,
        }
    }

    /// Mark a specific row as dirty.
    pub fn mark_row(&mut self, row: usize) {
        if row < self.dirty.len() {
            self.dirty[row] = true;
        }
    }

    /// Mark a range of rows as dirty.
    pub fn mark_range(&mut self, start: usize, end: usize) {
        for r in start..end.min(self.dirty.len()) {
            self.dirty[r] = true;
        }
    }

    /// Mark everything dirty (e.g. after resize or clear).
    pub fn mark_all(&mut self) {
        self.all_dirty = true;
        self.dirty.fill(true);
    }

    /// Check if a row needs redrawing.
    pub fn is_dirty(&self, row: usize) -> bool {
        self.all_dirty || self.dirty.get(row).copied().unwrap_or(false)
    }

    /// Check if anything needs redrawing.
    pub fn has_dirty(&self) -> bool {
        self.all_dirty || self.dirty.iter().any(|&d| d)
    }

    /// Clear all dirty flags after rendering.
    pub fn clear(&mut self) {
        self.all_dirty = false;
        self.dirty.fill(false);
    }

    /// Resize tracker (marks all dirty).
    pub fn resize(&mut self, rows: usize) {
        self.dirty = vec![true; rows];
        self.all_dirty = true;
    }

    /// Count of dirty rows.
    pub fn dirty_count(&self) -> usize {
        if self.all_dirty { self.dirty.len() }
        else { self.dirty.iter().filter(|&&d| d).count() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_all_dirty() {
        let dt = DirtyTracker::new(24);
        assert!(dt.has_dirty());
        assert!(dt.is_dirty(0));
        assert_eq!(dt.dirty_count(), 24);
    }

    #[test]
    fn test_clear_then_mark() {
        let mut dt = DirtyTracker::new(10);
        dt.clear();
        assert!(!dt.has_dirty());
        assert!(!dt.is_dirty(0));
        assert_eq!(dt.dirty_count(), 0);

        dt.mark_row(3);
        assert!(dt.has_dirty());
        assert!(dt.is_dirty(3));
        assert!(!dt.is_dirty(4));
        assert_eq!(dt.dirty_count(), 1);
    }

    #[test]
    fn test_mark_range() {
        let mut dt = DirtyTracker::new(10);
        dt.clear();
        dt.mark_range(2, 5);
        assert!(dt.is_dirty(2));
        assert!(dt.is_dirty(4));
        assert!(!dt.is_dirty(5));
        assert_eq!(dt.dirty_count(), 3);
    }

    #[test]
    fn test_resize() {
        let mut dt = DirtyTracker::new(10);
        dt.clear();
        dt.resize(20);
        assert!(dt.has_dirty());
        assert_eq!(dt.dirty_count(), 20);
    }

    #[test]
    fn test_mark_all() {
        let mut dt = DirtyTracker::new(10);
        dt.clear();
        dt.mark_all();
        assert_eq!(dt.dirty_count(), 10);
        assert!(dt.is_dirty(9));
    }
}
