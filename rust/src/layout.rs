//! Backbone Layout Phase (Phase 2).
//!
//! Extracts the spanning tree (main happy path) and lays it out
//! using boustrophedon folding: a serpentine traversal that wraps
//! nodes to optimally fill the user-provided viewport.
//!
//! Octilinear aesthetic: nodes are placed on an integer grid with
//! uniform spacing. Only 0°, 45°, 90°, 135°, etc. connections.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::{Diagram, Point, Viewport};
use crate::measure::MAX_NODE_WIDTH;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Horizontal spacing between adjacent nodes on the grid.
pub const GRID_SPACING_X: i32 = 200;

/// Vertical spacing between rows.
pub const GRID_SPACING_Y: i32 = 150;

/// Half-side of the smallest node; fallback when no node_size is set.
pub const NODE_RADIUS: i32 = 30;

/// Minimum viewport edge to avoid degenerate layouts.
const MIN_VIEWPORT: u32 = 400;

// ---------------------------------------------------------------------------
// Spanning tree extraction
// ---------------------------------------------------------------------------

/// Extract the main "happy path" spanning tree from the diagram.
///
/// Uses a BFS from the first node in insertion order (assumed root).
/// Returns an ordered list of node IDs in traversal order.
pub fn extract_spanning_tree(diagram: &Diagram) -> Vec<String> {
    if diagram.nodes.is_empty() {
        return Vec::new();
    }

    // Build adjacency from the directed edges
    let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();

    for edge in &diagram.edges {
        forward.entry(edge.from.as_str()).or_default().push(edge.to.as_str());
        reverse.entry(edge.to.as_str()).or_default().push(edge.from.as_str());
    }

    // Root heuristics: prefer node with no in-edges, else first node
    let root = diagram
        .nodes
        .iter()
        .find(|n| !reverse.contains_key(n.id.as_str()))
        .map(|n| n.id.as_str())
        .unwrap_or(diagram.nodes[0].id.as_str());

    // BFS to produce an ordered spanning tree
    let mut visited: HashSet<&str> = HashSet::new();
    let mut order: Vec<String> = Vec::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    queue.push_back(root);
    visited.insert(root);

    while let Some(id) = queue.pop_front() {
        order.push(id.to_string());

        if let Some(neighbors) = forward.get(id) {
            for next in neighbors {
                if visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }

    // If a topology-first BFS missed disconnected stragglers, append them
    for node in &diagram.nodes {
        if !visited.contains(node.id.as_str()) {
            visited.insert(node.id.as_str());
            order.push(node.id.clone());
        }
    }

    order
}

// ---------------------------------------------------------------------------
// Boustrophedon grid placement
// ---------------------------------------------------------------------------

/// Place nodes on the grid using a serpentine (boustrophedon) row layout.
///
/// The spanning tree is laid out left-to-right on the first row, then
/// right-to-left on the second (like an ox ploughing a field), wrapping
/// at a row width computed from the viewport.
///
/// Returns a map of node ID -> grid position.
pub fn place_backbone(
    spanning_order: &[String],
    viewport: &Viewport,
) -> HashMap<String, Point> {
    if spanning_order.is_empty() {
        return HashMap::new();
    }

    let vw = viewport.width.max(MIN_VIEWPORT) as i32;

    // Estimate maximum node half-width from the text cap + padding.
    let node_half_w = (MAX_NODE_WIDTH as i32 + 24) / 2;

    // Compute how many nodes fit per row.
    let usable_width = vw - 4 * node_half_w;
    let per_row = ((usable_width / GRID_SPACING_X).max(1)) as usize;

    let mut positions = HashMap::new();

    for (idx, node_id) in spanning_order.iter().enumerate() {
        let row = idx / per_row;
        let col = idx % per_row;

        // Boustrophedon: even rows go left-to-right, odd rows go right-to-left
        let x = if row % 2 == 0 {
            node_half_w + (col as i32) * GRID_SPACING_X
        } else {
            // Right-to-left: compute from the right edge
            node_half_w + ((per_row - 1 - col) as i32) * GRID_SPACING_X
        };

        let y = node_half_w + (row as i32) * GRID_SPACING_Y;

        positions.insert(node_id.clone(), Point::new(x, y));
    }

    positions
}

// ---------------------------------------------------------------------------
// Phase 2 entry point
// ---------------------------------------------------------------------------

/// Run the backbone layout: extract spanning tree and place nodes.
///
/// Mutates the Diagram in place, setting each node's `position`.
pub fn layout_backbone(diagram: &mut Diagram) {
    let spanning = extract_spanning_tree(diagram);
    let positions = place_backbone(&spanning, &diagram.viewport);

    // Tag nodes with their spanning index and grid position
    for (idx, node_id) in spanning.iter().enumerate() {
        if let Some(node) = diagram.node_mut(node_id) {
            node.spanning_index = Some(idx);
            node.position = positions.get(node_id).copied();
        }
    }

    // Tag cyclic edges — any edge where the destination is earlier in the
    // spanning tree than the source (or on a different branch) is cyclic.
    let order_map: HashMap<&str, usize> = spanning
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    for edge in &mut diagram.edges {
        let is_cyclic = match (order_map.get(edge.from.as_str()), order_map.get(edge.to.as_str())) {
            (Some(&fi), Some(&ti)) => fi >= ti, // back-edge or self-loop
            _ => true, // reference to un-indexed node → cyclic
        };
        edge.is_cyclic = is_cyclic;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_dsl;

    #[test]
    fn test_spanning_tree_linear() {
        let d = parse_dsl("A -> B : x\nB -> C : y\n").unwrap();
        let tree = extract_spanning_tree(&d);
        assert_eq!(tree, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_spanning_tree_single_node() {
        let d = parse_dsl("Solo -> Solo\n").unwrap();
        let tree = extract_spanning_tree(&d);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0], "Solo");
    }

    #[test]
    fn test_spanning_tree_empty() {
        let d = parse_dsl("").unwrap();
        let tree = extract_spanning_tree(&d);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_spanning_tree_disconnected() {
        let d = parse_dsl("A -> B\nX -> Y\n").unwrap();
        let tree = extract_spanning_tree(&d);
        // BFS root is the first node in the node list (HashMap iteration).
        // Both A and X have no in-edges.
        // All 4 nodes must appear in the spanning tree.
        assert_eq!(tree.len(), 4);
        assert!(tree.contains(&"A".to_string()));
        assert!(tree.contains(&"B".to_string()));
        assert!(tree.contains(&"X".to_string()));
        assert!(tree.contains(&"Y".to_string()));
        // Should form two connected sub-sequences
        let a_pos = tree.iter().position(|s| s == "A").unwrap();
        let b_pos = tree.iter().position(|s| s == "B").unwrap();
        let x_pos = tree.iter().position(|s| s == "X").unwrap();
        let y_pos = tree.iter().position(|s| s == "Y").unwrap();
        // A and B must be adjacent in the sequence (BFS from one root)
        assert!((a_pos as isize - b_pos as isize).abs() == 1);
        // X and Y must be adjacent
        assert!((x_pos as isize - y_pos as isize).abs() == 1);
    }

    #[test]
    fn test_boustrophedon_placement() {
        let tree: Vec<String> = (0..10).map(|i| format!("S{i}")).collect();
        let viewport = Viewport {
            width: 1200,
            height: 600,
        };
        let positions = place_backbone(&tree, &viewport);

        let node_half_w = (MAX_NODE_WIDTH as i32 + 24) / 2;
        assert_eq!(positions.get("S0").unwrap().x, node_half_w);
        assert_eq!(
            positions.get("S1").unwrap().x,
            node_half_w + GRID_SPACING_X
        );
        assert!(positions.len() == 10);
        // All positions should be non-negative
        for id in &tree {
            let p = positions.get(id.as_str()).unwrap();
            assert!(p.x >= 0);
            assert!(p.y >= 0);
        }
    }

    #[test]
    fn test_boustrophedon_single_node() {
        let tree: Vec<String> = vec!["Alone".into()];
        let viewport = Viewport::default();
        let positions = place_backbone(&tree, &viewport);
        let p = positions.get("Alone").unwrap();
        let node_half_w = (MAX_NODE_WIDTH as i32 + 24) / 2;
        assert_eq!(p.x, node_half_w);
        assert_eq!(p.y, node_half_w);
    }

    #[test]
    fn test_boustrophedon_tiny_viewport() {
        let tree: Vec<String> = (0..5).map(|i| format!("N{i}")).collect();
        let viewport = Viewport { width: 100, height: 100 };
        let positions = place_backbone(&tree, &viewport);
        // Tiny viewport → per_row should be at least 1
        assert!(positions.len() == 5);
        // Each node is placed at a valid coordinate
        for id in &tree {
            let p = positions.get(id.as_str()).unwrap();
            assert!(p.x >= 0);
            assert!(p.y >= 0);
        }
    }

    #[test]
    fn test_layout_backbone_tags_cyclic_edges() {
        let mut d = parse_dsl("A -> B\nB -> C\nC -> A\n").unwrap();
        layout_backbone(&mut d);
        // In a 3-node cycle, at least one edge should be cyclic
        assert!(d.edges.iter().any(|e| e.is_cyclic), "no cyclic edge tagged");
        // Only one edge should be cyclic (the back-edge)
        let cyclic_count = d.edges.iter().filter(|e| e.is_cyclic).count();
        assert_eq!(cyclic_count, 1, "expected exactly 1 cyclic edge in a 3-node cycle");
    }

    #[test]
    fn test_layout_backbone_positions() {
        // Need three separate edges: X->Y, Y->Z
        let mut d2 = parse_dsl("X -> Y\nY -> Z\n").unwrap();
        layout_backbone(&mut d2);
        for node in &d2.nodes {
            assert!(node.position.is_some(), "Node {} has no position", node.id);
        }
    }
}
