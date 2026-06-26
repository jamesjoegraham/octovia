//! Port computation — geometry-aware port candidates for Top-to-Bottom
//! vertical layout.
//!
//! In TTB layout, time flows downward. Layer indices increase from top
//! to bottom. Forward edges exit the South face of the source node and
//! enter the North face of the target node. Back-edges (feedback loops)
//! use East→East or West→West to route up the vertical flanks of the
//! diagram. Same-layer edges (two nodes on the same Y level but
//! different X positions) use East↔West.
//!
//! Three cases:
//!
//! 1. **Standard forward edges** — source layer < target layer.
//!    Default: South → North. This applies to both adjacent layers and
//!    edges that skip layers. With South→North, A* routes downward
//!    through the LAYER_GUTTER channels. Perpendicular alternates
//!    (East→North, West→North) are provided so A* can detour around
//!    obstacles.
//!
//! 2. **Back-edges (feedback loops)** — source layer >= target layer
//!    after topological sort. Default pairs force the A* router to
//!    exit the side of the node, travel up the clear vertical flanks
//!    of the diagram, and re-enter the side of the target.
//!
//! 3. **Same-layer edges** — source and target at the same Y position.
//!    Use East → West or West → East based on their X ordering.

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
#[cfg(test)]
pub(crate) fn pick_forward_ports(from: Point, to: Point) -> (PortDirection, PortDirection) {
    forward_port_candidates(from, to)[0]
}

/// Generate a small, ranked set of (source, target) port pair candidates
/// for a non-cyclic edge in a Top-to-Bottom layout.
///
/// **TTB semantics**: time flows downward. The primary candidate is
/// South→North (source exits bottom, target enters top). When the edge
/// is between nodes at the same Y (same-layer), East↔West or West↔East
/// is used instead, based on their X ordering.
///
/// Perpendicular alternates (East→North, West→North) let A* detour
/// around obstacles while still entering the target from the top.
pub(crate) fn forward_port_candidates(
    from: Point,
    to: Point,
) -> Vec<(PortDirection, PortDirection)> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    // TTB: default flow is downward (South→North).
    // If nodes are at the same Y (or very close), treat as same-layer.
    let same_layer = dy == 0;

    if same_layer {
        // Same-layer edge: use East→West or West→East based on X ordering.
        if dx >= 0 {
            vec![
                (PortDirection::East, PortDirection::West),
                (PortDirection::North, PortDirection::South),
                (PortDirection::South, PortDirection::North),
            ]
        } else {
            vec![
                (PortDirection::West, PortDirection::East),
                (PortDirection::North, PortDirection::South),
                (PortDirection::South, PortDirection::North),
            ]
        }
    } else {
        // Standard forward edge flowing downward: South→North is primary.
        // Perpendicular alternates let A* route around congestion.
        let (perp_near, perp_far) = if dx >= 0 {
            (PortDirection::East, PortDirection::West)
        } else {
            (PortDirection::West, PortDirection::East)
        };

        vec![
            (PortDirection::South, PortDirection::North),
            (perp_near, PortDirection::North),
            (perp_far, PortDirection::North),
        ]
    }
}

/// Generate the candidate (source, target) port pairs for a **back-edge**
/// (or any edge where `is_cyclic == true`).
///
/// In TTB layout, back-edges must route from a lower layer back up to a
/// higher layer (feedback from the bottom of the diagram to the top).
/// The most reliable path uses the vertical flanks of the diagram —
/// exit the side of the source, travel up, and re-enter the side of the
/// target. East→East or West→West pairs (same-side routing) produce a
/// clean U-bend that stays well clear of forward edges running down the
/// centre.
///
/// If the sides are blocked (congested), fall through to South→South
/// (bottom-out-and-around) and then to mixed South→side approaches.
pub(crate) fn back_port_candidates(
    from: Point,
    to: Point,
) -> Vec<(PortDirection, PortDirection)> {
    use PortDirection::*;

    // Determine which side of the target the source sits on.
    let dx = to.x - from.x;
    let near_side = if dx >= 0 { West } else { East };
    let far_side = match near_side {
        West => East,
        _ => West,
    };

    vec![
        // Preferred: side-to-side U-bends up the flanks.
        (near_side, near_side),
        (far_side, far_side),
        // Side-out → mixed re-entry
        (near_side, South),
        (far_side, South),
        (near_side, North),
        (far_side, North),
        // Bottom-out alternatives (fallback when sides are blocked).
        (South, South),
        (South, near_side),
        (South, far_side),
        // Last-resort top-channel.
        (South, North),
        (North, North),
        (North, South),
    ]
}

/// Generate candidate (source, target) port pairs for a **star/hub**
/// edge — an edge from a central hub to a spoke node placed at a
/// compass point around the hub.
///
/// The spoke's position relative to the hub tells us which direction
/// to use. For cardinal directions (N, S, E, W) there's exactly one
/// optimal port pair. For intercardinal directions (NE, NW, SE, SW)
/// we try both the two cardinal faces that bracket the direction.
pub(crate) fn star_port_candidates(
    from: Point,
    to: Point,
) -> Vec<(PortDirection, PortDirection)> {
    use PortDirection::*;

    let dx = to.x - from.x;
    let dy = to.y - from.y;

    // Determine the primary direction: whichever axis has the larger
    // absolute delta dominates.
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();

    if abs_dx >= abs_dy * 2 {
        // Strongly horizontal: use East→West or West→East
        if dx >= 0 {
            vec![(East, West), (South, North), (North, South)]
        } else {
            vec![(West, East), (South, North), (North, South)]
        }
    } else if abs_dy >= abs_dx * 2 {
        // Strongly vertical: use South→North or North→South
        if dy >= 0 {
            // Spoke is below the hub (South / SE / SW quadrant)
            vec![(South, North), (East, North), (West, North)]
        } else {
            // Spoke is above the hub (North / NE / NW quadrant)
            vec![(North, South), (East, South), (West, South)]
        }
    } else if abs_dx > abs_dy {
        // Intercardinal: more horizontal than vertical, but close.
        // Use the side port for exit and North/South for entry.
        let side = if dx >= 0 { East } else { West };
        let entry = if dy >= 0 { North } else { South };
        vec![(side, entry), (South, North), (North, South)]
    } else {
        // Intercardinal: more vertical than horizontal, or equal.
        let vertical = if dy >= 0 { South } else { North };
        let side = if dx >= 0 { East } else { West };
        vec![(vertical, if dy >= 0 { North } else { South }), (side, if dy >= 0 { North } else { South }), (match side { East => West, West => East, _ => side }, if dy >= 0 { North } else { South })]
    }
}
