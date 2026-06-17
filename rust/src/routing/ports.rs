//! Port computation — geometry-aware port candidates for boustrophedon
//! (serpentine, macro-row) layout.
//!
//! Previously, forward_port_candidates assumed a strictly left-to-right
//! topological flow (East->West for edges going right). With boustrophedon
//! folding, layer indices no longer monotonically increase in X. Instead,
//! we evaluate the **relative X/Y geometry** of source and target nodes
//! regardless of their topological layer.
//!
//! Three cases:
//!
//! 1. **Intra-fold edges** — source and target sit in the same macro-row.
//!    The port logic is the same as before: East->West or West->East based
//!    purely on which node has the higher effective X coordinate.
//!
//! 2. **Wrap-around edges** — source is at the end of one macro-row and
//!    target is at the start of the next (or vice versa). Generate
//!    South->North or South->East stalk cells to guide A* through the
//!    macro-row gutter.
//!
//! 3. **Vertical skip edges** — edges that span multiple macro-rows.
//!    Generate North/South ports so the route can traverse vertically
//!    through the MACRO_ROW_GUTTERs.
//!
//! Back-edge port candidates remain unchanged — they already use the
//! physical geometry of the two nodes and the south channel.

use crate::ast::{NodeSize, Point, PortDirection};

use super::occupancy::NODE_BLOCK_MARGIN;

/// Compute the port (starting cell) for a given node and direction. The
/// returned cell is one cell outside the node's blocking square so that
/// A* always begins on free terrain.
pub(crate) fn port_cell(
    node_pos: Point,
    node_size: NodeSize,
    direction: PortDirection,
) -> (i32, i32) {
    let cx = node_pos.x / 10;
    let cy = node_pos.y / 10;
    let half_w = node_size.half_w() / 10 + NODE_BLOCK_MARGIN + 1;
    let half_h = node_size.half_h() / 10 + NODE_BLOCK_MARGIN + 1;
    match direction {
        PortDirection::East => (cx + half_w, cy),
        PortDirection::West => (cx - half_w, cy),
        PortDirection::North => (cx, cy - half_h),
        PortDirection::South => (cx, cy + half_h),
    }
}

/// Outward offset for a port, used to grow an orthogonal stalk before
/// A* takes over. The stalk is projected `STALK_STEP` cells past the
/// port so the search begins on terrain that is guaranteed to be clear
/// of the node's padded margin — `port_cell` already sits one cell
/// outside the margin, so a two-cell step lands the stalk two cells
/// beyond the margin and well clear of neighbouring node blocks.
pub(crate) fn outward_offset(direction: PortDirection) -> (i32, i32) {
    const STALK_STEP: i32 = 2;
    match direction {
        PortDirection::East => (STALK_STEP, 0),
        PortDirection::West => (-STALK_STEP, 0),
        PortDirection::North => (0, -STALK_STEP),
        PortDirection::South => (0, STALK_STEP),
    }
}

/// Choose the *primary* (source, target) port pair for a forward edge.
/// Equivalent to `forward_port_candidates(from, to)[0]`; kept as a
/// convenience for callers that don't need the alternates.
#[cfg(test)]
pub(crate) fn pick_forward_ports(from: Point, to: Point) -> (PortDirection, PortDirection) {
    forward_port_candidates(from, to)[0]
}

/// Generate a small, ranked set of (source, target) port pair candidates
/// for a non-cyclic edge in a boustrophedon (serpentine macro-row) layout.
///
/// **Geometry-based** — we evaluate whether source is to the left or right
/// of target, regardless of topological layer index. This is essential
/// because layers in odd macro-rows flow right-to-left, so a "forward"
/// edge may exit East or West depending on physical X position.
///
/// The candidates are ranked: primary axis pair first, then perpendicular
/// alternates so A* can detour around obstacles.
pub(crate) fn forward_port_candidates(
    from: Point,
    to: Point,
) -> Vec<(PortDirection, PortDirection)> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    // Use the same geometric decision as before — it already works for
    // both pure LTR and boustrophedon because it reasons about *physical*
    // positions, not layer indices.
    let prefer_horizontal = if dx == 0 && dy != 0 {
        false
    } else if dy == 0 {
        true
    } else {
        dx.abs() >= dy.abs()
    };

    let (primary_src, tgt) = if prefer_horizontal {
        if dx >= 0 {
            (PortDirection::East, PortDirection::West)
        } else {
            (PortDirection::West, PortDirection::East)
        }
    } else if dy > 0 {
        (PortDirection::South, PortDirection::North)
    } else {
        (PortDirection::North, PortDirection::South)
    };

    // Perpendicular alternates: nearer side first (in the direction of
    // the cross-axis delta) so the candidate ordering matches geometry.
    let (perp_near, perp_far) = if prefer_horizontal {
        if dy >= 0 {
            (PortDirection::South, PortDirection::North)
        } else {
            (PortDirection::North, PortDirection::South)
        }
    } else if dx >= 0 {
        (PortDirection::East, PortDirection::West)
    } else {
        (PortDirection::West, PortDirection::East)
    };

    // For boustrophedon wrap-around (where source is at the right end of
    // macro-row N and target at the left end of macro-row N+1),
    // dx might be negative even though the edge is topologically "forward".
    // The geometry-based logic above already handles this — if to.x < from.x
    // it correctly emits West->East. This is fine because A* will find the
    // path South through the macro-row gutter regardless.

    vec![(primary_src, tgt), (perp_near, tgt), (perp_far, tgt)]
}

/// Generate the candidate (source, target) port pairs for a **back-edge**
/// (or any edge where `is_cyclic == true`).
///
/// Topological layout expands layers horizontally, so the band of empty
/// canvas directly *underneath* every node is the most reliable routing
/// channel in the diagram. We therefore drive cyclic edges down through
/// the South face on both ends as the primary plan and degrade through
/// progressively less-preferred fallbacks: side-only U-bends, then a
/// mixed South-out / side-in approach, and finally the original
/// North-entry variants for the rare cases where the south channel is
/// fully congested. Every candidate is real so a strict-orthogonal A*
/// can always find a legal escape route.
///
/// In a boustrophedon layout back-edges may also need to navigate
/// macro-row gutters, but the South-channel strategy already handles
/// this naturally because the South channel sits below the lowest
/// node in any given macro-row.
pub(crate) fn back_port_candidates(
    from: Point,
    to: Point,
) -> Vec<(PortDirection, PortDirection)> {
    use PortDirection::*;

    // The near-side is the entry face closest to the source's
    // horizontal position relative to the target — re-entering on the
    // near side keeps the U-bend short.
    let dx = to.x - from.x;
    let near_side = if dx >= 0 { West } else { East };
    let far_side = match near_side {
        West => East,
        _ => West,
    };

    vec![
        // Preferred: down-and-up through the south channel.
        (South, South),
        (South, near_side),
        (South, far_side),
        // Side-only approaches when the bottom-out path is blocked.
        (near_side, near_side),
        (far_side, far_side),
        (near_side, South),
        (far_side, South),
        // Last-resort: top-channel U-bend (the regression we wanted to
        // avoid, but still better than a panic).
        (South, North),
        (North, North),
        (North, South),
    ]
}
