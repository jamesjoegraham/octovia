//! Edge routing phase.
//!
//! Back-edges and cycles are routed using a deterministic three-segment
//! U-bend; forward edges prefer a single axis-aligned straight line and
//! fall back to A* through the occupancy grid for true diagonals. The
//! transit-map aesthetic is enforced by:
//!
//! -   **Octilinearity**: only axis-aligned and 45° diagonal moves.
//! -   **Port assignment**: forward edges use East/West/North/South ports;
//!     cyclic edges use North/South ports to prevent overlap.
//! -   **Lane spreading**: parallel forward edges that share a straight
//!     segment are fanned out by one grid cell each.
//!
//! The submodules:
//! -   [`occupancy`] — `GridOccupancy`, the cell-blocking grid.
//! -   [`ports`] — port placement and forward-edge port selection.
//! -   [`astar`] — straight-line and A* path search primitives.
//! -   [`cyclic`] — cyclic U-bend routing.
//! -   [`lanes`] — post-routing parallel-lane offset pass.

mod astar;
mod cyclic;
mod lanes;
mod occupancy;
mod ports;

pub use occupancy::GridOccupancy;

use std::collections::HashMap;

use crate::ast::{Diagram, Point};

use astar::{astar_cells, straight_line_cells};
use cyclic::route_cyclic_u_bend;
use lanes::assign_parallel_lanes;
use ports::{pick_forward_ports, port_cell};

/// Route all cyclic/back-edges in a diagram, mutating them in place.
/// Forward edges are routed as simple straight lines between East/West ports.
pub fn route_all_edges(diagram: &mut Diagram) {
    let mut occupancy = GridOccupancy::new(diagram);
    let positions: HashMap<&str, Point> = diagram
        .nodes
        .iter()
        .filter_map(|n| n.position.map(|p| (n.id.as_str(), p)))
        .collect();

    // First pass: route forward (non-cyclic) edges. Ports are picked from
    // the relative position of the two nodes, so the path is a clean
    // single-axis straight line whenever the geometry permits (E↔W for
    // same-row, N↔S for same-column). True diagonals — forward edges that
    // skip ahead across both axes — fall back to A* through the occupancy
    // grid; this still respects the octilinear / 8-port aesthetic.
    for edge in &mut diagram.edges {
        let from = match positions.get(edge.from.as_str()) {
            Some(p) => *p,
            None => continue,
        };
        let to = match positions.get(edge.to.as_str()) {
            Some(p) => *p,
            None => continue,
        };

        if edge.is_cyclic {
            // Will be routed in second pass
            continue;
        }

        let (src_port, tgt_port) = pick_forward_ports(from, to);
        let start = port_cell(from, src_port);
        let end = port_cell(to, tgt_port);

        let cells = match straight_line_cells(start, end) {
            Some(line) => line,
            None => astar_cells(start, end, &occupancy)
                .unwrap_or_else(|| vec![start, end]),
        };

        edge.route = cells.iter().map(|&(cx, cy)| Point::new(cx * 10, cy * 10)).collect();
        occupancy.occupy_path(&cells);
    }

    // Second pass: route cyclic edges as deterministic U-bends below the
    // backbone — exit source.S, run along a clear corridor, enter target.N.
    // Each route's corridor row scoots down on collision so back-edges
    // stack instead of overlapping.
    for edge in &mut diagram.edges {
        if !edge.is_cyclic {
            continue;
        }

        let from = match positions.get(edge.from.as_str()) {
            Some(p) => *p,
            None => continue,
        };
        let to = match positions.get(edge.to.as_str()) {
            Some(p) => *p,
            None => continue,
        };

        let cells = route_cyclic_u_bend(from, to, &occupancy);
        edge.route = cells.iter().map(|&(cx, cy)| Point::new(cx * 10, cy * 10)).collect();
        occupancy.occupy_path(&cells);
    }

    // Third pass: assign lanes to parallel forward edges so they're spaced
    // one grid cell (10 px) apart instead of drawing on top of each other.
    assign_parallel_lanes(diagram);
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::occupancy::BLOCK_HALF;
    use super::ports::pick_forward_ports;
    use crate::ast::PortDirection;
    use crate::layout::layout_backbone;
    use crate::parser::parse_dsl;

    #[test]
    fn test_forward_edge_routing() {
        let mut d = parse_dsl("A -> B : x\nB -> C : y\n").unwrap();
        layout_backbone(&mut d);

        // Route edges
        route_all_edges(&mut d);

        // Forward edges should have routes
        for edge in &d.edges {
            assert!(!edge.route.is_empty());
            assert!(!edge.is_cyclic);
        }
    }

    #[test]
    fn test_cyclic_edge_routing() {
        let mut d = parse_dsl("A -> B\nB -> C\nC -> A\n").unwrap();
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let cyclic_count = d.edges.iter().filter(|e| e.is_cyclic).count();
        assert!(cyclic_count >= 1);

        for edge in &d.edges {
            assert!(!edge.route.is_empty());
        }
    }

    #[test]
    fn test_grid_occupancy_node_blocks() {
        let mut d = parse_dsl("A -> B\nB -> C\n").unwrap();
        layout_backbone(&mut d);
        let occ = GridOccupancy::new(&d);
        for node in &d.nodes {
            if let Some(pos) = node.position {
                let cell = (pos.x / 10, pos.y / 10);
                assert!(!occ.is_free(cell),
                    "cell ({},{}) should be occupied by node {}", cell.0, cell.1, node.id);
            }
        }
    }

    #[test]
    fn test_route_forward_edge_straight_line() {
        let mut d = parse_dsl("A -> B\n").unwrap();
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let edge = &d.edges[0];
        assert!(!edge.route.is_empty());
        if !edge.route.is_empty() {
            let y = edge.route[0].y;
            for p in &edge.route {
                assert_eq!(p.y, y, "forward edge should be horizontal");
            }
        }
    }

    #[test]
    fn test_route_cyclic_edge_different_port() {
        let mut d = parse_dsl("A -> B\nB -> C\nC -> A\n").unwrap();
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let c_to_a = d.edges.iter().find(|e| e.from == "C" && e.to == "A").unwrap();
        assert!(c_to_a.route.len() >= 2);
    }

    #[test]
    fn test_route_diamond_with_cycle() {
        let mut d = parse_dsl("A -> B\nB -> D\nA -> C\nC -> D\nD -> A\n").unwrap();
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        for edge in &d.edges {
            assert!(!edge.route.is_empty(), "edge {} -> {} has no route", edge.from, edge.to);
        }
        // At least one edge should be detected as cyclic
        assert!(d.edges.iter().any(|e| e.is_cyclic), "no cyclic edges detected");
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

    // ---- 8-port forward-edge selection -------------------------------------

    #[test]
    fn test_pick_forward_ports_same_row_east() {
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(300, 100));
        assert_eq!(s, PortDirection::East);
        assert_eq!(t, PortDirection::West);
    }

    #[test]
    fn test_pick_forward_ports_same_row_west() {
        // Target to the left of source on the same row (e.g. boustrophedon RTL row)
        let (s, t) = pick_forward_ports(Point::new(500, 250), Point::new(300, 250));
        assert_eq!(s, PortDirection::West);
        assert_eq!(t, PortDirection::East);
    }

    #[test]
    fn test_pick_forward_ports_same_column_below() {
        // Vertical wrap: same x, target below source (the case from the
        // "Document Workflow" example that was previously broken).
        let (s, t) = pick_forward_ports(Point::new(500, 100), Point::new(500, 250));
        assert_eq!(s, PortDirection::South);
        assert_eq!(t, PortDirection::North);
    }

    #[test]
    fn test_pick_forward_ports_same_column_above() {
        let (s, t) = pick_forward_ports(Point::new(100, 250), Point::new(100, 100));
        assert_eq!(s, PortDirection::North);
        assert_eq!(t, PortDirection::South);
    }

    #[test]
    fn test_pick_forward_ports_diagonal_dominant_horizontal() {
        // |dx| > |dy| → horizontal ports
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(500, 200));
        assert_eq!(s, PortDirection::East);
        assert_eq!(t, PortDirection::West);
    }

    #[test]
    fn test_pick_forward_ports_diagonal_dominant_vertical() {
        // |dy| > |dx| → vertical ports
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(150, 400));
        assert_eq!(s, PortDirection::South);
        assert_eq!(t, PortDirection::North);
    }

    #[test]
    fn test_vertical_forward_edge_is_routed() {
        // Reproducer for the "Document Workflow" bug:
        // the spanning-tree wrap edge connects a node to one directly below
        // it on the next boustrophedon row. It must produce a real vertical
        // route, not collapse to a horizontal stub at the source's y.
        let mut d = parse_dsl(
            "Draft -> Review : submit\n\
             Review -> Approved : approve\n\
             Review -> Revisions : revise\n\
             Revisions -> Draft : redraft\n\
             Revisions -> Review : resubmit\n\
             Approved -> Published : publish\n",
        )
        .unwrap();
        // Narrow viewport → 2 nodes per row → forces the wrap edge to be
        // vertical (matches the playground panel size in the screenshot).
        d.viewport = crate::ast::Viewport { width: 900, height: 800 };
        crate::measure::measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        // Locate the wrap edge: a forward edge whose endpoints are on
        // different rows (different y).
        let wrap_edge = d
            .edges
            .iter()
            .find(|e| {
                if e.is_cyclic {
                    return false;
                }
                let from_y = d.node(&e.from).and_then(|n| n.position).map(|p| p.y);
                let to_y = d.node(&e.to).and_then(|n| n.position).map(|p| p.y);
                matches!((from_y, to_y), (Some(a), Some(b)) if a != b)
            })
            .expect("expected at least one cross-row forward edge");

        // The route must actually traverse meaningful y-distance — not be
        // pinned to the source's row.
        assert!(wrap_edge.route.len() >= 2, "wrap edge must have a real path");
        let y_min = wrap_edge.route.iter().map(|p| p.y).min().unwrap();
        let y_max = wrap_edge.route.iter().map(|p| p.y).max().unwrap();
        assert!(
            y_max - y_min >= 80,
            "wrap edge {} -> {} collapsed to {}..{} px (delta {}); expected >= 80",
            wrap_edge.from,
            wrap_edge.to,
            y_min,
            y_max,
            y_max - y_min
        );

        // And it must end inside the target node's catchment, not floating
        // at the source's y-coordinate. Cardinal ports sit 30px from centre
        // (3 grid cells), so allow up to ~40px slack from the node centre.
        let tgt = d.node(&wrap_edge.to).unwrap().position.unwrap();
        let last = *wrap_edge.route.last().unwrap();
        assert!(
            (last.y - tgt.y).abs() <= 40,
            "edge endpoint y={} should land near target {} y={}",
            last.y,
            wrap_edge.to,
            tgt.y
        );
    }

    #[test]
    fn test_vertical_forward_edge_is_axis_aligned() {
        // Same-column forward edge → all route points share the same x.
        let mut d = parse_dsl("A -> B\n").unwrap();
        crate::measure::measure_diagram(&mut d);
        layout_backbone(&mut d);
        // Force same-column placement.
        if let Some(a) = d.node_mut("A") { a.position = Some(Point::new(300, 100)); }
        if let Some(b) = d.node_mut("B") { b.position = Some(Point::new(300, 300)); }
        route_all_edges(&mut d);
        let edge = &d.edges[0];
        assert!(!edge.is_cyclic);
        let xs: std::collections::HashSet<i32> = edge.route.iter().map(|p| p.x).collect();
        assert_eq!(xs.len(), 1, "vertical forward edge must have constant x");
    }

    // ---- Cyclic U-bend ------------------------------------------------------

    /// Helper: classify direction transitions in a route to verify that it
    /// is a clean three-segment U-bend (down → across → up).
    fn segment_dirs(route: &[Point]) -> Vec<&'static str> {
        if route.len() < 2 {
            return Vec::new();
        }
        let mut dirs = Vec::new();
        for w in route.windows(2) {
            let dx = (w[1].x - w[0].x).signum();
            let dy = (w[1].y - w[0].y).signum();
            let d = match (dx, dy) {
                (0, 1) => "S",
                (0, -1) => "N",
                (1, 0) => "E",
                (-1, 0) => "W",
                _ => "?",
            };
            if dirs.last().copied() != Some(d) {
                dirs.push(d);
            }
        }
        dirs
    }

    #[test]
    fn test_traffic_light_cycle_renders_u_bend() {
        // Reproducer for the "Traffic Light" screenshot: Red -> Green is a
        // back-edge that previously vanished because A* started inside the
        // node's blocked square. It must now route South -> West -> North.
        let mut d = parse_dsl(
            "Green -> Yellow : timer\n\
             Yellow -> Red : timer\n\
             Red -> Green : timer\n",
        )
        .unwrap();
        crate::measure::measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let back = d
            .edges
            .iter()
            .find(|e| e.from == "Red" && e.to == "Green")
            .expect("Red -> Green edge must exist");
        assert!(back.is_cyclic, "Red -> Green must be classified cyclic");
        assert!(back.route.len() >= 3, "U-bend must have at least 3 cells");

        // Three-segment U-bend: down, then west, then up.
        assert_eq!(
            segment_dirs(&back.route),
            vec!["S", "W", "N"],
            "Red -> Green back-edge must route S -> W -> N"
        );

        // Endpoints land near the source's South port and target's North port.
        // Compare in grid cells (10px) since ports snap to the grid.
        let red = d.node("Red").unwrap().position.unwrap();
        let green = d.node("Green").unwrap().position.unwrap();
        let first = back.route.first().unwrap();
        let last = back.route.last().unwrap();
        assert_eq!(first.x / 10, red.x / 10, "U-bend must start in source's column");
        assert!(first.y > red.y, "U-bend must start by moving down from source");
        assert_eq!(last.x / 10, green.x / 10, "U-bend must end in target's column");
        assert!(last.y < red.y + 200, "U-bend must end above the corridor");
    }

    #[test]
    fn test_cyclic_corridor_below_both_nodes() {
        // The horizontal segment of the U-bend must sit *below* every node
        // block — it must not cross through any node.
        let mut d = parse_dsl(
            "A -> B : x\n\
             B -> C : y\n\
             C -> A : back\n",
        )
        .unwrap();
        crate::measure::measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let occupancy = GridOccupancy::new(&d);
        let back = d.edges.iter().find(|e| e.is_cyclic).expect("cyclic edge");

        // The corridor is the route's run of points sharing a y-coordinate.
        // Every cell strictly between the first and last must be free of
        // any node block (endpoints are allowed to ride through their own
        // node's block — the rectangle hides the segment in z-order).
        for (i, p) in back.route.iter().enumerate() {
            if i == 0 || i == back.route.len() - 1 {
                continue;
            }
            let cell = (p.x / 10, p.y / 10);
            // Allow cells inside the source or target's own block (entry/exit
            // tail), but not any *other* node.
            let mut blocked_by_other = false;
            for node in &d.nodes {
                if node.id == back.from || node.id == back.to {
                    continue;
                }
                if let Some(np) = node.position {
                    let dx = (cell.0 - np.x / 10).abs();
                    let dy = (cell.1 - np.y / 10).abs();
                    if dx <= BLOCK_HALF && dy <= BLOCK_HALF {
                        blocked_by_other = true;
                        break;
                    }
                }
            }
            assert!(
                !blocked_by_other,
                "cyclic corridor cell {:?} crosses an unrelated node block",
                cell
            );
            let _ = occupancy; // keep `occupancy` referenced for clarity
        }
    }

    #[test]
    fn test_multiple_cyclic_edges_stack() {
        // Two back-edges over the same nodes must use *different* corridor
        // rows so they don't draw on top of each other.
        let mut d = parse_dsl(
            "A -> B : x\n\
             B -> C : y\n\
             C -> A : back1\n\
             B -> A : back2\n",
        )
        .unwrap();
        crate::measure::measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let cyclics: Vec<&_> = d.edges.iter().filter(|e| e.is_cyclic).collect();
        assert!(cyclics.len() >= 2, "expected at least two cyclic edges");

        // Each cyclic edge has a horizontal corridor — find the y of any
        // run of three or more points sharing a y. Those y-values must
        // differ between the two cyclics.
        fn corridor_y(route: &[Point]) -> Option<i32> {
            let mut counts: HashMap<i32, usize> = HashMap::new();
            for p in route {
                *counts.entry(p.y).or_default() += 1;
            }
            counts.into_iter().filter(|(_, c)| *c >= 3).map(|(y, _)| y).max()
        }

        let ys: Vec<i32> = cyclics.iter().filter_map(|e| corridor_y(&e.route)).collect();
        assert_eq!(ys.len(), cyclics.len(), "every cyclic must have a corridor");
        let unique: std::collections::HashSet<i32> = ys.iter().copied().collect();
        assert_eq!(
            unique.len(),
            cyclics.len(),
            "cyclic corridors must occupy distinct rows; got {:?}",
            ys
        );
    }
}
