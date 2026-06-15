//! Cyclic Routing Phase (Phase 3): A* pathfinding through empty grid space.
//!
//! Back-edges and cycles are routed using a modified A* that enforces
//! the transit-map aesthetic:
//!
//! -   **Octilinearity**: Only axis-aligned and 45° diagonal moves.
//! -   **Turn penalty** `P_turn`: heavily penalises directional changes to
//!     force long, straight runs.
//! -   **Crossing penalty** `P_cross`: penalises intersecting previously
//!     placed tracks.
//! -   **Port assignment**: forward edges use East/West ports; cyclic
//!     edges use North/South ports to prevent overlap.

use std::collections::{HashMap, HashSet};

use pathfinding::prelude::astar;

use crate::ast::{Diagram, Point, PortDirection};
use crate::layout::NODE_RADIUS;

// ---------------------------------------------------------------------------
// Grid occupancy
// ---------------------------------------------------------------------------

/// Tracks which grid cells are occupied by nodes or already-routed edges.
#[derive(Debug, Clone)]
pub struct GridOccupancy {
    /// Cells occupied by nodes (no routing through these).
    node_cells: HashSet<(i32, i32)>,
    /// Cells occupied by routed edge segments.
    edge_cells: HashSet<(i32, i32)>,
}

impl GridOccupancy {
    pub fn new(diagram: &Diagram) -> Self {
        let mut node_cells = HashSet::new();
        for node in &diagram.nodes {
            if let Some(pos) = node.position {
                // Mark a square around each node as occupied
                let r = BLOCK_HALF;
                for dx in -r..=r {
                    for dy in -r..=r {
                        node_cells.insert((pos.x / 10 + dx, pos.y / 10 + dy));
                    }
                }
            }
        }
        Self {
            node_cells,
            edge_cells: HashSet::new(),
        }
    }

    /// Check if a cell is free (neither node nor existing edge occupies it).
    pub fn is_free(&self, cell: (i32, i32)) -> bool {
        !self.node_cells.contains(&cell) && !self.edge_cells.contains(&cell)
    }

    /// Occupy cells along a routed path.
    pub fn occupy_path(&mut self, path: &[(i32, i32)]) {
        for &cell in path {
            self.edge_cells.insert(cell);
        }
    }

    /// Check how many times a path would cross occupied cells.
    pub fn crossing_count(&self, path: &[(i32, i32)]) -> u32 {
        if path.len() < 2 {
            return 0;
        }
        // Count intermediate cells that intersect the edge grid
        path.iter()
            .skip(1)
            .take(path.len().saturating_sub(2))
            .filter(|&&c| self.edge_cells.contains(&c))
            .count() as u32
    }
}

// ---------------------------------------------------------------------------
// Port computation
// ---------------------------------------------------------------------------

/// Compute the port (starting cell) for a given node and direction.
fn port_cell(node_pos: Point, direction: PortDirection) -> (i32, i32) {
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
/// The 8-port rule: forward edges always exit/enter on a cardinal port,
/// chosen so the resulting path is a clean straight line along a single
/// axis whenever the geometry permits.
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

/// Build a straight-line path of grid cells between two cells that share
/// either an x- or a y-coordinate. Returns `None` when the cells are not
/// axis-aligned (in which case A* should be used instead).
fn straight_line_cells(start: (i32, i32), end: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    if start.0 == end.0 {
        // Vertical run
        let step = (end.1 - start.1).signum();
        if step == 0 {
            return Some(vec![start]);
        }
        let mut cells = Vec::new();
        let mut y = start.1;
        loop {
            cells.push((start.0, y));
            if y == end.1 {
                break;
            }
            y += step;
        }
        Some(cells)
    } else if start.1 == end.1 {
        // Horizontal run
        let step = (end.0 - start.0).signum();
        if step == 0 {
            return Some(vec![start]);
        }
        let mut cells = Vec::new();
        let mut x = start.0;
        loop {
            cells.push((x, start.1));
            if x == end.0 {
                break;
            }
            x += step;
        }
        Some(cells)
    } else {
        None
    }
}

/// Run A* between two cells using the existing octilinear cost function.
/// Used as the fallback for forward edges that cannot be routed as a single
/// axis-aligned straight line.
fn astar_cells(
    start: (i32, i32),
    end: (i32, i32),
    occupancy: &GridOccupancy,
) -> Option<Vec<(i32, i32)>> {
    astar(
        &start,
        |&cell| {
            neighbours(cell)
                .into_iter()
                .filter(|(c, _)| occupancy.is_free(*c) || *c == end)
                .collect::<Vec<_>>()
        },
        |&cell| octile_distance(cell, end),
        |&cell| cell == end,
    )
    .map(|(path, _cost)| path)
}

/// Block half-width (in cells) used by `GridOccupancy` to mark each node
/// as impassable. A path may cross *outside* this radius without colliding.
const BLOCK_HALF: i32 = NODE_RADIUS / 10 + 2;

/// Route a cyclic (back-)edge as a deterministic 3-segment U-bend:
///
/// ```text
///       ┌── source ──┐                     ┌── target ──┐
///       │            │                     │            │
///       └─────●──────┘                     └─────●──────┘
///             │ S                                ▲ N
///             │                                  │
///             ▼          (corridor band)         │
///             ●──────────────────────────────────●
/// ```
///
/// 1. Exit `source.S`, run vertically down to a horizontal corridor that
///    lives **below both nodes' blocking squares**.
/// 2. Run horizontally along the corridor to the target's column.
/// 3. Run vertically up into `target.N`.
///
/// The corridor row scoots further down on collision so multiple cyclic
/// edges stack instead of overlapping. Endpoints are recorded directly —
/// they ride *through* the source/target blocks because the rectangles
/// are drawn on top in z-order, hiding the segment that's inside the node.
fn route_cyclic_u_bend(
    from: Point,
    to: Point,
    occupancy: &GridOccupancy,
) -> Vec<(i32, i32)> {
    let start = port_cell(from, PortDirection::South);
    let end = port_cell(to, PortDirection::North);

    // The corridor must clear both source and target blocks. Each block
    // extends `BLOCK_HALF` cells below its node centre.
    let from_block_bottom = from.y / 10 + BLOCK_HALF;
    let to_block_bottom = to.y / 10 + BLOCK_HALF;
    let base_band = from_block_bottom.max(to_block_bottom) + 1;

    let x_lo = start.0.min(end.0);
    let x_hi = start.0.max(end.0);

    // Find a free row for the horizontal corridor; if congested, drop
    // further down. Capped to keep the search bounded.
    let mut band = base_band;
    for _ in 0..16 {
        let clear = (x_lo..=x_hi).all(|x| occupancy.is_free((x, band)));
        if clear {
            break;
        }
        band += 2;
    }

    let mut path: Vec<(i32, i32)> = Vec::new();

    // Segment 1: source.S downward to corridor
    let mut y = start.1;
    while y < band {
        path.push((start.0, y));
        y += 1;
    }

    // Segment 2: along the corridor
    let dx = (end.0 - start.0).signum();
    if dx == 0 {
        path.push((start.0, band));
    } else {
        let mut x = start.0;
        loop {
            path.push((x, band));
            if x == end.0 {
                break;
            }
            x += dx;
        }
    }

    // Segment 3: corridor upward into target.N
    let mut y = band - 1;
    while y >= end.1 {
        path.push((end.0, y));
        y -= 1;
    }

    path
}


// ---------------------------------------------------------------------------
// A* with transit-map cost function
// ---------------------------------------------------------------------------

/// Heuristic: octile distance (Chebyshev with diagonal cost √2 ≈ 14/10).
fn octile_distance(a: (i32, i32), b: (i32, i32)) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs();
    let dy = (a.1 - b.1).unsigned_abs();
    let d = dx.min(dy);
    // Straight distance + diagonal distance (14/10 ≈ √2)
    (dx + dy - d) * 10 + d * 14
}

/// Neighbours for octilinear movement: 8 directions.
fn neighbours(cell: (i32, i32)) -> Vec<((i32, i32), u32)> {
    let (x, y) = cell;
    vec![
        ((x + 1, y), 10),      // East
        ((x - 1, y), 10),      // West
        ((x, y + 1), 10),      // South
        ((x, y - 1), 10),      // North
        ((x + 1, y - 1), 14),  // NE
        ((x - 1, y - 1), 14),  // NW
        ((x + 1, y + 1), 14),  // SE
        ((x - 1, y + 1), 14),  // SW
    ]
}

// ---------------------------------------------------------------------------
// Batch route all cyclic edges
// ---------------------------------------------------------------------------

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_dsl;
    use crate::layout::layout_backbone;

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
