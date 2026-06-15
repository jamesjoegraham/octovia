//! JSON-format parser — for structured input from LLMs / adapter layers.

use crate::ast::{resolve_theme, Background, Diagram, Edge, Node, Viewport};

/// A JSON-serializable snapshot for alternate DSL input.
#[derive(serde::Deserialize)]
struct JsonDiagram {
    title: Option<String>,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    background: Option<String>,
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
                label_anchor: None,
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
        theme: jd.theme.and_then(|t| resolve_theme(&t)).unwrap_or_default(),
        viewport,
        background: jd
            .background
            .map(|s| Background::parse_value(&s))
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_parse_json_background_field() {
        let json = r#"{
            "background": "transparent",
            "states": ["A", "B"],
            "transitions": [{"from": "A", "to": "B"}]
        }"#;
        let diagram = parse_json(json).unwrap();
        assert_eq!(diagram.background, Background::Transparent);
    }

    #[test]
    fn test_parse_json_background_theme_keyword() {
        let json = r#"{
            "background": "theme",
            "states": ["A", "B"],
            "transitions": [{"from": "A", "to": "B"}]
        }"#;
        let diagram = parse_json(json).unwrap();
        assert_eq!(diagram.background, Background::Theme);
    }
}
