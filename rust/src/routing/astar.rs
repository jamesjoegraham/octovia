//! Octilinear path search.
//!
//! Two strategies live here:
//! -   `straight_line_cells` — fast path when source and target share an
//!     axis; emits the cells of an axis-aligned run with no allocation
//!     for unrelated branches.
//! -   `astar_cells` — fallback A* over the 8-direction grid using the
//!     octile distance heuristic. Used for forward edges that aren't
//!     axis-aligned (true diagonals) and as a generic primitive any
//!     future routing strategy can call.

use pathfinding::prelude::astar;

use super::occupancy::GridOccupancy;

/// Build a straight-line path of grid cells between two cells that share
/// either an x- or a y-coordinate. Returns `None` when the cells are not
/// axis-aligned (in which case A* should be used instead).
pub(crate) fn straight_line_cells(
    start: (i32, i32),
    end: (i32, i32),
) -> Option<Vec<(i32, i32)>> {
    if start.0 == end.0 {
        // Vertical run
        let step = (end.1 - start.1).signum();
        if step == 0 {
            return Some(vec![start]);
        }
        let mut cells = Vec::new();
        let mut y = start.1;
        loop {
            cells.push((start.0, y));
            if y == end.1 {
                break;
            }
            y += step;
        }
        Some(cells)
    } else if start.1 == end.1 {
        // Horizontal run
        let step = (end.0 - start.0).signum();
        if step == 0 {
            return Some(vec![start]);
        }
        let mut cells = Vec::new();
        let mut x = start.0;
        loop {
            cells.push((x, start.1));
            if x == end.0 {
                break;
            }
            x += step;
        }
        Some(cells)
    } else {
        None
    }
}

/// Run A* between two cells using the octilinear cost function. Used as
/// the fallback for forward edges that cannot be routed as a single
/// axis-aligned straight line.
pub(crate) fn astar_cells(
    start: (i32, i32),
    end: (i32, i32),
    occupancy: &GridOccupancy,
) -> Option<Vec<(i32, i32)>> {
    astar(
        &start,
        |&cell| {
            neighbours(cell)
                .into_iter()
                .filter(|(c, _)| occupancy.is_free(*c) || *c == end)
                .collect::<Vec<_>>()
        },
        |&cell| octile_distance(cell, end),
        |&cell| cell == end,
    )
    .map(|(path, _cost)| path)
}

/// Heuristic: octile distance (Chebyshev with diagonal cost √2 ≈ 14/10).
fn octile_distance(a: (i32, i32), b: (i32, i32)) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs();
    let dy = (a.1 - b.1).unsigned_abs();
    let d = dx.min(dy);
    // Straight distance + diagonal distance (14/10 ≈ √2)
    (dx + dy - d) * 10 + d * 14
}

/// Neighbours for octilinear movement: 8 directions.
fn neighbours(cell: (i32, i32)) -> Vec<((i32, i32), u32)> {
    let (x, y) = cell;
    vec![
        ((x + 1, y), 10),      // East
        ((x - 1, y), 10),      // West
        ((x, y + 1), 10),      // South
        ((x, y - 1), 10),      // North
        ((x + 1, y - 1), 14),  // NE
        ((x - 1, y - 1), 14),  // NW
        ((x + 1, y + 1), 14),  // SE
        ((x - 1, y + 1), 14),  // SW
    ]
}
