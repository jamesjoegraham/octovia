//! Grid occupancy tracking — which cells are blocked by nodes, routed
//! edges, or already-placed labels.
//!
//! The router operates on a 10×10 px sub-grid: every diagram coordinate
//! `(x, y)` maps to a cell `(x / 10, y / 10)`. Three disjoint sets are
//! tracked:
//!
//! * `node_cells`   — the rectangular block carved out by each placed
//!   node (variable size, padded by a 1-cell margin).
//! * `edge_cells`   — every cell occupied by an already-routed polyline.
//! * `label_cells`  — the bounding box of every already-placed edge label.
//!
//! `is_free(c)` is the conjunction over all three; routing and label
//! placement consult it before committing to a cell.

use std::collections::HashSet;

use crate::ast::{Diagram, EdgeLabelAnchor, TextExtents};

/// One-cell padding around node rectangles so paths don't kiss the border.
pub(crate) const NODE_BLOCK_MARGIN: i32 = 1;

/// Extra cell margin around the world bounding box, large enough to let
/// A* detour around the perimeter of the layout but small enough that the
/// search space stays finite.
pub(crate) const WORLD_PAD_CELLS: i32 = 12;

/// Tracks which grid cells are occupied by nodes, edges, or labels and
/// the rectangular bounding box A* searches are allowed to explore.
#[derive(Debug, Clone)]
pub struct GridOccupancy {
    node_cells: HashSet<(i32, i32)>,
    edge_cells: HashSet<(i32, i32)>,
    label_cells: HashSet<(i32, i32)>,
    bounds: (i32, i32, i32, i32), // (x_min, y_min, x_max, y_max), inclusive
}

impl GridOccupancy {
    /// Build a fresh occupancy grid from a diagram whose nodes already
    /// have positions and sizes assigned. World bounds are derived from
    /// the union of every node block plus a fixed margin, so A* searches
    /// can detour around the layout without exploring infinite space.
    pub fn new(diagram: &Diagram) -> Self {
        let mut node_cells = HashSet::new();
        let mut x_min = i32::MAX;
        let mut y_min = i32::MAX;
        let mut x_max = i32::MIN;
        let mut y_max = i32::MIN;
        for node in &diagram.nodes {
            if let (Some(pos), Some(size)) = (node.position, node.node_size) {
                let cx = pos.x / 10;
                let cy = pos.y / 10;
                let half_w = size.half_w() / 10 + NODE_BLOCK_MARGIN;
                let half_h = size.half_h() / 10 + NODE_BLOCK_MARGIN;
                for dy in -half_h..=half_h {
                    for dx in -half_w..=half_w {
                        node_cells.insert((cx + dx, cy + dy));
                    }
                }
                x_min = x_min.min(cx - half_w);
                y_min = y_min.min(cy - half_h);
                x_max = x_max.max(cx + half_w);
                y_max = y_max.max(cy + half_h);
            }
        }
        if x_min == i32::MAX {
            // Diagram has no positioned nodes; pick a small default box.
            x_min = 0;
            y_min = 0;
            x_max = 1;
            y_max = 1;
        }
        let bounds = (
            x_min - WORLD_PAD_CELLS,
            y_min - WORLD_PAD_CELLS,
            x_max + WORLD_PAD_CELLS,
            y_max + WORLD_PAD_CELLS,
        );
        Self {
            node_cells,
            edge_cells: HashSet::new(),
            label_cells: HashSet::new(),
            bounds,
        }
    }

    /// True when the cell sits inside the world bounding box A* is
    /// allowed to explore. Out-of-bounds cells are treated as walls.
    pub fn in_bounds(&self, cell: (i32, i32)) -> bool {
        let (x_min, y_min, x_max, y_max) = self.bounds;
        cell.0 >= x_min && cell.0 <= x_max && cell.1 >= y_min && cell.1 <= y_max
    }

    /// True when no node, routed edge, or placed label occupies the cell.
    pub fn is_free(&self, cell: (i32, i32)) -> bool {
        !self.node_cells.contains(&cell)
            && !self.edge_cells.contains(&cell)
            && !self.label_cells.contains(&cell)
    }

    /// True when the cell is occupied specifically by a node block.
    pub fn is_node(&self, cell: (i32, i32)) -> bool {
        self.node_cells.contains(&cell)
    }

    /// Reserve every cell along a routed path so subsequent searches see it.
    pub fn occupy_path(&mut self, path: &[(i32, i32)]) {
        for &cell in path {
            self.edge_cells.insert(cell);
        }
    }

    /// Reserve the bounding box of an edge label (in grid cells) so
    /// subsequent A* routes treat the label as impassable terrain.
    ///
    /// `extents` is in pixels; the box is anchor-aligned (`anchor.anchor`
    /// can be `"start"`, `"middle"`, or `"end"`).
    pub fn occupy_label(&mut self, anchor: EdgeLabelAnchor, extents: TextExtents) {
        for cell in label_cells(anchor, extents) {
            self.label_cells.insert(cell);
        }
    }

    /// Predict whether a label's bounding box would collide with anything
    /// already on the grid (nodes, edges, or other labels).
    pub fn label_collides(&self, anchor: EdgeLabelAnchor, extents: TextExtents) -> bool {
        label_cells(anchor, extents)
            .into_iter()
            .any(|cell| !self.is_free(cell))
    }

    /// Crossings of a path against already-routed edges. Endpoints are
    /// excluded so trimmed terminals don't count.
    pub fn crossing_count(&self, path: &[(i32, i32)]) -> u32 {
        if path.len() < 2 {
            return 0;
        }
        path.iter()
            .skip(1)
            .take(path.len().saturating_sub(2))
            .filter(|&&c| self.edge_cells.contains(&c))
            .count() as u32
    }
}

/// Enumerate the grid cells inside a label's bounding box.
fn label_cells(anchor: EdgeLabelAnchor, extents: TextExtents) -> Vec<(i32, i32)> {
    let w = extents.width.ceil() as i32;
    let h = extents.height.ceil() as i32;
    // Anchor-aligned x range.
    let (x_lo, x_hi) = match anchor.anchor {
        "start" => (anchor.x, anchor.x + w),
        "end" => (anchor.x - w, anchor.x),
        _ => {
            let half = w / 2;
            (anchor.x - half, anchor.x + (w - half))
        }
    };
    // Vertically centred on the anchor (matches `dominant-baseline="central"`).
    let half_h = h / 2;
    let (y_lo, y_hi) = (anchor.y - half_h, anchor.y + (h - half_h));

    let mut out = Vec::new();
    let cx_lo = x_lo.div_euclid(10);
    let cx_hi = x_hi.div_euclid(10);
    let cy_lo = y_lo.div_euclid(10);
    let cy_hi = y_hi.div_euclid(10);
    for cy in cy_lo..=cy_hi {
        for cx in cx_lo..=cx_hi {
            out.push((cx, cy));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{EdgeLabelAnchor, TextExtents};

    #[test]
    fn test_label_box_marks_cells() {
        let d = crate::parser::parse_dsl("A -> B\n").unwrap();
        let mut occ = GridOccupancy::new(&d);
        let anchor = EdgeLabelAnchor { x: 100, y: 100, anchor: "middle" };
        let extents = TextExtents { width: 30.0, height: 12.0 };
        assert!(!occ.label_collides(anchor, extents));
        occ.occupy_label(anchor, extents);
        assert!(occ.label_collides(anchor, extents));
    }
}
