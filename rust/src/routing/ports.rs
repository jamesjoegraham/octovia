//! Port computation — translating "the source's East side" to a grid cell
//! that sits just outside the node's blocking square.
//!
//! The 8-port rule is preserved: every node exposes E/W/N/S ports and the
//! four diagonal slots are reserved for label placement. With variable
//! node widths, the port offset is now derived from each node's actual
//! `NodeSize` rather than a fixed constant.

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

/// Unit-cell outward offset for a port, used to grow a 1-cell orthogonal
/// stalk before A* takes over. Returning the cell *outside* the port
/// guarantees the first segment of every route is a clean axis-aligned
/// step rather than a 45° diagonal snapping straight into the node.
pub(crate) fn outward_offset(direction: PortDirection) -> (i32, i32) {
    match direction {
        PortDirection::East => (1, 0),
        PortDirection::West => (-1, 0),
        PortDirection::North => (0, -1),
        PortDirection::South => (0, 1),
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
/// for a forward edge. The first entry is the geometric primary; the
/// remaining entries fix the target port (the "natural" approach axis)
/// and try perpendicular source ports so A* can detour around obstacles
/// without the router being locked to a single axis pair.
pub(crate) fn forward_port_candidates(
    from: Point,
    to: Point,
) -> Vec<(PortDirection, PortDirection)> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

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

    vec![(primary_src, tgt), (perp_near, tgt), (perp_far, tgt)]
}

/// Generate the candidate (source, target) port pairs for a *back-edge*.
/// Back-edges must wrap around the layered backbone, so the canonical
/// pair is South → North. Side approaches (East/West out of source) are
/// included so the cost-based selector can pick a tighter U-turn when
/// the natural channel below the diagram is congested.
pub(crate) fn back_port_candidates(
    from: Point,
    to: Point,
) -> Vec<(PortDirection, PortDirection)> {
    // The dominant assumption: the spanning tree pushed the target
    // *above* the source visually. We always re-enter North; we just
    // vary the exit side based on where the target sits horizontally.
    let dx = to.x - from.x;
    let side_first = if dx >= 0 {
        PortDirection::East
    } else {
        PortDirection::West
    };
    let side_second = match side_first {
        PortDirection::East => PortDirection::West,
        _ => PortDirection::East,
    };
    vec![
        (PortDirection::South, PortDirection::North),
        (side_first, PortDirection::North),
        (side_second, PortDirection::North),
    ]
}
