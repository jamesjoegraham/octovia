//! Abstract Syntax Tree for the state diagram DSL.
//!
//! The DSL is sequence-first: the happy path (spanning tree) is declared
//! as an ordered list of state transitions. Back-edges and cycles are
//! modelled as additional edges that reference states already seen.
//!
//! Syntax example:
//! ```text
//! Idle -> Active : recheck
//! Active -> Processing : submit
//! Processing -> Done : complete
//! Done -> Idle : reset
//! Processing -> Error : timeout
//! ```
//!
//! The module is split into three files:
//! - [`geom`] — pure geometry primitives (`Point`, `NodeSize`, `Viewport`, …)
//! - [`theme`] — colour themes and SVG canvas background
//! - this file — the domain model (`Node`, `Edge`, `Diagram`)
//!
//! All public items are re-exported here so external code keeps using
//! `crate::ast::Foo` paths unchanged.

mod geom;
mod theme;

pub use geom::{
    AnchorSlot, EdgeLabelAnchor, NodeSize, Point, PortDirection, TextExtents, Viewport,
    MIN_NODE_SIDE, NODE_PADDING,
};
pub use theme::{
    default_theme, list_themes, load_builtin_themes, resolve_theme, Background, ThemeColors,
    ThemeEntry, ThemeManifest,
};

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

/// An edge between two states.
#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// Optional label / trigger.
    pub label: Option<String>,
    /// Measured text extents (filled in Phase 1).
    pub label_extents: Option<TextExtents>,
    /// Whether this edge is part of the spanning tree (happy path)
    /// or a back-edge / cycle.
    pub is_cyclic: bool,
    /// Resolved route through the grid (filled in Phase 3).
    pub route: Vec<Point>,
    /// Resolved label anchor (filled in Phase 3 alongside the route).
    pub label_anchor: Option<EdgeLabelAnchor>,
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    /// Label displayed inside the node.
    pub label: String,
    /// Measured text extents (Phase 1).
    pub label_extents: Option<TextExtents>,
    /// Computed node dimensions from label_extents + padding (Phase 1).
    pub node_size: Option<NodeSize>,
    /// Position on the grid (Phase 2).
    pub position: Option<Point>,
    /// Depth layer assigned by the longest-path topological sort
    /// (`None` until the layout phase has run).
    pub layer: Option<usize>,
}

// ---------------------------------------------------------------------------
// Diagram
// ---------------------------------------------------------------------------

/// A complete state-machine diagram ready for rendering.
#[derive(Debug, Clone)]
pub struct Diagram {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub title: Option<String>,
    /// User-supplied viewport constraint.
    pub viewport: Viewport,
    /// Colour theme for rendering — now data-driven.
    pub theme: ThemeColors,
    /// SVG canvas background. Defaults to `Transparent`.
    pub background: Background,
}

impl Diagram {
    /// Look up a node by ID.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Mutable node lookup.
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Build a quick adjacency list keyed by node id -> indices of outgoing edges.
    pub fn adjacency(&self) -> HashMap<String, Vec<usize>> {
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, edge) in self.edges.iter().enumerate() {
            map.entry(edge.from.clone()).or_default().push(idx);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diagram() -> Diagram {
        Diagram {
            nodes: vec![
                Node {
                    id: "A".into(),
                    label: "State A".into(),
                    label_extents: Some(TextExtents { width: 40.0, height: 16.0 }),
                    position: Some(Point::new(100, 100)),
                    node_size: Some(NodeSize { width: 64, height: 40 }),
                    layer: Some(0),
                },
                Node {
                    id: "B".into(),
                    label: "State B".into(),
                    label_extents: None,
                    node_size: None,
                    position: None,
                    layer: None,
                },
            ],
            edges: vec![
                Edge {
                    from: "A".into(),
                    to: "B".into(),
                    label: Some("trigger".into()),
                    label_extents: Some(TextExtents { width: 30.0, height: 12.0 }),
                    is_cyclic: false,
                    route: vec![Point::new(100, 100), Point::new(200, 100)],
                    label_anchor: None,
                },
            ],
            title: None,
            viewport: Viewport::default(),
            theme: ThemeColors::default(),
            background: Background::default(),
        }
    }

    #[test]
    fn test_node_lookup() {
        let d = make_diagram();
        assert!(d.node("A").is_some());
        assert!(d.node("B").is_some());
        assert!(d.node("C").is_none());
        assert_eq!(d.node("A").unwrap().label, "State A");
    }

    #[test]
    fn test_node_mutate() {
        let mut d = make_diagram();
        let n = d.node_mut("A").unwrap();
        n.position = Some(Point::new(999, 999));
        assert_eq!(d.node("A").unwrap().position, Some(Point::new(999, 999)));
    }

    #[test]
    fn test_adjacency() {
        let d = make_diagram();
        let adj = d.adjacency();
        assert_eq!(adj.len(), 1); // only A has outgoing
        assert_eq!(adj["A"], vec![0]);
        assert!(!adj.contains_key("B"));
    }

    #[test]
    fn test_empty_diagram() {
        let d = Diagram {
            nodes: vec![],
            edges: vec![],
            title: None,
            viewport: Viewport::default(),
            theme: ThemeColors::default(),
            background: Background::default(),
        };
        assert!(d.node("anything").is_none());
        assert!(d.adjacency().is_empty());
    }
}
