/// Inline image support: Kitty graphics protocol (basic).
/// Handles image placement metadata; actual rendering is platform-specific.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ImagePlacement {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub row: usize,
    pub col: usize,
    pub data: Vec<u8>, // raw RGBA pixels
}

/// Image manager: stores placed images for rendering.
pub struct ImageManager {
    images: HashMap<u32, ImagePlacement>,
    next_id: u32,
}

impl ImageManager {
    pub fn new() -> Self {
        Self { images: HashMap::new(), next_id: 1 }
    }

    /// Place an image at the given grid position.
    pub fn place(&mut self, width: u32, height: u32, row: usize, col: usize, data: Vec<u8>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.images.insert(id, ImagePlacement { id, width, height, row, col, data });
        id
    }

    /// Remove an image by ID.
    pub fn remove(&mut self, id: u32) -> bool {
        self.images.remove(&id).is_some()
    }

    /// Get all visible images (for rendering).
    pub fn visible(&self, scroll_top: usize, scroll_bottom: usize) -> Vec<&ImagePlacement> {
        self.images.values()
            .filter(|img| img.row >= scroll_top && img.row <= scroll_bottom)
            .collect()
    }

    /// Clear all images.
    pub fn clear(&mut self) {
        self.images.clear();
    }

    pub fn count(&self) -> usize { self.images.len() }
}

/// Parse Kitty graphics protocol APC sequence.
/// Format: `\x1b_Gkey=value,key=value;BASE64_DATA\x1b\\`
pub fn parse_kitty_graphics(payload: &str) -> Option<KittyCommand> {
    let (params_str, _data) = payload.split_once(';').unwrap_or((payload, ""));
    let mut params = HashMap::new();
    for kv in params_str.split(',') {
        if let Some((k, v)) = kv.split_once('=') {
            params.insert(k.to_string(), v.to_string());
        }
    }
    let action = params.get("a").map(|s| s.as_str()).unwrap_or("t");
    match action {
        "t" | "T" => Some(KittyCommand::Transmit),
        "p" => Some(KittyCommand::Place),
        "d" => Some(KittyCommand::Delete),
        "q" => Some(KittyCommand::Query),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum KittyCommand {
    Transmit,
    Place,
    Delete,
    Query,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_place_and_remove() {
        let mut mgr = ImageManager::new();
        let id = mgr.place(100, 50, 5, 0, vec![0u8; 100 * 50 * 4]);
        assert_eq!(mgr.count(), 1);
        assert!(mgr.remove(id));
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_image_visible() {
        let mut mgr = ImageManager::new();
        mgr.place(10, 10, 2, 0, vec![]);
        mgr.place(10, 10, 50, 0, vec![]);
        let visible = mgr.visible(0, 24);
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn test_image_clear() {
        let mut mgr = ImageManager::new();
        mgr.place(10, 10, 0, 0, vec![]);
        mgr.place(10, 10, 1, 0, vec![]);
        mgr.clear();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_parse_kitty_transmit() {
        assert_eq!(parse_kitty_graphics("a=t,f=100"), Some(KittyCommand::Transmit));
    }

    #[test]
    fn test_parse_kitty_delete() {
        assert_eq!(parse_kitty_graphics("a=d"), Some(KittyCommand::Delete));
    }

    #[test]
    fn test_parse_kitty_query() {
        assert_eq!(parse_kitty_graphics("a=q,i=1"), Some(KittyCommand::Query));
    }

    #[test]
    fn test_parse_kitty_default_transmit() {
        // No 'a' param defaults to transmit
        assert_eq!(parse_kitty_graphics("f=100,s=10"), Some(KittyCommand::Transmit));
    }
}
