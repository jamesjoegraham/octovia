//! Abstract Syntax Tree types for the state diagram DSL.
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

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Geometry primitives (integer grid — final output may scale)
// ---------------------------------------------------------------------------

/// Position on the invisible geometric grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Pixel dimensions of a node's rendered rectangle.
#[derive(Debug, Clone, Copy)]
pub struct NodeSize {
    pub width: i32,
    pub height: i32,
}

impl NodeSize {
    /// Compute node dimensions from measured text extents plus padding.
    pub fn from_extents(extents: &TextExtents, padding: i32) -> Self {
        let w = (extents.width as i32 + padding).max(MIN_NODE_SIDE);
        let h = (extents.height as i32 + padding).max(MIN_NODE_SIDE);
        // Proportional but minimum: make it at least wide enough to be comfortable
        let w = w.max(h);
        Self { width: w, height: h }
    }

    /// Half-width (radius-like offset from centre to left/right edge).
    pub fn half_w(&self) -> i32 {
        self.width / 2
    }

    /// Half-height.
    pub fn half_h(&self) -> i32 {
        self.height / 2
    }
}

/// Minimum side length for a node in pixels.
pub const MIN_NODE_SIDE: i32 = 60;

/// Padding added around the label text to compute node size.
pub const NODE_PADDING: i32 = 24;

/// Viewport constraint — the backbone layout attempts to fit within this.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
        }
    }
}

/// Anchor slot around a node for label placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnchorSlot {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

// ---------------------------------------------------------------------------
// Text measurement results
// ---------------------------------------------------------------------------

/// Measured dimensions of a text label.
#[derive(Debug, Clone, Copy)]
pub struct TextExtents {
    pub width: f32,
    pub height: f32,
}

// ---------------------------------------------------------------------------
// Port directions on a node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    East,
    West,
    North,
    South,
}

// ---------------------------------------------------------------------------
// Edge types
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
}

// ---------------------------------------------------------------------------
// Core Node
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
    /// Which index in the spanning tree ordering (None if not on spanning tree).
    pub spanning_index: Option<usize>,
}

// ---------------------------------------------------------------------------
// Document / diagram
// ---------------------------------------------------------------------------

/// A complete state-machine diagram ready for rendering.
#[derive(Debug, Clone)]
pub struct Diagram {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub title: Option<String>,
    /// User-supplied viewport constraint.
    pub viewport: Viewport,
    /// Colour theme for rendering.
    pub theme: Theme,
}

// ---------------------------------------------------------------------------
// Theme support
// ---------------------------------------------------------------------------

/// Built-in colour themes for the diagram renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    /// Deep navy/blue transit-map look (the classic default).
    Transit,
    /// Warm amber/copper on dark.
    Ember,
    /// Cool teal/emerald tones.
    Forest,
    /// Clean light-background theme.
    Light,
    /// Monochrome greyscale.
    Monochrome,
}

impl Theme {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "transit" | "dark" => Some(Self::Transit),
            "ember" | "amber" => Some(Self::Ember),
            "forest" | "teal" | "green" => Some(Self::Forest),
            "light" | "white" | "paper" => Some(Self::Light),
            "monochrome" | "gray" | "grey" | "bw" => Some(Self::Monochrome),
            _ => None,
        }
    }
}

/// Pre-defined colour palettes keyed by theme.
pub struct ThemeColors {
    pub bg: &'static str,
    pub node_fill: &'static str,
    pub node_stroke: &'static str,
    pub edge_forward: &'static str,
    pub edge_cyclic: &'static str,
    pub label_fill: &'static str,
    pub title_fill: &'static str,
}

impl Theme {
    pub fn colors(&self) -> ThemeColors {
        match self {
            Self::Transit => ThemeColors {
                bg: "#1A1A2E",
                node_fill: "#16213E",
                node_stroke: "#4A90D9",
                edge_forward: "#4A90D9",
                edge_cyclic: "#E67E22",
                label_fill: "#E0E0E0",
                title_fill: "#C0C0C0",
            },
            Self::Ember => ThemeColors {
                bg: "#1C1410",
                node_fill: "#2A1D16",
                node_stroke: "#D4803A",
                edge_forward: "#D4803A",
                edge_cyclic: "#E8A838",
                label_fill: "#E8D5C0",
                title_fill: "#C8B094",
            },
            Self::Forest => ThemeColors {
                bg: "#0F1A14",
                node_fill: "#16251D",
                node_stroke: "#3D9B6B",
                edge_forward: "#3D9B6B",
                edge_cyclic: "#7CC49E",
                label_fill: "#CDE0D5",
                title_fill: "#A0C0B0",
            },
            Self::Light => ThemeColors {
                bg: "#F5F5F0",
                node_fill: "#FFFFFF",
                node_stroke: "#4A6FA5",
                edge_forward: "#4A6FA5",
                edge_cyclic: "#C06030",
                label_fill: "#2C2C2E",
                title_fill: "#555555",
            },
            Self::Monochrome => ThemeColors {
                bg: "#111112",
                node_fill: "#1C1C1E",
                node_stroke: "#888899",
                edge_forward: "#888899",
                edge_cyclic: "#BBBBC8",
                label_fill: "#D0D0D6",
                title_fill: "#A0A0A8",
            },
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::Transit
    }
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
                    spanning_index: Some(0),
                },
                Node {
                    id: "B".into(),
                    label: "State B".into(),
                    label_extents: None,
                    node_size: None,
                    position: None,
                    spanning_index: None,
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
                },
            ],
            title: None,
            viewport: Viewport::default(),
            theme: Theme::default(),
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
            theme: Theme::default(),
        };
        assert!(d.node("anything").is_none());
        assert!(d.adjacency().is_empty());
    }

    #[test]
    fn test_point_new() {
        let p = Point::new(42, -7);
        assert_eq!(p.x, 42);
        assert_eq!(p.y, -7);
    }

    #[test]
    fn test_point_equality_hash() {
        use std::collections::HashSet;
        let a = Point::new(1, 2);
        let b = Point::new(1, 2);
        let c = Point::new(2, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b); // duplicate, should be 1 entry
        set.insert(c);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_default_viewport() {
        let v: Viewport = Default::default();
        assert_eq!(v.width, 1200);
        assert_eq!(v.height, 800);
    }

    #[test]
    fn test_self_loop_edge() {
        let mut d = Diagram {
            nodes: vec![
                Node {
                    id: "X".into(),
                    label: "X".into(),
                    label_extents: None,
                    node_size: None,
                    position: Some(Point::new(50, 50)),
                    spanning_index: Some(0),
                },
            ],
            edges: vec![
                Edge {
                    from: "X".into(),
                    to: "X".into(),
                    label: Some("loop".into()),
                    label_extents: None,
                    is_cyclic: true,
                    route: vec![],
                },
            ],
            title: None,
            viewport: Viewport::default(),
            theme: Theme::default(),
        };
        assert!(d.node("X").is_some());
        let adj = d.adjacency();
        assert_eq!(adj["X"], vec![0]);

        // Self-loop: node_mut should still find it
        let n = d.node_mut("X").unwrap();
        n.position = Some(Point::new(100, 100));
        assert_eq!(d.node("X").unwrap().position, Some(Point::new(100, 100)));
    }

    #[test]
    fn test_adjacency_multiple_edges() {
        let mut d = make_diagram();
        d.edges.push(Edge {
            from: "A".into(),
            to: "B".into(),
            label: Some("retry".into()),
            label_extents: None,
            is_cyclic: true,
            route: vec![],
        });
        let adj = d.adjacency();
        assert_eq!(adj["A"].len(), 2);
    }

    #[test]
    fn test_theme_from_str() {
        assert_eq!(Theme::from_str("transit"), Some(Theme::Transit));
        assert_eq!(Theme::from_str("dark"), Some(Theme::Transit));
        assert_eq!(Theme::from_str("ember"), Some(Theme::Ember));
        assert_eq!(Theme::from_str("amber"), Some(Theme::Ember));
        assert_eq!(Theme::from_str("forest"), Some(Theme::Forest));
        assert_eq!(Theme::from_str("teal"), Some(Theme::Forest));
        assert_eq!(Theme::from_str("light"), Some(Theme::Light));
        assert_eq!(Theme::from_str("paper"), Some(Theme::Light));
        assert_eq!(Theme::from_str("monochrome"), Some(Theme::Monochrome));
        assert_eq!(Theme::from_str("grey"), Some(Theme::Monochrome));
        assert_eq!(Theme::from_str("bw"), Some(Theme::Monochrome));
        assert_eq!(Theme::from_str("unknown"), None);
        assert_eq!(Theme::from_str(&"  Dark  ".to_string()), Some(Theme::Transit));
    }

    #[test]
    fn test_theme_colors_not_empty() {
        for theme in [Theme::Transit, Theme::Ember, Theme::Forest, Theme::Light, Theme::Monochrome] {
            let c = theme.colors();
            assert!(!c.bg.is_empty());
            assert!(!c.node_fill.is_empty());
            assert!(!c.node_stroke.is_empty());
        }
    }

    #[test]
    fn test_theme_default_is_transit() {
        assert_eq!(Theme::default(), Theme::Transit);
    }
}
