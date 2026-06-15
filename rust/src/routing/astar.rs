//! Octilinear path search.
//!
//! The router uses A* over the 8-direction grid with the octile distance
//! heuristic (cost 10 for orthogonal moves, 14 for diagonal — a rational
//! approximation of √2). Every edge — forward or back-edge — flows through
//! `astar_cells`; the deterministic neighbour ordering keeps results
//! reproducible across runs.

use pathfinding::prelude::astar;

use super::occupancy::GridOccupancy;

/// Run A* between two cells using the octilinear cost function. Returns
/// both the cell path and its accumulated cost so callers can compare
/// alternative port pairings and pick the cheapest route.
///
/// Neighbour expansion is bounded by the occupancy grid's world
/// rectangle so the search is always finite, even when the goal is
/// surrounded by reserved cells.
pub(crate) fn astar_cells(
    start: (i32, i32),
    end: (i32, i32),
    occupancy: &GridOccupancy,
) -> Option<(Vec<(i32, i32)>, u32)> {
    astar(
        &start,
        |&cell| {
            neighbours(cell)
                .into_iter()
                .filter(|(c, _)| {
                    if !occupancy.in_bounds(*c) {
                        return false;
                    }
                    occupancy.is_free(*c) || *c == end || *c == start
                })
                .collect::<Vec<_>>()
        },
        |&cell| octile_distance(cell, end),
        |&cell| cell == end,
    )
}

/// Heuristic: octile distance (Chebyshev with diagonal cost √2 ≈ 14/10).
fn octile_distance(a: (i32, i32), b: (i32, i32)) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs();
    let dy = (a.1 - b.1).unsigned_abs();
    let d = dx.min(dy);
    (dx + dy - d) * 10 + d * 14
}

/// Neighbours for octilinear movement: 8 directions in deterministic
/// order. Cost 10 for orthogonal, 14 for diagonal.
fn neighbours(cell: (i32, i32)) -> Vec<((i32, i32), u32)> {
    let (x, y) = cell;
    vec![
        ((x + 1, y), 10),     // East
        ((x - 1, y), 10),     // West
        ((x, y + 1), 10),     // South
        ((x, y - 1), 10),     // North
        ((x + 1, y - 1), 14), // NE
        ((x - 1, y - 1), 14), // NW
        ((x + 1, y + 1), 14), // SE
        ((x - 1, y + 1), 14), // SW
    ]
}
