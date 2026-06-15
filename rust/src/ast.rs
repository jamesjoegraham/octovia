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
// Theme system — data-driven from themes.json
// ---------------------------------------------------------------------------

/// A single theme's colour palette.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThemeColors {
    pub bg: String,
    pub node_fill: String,
    pub node_stroke: String,
    pub edge_forward: String,
    pub edge_cyclic: String,
    pub label_fill: String,
    pub title_fill: String,
    pub node_glow: String,
    pub gradient_start: String,
    pub gradient_end: String,
}

impl ThemeColors {
    /// Convenience accessor for backwards compatibility — returns self.
    pub fn colors(&self) -> &ThemeColors {
        self
    }

    /// Resolve a theme name to ThemeColors.
    pub fn from_str(name: &str) -> Option<ThemeColors> {
        resolve_theme(name)
    }
}

/// A theme entry from the themes.json file.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThemeEntry {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub name: String,
    pub description: String,
    pub bg: String,
    pub node_fill: String,
    pub node_stroke: String,
    pub edge_forward: String,
    pub edge_cyclic: String,
    pub label_fill: String,
    pub title_fill: String,
    pub node_glow: String,
    pub gradient_start: String,
    pub gradient_end: String,
}

/// The top-level themes.json structure.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThemeManifest {
    pub themes: Vec<ThemeEntry>,
}

/// Load and parse the embedded themes.json at compile time.
pub fn load_builtin_themes() -> ThemeManifest {
    serde_json::from_str(include_str!("themes.json"))
        .expect("themes.json is invalid JSON or mismatched schema")
}

/// Resolve a theme name (or alias) to a `ThemeColors`, along with metadata.
/// Returns `None` if the name doesn't match any theme.
pub fn resolve_theme(name: &str) -> Option<ThemeColors> {
    let name_lower = name.trim().to_lowercase();
    let manifest = load_builtin_themes();
    manifest.themes.into_iter().find(|entry| {
        entry.id == name_lower
            || (entry.id.len() > name_lower.len() && entry.id[..name_lower.len()] == name_lower)
            || entry.aliases.iter().any(|a| *a == name_lower)
    }).map(|entry| ThemeColors {
        bg: entry.bg,
        node_fill: entry.node_fill,
        node_stroke: entry.node_stroke,
        edge_forward: entry.edge_forward,
        edge_cyclic: entry.edge_cyclic,
        label_fill: entry.label_fill,
        title_fill: entry.title_fill,
        node_glow: entry.node_glow,
        gradient_start: entry.gradient_start,
        gradient_end: entry.gradient_end,
    })
}

/// Get all built-in theme names and their display names.
pub fn list_themes() -> Vec<(String, String)> {
    let manifest = load_builtin_themes();
    manifest.themes.into_iter().map(|e| (e.id, e.name)).collect()
}

/// Get the default theme (Transit) colours.
pub fn default_theme() -> ThemeColors {
    resolve_theme("transit").expect("default theme 'transit' must exist in themes.json")
}

// ---------------------------------------------------------------------------
// Document / diagram
// ---------------------------------------------------------------------------

/// Canvas background mode for the rendered SVG.
///
/// This is intentionally separate from the theme — the theme's `bg` is still
/// used for label halos and other opaque-on-canvas effects, while this enum
/// controls only the outermost `<svg style="background-color: ...">`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Background {
    /// No canvas fill — the SVG renders against whatever sits behind it.
    #[default]
    Transparent,
    /// Use the active theme's `bg` colour. Resolved at render time so a
    /// theme override applied after parsing still takes effect.
    Theme,
    /// An explicit CSS colour (hex, rgb(...), named colour, etc.).
    Custom(String),
}

impl Background {
    /// Parse a CLI / DSL value into a `Background`.
    ///
    /// - `"transparent"` (case-insensitive) → `Transparent`
    /// - `"theme"` (case-insensitive) → `Theme`
    /// - anything else → `Custom(value)`
    pub fn parse_value(value: &str) -> Self {
        let trimmed = value.trim();
        match trimmed.to_lowercase().as_str() {
            "transparent" => Background::Transparent,
            "theme" => Background::Theme,
            _ => Background::Custom(trimmed.to_string()),
        }
    }

    /// Resolve to the concrete CSS colour string used in the SVG output.
    pub fn resolve<'a>(&'a self, theme: &'a ThemeColors) -> &'a str {
        match self {
            Background::Transparent => "transparent",
            Background::Theme => &theme.bg,
            Background::Custom(s) => s.as_str(),
        }
    }
}

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

impl Default for ThemeColors {
    fn default() -> Self {
        default_theme()
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

    #[test]
    fn test_load_themes_non_empty() {
        let m = load_builtin_themes();
        assert!(m.themes.len() >= 40, "expected >= 40 themes, got {}", m.themes.len());
    }

    #[test]
    fn test_resolve_transit() {
        let c = resolve_theme("transit").expect("transit");
        assert_eq!(c.bg, "#1A1A2E");
    }

    #[test]
    fn test_resolve_aliases() {
        assert!(resolve_theme("dark").is_some(), "alias 'dark' for transit");
        assert!(resolve_theme("teal").is_some(), "alias 'teal' for forest");
        assert!(resolve_theme("grey").is_some(), "alias 'grey' for monochrome");
    }

    #[test]
    fn test_resolve_unknown() {
        assert!(resolve_theme("nonexistent").is_none());
    }

    #[test]
    fn test_default_theme() {
        let c = default_theme();
        assert_eq!(c.bg, "#1A1A2E");
    }

    #[test]
    fn test_list_themes() {
        let list = list_themes();
        assert!(list.len() >= 40);
        assert!(list.iter().any(|(id, _)| id == "transit"));
    }

    #[test]
    fn test_every_theme_has_valid_colors() {
        let m = load_builtin_themes();
        for entry in &m.themes {
            assert!(!entry.bg.is_empty(), "theme '{}' missing bg", entry.id);
            assert!(!entry.node_fill.is_empty(), "theme '{}' missing node_fill", entry.id);
            assert!(!entry.node_stroke.is_empty(), "theme '{}' missing node_stroke", entry.id);
            assert!(!entry.node_glow.is_empty(), "theme '{}' missing node_glow", entry.id);
            assert!(!entry.gradient_start.is_empty(), "theme '{}' missing gradient_start", entry.id);
        }
    }

    // ---- Existing tests from the old ast.rs (preserved) ----

    fn default_node(id: &str) -> Node {
        Node {
            id: id.to_string(),
            label: id.to_string(),
            label_extents: None,
            node_size: None,
            position: None,
            spanning_index: None,
        }
    }

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

    #[test]
    fn test_point_new() {
        let p = Point::new(42, -7);
        assert_eq!(p.x, 42);
        assert_eq!(p.y, -7);
    }

    #[test]
    fn test_node_size_extents() {
        let e = TextExtents { width: 80.0, height: 32.0 };
        let ns = NodeSize::from_extents(&e, 24);
        assert!(ns.width >= 104);
        assert!(ns.height >= 56);
    }

    #[test]
    fn test_node_size_minimum() {
        let e = TextExtents { width: 10.0, height: 10.0 };
        let ns = NodeSize::from_extents(&e, 24);
        assert!(ns.width >= MIN_NODE_SIDE);
        assert!(ns.height >= MIN_NODE_SIDE);
    }

    #[test]
    fn test_default_viewport() {
        let v: Viewport = Default::default();
        assert_eq!(v.width, 1200);
        assert_eq!(v.height, 800);
    }

    #[test]
    fn test_background_default_is_transparent() {
        let bg = Background::default();
        assert_eq!(bg, Background::Transparent);
        let theme = default_theme();
        assert_eq!(bg.resolve(&theme), "transparent");
    }

    #[test]
    fn test_background_theme_resolves_to_theme_bg() {
        let theme = default_theme();
        assert_eq!(Background::Theme.resolve(&theme), theme.bg.as_str());
    }

    #[test]
    fn test_background_custom_resolves_verbatim() {
        let theme = default_theme();
        let bg = Background::Custom("#abcdef".to_string());
        assert_eq!(bg.resolve(&theme), "#abcdef");
    }

    #[test]
    fn test_background_parse_value() {
        assert_eq!(Background::parse_value("transparent"), Background::Transparent);
        assert_eq!(Background::parse_value("  Transparent  "), Background::Transparent);
        assert_eq!(Background::parse_value("theme"), Background::Theme);
        assert_eq!(Background::parse_value("THEME"), Background::Theme);
        assert_eq!(
            Background::parse_value("#112233"),
            Background::Custom("#112233".to_string())
        );
    }
}
