//! Layered Topological Layout (lightweight Sugiyama) with boustrophedon
//! (serpentine) macro-row folding.
//!
//! 1. Classify back-edges via DFS so the remaining graph is a DAG.
//! 2. Assign each node a layer `L(v)` via longest-path topological sort
//!    (Kahn's algorithm — **unchanged**).
//! 3. Within each layer, stack nodes vertically in input order.
//! 4. Fold the layers into macro-rows of at most `MAX_COLUMNS` columns
//!    each. Even macro-rows flow LTR, odd macro-rows flow RTL (serpentine).
//!    This produces a compact width ideal for mobile screens while
//!    preserving topological ordering within each fold.
//! 5. Map macro-rows → Y positions and effective-columns → X positions
//!    on a pixel grid, with explicit gutters between adjacent layers
//!    (LAYER_GUTTER), rows (ROW_GUTTER), and macro-rows
//!    (MACRO_ROW_GUTTER). The gutters are the routing channels A* uses
//!    for forward, back, and wrap-around edges.
//!
//! Every forward edge `e = (u, v)` still satisfies `L(u) < L(v)`, but
//! the physical X-coordinate may wrap: a node in layer 5 folded into
//! macro-row 1 appears to the *left* of a node in layer 4 in macro-row 0.
//! The A* router already reasons about geometry, not layer indices, so
//! this is transparent to the routing phase.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::ast::{Diagram, Point, Viewport};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of topological layers per macro-row. When the diagram
/// exceeds this many layers, the layout folds into serpentine macro-rows.
/// Default 4 layers fits comfortably on a mobile screen (360–480 dp).
pub const MAX_COLUMNS: i32 = 4;

/// Horizontal gutter (px) between adjacent layer columns. This is the
/// dedicated routing channel A* uses for forward and back-edge passes.
pub const LAYER_GUTTER: i32 = 90;

/// Vertical gutter (px) between adjacent rows within a macro-row.
pub const ROW_GUTTER: i32 = 70;

/// Vertical gutter (px) between adjacent macro-rows. Larger than
/// ROW_GUTTER to give wrap-around edge routing room to manoeuvre.
pub const MACRO_ROW_GUTTER: i32 = 90;

/// Outer margin around the diagram bounding box.
pub const MARGIN: i32 = 50;

/// Half-side of the smallest node; fallback when no node_size is set.
pub const NODE_RADIUS: i32 = 30;

// ---------------------------------------------------------------------------
// Back-edge classification (DFS)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

/// Identify back-edges: edges that close a cycle when the graph is DFS'd
/// in node insertion order. The set returned is a valid feedback arc set
/// — removing them leaves a DAG that we can topologically sort.
fn classify_back_edges(diagram: &Diagram) -> HashSet<usize> {
    let n = diagram.nodes.len();
    let id_to_idx: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Edges out of each node, recorded by their *edge index* so the result
    // points back into `diagram.edges`.
    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (ei, edge) in diagram.edges.iter().enumerate() {
        if let (Some(&fi), Some(&ti)) =
            (id_to_idx.get(edge.from.as_str()), id_to_idx.get(edge.to.as_str()))
        {
            adj[fi].push((ti, ei));
        }
    }

    let mut color = vec![Color::White; n];
    let mut back: HashSet<usize> = HashSet::new();

    // Iterative DFS to avoid Rust's stack-overflow risk on deep graphs.
    for start in 0..n {
        if color[start] != Color::White {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        color[start] = Color::Gray;
        while let Some(&(u, i)) = stack.last() {
            if let Some(&(v, ei)) = adj[u].get(i) {
                stack.last_mut().unwrap().1 += 1;
                match color[v] {
                    Color::White => {
                        color[v] = Color::Gray;
                        stack.push((v, 0));
                    }
                    Color::Gray => {
                        // Back-edge (also catches self-loops where u == v).
                        back.insert(ei);
                    }
                    Color::Black => {
                        // Cross / forward edge — keep in the DAG.
                    }
                }
            } else {
                color[u] = Color::Black;
                stack.pop();
            }
        }
    }

    back
}

// ---------------------------------------------------------------------------
// Longest-path layer assignment
// ---------------------------------------------------------------------------

/// Compute layer indices via longest-path topological sort over the DAG
/// formed by every non-back edge. Disconnected components share layer
/// indices — they all start at layer 0.
fn compute_layers(diagram: &Diagram, back: &HashSet<usize>) -> HashMap<String, i32> {
    let n = diagram.nodes.len();
    let id_to_idx: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    let mut forward_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_deg: Vec<usize> = vec![0; n];
    for (ei, edge) in diagram.edges.iter().enumerate() {
        if back.contains(&ei) {
            continue;
        }
        if let (Some(&fi), Some(&ti)) =
            (id_to_idx.get(edge.from.as_str()), id_to_idx.get(edge.to.as_str()))
        {
            // Skip self-edges defensively (DFS already classifies them as back).
            if fi == ti {
                continue;
            }
            forward_adj[fi].push(ti);
            in_deg[ti] += 1;
        }
    }

    let mut layer: Vec<i32> = vec![0; n];
    let mut queue: VecDeque<usize> = VecDeque::new();
    // Seed in node insertion order for determinism.
    for i in 0..n {
        if in_deg[i] == 0 {
            queue.push_back(i);
        }
    }

    while let Some(u) = queue.pop_front() {
        for &v in &forward_adj[u] {
            let candidate = layer[u] + 1;
            if candidate > layer[v] {
                layer[v] = candidate;
            }
            in_deg[v] -= 1;
            if in_deg[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.clone(), layer[i]))
        .collect()
}

// ---------------------------------------------------------------------------
// Macro-row helpers (boustrophedon folding)
// ---------------------------------------------------------------------------

/// Compute the effective column within a macro-row for a given layer.
///
/// Even macro-rows flow left-to-right: effective_col = layer % k.
/// Odd macro-rows flow right-to-left: effective_col = k - 1 - (layer % k).
fn effective_column(layer: i32, k: i32) -> i32 {
    let within_fold = layer % k;
    let macro_row = layer / k;
    if macro_row % 2 == 0 {
        within_fold
    } else {
        k - 1 - within_fold
    }
}

/// Ceil a value to the nearest 10 px grid cell boundary.
pub(crate) fn ceil_to_grid(n: i32) -> i32 {
    ((n + 9) / 10) * 10
}

// ---------------------------------------------------------------------------
// Coordinate placement (boustrophedon)
// ---------------------------------------------------------------------------

/// Final entry point: classify cycles, assign layers, place nodes on the
/// pixel grid using serpentine macro-row folding. Mutates
/// `diagram.nodes[*].position` and `diagram.edges[*].is_cyclic`.
///
/// Topological phase 2b (Kahn's longest-path sort) remains unchanged.
/// Only the physical coordinate assignment in this function uses macro-rows.
pub fn layout_backbone(diagram: &mut Diagram) {
    if diagram.nodes.is_empty() {
        return;
    }

    let back = classify_back_edges(diagram);
    let layers = compute_layers(diagram, &back);

    // Bucket node indices by layer in input order.
    let mut groups: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        let l = layers.get(&node.id).copied().unwrap_or(0);
        groups.entry(l).or_default().push(i);
    }

    let k = MAX_COLUMNS;

    // Per-layer column width = max node width in that layer.
    let mut layer_widths: BTreeMap<i32, i32> = BTreeMap::new();
    for (&l, indices) in &groups {
        let mut max_w = 2 * NODE_RADIUS;
        for &i in indices {
            let size = diagram.nodes[i].node_size.unwrap_or(crate::ast::NodeSize {
                width: 60,
                height: 60,
            });
            if size.width > max_w {
                max_w = size.width;
            }
        }
        layer_widths.insert(l, max_w);
    }

    // Compute per-effective-column (within each macro-row) the max width.
    // Also track per-row heights per macro-row so within each macro-row
    // rows top-align.
    let mut macro_row_count: i32 = 0;
    let mut macro_row_max_heights: Vec<Vec<i32>> = Vec::new(); // [macro_row][row]
    let mut macro_row_max_widths: Vec<Vec<i32>> = Vec::new(); // [macro_row][eff_col]

    for (&l, indices) in &groups {
        let mr = l / k;
        while macro_row_max_heights.len() <= mr as usize {
            macro_row_max_heights.push(Vec::new());
            macro_row_max_widths.push(Vec::new());
        }
        let ecol = effective_column(l, k) as usize;

        // Expand widths array for this macro-row if needed.
        let w_row = &mut macro_row_max_widths[mr as usize];
        while w_row.len() <= ecol {
            w_row.push(0);
        }
        if layer_widths[&l] > w_row[ecol] {
            w_row[ecol] = layer_widths[&l];
        }

        // Expand heights array for each row within this macro-row.
        let h_row = &mut macro_row_max_heights[mr as usize];
        for (row, &i) in indices.iter().enumerate() {
            while h_row.len() <= row {
                h_row.push(0);
            }
            let size = diagram.nodes[i].node_size.unwrap_or(crate::ast::NodeSize {
                width: 60,
                height: 60,
            });
            if size.height > h_row[row] {
                h_row[row] = size.height;
            }
        }

        macro_row_count = macro_row_count.max(mr + 1);
    }

    // ---- Boustrophedon X-position computation ----
    // For each macro-row, lay out the effective columns on a cumulative
    // cursor that resets at every macro-row (so odd rows appear to flow
    // right-to-left but their absolute X positions increase monotonically).

    // First compute per-macro-row column centres.
    // macro_row_centres[mr][eff_col] = absolute x
    let mut macro_row_centres: Vec<Vec<i32>> = Vec::new();

    for mr in 0..macro_row_count {
        let w_row = &macro_row_max_widths[mr as usize];
        let n_cols = w_row.len().max(1);

        // Build effective-col → absolute-x mapping for this macro-row.
        // The cursor always moves left-to-right in absolute space; the
        // serpentine direction is captured by which layer maps to which
        // effective column.
        let mut centres: Vec<i32> = Vec::with_capacity(n_cols);
        let mut cursor = MARGIN;
        for ec in 0..n_cols {
            let w = w_row[ec].max(2 * NODE_RADIUS);
            cursor += w / 2;
            centres.push(cursor);
            cursor += (w - w / 2) + LAYER_GUTTER;
        }
        macro_row_centres.push(centres);
    }

    // ---- Boustrophedon Y-position computation ----
    // For each macro-row, compute the Y base (top of that macro-row's
    // bounding box), then centre each row within the macro-row.
    let mut macro_row_y_bases: Vec<i32> = Vec::with_capacity(macro_row_count as usize);
    let mut yc = MARGIN;
    for mr in 0..macro_row_count {
        macro_row_y_bases.push(yc);
        let h_row = &macro_row_max_heights[mr as usize];
        let macro_row_height: i32 = h_row.iter().sum::<i32>()
            + ((h_row.len() as i32).saturating_sub(1)) * ROW_GUTTER;
        yc += macro_row_height + MACRO_ROW_GUTTER;
    }

    // ---- Apply positions ----
    for (&l, indices) in &groups {
        let mr = (l / k) as usize;
        let ecol = effective_column(l, k) as usize;

        let x = macro_row_centres[mr]
            .get(ecol)
            .copied()
            .unwrap_or(MARGIN);
        let y_base = macro_row_y_bases[mr];

        // Compute per-row Y offsets within this macro-row.
        let h_row = &macro_row_max_heights[mr];
        let mut row_y_offsets: Vec<i32> = Vec::with_capacity(indices.len());
        let mut row_y = y_base;
        for (row, _) in indices.iter().enumerate() {
            let h = h_row.get(row).copied().unwrap_or(2 * NODE_RADIUS);
            row_y += h / 2;
            row_y_offsets.push(row_y);
            row_y += (h - h / 2) + ROW_GUTTER;
        }

        for (row, &i) in indices.iter().enumerate() {
            let pos = Point::new(x, row_y_offsets[row]);
            diagram.nodes[i].position = Some(pos);
            diagram.nodes[i].layer = Some(l as usize);
        }
    }

    // Tag edges with their cyclic status.
    for (ei, edge) in diagram.edges.iter_mut().enumerate() {
        edge.is_cyclic = back.contains(&ei);
    }

    // Viewport is informational only; layout is now fully label-driven.
    let _ = Viewport::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::measure_diagram;
    use crate::parser::parse_dsl;

    #[test]
    fn test_layered_linear_chain() {
        let mut d = parse_dsl("A -> B\nB -> C\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let xa = d.node("A").unwrap().position.unwrap().x;
        let xb = d.node("B").unwrap().position.unwrap().x;
        let xc = d.node("C").unwrap().position.unwrap().x;
        assert!(xa < xb && xb < xc, "linear chain must flow left-to-right");
    }

    #[test]
    fn test_layered_back_edge_classified() {
        let mut d = parse_dsl("A -> B\nB -> C\nC -> A\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let cyclic_count = d.edges.iter().filter(|e| e.is_cyclic).count();
        assert_eq!(cyclic_count, 1);
        let back = d.edges.iter().find(|e| e.is_cyclic).unwrap();
        assert_eq!(back.from, "C");
        assert_eq!(back.to, "A");
    }

    #[test]
    fn test_layered_branch_stacks_vertically() {
        let mut d = parse_dsl("A -> B\nA -> C\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let pa = d.node("A").unwrap().position.unwrap();
        let pb = d.node("B").unwrap().position.unwrap();
        let pc = d.node("C").unwrap().position.unwrap();
        // B and C are both successors of A — same layer, distinct rows.
        assert_eq!(pb.x, pc.x);
        assert_ne!(pb.y, pc.y);
        assert!(pa.x < pb.x);
    }

    #[test]
    fn test_layered_disconnected_components() {
        let mut d = parse_dsl("A -> B\nX -> Y\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        for node in &d.nodes {
            assert!(node.position.is_some(), "node {} must have a position", node.id);
        }
    }

    #[test]
    fn test_layered_self_loop_no_crash() {
        let mut d = parse_dsl("A -> A : retry\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        assert_eq!(d.edges.iter().filter(|e| e.is_cyclic).count(), 1);
    }

    #[test]
    fn test_layered_diamond_with_cycle() {
        let mut d = parse_dsl("A -> B\nB -> D\nA -> C\nC -> D\nD -> A\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        // D -> A is the only back-edge.
        let cyclic_count = d.edges.iter().filter(|e| e.is_cyclic).count();
        assert_eq!(cyclic_count, 1);
        // A < B,C < D in x order (all in macro-row 0 with MAX_COLUMNS=4).
        let xa = d.node("A").unwrap().position.unwrap().x;
        let xd = d.node("D").unwrap().position.unwrap().x;
        assert!(xa < xd);
    }

    // ---- Boustrophedon macro-row tests -----------------------------------

    #[test]
    fn test_boustrophedon_single_macro_row_is_linear() {
        // 3 layers ≤ MAX_COLUMNS (4) → single macro-row, straight LTR.
        let mut d = parse_dsl("A -> B\nB -> C\nC -> D\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let xa = d.node("A").unwrap().position.unwrap().x;
        let xb = d.node("B").unwrap().position.unwrap().x;
        let xc = d.node("C").unwrap().position.unwrap().x;
        let xd = d.node("D").unwrap().position.unwrap().x;
        assert!(xa < xb && xb < xc && xc < xd, "single macro-row must be strictly left-to-right");
    }

    #[test]
    fn test_boustrophedon_two_macro_rows_serpentine() {
        // 6 layers → 2 macro-rows (4 + 2). Layer 4 (second fold) should
        // have its effective col 0, placing it to the left of layer 3.
        let mut d = parse_dsl(
            "A -> B\n\
             B -> C\n\
             C -> D\n\
             D -> E\n\
             E -> F\n",
        )
        .unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let xd = d.node("D").unwrap().position.unwrap().x; // layer 3, macro-row 0
        let xe = d.node("E").unwrap().position.unwrap().x; // layer 4, macro-row 1
        // Layer 4 is the first layer of macro-row 1 (odd row → RTL).
        // effective_col = 3 (rightmost in second fold) so it should be
        // to the right of D's layer-3 position.
        assert!(xe > xd || xe == xd,
            "E (layer 4, odd macro-row, eff_col 3) should be at or right of D (layer 3, eff_col 3)");
    }

    #[test]
    fn test_boustrophedon_macro_rows_have_distinct_y_bases() {
        // 5 layers → 2 macro-rows. Nodes in macro-row 1 must be below
        // nodes in macro-row 0.
        let mut d = parse_dsl(
            "A -> B\n\
             B -> C\n\
             C -> D\n\
             D -> E\n",
        )
        .unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let ya = d.node("A").unwrap().position.unwrap().y;
        let ye = d.node("E").unwrap().position.unwrap().y;
        assert!(ye > ya, "macro-row 1 must be below macro-row 0");
    }

    #[test]
    fn test_boustrophedon_layer_preserved() {
        // Topological layers must be unchanged by boustrophedon folding.
        let mut d = parse_dsl("A -> B\nB -> C\nC -> D\nD -> E\nE -> F\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let la = d.node("A").unwrap().layer;
        let lb = d.node("B").unwrap().layer;
        let lc = d.node("C").unwrap().layer;
        let lf = d.node("F").unwrap().layer;
        assert_eq!(la, Some(0));
        assert_eq!(lb, Some(1));
        assert_eq!(lc, Some(2));
        assert_eq!(lf, Some(5));
    }
}
