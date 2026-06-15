//! Geometry primitives shared across all phases.
//!
//! These types are deliberately framework-free — they describe the
//! invisible integer grid the layout, routing, and renderer all reason
//! about. No serialization, no theming, no I/O.

// ---------------------------------------------------------------------------
// Geometry primitives (integer grid — final output may scale)
// ---------------------------------------------------------------------------

/// Position on the invisible geometric grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Pixel dimensions of a node's rendered rectangle.
#[derive(Debug, Clone, Copy)]
pub struct NodeSize {
    pub width: i32,
    pub height: i32,
}

impl NodeSize {
    /// Compute node dimensions from measured text extents plus padding.
    pub fn from_extents(extents: &TextExtents, padding: i32) -> Self {
        let w = (extents.width as i32 + padding).max(MIN_NODE_SIDE);
        let h = (extents.height as i32 + padding).max(MIN_NODE_SIDE);
        // Proportional but minimum: make it at least wide enough to be comfortable
        let w = w.max(h);
        Self { width: w, height: h }
    }

    /// Half-width (radius-like offset from centre to left/right edge).
    pub fn half_w(&self) -> i32 {
        self.width / 2
    }

    /// Half-height.
    pub fn half_h(&self) -> i32 {
        self.height / 2
    }
}

/// Minimum side length for a node in pixels.
pub const MIN_NODE_SIDE: i32 = 60;

/// Padding added around the label text to compute node size.
pub const NODE_PADDING: i32 = 24;

/// Viewport constraint — the backbone layout attempts to fit within this.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
        }
    }
}

/// Anchor slot around a node for label placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnchorSlot {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Resolved anchor for an edge label: the SVG `<text>` position and
/// `text-anchor` value. Filled by the routing phase so the same pass can
/// reserve the label's bounding box on the occupancy grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeLabelAnchor {
    pub x: i32,
    pub y: i32,
    /// SVG `text-anchor` value: `"start"`, `"middle"`, or `"end"`.
    pub anchor: &'static str,
}

// ---------------------------------------------------------------------------
// Text measurement results
// ---------------------------------------------------------------------------

/// Measured dimensions of a text label.
#[derive(Debug, Clone, Copy)]
pub struct TextExtents {
    pub width: f32,
    pub height: f32,
}

// ---------------------------------------------------------------------------
// Port directions on a node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    East,
    West,
    North,
    South,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_new() {
        let p = Point::new(42, -7);
        assert_eq!(p.x, 42);
        assert_eq!(p.y, -7);
    }

    #[test]
    fn test_node_size_extents() {
        let e = TextExtents { width: 80.0, height: 32.0 };
        let ns = NodeSize::from_extents(&e, 24);
        assert!(ns.width >= 104);
        assert!(ns.height >= 56);
    }

    #[test]
    fn test_node_size_minimum() {
        let e = TextExtents { width: 10.0, height: 10.0 };
        let ns = NodeSize::from_extents(&e, 24);
        assert!(ns.width >= MIN_NODE_SIDE);
        assert!(ns.height >= MIN_NODE_SIDE);
    }

    #[test]
    fn test_default_viewport() {
        let v: Viewport = Default::default();
        assert_eq!(v.width, 1200);
        assert_eq!(v.height, 800);
    }
}
