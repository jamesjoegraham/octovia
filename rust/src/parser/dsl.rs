//! Text-format DSL parser.
//!
//! Grammar:
//! ```text
//! title: My State Machine
//! theme: ember
//! background: #112233
//! Idle -> Active : recheck
//! Active -> Processing : submit
//! ```
//!
//! Lines beginning with `#` are comments. Empty lines are skipped.
//! `title:`, `theme:`, and `background:` are optional directives at the top.
//! Each transition line: `Source -> Target : optional_label`.

use crate::ast::{resolve_theme, Background, Diagram, Edge, Node, ThemeColors, Viewport};

/// Parse a text-format DSL string into a Diagram.
pub fn parse_dsl(input: &str) -> Result<Diagram, String> {
    let mut title: Option<String> = None;
    let mut theme: Option<ThemeColors> = None;
    let mut background: Background = Background::Transparent;
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
                resolve_theme(theme_name)
                    .ok_or_else(|| format!("Unknown theme: '{theme_name}'. Use a valid theme name (run without theme: to see defaults)."))?,
            );
            continue;
        }

        // Try to parse as background colour. Two forms are accepted:
        //   * `background`              → use the active theme's `bg`
        //   * `background: <css colour>` → use that explicit value
        //                                  (or the literal `transparent`/`theme`)
        if trimmed == "background" {
            background = Background::Theme;
            continue;
        }
        if trimmed.starts_with("background:") {
            let value = trimmed["background:".len()..].trim();
            if !value.is_empty() {
                background = Background::parse_value(value);
            }
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
        background,
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
    fn test_parse_theme_directive() {
        let input = "theme: ember\nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.theme.bg, "#1C1410");
        assert_eq!(diagram.theme.node_stroke, "#D4803A");
    }

    #[test]
    fn test_parse_theme_default_is_transit() {
        let input = "A -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.theme.bg, "#1A1A2E");
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
        assert_eq!(diagram.theme.node_fill, "#16251D");
        assert_eq!(diagram.theme.node_stroke, "#3D9B6B");
    }

    #[test]
    fn test_parse_theme_with_title() {
        let input = "theme: light\ntitle: My Diagram\nA -> B\n";
        let diagram = parse_dsl(input).unwrap();
        assert_eq!(diagram.theme.bg, "#F5F5F0");
        assert_eq!(diagram.title.as_deref(), Some("My Diagram"));
    }

    #[test]
    fn test_parse_background_default_is_transparent() {
        let diagram = parse_dsl("A -> B\n").unwrap();
        assert_eq!(diagram.background, Background::Transparent);
    }

    #[test]
    fn test_parse_background_directive() {
        let diagram = parse_dsl("background: #112233\nA -> B\n").unwrap();
        assert_eq!(diagram.background, Background::Custom("#112233".to_string()));
    }

    #[test]
    fn test_parse_background_directive_named_color() {
        let diagram = parse_dsl("background: white\nA -> B\n").unwrap();
        assert_eq!(diagram.background, Background::Custom("white".to_string()));
    }

    #[test]
    fn test_parse_background_bare_means_theme() {
        let diagram = parse_dsl("background\nA -> B\n").unwrap();
        assert_eq!(diagram.background, Background::Theme);
    }

    #[test]
    fn test_parse_background_bare_with_theme_directive() {
        let diagram = parse_dsl("theme: ember\nbackground\nA -> B\n").unwrap();
        assert_eq!(diagram.background, Background::Theme);
        // Theme.bg for ember is #1C1410.
        assert_eq!(diagram.background.resolve(&diagram.theme), "#1C1410");
    }

    #[test]
    fn test_parse_background_explicit_transparent() {
        let diagram = parse_dsl("background: transparent\nA -> B\n").unwrap();
        assert_eq!(diagram.background, Background::Transparent);
    }

    #[test]
    fn test_parse_background_value_theme_keyword() {
        let diagram = parse_dsl("background: theme\nA -> B\n").unwrap();
        assert_eq!(diagram.background, Background::Theme);
    }
}
