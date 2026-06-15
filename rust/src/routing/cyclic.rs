//! Cyclic edge routing — deterministic three-segment U-bends.
//!
//! Back-edges and cycles never use A*: they always exit the source's
//! South port, run along a horizontal corridor that lives below every
//! node, and re-enter the target's North port. The corridor row scoots
//! further down on collision so multiple cyclic edges stack instead of
//! overlapping.

use crate::ast::{Point, PortDirection};

use super::occupancy::{GridOccupancy, BLOCK_HALF};
use super::ports::port_cell;

/// Route a cyclic (back-)edge as a deterministic 3-segment U-bend:
///
/// ```text
///       ┌── source ──┐                     ┌── target ──┐
///       │            │                     │            │
///       └─────●──────┘                     └─────●──────┘
///             │ S                                ▲ N
///             │                                  │
///             ▼          (corridor band)         │
///             ●──────────────────────────────────●
/// ```
///
/// 1. Exit `source.S`, run vertically down to a horizontal corridor that
///    lives **below both nodes' blocking squares**.
/// 2. Run horizontally along the corridor to the target's column.
/// 3. Run vertically up into `target.N`.
///
/// Endpoints are recorded directly — they ride *through* the source/target
/// blocks because the rectangles are drawn on top in z-order, hiding the
/// segment that's inside the node.
pub(crate) fn route_cyclic_u_bend(
    from: Point,
    to: Point,
    occupancy: &GridOccupancy,
) -> Vec<(i32, i32)> {
    let start = port_cell(from, PortDirection::South);
    let end = port_cell(to, PortDirection::North);

    // The corridor must clear both source and target blocks. Each block
    // extends `BLOCK_HALF` cells below its node centre.
    let from_block_bottom = from.y / 10 + BLOCK_HALF;
    let to_block_bottom = to.y / 10 + BLOCK_HALF;
    let base_band = from_block_bottom.max(to_block_bottom) + 1;

    let x_lo = start.0.min(end.0);
    let x_hi = start.0.max(end.0);

    // Find a free row for the horizontal corridor; if congested, drop
    // further down. Capped to keep the search bounded.
    let mut band = base_band;
    for _ in 0..16 {
        let clear = (x_lo..=x_hi).all(|x| occupancy.is_free((x, band)));
        if clear {
            break;
        }
        band += 2;
    }

    let mut path: Vec<(i32, i32)> = Vec::new();

    // Segment 1: source.S downward to corridor
    let mut y = start.1;
    while y < band {
        path.push((start.0, y));
        y += 1;
    }

    // Segment 2: along the corridor
    let dx = (end.0 - start.0).signum();
    if dx == 0 {
        path.push((start.0, band));
    } else {
        let mut x = start.0;
        loop {
            path.push((x, band));
            if x == end.0 {
                break;
            }
            x += dx;
        }
    }

    // Segment 3: corridor upward into target.N
    let mut y = band - 1;
    while y >= end.1 {
        path.push((end.0, y));
        y -= 1;
    }

    path
}
