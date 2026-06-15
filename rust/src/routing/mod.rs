//! Unified routing + label placement phase.
//!
//! For every edge, in input order:
//!
//! 1. Pick ports based on the edge's role (forward → cardinal axis;
//!    back-edge → S/N so A* descends into the bottom routing channel).
//! 2. Run A* through the [`GridOccupancy`] to find an octilinear path.
//! 3. Reserve every cell of the resolved path back into the grid.
//! 4. If the edge has a label, search the local neighbourhood of the
//!    route for a free anchor and reserve that label's bounding box.
//! 5. Move on to the next edge.
//!
//! Because every committed route and every committed label updates the
//! same occupancy grid, subsequent A* searches treat both as impassable
//! terrain and naturally route around them.

mod astar;
mod lanes;
mod occupancy;
mod ports;

pub use occupancy::GridOccupancy;

use std::collections::HashMap;

use crate::ast::{Diagram, Point};
use crate::label_placement::seek_label_anchor;

use astar::astar_cells;
use lanes::assign_parallel_lanes;
use ports::{pick_back_ports, pick_forward_ports, port_cell};

/// Route every edge through the universal A* router, placing each edge's
/// label immediately after its route is committed.
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

        let (src_port, tgt_port) = if is_cyclic {
            pick_back_ports()
        } else {
            pick_forward_ports(from, to)
        };
        let start = port_cell(from, src_size, src_port);
        let end = port_cell(to, tgt_size, tgt_port);

        let cells = astar_cells(start, end, &occupancy).unwrap_or_else(|| vec![start, end]);

        // Convert grid cells back to pixel coordinates and bookend with the
        // source / target centres so SVG `trim_route_to_node_boundaries`
        // can clip the polyline cleanly to each rectangle's edge.
        let mut route: Vec<Point> = Vec::with_capacity(cells.len() + 2);
        route.push(from);
        for (cx, cy) in &cells {
            route.push(Point::new(cx * 10, cy * 10));
        }
        route.push(to);
        diagram.edges[ei].route = route.clone();
        occupancy.occupy_path(&cells);

        // Place the label and reserve its bounding box.
        if let Some(extents) = diagram.edges[ei].label_extents {
            if let Some(anchor) = seek_label_anchor(&route, extents, &occupancy) {
                diagram.edges[ei].label_anchor = Some(anchor);
                occupancy.occupy_label(anchor, extents);
            }
        }
    }

    // Post-pass: spread parallel forward edges that ended up sharing a
    // straight segment so they don't draw on top of each other.
    assign_parallel_lanes(diagram);
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
    fn test_back_edge_uses_south_north_ports() {
        // Back-edge ports are deterministic: S out of source, N into target.
        // Routes are bookended with node centres for clean trimming, so we
        // probe the polyline interior for the descent and ascent.
        let d = render_pipeline("A -> B\nB -> A\n");
        let back = d.edges.iter().find(|e| e.is_cyclic).expect("back-edge");
        let src = d.node(&back.from).unwrap().position.unwrap();
        let tgt = d.node(&back.to).unwrap().position.unwrap();
        let max_y = back.route.iter().map(|p| p.y).max().unwrap();
        let min_y_after_descent = back
            .route
            .iter()
            .skip_while(|p| p.y <= src.y)
            .map(|p| p.y)
            .min()
            .unwrap_or(src.y);
        assert!(max_y > src.y, "back-edge must descend below source");
        assert!(min_y_after_descent < tgt.y || max_y > tgt.y,
                "back-edge must reach target's vertical band");
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

    // ---- 8-port forward-edge selection -------------------------------------

    #[test]
    fn test_pick_forward_ports_same_row_east() {
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(300, 100));
        assert_eq!(s, PortDirection::East);
        assert_eq!(t, PortDirection::West);
    }

    #[test]
    fn test_pick_forward_ports_same_row_west() {
        let (s, t) = pick_forward_ports(Point::new(500, 250), Point::new(300, 250));
        assert_eq!(s, PortDirection::West);
        assert_eq!(t, PortDirection::East);
    }

    #[test]
    fn test_pick_forward_ports_same_column_below() {
        let (s, t) = pick_forward_ports(Point::new(500, 100), Point::new(500, 250));
        assert_eq!(s, PortDirection::South);
        assert_eq!(t, PortDirection::North);
    }

    #[test]
    fn test_pick_forward_ports_diagonal_dominant_horizontal() {
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(500, 200));
        assert_eq!(s, PortDirection::East);
        assert_eq!(t, PortDirection::West);
    }

    #[test]
    fn test_pick_forward_ports_diagonal_dominant_vertical() {
        let (s, t) = pick_forward_ports(Point::new(100, 100), Point::new(150, 400));
        assert_eq!(s, PortDirection::South);
        assert_eq!(t, PortDirection::North);
    }
}
