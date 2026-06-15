//! Layered Topological Layout (lightweight Sugiyama).
//!
//! Replaces the previous boustrophedon serpentine grid with a
//! depth-stratified placement:
//!
//! 1. Classify back-edges via DFS so the remaining graph is a DAG.
//! 2. Assign each node a layer `L(v)` via longest-path topological sort.
//! 3. Within each layer, stack nodes vertically in input order.
//! 4. Map layers → columns and row-indices → rows on a pixel grid, with
//!    explicit gutters between adjacent layers and rows. The gutters are
//!    the routing channels A* uses for forward and back-edges.
//!
//! The result is that *time flows left-to-right structurally*: every
//! forward edge `e = (u, v)` satisfies `L(u) < L(v)`, every back-edge
//! satisfies `L(u) >= L(v)`.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::ast::{Diagram, Point, Viewport};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Horizontal gutter (px) between adjacent layer columns. This is the
/// dedicated routing channel A* uses for forward and back-edge passes.
pub const LAYER_GUTTER: i32 = 90;

/// Vertical gutter (px) between adjacent rows.
pub const ROW_GUTTER: i32 = 70;

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
// Coordinate placement
// ---------------------------------------------------------------------------

/// Final entry point: classify cycles, assign layers, place nodes on the
/// pixel grid. Mutates `diagram.nodes[*].position` and `diagram.edges[*].is_cyclic`.
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

    // Per-layer column width = max node width in that layer.
    // Per-row height        = max node height across all layers at that row index.
    let max_rows = groups.values().map(|g| g.len()).max().unwrap_or(0);
    let mut layer_widths: BTreeMap<i32, i32> = BTreeMap::new();
    let mut row_heights: Vec<i32> = vec![0; max_rows];
    for (&l, indices) in &groups {
        let mut max_w = 2 * NODE_RADIUS;
        for (row, &i) in indices.iter().enumerate() {
            let size = diagram.nodes[i]
                .node_size
                .unwrap_or(crate::ast::NodeSize { width: 60, height: 60 });
            if size.width > max_w {
                max_w = size.width;
            }
            if size.height > row_heights[row] {
                row_heights[row] = size.height;
            }
        }
        layer_widths.insert(l, max_w);
    }

    // Cumulative x-centres for each layer.
    let mut x_centre: HashMap<i32, i32> = HashMap::new();
    let mut cursor = MARGIN;
    for (&l, _) in &groups {
        let w = layer_widths[&l];
        cursor += w / 2;
        x_centre.insert(l, cursor);
        cursor += (w - w / 2) + LAYER_GUTTER;
    }

    // Cumulative y-centres for each row index.
    let mut y_centre: Vec<i32> = vec![0; max_rows];
    let mut yc = MARGIN;
    for r in 0..max_rows {
        let h = row_heights[r];
        yc += h / 2;
        y_centre[r] = yc;
        yc += (h - h / 2) + ROW_GUTTER;
    }

    // Apply positions.
    for (&l, indices) in &groups {
        for (row, &i) in indices.iter().enumerate() {
            let pos = Point::new(*x_centre.get(&l).unwrap_or(&MARGIN), y_centre[row]);
            diagram.nodes[i].position = Some(pos);
            diagram.nodes[i].spanning_index = Some(l as usize);
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
        // A < B,C < D in x order.
        let xa = d.node("A").unwrap().position.unwrap().x;
        let xd = d.node("D").unwrap().position.unwrap().x;
        assert!(xa < xd);
    }
}
