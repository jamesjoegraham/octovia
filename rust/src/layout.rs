//! Top-to-Bottom Layered Topological Layout (Sugiyama-style).
//!
//! 1. Classify back-edges via DFS so the remaining graph is a DAG.
//! 2. Assign each node a layer `L(v)` via longest-path topological sort
//!    (Kahn's algorithm — **unchanged**, per architecture constraint).
//! 3. Within each layer, place nodes side-by-side horizontally in input
//!    order. Layers are stacked vertically top-to-bottom.
//! 4. Map layers → Y positions and intra-layer nodes → X positions on
//!    a pixel grid, with vertical gutters (LAYER_GUTTER) between layers
//!    and horizontal gutters (NODE_GUTTER) between nodes in the same
//!    layer. Each layer is horizontally centred around X = 0 so the
//!    diagram looks balanced.
//!
//! Every forward edge `e = (u, v)` satisfies `L(u) < L(v)`.
//! The Y-coordinate increases downward: L(u) → lower Y value (higher on
//! screen), L(v) → higher Y value (lower on screen). This aligns with
//! native vertical scrolling on mobile and desktop.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::ast::{Diagram, Point, Viewport};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Vertical gutter (px) between adjacent layers. This is the dedicated
/// routing channel A* uses for forward and back-edge passes.
pub const LAYER_GUTTER: i32 = 90;

/// Horizontal gutter (px) between adjacent nodes within the same layer.
pub const NODE_GUTTER: i32 = 70;

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
// TTB coordinate placement
// ---------------------------------------------------------------------------

/// Final entry point: classify cycles, assign layers, place nodes on the
/// pixel grid using Top-to-Bottom layout.
///
/// Topological phase 2b (Kahn's longest-path sort) remains unchanged.
/// Only the physical coordinate assignment in this function uses TTB.
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

    // ---- Layer height / Y placement ----
    // For each layer, compute its maximum node height. Layer Y positions
    // are accumulated vertically with LAYER_GUTTER between them.
    // Y is the centre of each layer's row of nodes.

    let mut layer_heights: BTreeMap<i32, i32> = BTreeMap::new(); // layer → max node height
    let mut layer_widths: BTreeMap<i32, i32> = BTreeMap::new();  // layer → total width of all nodes + gutters
    let mut layer_node_widths: BTreeMap<i32, Vec<i32>> = BTreeMap::new(); // layer → individual node widths

    for (&l, indices) in &groups {
        let mut max_h = 2 * NODE_RADIUS;
        let mut widths: Vec<i32> = Vec::new();
        for &i in indices {
            let size = diagram.nodes[i].node_size.unwrap_or(crate::ast::NodeSize {
                width: 60,
                height: 60,
            });
            if size.height > max_h {
                max_h = size.height;
            }
            widths.push(size.width);
        }
        layer_heights.insert(l, max_h);

        // Total width = sum of all node widths + (n-1) * NODE_GUTTER
        let n_nodes = widths.len() as i32;
        let total_w: i32 = widths.iter().sum::<i32>()
            + ((n_nodes - 1).max(0)) * NODE_GUTTER;
        layer_widths.insert(l, total_w);
        layer_node_widths.insert(l, widths);
    }

    // ---- Determine the global diagram width ----
    // Find the widest layer to set the overall diagram width.
    // Layers are centred within this width so the tree looks balanced.
    let global_width: i32 = layer_widths.values().copied().max().unwrap_or(2 * NODE_RADIUS);

    // ---- Compute Y positions for each layer ----
    // The layers stack vertically. Layer 0 is at the top.
    // Y-coordinate for a layer = MARGIN + accumulated heights of all previous layers
    //                           + number_of_previous_gutters * LAYER_GUTTER
    // The Y value stored is the centre row of that layer's nodes.
    let mut layer_y_centres: BTreeMap<i32, i32> = BTreeMap::new();
    let mut y_cursor = MARGIN;
    for (&l, _) in &groups {
        let h = layer_heights[&l];
        y_cursor += h / 2;
        layer_y_centres.insert(l, y_cursor);
        y_cursor += (h - h / 2) + LAYER_GUTTER;
    }

    // ---- Compute X positions for each node within its layer ----
    // Nodes are placed side-by-side within the layer, centred on the
    // global centre axis. Each layer's total width determines its
    // left padding so the widest layer spans from MARGIN to
    // MARGIN + global_width.
    for (&l, indices) in &groups {
        let widths = &layer_node_widths[&l];
        let total_w = layer_widths[&l];
        let left_pad = MARGIN + (global_width - total_w) / 2;

        let mut x_cursor = left_pad;
        for (row, &i) in indices.iter().enumerate() {
            let w = widths[row];
            x_cursor += w / 2;

            let y_centre = layer_y_centres[&l];
            let pos = Point::new(x_cursor, y_centre);
            diagram.nodes[i].position = Some(pos);
            diagram.nodes[i].layer = Some(l as usize);

            x_cursor += (w - w / 2) + NODE_GUTTER;
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
        let ya = d.node("A").unwrap().position.unwrap().y;
        let yb = d.node("B").unwrap().position.unwrap().y;
        let yc = d.node("C").unwrap().position.unwrap().y;
        assert!(ya < yb && yb < yc, "linear chain must flow top-to-bottom");
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
    fn test_layered_branch_places_horizontally() {
        let mut d = parse_dsl("A -> B\nA -> C\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let pa = d.node("A").unwrap().position.unwrap();
        let pb = d.node("B").unwrap().position.unwrap();
        let pc = d.node("C").unwrap().position.unwrap();
        // B and C are both successors of A — same layer, distinct columns (side-by-side).
        assert_eq!(pb.y, pc.y);
        assert_ne!(pb.x, pc.x);
        assert!(pa.y < pb.y);
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
        // A < B,C < D in y order (top to bottom).
        let ya = d.node("A").unwrap().position.unwrap().y;
        let yd = d.node("D").unwrap().position.unwrap().y;
        assert!(ya < yd);
    }

    #[test]
    fn test_ttb_layer_path_y_increases() {
        // A 4-layer chain must have Y positions increase strictly
        // from top to bottom.
        let mut d = parse_dsl("A -> B\nB -> C\nC -> D\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let ya = d.node("A").unwrap().position.unwrap().y;
        let yb = d.node("B").unwrap().position.unwrap().y;
        let yc = d.node("C").unwrap().position.unwrap().y;
        let yd = d.node("D").unwrap().position.unwrap().y;
        assert!(ya < yb && yb < yc && yc < yd, "TTB chain must be strictly top-to-bottom");
    }

    #[test]
    fn test_ttb_same_layer_siblings_same_y() {
        // A -> B and A -> C: B and C in same layer, same Y, different X.
        let mut d = parse_dsl("A -> B\nA -> C\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        let pb = d.node("B").unwrap().position.unwrap();
        let pc = d.node("C").unwrap().position.unwrap();
        assert_eq!(pb.y, pc.y, "siblings in same layer must share Y");
        assert_ne!(pb.x, pc.x, "siblings in same layer must have distinct X");
    }

    #[test]
    fn test_ttb_layer_preserved() {
        // Topological layers must be unchanged by TTB layout.
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
