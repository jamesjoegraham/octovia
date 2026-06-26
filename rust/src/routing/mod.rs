//! Unified routing + label placement phase.
//!
//! For every edge, in input order:
//!
//! 1. Generate a small set of candidate (source, target) port pairs from
//!    the relative geometry of the two nodes.
//! 2. For each candidate, run A* between *stalk cells* (one cell beyond
//!    each port) so every route enters and leaves its node along an
//!    orthogonal axis instead of snapping in at 45°.
//! 3. Keep the candidate with the lowest A* cost; reserve its cells in
//!    the [`GridOccupancy`] so subsequent edges treat it as a wall.
//! 4. Once every edge is routed, spread parallel lanes so coincident
//!    straight segments fan out instead of overlapping.
//! 5. Finally, with the post-lane geometry in hand, place each labelled
//!    edge's anchor against a fresh occupancy grid that reflects the
//!    actual final polylines.

mod astar;
mod lanes;
mod occupancy;
mod ports;

pub use occupancy::GridOccupancy;

use std::collections::HashMap;

use crate::ast::{Diagram, Point, PortDirection};
use crate::label_placement::seek_label_anchor;

use astar::astar_cells;
use lanes::assign_parallel_lanes;
use ports::{back_port_candidates, forward_port_candidates, star_port_candidates, outward_offset, port_cell};

/// Route every edge through the universal A* router, then spread
/// parallel lanes, then place edge labels against the final geometry.
pub fn route_all_edges(diagram: &mut Diagram) {
    let mut occupancy = GridOccupancy::new(diagram);

    let positions: HashMap<String, Point> = diagram
        .nodes
        .iter()
        .filter_map(|n| n.position.map(|p| (n.id.clone(), p)))
        .collect();
    let sizes: HashMap<String, crate::ast::NodeSize> = diagram
        .nodes
        .iter()
        .filter_map(|n| n.node_size.map(|s| (n.id.clone(), s)))
        .collect();

    // Process forward edges first so back-edges route around already-
    // committed forward routes (which sit in the layer-to-layer gutters).
    let order: Vec<usize> = (0..diagram.edges.len())
        .filter(|&i| !diagram.edges[i].is_cyclic)
        .chain((0..diagram.edges.len()).filter(|&i| diagram.edges[i].is_cyclic))
        .collect();

    for ei in order {
        let (from, to, src_size, tgt_size, is_cyclic) = {
            let e = &diagram.edges[ei];
            let from = match positions.get(&e.from) {
                Some(p) => *p,
                None => continue,
            };
            let to = match positions.get(&e.to) {
                Some(p) => *p,
                None => continue,
            };
            let src_size = sizes.get(&e.from).copied().unwrap_or(crate::ast::NodeSize {
                width: 60,
                height: 60,
            });
            let tgt_size = sizes.get(&e.to).copied().unwrap_or(crate::ast::NodeSize {
                width: 60,
                height: 60,
            });
            (from, to, src_size, tgt_size, e.is_cyclic)
        };

        let candidates = if is_cyclic {
            back_port_candidates(from, to)
        } else if diagram.edges[ei].is_star {
            // Star edges: use the compass position of the spoke relative
            // to the hub to determine port pairs. The spoke is either
            // directly N/S/E/W of the hub (single port pair) or at a
            // diagonal (use two candidate pairs).
            star_port_candidates(from, to)
        } else {
            forward_port_candidates(from, to)
        };

        let cells = if diagram.edges[ei].is_star {
            // Star edges: generate a direct orthogonal route from hub port
            // to spoke port. No A* needed — the compass directions tell us
            // exactly which way to go, and A* can produce jittery paths on
            // these short hops due to grid-cell quantization.
            let port_pair = &candidates[0];
            let port_src = port_cell(from, src_size, port_pair.0);
            let port_tgt = port_cell(to, tgt_size, port_pair.1);
            direct_star_route(port_src, port_tgt)
        } else {
            best_route(
                from,
                to,
                src_size,
                tgt_size,
                &candidates,
                &occupancy,
                &diagram.edges[ei].from,
                &diagram.edges[ei].to,
            )
        };

        // Convert grid cells back to pixel coordinates and bookend with
        // the source / target centres so SVG `trim_route_to_node_boundaries`
        // can clip the polyline cleanly to each rectangle's edge. The
        // bookend points are projected onto the port cell's axis so the
        // entry/exit segment is strictly orthogonal — without that
        // projection a 2-px jog appears whenever a node centre isn't
        // a multiple of 10 px, and the trim clip produces a faintly
        // diagonal "off-angle" entry into the node.
        let mut route: Vec<Point> = Vec::with_capacity(cells.len() + 2);
        if let Some(&(cx, cy)) = cells.first() {
            route.push(snap_endpoint_to_cell_axis(from, cx, cy));
        } else {
            route.push(from);
        }
        for (cx, cy) in &cells {
            route.push(Point::new(cx * 10, cy * 10));
        }
        if let Some(&(cx, cy)) = cells.last() {
            route.push(snap_endpoint_to_cell_axis(to, cx, cy));
        } else {
            route.push(to);
        }
        diagram.edges[ei].route = route;
        occupancy.occupy_path(&cells);
    }

    // Post-pass: spread parallel forward edges that ended up sharing a
    // straight segment so they don't draw on top of each other.
    assign_parallel_lanes(diagram);

    // Labels run *after* lane spreading so anchors track the final
    // polyline geometry. Rebuild occupancy from the freshly-shifted
    // routes; otherwise label placement would be testing collisions
    // against the pre-lane cells the router originally reserved.
    let mut label_occupancy = GridOccupancy::new(diagram);
    for edge in &diagram.edges {
        let cells = cells_along_polyline(&edge.route);
        label_occupancy.occupy_path(&cells);
    }

    for ei in 0..diagram.edges.len() {
        if let Some(extents) = diagram.edges[ei].label_extents {
            let route = diagram.edges[ei].route.clone();
            if let Some(anchor) = seek_label_anchor(&route, extents, &label_occupancy) {
                diagram.edges[ei].label_anchor = Some(anchor);
                label_occupancy.occupy_label(anchor, extents);
            }
        }
    }
}

/// For each candidate port pair run A* between the two *stalk* cells
/// (one orthogonal step outside each port) and return the cheapest
/// resulting cell sequence — including the port cells themselves so
/// the rendered polyline still terminates at the node boundary.
///
/// Panics if no candidate yields a path. The previous "always emit
/// something" contract silently fell back to a straight port-to-port
/// line, which sliced through any node sitting between the two
/// endpoints (the so-called "laser beam" bug). Panicking surfaces the
/// underlying occupancy violation immediately during testing so it can
/// be fixed at the source rather than papered over at the renderer.
fn best_route(
    from: Point,
    to: Point,
    src_size: crate::ast::NodeSize,
    tgt_size: crate::ast::NodeSize,
    candidates: &[(PortDirection, PortDirection)],
    occupancy: &GridOccupancy,
    from_id: &str,
    to_id: &str,
) -> Vec<(i32, i32)> {
    let mut best: Option<(u32, Vec<(i32, i32)>)> = None;

    for &(src_port, tgt_port) in candidates {
        let port_src = port_cell(from, src_size, src_port);
        let port_tgt = port_cell(to, tgt_size, tgt_port);
        let (sox, soy) = outward_offset(src_port);
        let (tox, toy) = outward_offset(tgt_port);
        let stalk_src = (port_src.0 + sox, port_src.1 + soy);
        let stalk_tgt = (port_tgt.0 + tox, port_tgt.1 + toy);

        let Some((path, cost)) = astar_cells(stalk_src, stalk_tgt, occupancy) else {
            continue;
        };

        // Compose: port → stalk → … → stalk → port.
        let mut full = Vec::with_capacity(path.len() + 2);
        full.push(port_src);
        full.extend(path);
        full.push(port_tgt);

        if best.as_ref().is_none_or(|(c, _)| cost < *c) {
            best = Some((cost, full));
        }
    }

    match best {
        Some((_, full)) => full,
        None => panic!(
            "A* routing failed for edge {from_id} -> {to_id}: no candidate \
             port pair yielded a path through the occupancy grid"
        ),
    }
}

/// Project a node-centre endpoint onto the axis shared with the
/// adjacent port cell so the trim segment that bridges the centre to
/// the port stays strictly orthogonal. Without this projection, any
/// node whose centre coordinate isn't a multiple of 10 px produces a
/// tiny diagonal jog where the polyline crosses the node boundary —
/// the visible "off-angle" entry that the transit-map aesthetic
/// can't tolerate.
fn snap_endpoint_to_cell_axis(endpoint: Point, cell_x: i32, cell_y: i32) -> Point {
    let port_px = cell_x * 10;
    let port_py = cell_y * 10;
    // The port cell sits on the node boundary in one axis and offset
    // in the other; snap the endpoint to share the boundary axis
    // (whichever has the smaller deviation from the endpoint).
    if (endpoint.x - port_px).abs() <= (endpoint.y - port_py).abs() {
        Point::new(port_px, endpoint.y)
    } else {
        Point::new(endpoint.x, port_py)
    }
}

/// Generate a clean orthogonal route between two port cells for a
/// star (hub-and-spoke) edge. Returns a cell sequence that takes
/// exactly one orthogonal step from the source port, walks directly
/// to the target's row/column, then steps into the target port.
///
/// This avoids running A* on very short star-hop distances where
/// grid-cell quantization creates jittery zigzags.
fn direct_star_route(port_src: (i32, i32), port_tgt: (i32, i32)) -> Vec<(i32, i32)> {
    let (sx, sy) = port_src;
    let (tx, ty) = port_tgt;

    // For a clean orthogonal L-shaped path: port_src → corner → port_tgt.
    let mut cells = Vec::with_capacity(8);
    cells.push(port_src);

    // Walk horizontally from source to target X, then vertically to target Y.
    let mut cx = sx;
    let mut cy = sy;
    let dx = (tx - cx).signum();
    let dy = (ty - cy).signum();

    if dx != 0 {
        // Move horizontally first.
        while cx != tx {
            cx += dx;
            cells.push((cx, cy));
        }
    }
    if dy != 0 {
        // Then move vertically.
        while cy != ty {
            cy += dy;
            cells.push((cx, cy));
        }
    }

    // Deduplicate consecutive identical entries.
    cells.dedup();
    cells
}

/// Rasterise an octilinear pixel polyline into the grid cells it covers.
fn cells_along_polyline(route: &[Point]) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    if route.is_empty() {
        return out;
    }
    out.push((route[0].x / 10, route[0].y / 10));
    for win in route.windows(2) {
        let (a, b) = (win[0], win[1]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let steps = (dx.abs() / 10).max(dy.abs() / 10).max(1);
        for s in 1..=steps {
            let x = a.x + dx * s / steps;
            let y = a.y + dy * s / steps;
            out.push((x / 10, y / 10));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::ports::pick_forward_ports;
    use crate::ast::PortDirection;
    use crate::layout::layout_backbone;
    use crate::measure::measure_diagram;
    use crate::parser::parse_dsl;

    fn render_pipeline(dsl: &str) -> crate::ast::Diagram {
        let mut d = parse_dsl(dsl).unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        d
    }

    #[test]
    fn test_forward_edge_routing() {
        let d = render_pipeline("A -> B : x\nB -> C : y\n");
        for edge in &d.edges {
            assert!(!edge.is_cyclic);
            assert!(!edge.route.is_empty(), "forward edge {} -> {} has no route", edge.from, edge.to);
        }
    }

    #[test]
    fn test_cyclic_edge_routing() {
        let d = render_pipeline("A -> B\nB -> C\nC -> A\n");
        let cyclic_count = d.edges.iter().filter(|e| e.is_cyclic).count();
        assert_eq!(cyclic_count, 1);
        for edge in &d.edges {
            assert!(!edge.route.is_empty());
        }
    }

    #[test]
    fn test_back_edge_routes_around_chain() {
        // With dynamic port candidates the back-edge is no longer pinned
        // to (South, North); the cost-based selector now picks whichever
        // pair yields the shortest legal A* path. The contract that
        // remains: a back-edge in a 3-node cycle must produce a
        // multi-segment polyline that differs from a straight node-to-
        // node connector.
        let d = render_pipeline("A -> B\nB -> C\nC -> A\n");
        let back = d.edges.iter().find(|e| e.is_cyclic).expect("back-edge");
        assert!(
            back.route.len() > 2,
            "back-edge must have intermediate waypoints, got {:?}",
            back.route
        );
    }

    #[test]
    fn test_grid_occupancy_node_blocks() {
        let d = render_pipeline("A -> B\nB -> C\n");
        let occ = GridOccupancy::new(&d);
        for node in &d.nodes {
            if let Some(pos) = node.position {
                let cell = (pos.x / 10, pos.y / 10);
                assert!(occ.is_node(cell), "node {} centre cell must be occupied", node.id);
            }
        }
    }

    #[test]
    fn test_route_no_crash_on_unpositioned_node() {
        let mut d = parse_dsl("A -> B\n").unwrap();
        for node in &mut d.nodes {
            node.position = None;
        }
        route_all_edges(&mut d);
        assert!(d.edges[0].route.is_empty());
    }

    #[test]
    fn test_route_diamond_with_cycle() {
        let d = render_pipeline("A -> B\nB -> D\nA -> C\nC -> D\nD -> A\n");
        for edge in &d.edges {
            assert!(!edge.route.is_empty(), "edge {} -> {} has no route", edge.from, edge.to);
        }
        assert!(d.edges.iter().any(|e| e.is_cyclic));
    }

    #[test]
    fn test_label_anchor_set_for_labelled_edges() {
        let d = render_pipeline("A -> B : trigger\n");
        assert!(d.edges[0].label_anchor.is_some());
    }

    // ---- TTB forward-edge selection ---------------------------------------

    #[test]
    fn test_pick_forward_ports_same_column_below() {
        let (s, t) = pick_forward_ports(Point::new(200, 100), Point::new(200, 300));
        assert_eq!(s, PortDirection::South);
        assert_eq!(t, PortDirection::North);
    }

    #[test]
    fn test_pick_forward_ports_vertical_dominant() {
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(150, 400));
        assert_eq!(s, PortDirection::South);
        assert_eq!(t, PortDirection::North);
    }

    #[test]
    fn test_pick_forward_ports_same_y_uses_east_west() {
        let (s, t) = pick_forward_ports(Point::new(100, 200), Point::new(300, 200));
        assert_eq!(s, PortDirection::East);
        assert_eq!(t, PortDirection::West);
    }

    #[test]
    fn test_pick_forward_ports_same_y_westward() {
        let (s, t) = pick_forward_ports(Point::new(500, 250), Point::new(300, 250));
        assert_eq!(s, PortDirection::West);
        assert_eq!(t, PortDirection::East);
    }

    #[test]
    fn test_pick_forward_ports_downward_dominant() {
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(200, 500));
        assert_eq!(s, PortDirection::South);
        assert_eq!(t, PortDirection::North);
    }
}
