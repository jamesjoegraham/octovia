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

/// Choose the (source, target) port pair for a *forward* edge based on
/// the relative position of the two nodes. With layered layouts forward
/// edges almost always cross horizontally between layers, so East ↔ West
/// is the dominant case.
///
/// -   Same column (`dx == 0`):  South ↔ North
/// -   Same row    (`dy == 0`):  East  ↔ West
/// -   Otherwise (true diagonal): pick the dominant axis. Horizontal wins
///     ties because layered diagrams flow left-to-right.
pub(crate) fn pick_forward_ports(from: Point, to: Point) -> (PortDirection, PortDirection) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    let prefer_horizontal = if dx == 0 && dy != 0 {
        false
    } else if dy == 0 {
        true
    } else {
        dx.abs() >= dy.abs()
    };

    if prefer_horizontal {
        if dx >= 0 {
            (PortDirection::East, PortDirection::West)
        } else {
            (PortDirection::West, PortDirection::East)
        }
    } else if dy > 0 {
        (PortDirection::South, PortDirection::North)
    } else {
        (PortDirection::North, PortDirection::South)
    }
}

/// Choose the (source, target) port pair for a *back-edge*. Back-edges
/// always exit South and re-enter North so A* is forced to route through
/// the routing channels below the layered backbone.
pub(crate) fn pick_back_ports() -> (PortDirection, PortDirection) {
    (PortDirection::South, PortDirection::North)
}
