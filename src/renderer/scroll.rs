/// Smooth scrolling: interpolates scroll offset for fluid visual scrolling.

pub struct SmoothScroll {
    /// Current visual offset in pixels (fractional rows)
    pub offset: f32,
    /// Target offset
    target: f32,
    /// Scroll speed (pixels per scroll event)
    pub lines_per_scroll: f32,
    /// Interpolation factor (0..1, higher = snappier)
    pub lerp_factor: f32,
}

impl SmoothScroll {
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            target: 0.0,
            lines_per_scroll: 3.0,
            lerp_factor: 0.3,
        }
    }

    /// Handle a scroll event (positive = scroll up/back, negative = scroll down/forward).
    pub fn scroll(&mut self, delta_lines: f32, cell_height: f32, max_scrollback: usize) {
        self.target += delta_lines * cell_height;
        let max = max_scrollback as f32 * cell_height;
        self.target = self.target.clamp(0.0, max);
    }

    /// Advance animation by one frame. Returns true if still animating.
    pub fn update(&mut self) -> bool {
        let diff = self.target - self.offset;
        if diff.abs() < 0.5 {
            self.offset = self.target;
            return false;
        }
        self.offset += diff * self.lerp_factor;
        true
    }

    /// Reset scroll to bottom (latest output).
    pub fn reset(&mut self) {
        self.target = 0.0;
        self.offset = 0.0;
    }

    /// Returns the number of whole rows scrolled back.
    pub fn scrollback_rows(&self, cell_height: f32) -> usize {
        (self.offset / cell_height).floor() as usize
    }

    /// Returns the sub-pixel offset within the current row.
    pub fn sub_pixel_offset(&self, cell_height: f32) -> f32 {
        self.offset % cell_height
    }

    pub fn is_at_bottom(&self) -> bool {
        self.target <= 0.0
    }
}

impl Default for SmoothScroll {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let s = SmoothScroll::new();
        assert_eq!(s.offset, 0.0);
        assert!(s.is_at_bottom());
        assert_eq!(s.scrollback_rows(16.0), 0);
    }

    #[test]
    fn test_scroll_up() {
        let mut s = SmoothScroll::new();
        s.scroll(3.0, 16.0, 1000); // scroll up 3 lines
        assert!(s.target > 0.0);
        assert!(!s.is_at_bottom());
    }

    #[test]
    fn test_scroll_clamp_max() {
        let mut s = SmoothScroll::new();
        s.scroll(99999.0, 16.0, 100); // way past max
        assert!(s.target <= 100.0 * 16.0);
    }

    #[test]
    fn test_scroll_clamp_min() {
        let mut s = SmoothScroll::new();
        s.scroll(-10.0, 16.0, 100); // scroll past bottom
        assert_eq!(s.target, 0.0);
    }

    #[test]
    fn test_update_converges() {
        let mut s = SmoothScroll::new();
        s.scroll(5.0, 16.0, 1000);
        for _ in 0..100 {
            s.update();
        }
        assert!((s.offset - s.target).abs() < 1.0);
    }

    #[test]
    fn test_reset() {
        let mut s = SmoothScroll::new();
        s.scroll(10.0, 16.0, 1000);
        for _ in 0..50 { s.update(); }
        s.reset();
        assert_eq!(s.offset, 0.0);
        assert!(s.is_at_bottom());
    }

    #[test]
    fn test_scrollback_rows() {
        let mut s = SmoothScroll::new();
        s.offset = 48.0; // 3 rows at 16px
        assert_eq!(s.scrollback_rows(16.0), 3);
        assert_eq!(s.sub_pixel_offset(16.0), 0.0);
    }

    #[test]
    fn test_sub_pixel_offset() {
        let mut s = SmoothScroll::new();
        s.offset = 50.0; // 3 rows + 2px
        assert_eq!(s.scrollback_rows(16.0), 3);
        assert!((s.sub_pixel_offset(16.0) - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_no_animation_when_at_target() {
        let mut s = SmoothScroll::new();
        assert!(!s.update()); // already at target
    }
}
