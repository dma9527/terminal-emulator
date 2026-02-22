/// Built-in multiplexer: manage multiple terminal panes in a single window.
/// Replaces tmux for basic split-pane use cases.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub struct Pane {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub active: bool,
}

/// Layout tree node.
#[derive(Debug)]
pub enum LayoutNode {
    Leaf(Pane),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

pub struct PaneManager {
    root: LayoutNode,
    next_id: u32,
    active_id: u32,
}

impl PaneManager {
    pub fn new() -> Self {
        let pane = Pane { id: 1, x: 0.0, y: 0.0, width: 1.0, height: 1.0, active: true };
        Self {
            root: LayoutNode::Leaf(pane),
            next_id: 2,
            active_id: 1,
        }
    }

    /// Split the active pane.
    pub fn split(&mut self, direction: SplitDirection) -> u32 {
        let new_id = self.next_id;
        self.next_id += 1;
        self.root = split_node(std::mem::replace(&mut self.root,
            LayoutNode::Leaf(Pane { id: 0, x: 0.0, y: 0.0, width: 0.0, height: 0.0, active: false })),
            self.active_id, new_id, direction);
        self.active_id = new_id;
        new_id
    }

    /// Get all panes with their computed bounds.
    pub fn panes(&self) -> Vec<Pane> {
        let mut result = Vec::new();
        collect_panes(&self.root, 0.0, 0.0, 1.0, 1.0, &mut result);
        result
    }

    /// Focus a specific pane.
    pub fn focus(&mut self, id: u32) {
        self.active_id = id;
    }

    /// Get active pane ID.
    pub fn active(&self) -> u32 { self.active_id }

    /// Count total panes.
    pub fn count(&self) -> usize { self.panes().len() }
}

fn split_node(node: LayoutNode, target_id: u32, new_id: u32, direction: SplitDirection) -> LayoutNode {
    match node {
        LayoutNode::Leaf(pane) if pane.id == target_id => {
            let mut first_pane = pane.clone();
            first_pane.active = false;
            let second_pane = Pane { id: new_id, x: 0.0, y: 0.0, width: 0.0, height: 0.0, active: true };
            LayoutNode::Split {
                direction,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf(first_pane)),
                second: Box::new(LayoutNode::Leaf(second_pane)),
            }
        }
        LayoutNode::Split { direction: d, ratio, first, second } => {
            LayoutNode::Split {
                direction: d, ratio,
                first: Box::new(split_node(*first, target_id, new_id, direction)),
                second: Box::new(split_node(*second, target_id, new_id, direction)),
            }
        }
        other => other,
    }
}

fn collect_panes(node: &LayoutNode, x: f32, y: f32, w: f32, h: f32, out: &mut Vec<Pane>) {
    match node {
        LayoutNode::Leaf(pane) => {
            out.push(Pane { id: pane.id, x, y, width: w, height: h, active: pane.active });
        }
        LayoutNode::Split { direction, ratio, first, second } => {
            match direction {
                SplitDirection::Vertical => {
                    let w1 = w * ratio;
                    collect_panes(first, x, y, w1, h, out);
                    collect_panes(second, x + w1, y, w - w1, h, out);
                }
                SplitDirection::Horizontal => {
                    let h1 = h * ratio;
                    collect_panes(first, x, y, w, h1, out);
                    collect_panes(second, x, y + h1, w, h - h1, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_single_pane() {
        let mgr = PaneManager::new();
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.active(), 1);
        let panes = mgr.panes();
        assert_eq!(panes[0].width, 1.0);
        assert_eq!(panes[0].height, 1.0);
    }

    #[test]
    fn test_vertical_split() {
        let mut mgr = PaneManager::new();
        let new_id = mgr.split(SplitDirection::Vertical);
        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.active(), new_id);
        let panes = mgr.panes();
        assert!((panes[0].width - 0.5).abs() < 0.01);
        assert!((panes[1].width - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_horizontal_split() {
        let mut mgr = PaneManager::new();
        mgr.split(SplitDirection::Horizontal);
        let panes = mgr.panes();
        assert_eq!(panes.len(), 2);
        assert!((panes[0].height - 0.5).abs() < 0.01);
        assert!((panes[1].height - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_nested_split() {
        let mut mgr = PaneManager::new();
        mgr.split(SplitDirection::Vertical);
        mgr.split(SplitDirection::Horizontal);
        assert_eq!(mgr.count(), 3);
    }

    #[test]
    fn test_focus() {
        let mut mgr = PaneManager::new();
        mgr.split(SplitDirection::Vertical);
        mgr.focus(1);
        assert_eq!(mgr.active(), 1);
    }

    #[test]
    fn test_pane_bounds_sum_to_one() {
        let mut mgr = PaneManager::new();
        mgr.split(SplitDirection::Vertical);
        let panes = mgr.panes();
        let total_width: f32 = panes.iter().map(|p| p.width).sum();
        assert!((total_width - 1.0).abs() < 0.01);
    }
}
