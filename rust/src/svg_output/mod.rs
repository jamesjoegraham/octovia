//! SVG output phase.
//!
//! Serialises the fully-resolved diagram (node positions + edge routes +
//! label anchors) into a clean, self-contained SVG string with a polished
//! transit-map aesthetic:
//!
//! -   Rounded-rectangle nodes with gradient fills and subtle glow
//! -   SVG filter definitions for dropshadow and glow effects
//! -   Edge paths with arrow markers and lane offsets
//! -   Edge labels with text-halo (paint-order stroke) for occlusion
//!
//! Submodules:
//! -   [`defs`] — `<defs>` block (filters, gradients, arrow markers).
//! -   [`elements`] — per-element builders (rect, path, text).
//! -   [`trim`] — route trimming so arrowheads aren't hidden by node rects.

mod defs;
mod elements;
mod trim;

use crate::ast::{Diagram, NodeSize};

use defs::build_defs;
use elements::{
    edge_label, edge_path, escape_xml, node_label_inside, node_rect, LABEL_FONT_FAMILY,
};

/// Serialise a fully-resolved diagram into an SVG string.
pub fn render_svg(diagram: &Diagram) -> String {
    let colors = diagram.theme.colors();
    let mut svg = String::with_capacity(4096);

    // Compute bounding box from all node positions and edge routes
    let (min_x, min_y, max_x, max_y) = compute_bounds(diagram);
    // Clamp to 0 to avoid underflow when negative coordinates cast to u32
    let min_x_clamped = min_x.max(0);
    let min_y_clamped = min_y.max(0);
    let (vw, vh) = (
        (max_x as u32).saturating_sub(min_x_clamped as u32) + 80,
        (max_y as u32).saturating_sub(min_y_clamped as u32) + 80,
    );
    let vx = min_x.saturating_sub(40);
    let vy = min_y.saturating_sub(40);

    // Resolve canvas background. Defaults to "transparent" so the SVG can
    // be embedded over arbitrary host backgrounds; users can opt in to the
    // theme's `bg` (or any custom CSS colour) via `Diagram::background`.
    let canvas_bg = diagram.background.resolve(&diagram.theme);

    // SVG open + background
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx} {vy} {vw} {vh}" width="{vw}" height="{vh}" style="background-color: {bg}">"#,
        vx = vx,
        vy = vy,
        vw = vw,
        vh = vh,
        bg = canvas_bg,
    ));
    svg.push('\n');

    // Definition block: glow filter, shadow, gradients, arrow markers
    svg.push_str(&build_defs(colors));
    svg.push('\n');

    // Optional title
    if let Some(ref title) = diagram.title {
        let tx = (min_x + max_x) / 2;
        let ty = min_y.saturating_sub(10);
        svg.push_str(&format!(
            r#"  <text x="{tx}" y="{ty}" text-anchor="middle" fill="{tfill}" font-family="{fam}" font-size="18" font-weight="600">{esc}</text>"#,
            tx = tx,
            ty = ty,
            tfill = colors.title_fill,
            fam = LABEL_FONT_FAMILY,
            esc = escape_xml(title),
        ));
        svg.push('\n');
    }

    // Edges (bottom layer — under nodes)
    for (i, edge) in diagram.edges.iter().enumerate() {
        let src = diagram.node(&edge.from);
        let tgt = diagram.node(&edge.to);
        svg.push_str(&edge_path(i, &edge.route, edge.is_cyclic, src, tgt, colors));
        svg.push('\n');
    }

    // Edge labels (between edges and nodes)
    for (i, edge) in diagram.edges.iter().enumerate() {
        if let Some(ref label) = edge.label {
            svg.push_str(&edge_label(i, label, edge, colors));
            svg.push('\n');
        }
    }

    // Nodes
    for node in &diagram.nodes {
        if let Some(pos) = node.position {
            let ns = node.node_size.unwrap_or(NodeSize { width: 60, height: 60 });
            svg.push_str(&node_rect(&node.id, pos, &ns, colors));
            svg.push('\n');

            // Node label (always centred inside node)
            svg.push_str(&node_label_inside(&node.id, &node.label, pos, colors));
            svg.push('\n');
        }
    }

    svg.push_str("</svg>");
    svg
}

/// Compute the bounding box enclosing all nodes and edge routes.
fn compute_bounds(diagram: &Diagram) -> (i32, i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for node in &diagram.nodes {
        if let Some(pos) = node.position {
            let ns = node.node_size.unwrap_or(NodeSize { width: 60, height: 60 });
            let margin = ns.half_w().max(ns.half_h()) + 20;
            min_x = min_x.min(pos.x - margin);
            min_y = min_y.min(pos.y - margin);
            max_x = max_x.max(pos.x + margin);
            max_y = max_y.max(pos.y + margin);
        }
    }

    for edge in &diagram.edges {
        for p in &edge.route {
            min_x = min_x.min(p.x - 10);
            min_y = min_y.min(p.y - 10);
            max_x = max_x.max(p.x + 10);
            max_y = max_y.max(p.y + 10);
        }
    }

    if min_x > max_x {
        // Fallback for empty diagram
        (0, 0, 800, 600)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

#[cfg(test)]
mod tests {
    use super::defs::{ARROW_CYCLIC_ID, ARROW_FORWARD_ID, GLOW_FILTER_ID};
    use super::trim::point_inside_rect;
    use super::*;
    use crate::ast::{
        Background, Diagram, Node, NodeSize, Point, TextExtents, ThemeColors, Viewport,
    };
    use crate::layout::layout_backbone;
    use crate::measure::measure_diagram;
    use crate::parser::parse_dsl;
    use crate::routing::route_all_edges;

    #[test]
    fn test_render_simple_diagram() {
        let mut d = parse_dsl("Idle -> Active : check\nActive -> Done : finish").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let svg = render_svg(&d);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("node-Idle"));
        assert!(svg.contains("node-Active"));
        assert!(svg.contains("edge-0"));
        assert!(svg.contains("label-Idle"));
    }

    #[test]
    fn test_render_cycle_diagram() {
        let mut d = parse_dsl("A -> B\nB -> C\nC -> A").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let svg = render_svg(&d);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("node-A"));
        assert!(svg.contains("node-B"));
        assert!(svg.contains("node-C"));
    }

    #[test]
    fn test_render_empty_diagram() {
        let d = Diagram {
            nodes: vec![],
            edges: vec![],
            title: None,
            viewport: Viewport::default(),
            theme: ThemeColors::default(),
            background: Background::default(),
        };
        let svg = render_svg(&d);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("viewBox"));
        // Default canvas background is transparent.
        assert!(svg.contains("background-color: transparent"));
    }

    #[test]
    fn test_render_with_title() {
        let mut d = parse_dsl("title: My Machine\nA -> B\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let svg = render_svg(&d);
        assert!(svg.contains("My Machine"));
        assert!(svg.contains("font-weight=\"600\""));
    }

    #[test]
    fn test_render_xml_escaping() {
        let d = Diagram {
            nodes: vec![
                Node {
                    id: "X".into(),
                    label: "A & B < C > D".into(),
                    label_extents: Some(TextExtents { width: 80.0, height: 16.0 }),
                    node_size: Some(NodeSize { width: 104, height: 40 }),
                    position: Some(Point::new(100, 100)),
                    layer: Some(0),
                },
            ],
            edges: vec![],
            title: Some("Title & <special> \"chars\"".into()),
            viewport: Viewport::default(),
            theme: ThemeColors::default(),
            background: Background::default(),
        };
        let svg = render_svg(&d);
        assert!(svg.contains("&amp;"));
        assert!(svg.contains("&lt;"));
        assert!(svg.contains("&gt;"));
        assert!(svg.contains("&quot;"));
    }

    #[test]
    fn test_render_edge_labels() {
        let mut d = parse_dsl("X -> Y : transition-label\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        assert!(svg.contains("transition-label"));
        assert!(svg.contains("elabel-0"));
    }

    #[test]
    fn test_render_defs_contains_gradients_and_filters() {
        let mut d = parse_dsl("A -> B\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);

        // Must have gradient definitions
        assert!(svg.contains("linearGradient"), "gradient defs missing");
        assert!(svg.contains("node-grad"), "node gradient missing");

        // Must have glow filter
        assert!(svg.contains(GLOW_FILTER_ID), "glow filter missing");
        assert!(svg.contains("feGaussianBlur"), "blur filter missing");
    }

    #[test]
    fn test_render_theme_colors_applied() {
        let mut d = parse_dsl("A -> B : go\n").unwrap();
        let ember = ThemeColors::from_str("ember").unwrap();
        d.theme = ember;
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        // Ember uses warm copper tones; bg appears in label halo stroke.
        assert!(svg.contains("#D4803A"), "ember forward edge color");
        assert!(svg.contains("#1C1410"), "ember bg color (label halo)");
    }

    #[test]
    fn test_render_light_theme() {
        let mut d = parse_dsl("A -> B : go\n").unwrap();
        let paper = ThemeColors::from_str("paper").unwrap();
        d.theme = paper;
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        assert!(svg.contains("#F5F0E8"), "paper bg (label halo)");
        assert!(svg.contains("#1F2937"), "paper label color");
    }

    #[test]
    fn test_render_jetbrains_mono_in_output() {
        let mut d = parse_dsl("A -> B\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        assert!(svg.contains("JetBrains Mono"), "JetBrains Mono in font-family");
        assert!(svg.contains("monospace"), "monospace fallback present");
    }

    #[test]
    fn test_render_emits_arrow_markers() {
        let mut d = parse_dsl("A -> B\nB -> C\nC -> A\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);

        // <defs> with arrow markers must be present.
        assert!(svg.contains("<defs>"), "defs block missing");
        assert!(
            svg.contains(&format!(r#"id="{}""#, ARROW_FORWARD_ID)),
            "forward arrow marker missing"
        );
        assert!(
            svg.contains(&format!(r#"id="{}""#, ARROW_CYCLIC_ID)),
            "cyclic arrow marker missing"
        );

        // Every edge path must reference an arrow marker.
        let edge_lines: Vec<&str> = svg.lines().filter(|l| l.contains("<path id=\"edge-")).collect();
        assert!(!edge_lines.is_empty(), "no edge paths emitted");
        for line in &edge_lines {
            assert!(
                line.contains("marker-end="),
                "edge path missing marker-end: {line}"
            );
        }

        // Forward edges use the forward marker; cyclic edges the cyclic one.
        let forward_marker_url = format!("url(#{})", ARROW_FORWARD_ID);
        let cyclic_marker_url = format!("url(#{})", ARROW_CYCLIC_ID);
        let forward_count = edge_lines
            .iter()
            .filter(|l| !l.contains("stroke-dasharray"))
            .filter(|l| l.contains(&forward_marker_url))
            .count();
        let cyclic_count = edge_lines
            .iter()
            .filter(|l| l.contains("stroke-dasharray"))
            .filter(|l| l.contains(&cyclic_marker_url))
            .count();
        assert!(forward_count >= 1, "no forward edges using forward marker");
        assert!(cyclic_count >= 1, "no cyclic edges using cyclic marker");
    }

    #[test]
    fn test_edge_path_trimmed_to_node_boundary() {
        let mut d = parse_dsl("Closed -> SynReceived : open\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);

        let target = d.node("SynReceived").expect("target node").clone();
        let tgt_pos = target.position.unwrap();
        let tgt_size = target.node_size.unwrap();

        let svg = render_svg(&d);

        let edge_d = svg
            .lines()
            .find(|l| l.contains("<path id=\"edge-0\""))
            .and_then(|l| l.split(r#" d=""#).nth(1))
            .and_then(|rest| rest.split('"').next())
            .expect("edge-0 path d attribute");

        let last_pair = edge_d
            .rsplit_once('L')
            .map(|(_, rhs)| rhs.trim())
            .expect("path must contain an L command");
        let mut nums = last_pair.split_whitespace();
        let lx: i32 = nums.next().unwrap().parse().unwrap();
        let ly: i32 = nums.next().unwrap().parse().unwrap();

        let half_w = tgt_size.half_w();
        let half_h = tgt_size.half_h();
        let on_left = (lx - (tgt_pos.x - half_w)).abs() <= 1;
        let on_right = (lx - (tgt_pos.x + half_w)).abs() <= 1;
        let on_top = (ly - (tgt_pos.y - half_h)).abs() <= 1;
        let on_bottom = (ly - (tgt_pos.y + half_h)).abs() <= 1;
        assert!(
            on_left || on_right || on_top || on_bottom,
            "edge endpoint ({lx},{ly}) should sit on target rect boundary \
             (centre={:?}, half_w={half_w}, half_h={half_h})",
            tgt_pos
        );
        assert!(
            !point_inside_rect(Point::new(lx, ly), tgt_pos, half_w - 1, half_h - 1),
            "edge endpoint sits inside the node rectangle"
        );
    }

    #[test]
    fn test_edge_label_perpendicular_offset() {
        // Vertical edge (TTB) — actual pipeline output should anchor start.
        let mut d_v = parse_dsl("A -> B : go\n").unwrap();
        measure_diagram(&mut d_v);
        layout_backbone(&mut d_v);
        route_all_edges(&mut d_v);
        let svg_v = render_svg(&d_v);

        let any_vertical = svg_v
            .lines()
            .filter(|l| l.contains("elabel-"))
            .any(|l| l.contains(r#"text-anchor="start""#));
        assert!(
            any_vertical,
            "expected vertical (TTB) edge label using text-anchor=start"
        );

        // Horizontal (same-layer) edge — hand-craft two nodes at the same
        // Y level with a horizontal route so we can verify the anchor
        // field flows through to `text-anchor="middle"`.
        let mut d_h = parse_dsl("A -> B : go\n").unwrap();
        measure_diagram(&mut d_h);
        layout_backbone(&mut d_h);

        // Place A and B at the same Y, different X.
        let a_pos = d_h.node("A").unwrap().position.unwrap();
        let b_pos_target = Point::new(a_pos.x + 200, a_pos.y);
        d_h.node_mut("B").unwrap().position = Some(b_pos_target);
        d_h.edges[0].route = vec![
            a_pos,
            Point::new(a_pos.x + 50, a_pos.y),
            Point::new(a_pos.x + 100, a_pos.y),
            Point::new(a_pos.x + 150, a_pos.y),
            b_pos_target,
        ];
        d_h.edges[0].label_anchor = crate::label_placement::place_edge_label(&d_h.edges[0].route);

        let svg_h = render_svg(&d_h);
        let label_line = svg_h
            .lines()
            .find(|l| l.contains("elabel-0"))
            .expect("edge label missing");
        assert!(
            label_line.contains(r#"text-anchor="middle""#),
            "horizontal (same-layer) edge label should anchor middle: {label_line}"
        );
    }

    #[test]
    fn test_edge_label_has_halo_for_line_occlusion() {
        let mut d = parse_dsl("A -> B : trigger\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);

        let label_line = svg
            .lines()
            .find(|l| l.contains("elabel-0"))
            .expect("edge label missing");
        assert!(
            label_line.contains(r#"paint-order="stroke""#),
            "edge label missing paint-order halo: {label_line}"
        );
        // The halo stroke must be the bg colour (Transit theme: #1A1A2E).
        assert!(
            label_line.contains(r##"stroke="#1A1A2E""##),
            "edge label halo must use bg colour: {label_line}"
        );
    }

    #[test]
    fn test_canvas_background_defaults_to_transparent() {
        let mut d = parse_dsl("A -> B\n").unwrap();
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        assert!(
            svg.contains("background-color: transparent"),
            "default canvas background should be transparent"
        );
    }

    #[test]
    fn test_canvas_background_override_applied() {
        let mut d = parse_dsl("A -> B\n").unwrap();
        d.background = Background::Custom("#abcdef".to_string());
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        assert!(
            svg.contains("background-color: #abcdef"),
            "canvas background should honour the diagram.background override"
        );
    }

    #[test]
    fn test_canvas_background_theme_uses_theme_bg() {
        let mut d = parse_dsl("A -> B\n").unwrap();
        d.theme = ThemeColors::from_str("ember").unwrap();
        d.background = Background::Theme;
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        // Ember bg is #1C1410 — it must show up in the SVG canvas style.
        assert!(
            svg.contains("background-color: #1C1410"),
            "Background::Theme should render the theme.bg colour as the canvas"
        );
    }
}
