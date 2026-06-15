//! Port computation — translating "the source's East side" to a grid cell,
//! and picking the right cardinal pair for a forward edge.
//!
//! The 8-port rule: every node exposes E/W/N/S ports (and the four
//! diagonal slots reserved for label placement). Forward edges always
//! exit/enter on a cardinal port, chosen so the resulting path is a
//! clean straight line along a single axis whenever the geometry permits.

use crate::ast::{Point, PortDirection};

/// Compute the port (starting cell) for a given node and direction.
pub(crate) fn port_cell(node_pos: Point, direction: PortDirection) -> (i32, i32) {
    let cx = node_pos.x / 10;
    let cy = node_pos.y / 10;
    let offset = 3i32; // start a few cells out from the node centre
    match direction {
        PortDirection::East => (cx + offset, cy),
        PortDirection::West => (cx - offset, cy),
        PortDirection::North => (cx, cy - offset),
        PortDirection::South => (cx, cy + offset),
    }
}

/// Choose the (source, target) port pair for a forward edge based on the
/// relative position of the two nodes.
///
/// -   Same column (`dx == 0`):  South ↔ North
/// -   Same row    (`dy == 0`):  East  ↔ West
/// -   Otherwise (true diagonal): pick the dominant axis. Horizontal wins
///     ties because rows are wider than tall in the boustrophedon grid.
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
