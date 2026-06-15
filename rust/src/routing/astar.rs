//! Orthogonal path search.
//!
//! The router uses A* over the 4-direction grid (N/S/E/W only). Diagonal
//! moves are deliberately excluded so every routed segment is strictly
//! axis-aligned — the transit-map aesthetic the project is going for.
//! A `TURN_PENALTY` is added whenever the next step differs from the
//! previous step's direction; this biases the search toward long
//! straight runs and clean 90° corners instead of staircases. Every
//! edge — forward or back-edge — flows through `astar_cells`; the
//! deterministic neighbour ordering keeps results reproducible across
//! runs.

use pathfinding::prelude::astar;

use super::occupancy::GridOccupancy;

/// Extra cost charged whenever the path changes direction. Large enough
/// (vs. the 10-cost orthogonal step) that A* will reliably prefer a
/// long straight run over a kinked detour, but small enough that real
/// obstacles still trigger an L-bend.
const TURN_PENALTY: u32 = 20;

/// A* search state: the grid cell plus the step direction used to enter
/// it. The direction component lets us levy a turn penalty by comparing
/// each neighbour's outgoing direction against the incoming one. The
/// sentinel direction `(0, 0)` marks the start cell so the very first
/// move is never penalised.
type State = ((i32, i32), (i32, i32));

/// 4-direction neighbour table: (dx, dy, base step cost). Order is
/// deterministic so repeated runs over the same input produce identical
/// tie-breaking.
const DIRS: [(i32, i32, u32); 4] = [
    (1, 0, 10),  // East
    (-1, 0, 10), // West
    (0, 1, 10),  // South
    (0, -1, 10), // North
];

/// Run A* between two cells using a strictly orthogonal cost function
/// with a turn penalty. Returns both the cell path and its accumulated
/// cost so callers can compare alternative port pairings and pick the
/// cheapest route.
///
/// Neighbour expansion is bounded by the occupancy grid's world
/// rectangle so the search is always finite, even when the goal is
/// surrounded by reserved cells.
pub(crate) fn astar_cells(
    start: (i32, i32),
    end: (i32, i32),
    occupancy: &GridOccupancy,
) -> Option<(Vec<(i32, i32)>, u32)> {
    let start_state: State = (start, (0, 0));
    let (path, cost) = astar(
        &start_state,
        |&(cell, prev_dir)| {
            let (x, y) = cell;
            let mut out: Vec<(State, u32)> = Vec::with_capacity(4);
            for &(dx, dy, base) in &DIRS {
                let next = (x + dx, y + dy);
                if !occupancy.in_bounds(next) {
                    continue;
                }
                if !(occupancy.is_free(next) || next == end || next == start) {
                    continue;
                }
                let step_dir = (dx, dy);
                let cost = if prev_dir == (0, 0) || prev_dir == step_dir {
                    base
                } else {
                    base + TURN_PENALTY
                };
                out.push(((next, step_dir), cost));
            }
            out
        },
        |&(cell, _)| manhattan_distance(cell, end),
        |&(cell, _)| cell == end,
    )?;
    Some((path.into_iter().map(|(c, _)| c).collect(), cost))
}

/// Heuristic: Manhattan distance scaled by the orthogonal step cost.
/// Admissible alongside `TURN_PENALTY` because it ignores turn cost,
/// which can only increase the true path length.
fn manhattan_distance(a: (i32, i32), b: (i32, i32)) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs();
    let dy = (a.1 - b.1).unsigned_abs();
    (dx + dy) * 10
}
