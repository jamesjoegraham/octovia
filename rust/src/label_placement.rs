//! Phase 5 — Label Placement.
//!
//! Owns the **decision** of where edge labels go, separate from the SVG
//! formatting concerns in [`crate::svg_output`]. Today the heuristic is
//! simple (perpendicular offset from the route midpoint); the eventual
//! 8-slot anchor system documented in `ARCHITECTURE.md` will live here
//! too without disturbing the renderer.
//!
//! Node labels are trivial (always centred in the node rect) and remain
//! inlined in the SVG layer.
//!
//! # Public surface
//! - [`EdgeLabelAnchor`] — placement result: `(x, y, text-anchor)`.
//! - [`place_edge_label`] — heuristic placement for a routed edge.

use crate::ast::Point;

/// Placement result for an edge label: where to anchor the SVG `<text>`
/// element and which `text-anchor` value to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeLabelAnchor {
    pub x: i32,
    pub y: i32,
    /// SVG `text-anchor` value: `"start"`, `"middle"`, or `"end"`.
    pub anchor: &'static str,
}

/// Compute an edge label anchor for a routed edge.
///
/// Strategy: take the route midpoint and offset perpendicular to the
/// local segment direction so the label never sits on top of the line.
/// Horizontal segments → label above; vertical segments → label to the
/// right.
///
/// Returns `None` when the route has fewer than two points (no edge to
/// label).
pub fn place_edge_label(route: &[Point]) -> Option<EdgeLabelAnchor> {
    if route.len() < 2 {
        return None;
    }

    let mid = route.len() / 2;
    let p = route[mid];

    // Look at the segment *around* the midpoint to determine orientation.
    let prev = route[mid.saturating_sub(1)];
    let next_idx = (mid + 1).min(route.len() - 1);
    let next = route[next_idx];
    let dx = (next.x - prev.x).abs();
    let dy = (next.y - prev.y).abs();

    let (x, y, anchor) = if dx >= dy {
        (p.x, p.y - 10, "middle")
    } else {
        (p.x + 10, p.y, "start")
    };

    Some(EdgeLabelAnchor { x, y, anchor })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    #[test]
    fn empty_route_has_no_anchor() {
        assert_eq!(place_edge_label(&[]), None);
        assert_eq!(place_edge_label(&[pt(0, 0)]), None);
    }

    #[test]
    fn horizontal_segment_places_above_midpoint() {
        let route = [pt(0, 50), pt(100, 50), pt(200, 50)];
        let a = place_edge_label(&route).unwrap();
        assert_eq!(a.anchor, "middle");
        assert_eq!(a.x, 100);
        assert_eq!(a.y, 40);
    }

    #[test]
    fn vertical_segment_places_to_right_of_midpoint() {
        let route = [pt(50, 0), pt(50, 100), pt(50, 200)];
        let a = place_edge_label(&route).unwrap();
        assert_eq!(a.anchor, "start");
        assert_eq!(a.x, 60);
        assert_eq!(a.y, 100);
    }
}
