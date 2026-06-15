//! Grid occupancy tracking — which cells are blocked by nodes or routes.
//!
//! The router operates on a 10×10 px sub-grid: every diagram coordinate
//! `(x, y)` maps to a cell `(x / 10, y / 10)`. Each placed node carves out
//! a square of `BLOCK_HALF` cells around its centre that paths must avoid.
//! Already-routed edges add their cells too, so subsequent edges re-route
//! to avoid pile-ups (and pay a crossing penalty when they can't).

use std::collections::HashSet;

use crate::ast::Diagram;
use crate::layout::NODE_RADIUS;

/// Block half-width (in cells) used by `GridOccupancy` to mark each node
/// as impassable. A path may cross *outside* this radius without colliding.
pub(crate) const BLOCK_HALF: i32 = NODE_RADIUS / 10 + 2;

/// Tracks which grid cells are occupied by nodes or already-routed edges.
#[derive(Debug, Clone)]
pub struct GridOccupancy {
    /// Cells occupied by nodes (no routing through these).
    node_cells: HashSet<(i32, i32)>,
    /// Cells occupied by routed edge segments.
    edge_cells: HashSet<(i32, i32)>,
}

impl GridOccupancy {
    pub fn new(diagram: &Diagram) -> Self {
        let mut node_cells = HashSet::new();
        for node in &diagram.nodes {
            if let Some(pos) = node.position {
                // Mark a square around each node as occupied
                let r = BLOCK_HALF;
                for dx in -r..=r {
                    for dy in -r..=r {
                        node_cells.insert((pos.x / 10 + dx, pos.y / 10 + dy));
                    }
                }
            }
        }
        Self {
            node_cells,
            edge_cells: HashSet::new(),
        }
    }

    /// Check if a cell is free (neither node nor existing edge occupies it).
    pub fn is_free(&self, cell: (i32, i32)) -> bool {
        !self.node_cells.contains(&cell) && !self.edge_cells.contains(&cell)
    }

    /// Occupy cells along a routed path.
    pub fn occupy_path(&mut self, path: &[(i32, i32)]) {
        for &cell in path {
            self.edge_cells.insert(cell);
        }
    }

    /// Check how many times a path would cross occupied cells.
    pub fn crossing_count(&self, path: &[(i32, i32)]) -> u32 {
        if path.len() < 2 {
            return 0;
        }
        // Count intermediate cells that intersect the edge grid
        path.iter()
            .skip(1)
            .take(path.len().saturating_sub(2))
            .filter(|&&c| self.edge_cells.contains(&c))
            .count() as u32
    }
}
