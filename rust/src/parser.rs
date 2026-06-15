//! DSL parser using nom.
//!
//! Accepts two formats:
//!
//! **Text format** (sequence-first):
//! ```text
//! title: My State Machine
//! Idle -> Active : recheck
//! Active -> Processing : submit
//! ```
//!
//! Lines beginning with `#` are comments. Empty lines are skipped.
//! A `title:` directive at the top sets the diagram title.
//! Each transition line: `Source -> Target : optional_label`

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{multispace0, not_line_ending},
    combinator::{map, opt},
    sequence::{preceded, tuple},
    IResult,
};

use crate::ast::{Diagram, Edge, Node, Theme, Viewport};

// ---------------------------------------------------------------------------
// Helper combinators
// ---------------------------------------------------------------------------

/// Whitespace (including newlines) — we handle lines ourselves.
fn ws(input: &str) -> IResult<&str, &str> {
    multispace0(input)
}

/// A state identifier: non-empty string, no whitespace, no `->` or `:`
fn identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-')(input)
}

/// A label for a transition (rest of the line after `:`)
fn transition_label(input: &str) -> IResult<&str, &str> {
    map(
        preceded(tag(":"), take_while1(|c: char| !c.is_control() && c != '\n' && c != '\r')),
        |s: &str| s.trim(),
    )(input)
}

// ---------------------------------------------------------------------------
// Transition line parser: `Source -> Target : label`
// ---------------------------------------------------------------------------

fn transition_line(input: &str) -> IResult<&str, Edge> {
    let (rest, (from, _, _, _, to, _, label)) = tuple((
        identifier,
        ws,
        tag("->"),
        ws,
        identifier,
        ws,
        opt(transition_label),
    ))(input)?;
    Ok((
        rest,
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            label: label.map(|s| s.to_string()),
            label_extents: None,
            is_cyclic: false,
            route: Vec::new(),
        },
    ))
}

// ---------------------------------------------------------------------------
// Title line parser: `title: My Title`
// ---------------------------------------------------------------------------

fn title_directive(input: &str) -> IResult<&str, String> {
    let (rest, (_, t)) = tuple((
        tag("title:"),
        alt((
            // rest of line, trimmed
            map(not_line_ending, |s: &str| s.trim().to_string()),
        )),
    ))(input)?;
    Ok((rest, t))
}

// ---------------------------------------------------------------------------
// A single line (skip comments and blanks; parse title or transition)
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Line {
    Title(String),
    Transition(Edge),
    Skip,
}

fn line(input: &str) -> IResult<&str, Line> {
    let (rest, _) = ws(input)?;
    // At start of line — check for comment, empty, title, or transition
    let (rest, _) = opt(tag("\n"))(rest)?; // handle leftover newline from previous line
    // Actually, let's do a per-line approach by splitting on newlines manually.
    // Better approach: match the whole line as a single chunk.
    alt((
        // Comment line
        map(preceded(tag("#"), not_line_ending), |_| Line::Skip),
        // Empty line
        map(tag(""), |_| Line::Skip),
        // Title directive
        map(title_directive, Line::Title),
        // Transition line
        map(transition_line, Line::Transition),
    ))(input)
}

// ---------------------------------------------------------------------------
// Full document parser
// ---------------------------------------------------------------------------

/// Parse a text-format DSL string into a Diagram.
pub fn parse_dsl(input: &str) -> Result<Diagram, String> {
    let mut title: Option<String> = None;
    let mut theme: Option<Theme> = None;
    // Insertion-order node tracking: `nodes_order` preserves declaration order
    // (load-bearing for deterministic, narrative-first layout); `seen` is just
    // for O(1) dedupe membership.
    let mut nodes_order: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut edges: Vec<Edge> = Vec::new();

    for raw_line in input.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Try to parse as title
        if trimmed.starts_with("title:") {
            title = Some(trimmed[6..].trim().to_string());
            continue;
        }

        // Try to parse as theme
        if trimmed.starts_with("theme:") {
            let theme_name = trimmed[6..].trim();
            theme = Some(
                Theme::from_str(theme_name)
                    .ok_or_else(|| format!("Unknown theme: '{theme_name}'. Options: transit, ember, forest, light, monochrome"))?,
            );
            continue;
        }

        // Parse `Source -> Target : label`
        if let Some(arrow_pos) = trimmed.find("->") {
            let from = trimmed[..arrow_pos].trim();
            let after_arrow = trimmed[arrow_pos + 2..].trim();

            let (to, label) = if let Some(colon_pos) = after_arrow.find(':') {
                let to_part = after_arrow[..colon_pos].trim();
                let label_part = after_arrow[colon_pos + 1..].trim();
                (to_part, if label_part.is_empty() { None } else { Some(label_part.to_string()) })
            } else {
                (after_arrow, None)
            };

            // Track nodes in declaration order (source first, then target).
            if seen.insert(from.to_string()) {
                nodes_order.push(from.to_string());
            }
            if seen.insert(to.to_string()) {
                nodes_order.push(to.to_string());
            }

            edges.push(Edge {
                from: from.to_string(),
                to: to.to_string(),
                label,
                label_extents: None,
                is_cyclic: false,
                route: Vec::new(),
            });
        } else {
            return Err(format!("Unrecognised line: {trimmed}"));
        }
    }

    // Build nodes list in insertion order
    let nodes: Vec<Node> = nodes_order
        .into_iter()
        .map(|id| Node {
            label: id.clone(),
            id,
            label_extents: None,
            node_size: None,
            position: None,
            spanning_index: None,
        })
        .collect();

    Ok(Diagram {
        nodes,
        edges,
        title,
        theme: theme.unwrap_or_default(),
        viewport: Viewport::default(),
    })
}

// ---------------------------------------------------------------------------
// JSON format parser — for structured input from LLMs / adapter layers
// ---------------------------------------------------------------------------

/// A JSON-serializable snapshot for alternate DSL input.
#[derive(serde::Deserialize)]
struct JsonDiagram {
    title: Option<String>,
    #[serde(default)]
    theme: Option<String>,
    states: Vec<String>,
    transitions: Vec<JsonTransition>,
    viewport: Option<JsonViewport>,
}

#[derive(serde::Deserialize)]
struct JsonTransition {
    from: String,
    to: String,
    label: Option<String>,
}

#[derive(serde::Deserialize)]
struct JsonViewport {
    width: u32,
    height: u32,
}

/// Parse a JSON-format diagram description.
pub fn parse_json(json: &str) -> Result<Diagram, String> {
    let jd: JsonDiagram = serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;

    let mut nodes = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for s in &jd.states {
        if seen.insert(s.clone()) {
            nodes.push(Node {
                id: s.clone(),
                label: s.clone(),
                label_extents: None,
                node_size: None,
                position: None,
                spanning_index: None,
            });
        }
    }

    let edges: Vec<Edge> = jd
        .transitions
        .into_iter()
        .map(|jt| {
            // Ensure target state exists
            if seen.insert(jt.to.clone()) {
                // Actually we shouldn't add here in a pure pass — the JSON must declare all states
            }
            Edge {
                from: jt.from,
                to: jt.to,
                label: jt.label,
                label_extents: None,
                is_cyclic: false,
                route: Vec::new(),
            }
        })
        .collect();

    // Ensure all referenced states exist
    for edge in &edges {
        if !nodes.iter().any(|n| n.id == edge.from) {
            nodes.push(Node {
                id: edge.from.clone(),
                label: edge.from.clone(),
                label_extents: None,
                node_size: None,
                position: None,
                spanning_index: None,
            });
        }
        if !nodes.iter().any(|n| n.id == edge.to) {
            nodes.push(Node {
                id: edge.to.clone(),
                label: edge.to.clone(),
                label_extents: None,
                node_size: None,
                position: None,
                spanning_index: None,
            });
        }
    }

    let viewport = jd
        .viewport
        .map(|v| Viewport {
            width: v.width,
            height: v.height,
        })
        .unwrap_or_default();

    Ok(Diagram {
        nodes,
        edges,
        title: jd.title,
        theme: jd.theme.and_then(|t| Theme::from_str(&t)).unwrap_or_default(),
        viewport,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parse() {
        let input = "Idle -> Active : recheck\nActive -> Processing : submit\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.nodes.len(), 3);
        assert_eq!(diagram.edges.len(), 2);
        assert_eq!(diagram.edges[0].label.as_deref(), Some("recheck"));
    }

    #[test]
    fn test_with_title() {
        let input = "title: My Machine\nIdle -> Active\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.title.as_deref(), Some("My Machine"));
    }

    #[test]
    fn test_comments() {
        let input = "# this is a comment\nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.nodes.len(), 2);
        assert_eq!(diagram.edges.len(), 1);
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.nodes.len(), 0);
        assert_eq!(diagram.edges.len(), 0);
        assert!(diagram.title.is_none());
    }

    #[test]
    fn test_only_comments() {
        let input = "# comment 1\n# comment 2\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.nodes.len(), 0);
        assert_eq!(diagram.edges.len(), 0);
    }

    #[test]
    fn test_duplicate_transitions() {
        let input = "A -> B\nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.edges.len(), 2);
        // Nodes should not duplicate
        assert_eq!(diagram.nodes.len(), 2);
    }

    #[test]
    fn test_cyclic_graph() {
        let input = "A -> B\nB -> C\nC -> A\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.nodes.len(), 3);
        assert_eq!(diagram.edges.len(), 3);
    }

    #[test]
    fn test_edge_without_label() {
        let input = "Start -> End\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.edges.len(), 1);
        assert!(diagram.edges[0].label.is_none());
    }

    #[test]
    fn test_multi_word_label() {
        let input = "A -> B : transition with spaces\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.edges[0].label.as_deref(), Some("transition with spaces"));
    }

    #[test]
    fn test_single_node() {
        let input = "Solo -> Solo : loop\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.nodes.len(), 1);
        assert_eq!(diagram.edges.len(), 1);
    }

    #[test]
    fn test_title_with_special_chars() {
        let input = "title: Machine v2.0 (alpha)\nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.title.as_deref(), Some("Machine v2.0 (alpha)"));
    }

    #[test]
    fn test_invalid_line() {
        let input = "this is not valid\n";
        let result = parse_dsl(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_trailing_whitespace() {
        let input = "  A -> B   \n  X -> Y : hello  \n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.nodes.len(), 4);
        assert_eq!(diagram.edges[1].label.as_deref(), Some("hello"));
    }

    #[test]
    fn test_json_parse_basic() {
        let json = r#"{
            "title": "JSON Test",
            "states": ["X", "Y"],
            "transitions": [
                {"from": "X", "to": "Y", "label": "go"}
            ]
        }"#;
        let diagram = parse_json(json).unwrap();
        assert_eq!(diagram.title.as_deref(), Some("JSON Test"));
        assert_eq!(diagram.nodes.len(), 2);
        assert_eq!(diagram.edges.len(), 1);
        assert_eq!(diagram.edges[0].label.as_deref(), Some("go"));
    }

    #[test]
    fn test_json_parse_with_viewport() {
        let json = r#"{
            "states": ["A"],
            "transitions": [],
            "viewport": {"width": 800, "height": 600}
        }"#;
        let diagram = parse_json(json).unwrap();
        assert_eq!(diagram.viewport.width, 800);
        assert_eq!(diagram.viewport.height, 600);
    }

    #[test]
    fn test_json_parse_empty() {
        let json = r#"{"states": [], "transitions": []}"#;
        let diagram = parse_json(json).unwrap();
        assert_eq!(diagram.nodes.len(), 0);
        assert_eq!(diagram.edges.len(), 0);
    }

    #[test]
    fn test_json_parse_invalid() {
        let json = r#"{"bad": "data"}"#;
        let result = parse_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_with_implicit_states() {
        // States referenced in transitions but not declared in "states" array
        let json = r#"{
            "states": ["A"],
            "transitions": [
                {"from": "A", "to": "B"}
            ]
        }"#;
        let diagram = parse_json(json).unwrap();
        // B should be added implicitly
        assert!(diagram.node("B").is_some());
    }

    #[test]
    fn test_parse_theme_directive() {
        let input = "theme: ember\nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.theme, Theme::Ember);
    }

    #[test]
    fn test_parse_theme_default_is_transit() {
        let input = "A -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.theme, Theme::Transit);
    }

    #[test]
    fn test_parse_theme_unknown_errors() {
        let input = "theme: rainbow\nA -> B\n";
        let result = parse_dsl(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_theme_case_insensitive() {
        let input = "theme:  Forest  \nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.theme, Theme::Forest);
    }

    #[test]
    fn test_parse_theme_with_title() {
        let input = "theme: light\ntitle: My Diagram\nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.theme, Theme::Light);
        assert_eq!(diagram.title.as_deref(), Some("My Diagram"));
    }
}
