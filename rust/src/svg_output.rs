//! SVG Output Phase (Phase 5).
//!
//! Serialises the fully-resolved diagram (node positions + edge routes +
//! label anchors) into a clean, self-contained SVG string.
//!
//! The output features a polished transit-map aesthetic with:
//! -   Rounded-rectangle nodes with gradient fills and subtle glow
//! -   SVG filter definitions for dropshadow and glow effects
//! -   Edge paths with arrow markers and lane offsets
//! -   Edge labels with text-halo (paint-order stroke) for occlusion

use crate::ast::{Diagram, Node, NodeSize, Point, ThemeColors};

// ---------------------------------------------------------------------------
// SVG constants
// ---------------------------------------------------------------------------

/// Font used for all text in the diagram.
const LABEL_FONT_FAMILY: &str = "'JetBrains Mono', 'Fira Code', 'Cascadia Code', 'SF Mono', monospace";
const LABEL_FONT_SIZE: f64 = 13.0;

/// SVG marker id for forward (solid) edge arrowheads.
const ARROW_FORWARD_ID: &str = "octovia-arrow-forward";
/// SVG marker id for cyclic (dashed) edge arrowheads.
const ARROW_CYCLIC_ID: &str = "octovia-arrow-cyclic";

/// Glow filter id.
const GLOW_FILTER_ID: &str = "octovia-glow";
/// Drop shadow filter id.
const SHADOW_FILTER_ID: &str = "octovia-shadow";

// ---------------------------------------------------------------------------
// SVG element builders
// ---------------------------------------------------------------------------

/// Build the full `<defs>` block: filters, gradients, and arrow markers.
fn build_defs(colors: &ThemeColors) -> String {
    let mut defs = String::from("  <defs>\n");

    // --- Glow filter (applied to node rectangles) ---
    defs.push_str(&format!(
        r#"    <filter id="{glow}" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="4" result="blur"/>
      <feFlood flood-color="{glow_color}" flood-opacity="0.6" result="color"/>
      <feComposite in="color" in2="blur" operator="in" result="shadow"/>
      <feMerge>
        <feMergeNode in="shadow"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
"#,
        glow = GLOW_FILTER_ID,
        glow_color = colors.node_glow,
    ));

    // --- Drop shadow filter (subtle under nodes) ---
    let black = "#000000";
    defs.push_str(&format!(
        r#"    <filter id="{shadow}" x="-10%" y="-10%" width="130%" height="130%">
      <feDropShadow dx="0" dy="3" stdDeviation="4" flood-color="{black}" flood-opacity="0.5"/>
    </filter>
"#,
        shadow = SHADOW_FILTER_ID,
        black = black,
    ));

    // --- Node gradient (vertical linear gradient per node fill) ---
    defs.push_str(&format!(
        r#"    <linearGradient id="node-grad" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="{start}"/>
      <stop offset="100%" stop-color="{end}"/>
    </linearGradient>
"#,
        start = colors.gradient_start,
        end = colors.gradient_end,
    ));

    // --- Arrowhead marker: forward ---
    defs.push_str(&format!(
        r#"    <marker id="{fwd}" viewBox="0 0 14 14" refX="12" refY="7" markerWidth="12" markerHeight="12" orient="auto-start-reverse" markerUnits="userSpaceOnUse">
      <path d="M 0 0 L 14 7 L 0 14 z" fill="{fwd_color}"/>
    </marker>
"#,
        fwd = ARROW_FORWARD_ID,
        fwd_color = colors.edge_forward,
    ));

    // --- Arrowhead marker: cyclic ---
    defs.push_str(&format!(
        r#"    <marker id="{cyc}" viewBox="0 0 14 14" refX="12" refY="7" markerWidth="12" markerHeight="12" orient="auto-start-reverse" markerUnits="userSpaceOnUse">
      <path d="M 0 0 L 14 7 L 0 14 z" fill="{cyc_color}"/>
    </marker>
"#,
        cyc = ARROW_CYCLIC_ID,
        cyc_color = colors.edge_cyclic,
    ));

    defs.push_str("  </defs>");
    defs
}

/// Build a `<rect>` string for a node with gradient fill, glow filter, and
/// a subtle inner accent line.
fn node_rect(node_id: &str, pos: Point, node_size: &NodeSize, colors: &ThemeColors) -> String {
    let x = pos.x - node_size.half_w();
    let y = pos.y - node_size.half_h();
    let r = 8.0; // corner radius

    format!(
        r#"  <rect id="node-{node_id}" x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" ry="{r}"
        fill="url(#node-grad)" stroke="{stroke}" stroke-width="2.0"
        filter="url(#{glow})"/>"""#,
        node_id = node_id,
        x = x,
        y = y,
        w = node_size.width,
        h = node_size.height,
        r = r,
        stroke = colors.node_stroke,
        glow = GLOW_FILTER_ID,
    )
}

/// Build a `<path>` string for an edge route. The route is trimmed so that
/// the visible polyline starts and ends on the source / target rectangle
/// boundaries — this keeps the arrowhead from being hidden under the node
/// rectangle that's drawn on top of the edge layer.
fn edge_path(
    edge_id: usize,
    route: &[Point],
    is_cyclic: bool,
    src: Option<&Node>,
    tgt: Option<&Node>,
    colors: &ThemeColors,
) -> String {
    if route.len() < 2 {
        return String::new();
    }

    let trimmed = trim_route_to_node_boundaries(route, src, tgt);
    if trimmed.len() < 2 {
        return String::new();
    }

    let d: String = trimmed
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                format!("M {} {}", p.x, p.y)
            } else {
                format!("L {} {}", p.x, p.y)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let stroke = if is_cyclic {
        &colors.edge_cyclic
    } else {
        &colors.edge_forward
    };

    let dash = if is_cyclic { r#" stroke-dasharray="6,4""# } else { "" };
    let marker_id = if is_cyclic { ARROW_CYCLIC_ID } else { ARROW_FORWARD_ID };

    format!(
        r#"  <path id="edge-{edge_id}" d="{d}" fill="none" stroke="{stroke}" stroke-width="3.5" stroke-linecap="butt" stroke-linejoin="round"{dash} marker-end="url(#{mid})" />"#,
        edge_id = edge_id,
        d = d,
        stroke = stroke,
        dash = dash,
        mid = marker_id,
    )
}

/// Build a `<text>` element for a node label (always centred inside the node).
fn node_label_inside(node_id: &str, text: &str, pos: Point, colors: &ThemeColors) -> String {
    format!(
        r#"  <text id="label-{nid}" x="{x}" y="{y}" text-anchor="middle" dominant-baseline="central" fill="{fill}" font-family="{fam}" font-size="{size}">{esc}</text>"#,
        nid = node_id,
        x = pos.x,
        y = pos.y,
        fill = colors.label_fill,
        fam = LABEL_FONT_FAMILY,
        size = LABEL_FONT_SIZE,
        esc = escape_xml(text),
    )
}

/// Build a `<text>` element for an edge label.
///
/// The label is placed at the route's midpoint and offset *perpendicular*
/// to the local segment direction (above for horizontal segments, beside
/// for vertical segments) so it never sits on top of the line it
/// describes. A halo stroke matching the canvas background is added via
/// `paint-order` so any line that does pass behind the label is masked
/// out cleanly.
fn edge_label(edge_id: usize, text: &str, route: &[Point], colors: &ThemeColors) -> String {
    if route.len() < 2 {
        return String::new();
    }

    let mid = route.len() / 2;
    let p = route[mid];

    // Look at the segment *around* the midpoint to determine orientation.
    let prev = route[mid.saturating_sub(1)];
    let next_idx = (mid + 1).min(route.len() - 1);
    let next = route[next_idx];
    let dx = (next.x - prev.x).abs();
    let dy = (next.y - prev.y).abs();

    // Perpendicular offset: horizontal segment → place label above the line;
    // vertical segment → place label to the right of the line.
    let (lx, ly, anchor) = if dx >= dy {
        (p.x, p.y - 10, "middle")
    } else {
        (p.x + 10, p.y, "start")
    };

    format!(
        r#"  <text id="elabel-{eid}" x="{lx}" y="{ly}" text-anchor="{anc}" dominant-baseline="central" fill="{fill}" font-family="{fam}" font-size="{size}" paint-order="stroke" stroke="{bg}" stroke-width="3" stroke-linejoin="round" stroke-linecap="round">{esc}</text>"#,
        eid = edge_id,
        lx = lx,
        ly = ly,
        anc = anchor,
        fill = colors.label_fill,
        fam = LABEL_FONT_FAMILY,
        size = LABEL_FONT_SIZE - 1.0,
        bg = colors.bg,
        esc = escape_xml(text),
    )
}

// ---------------------------------------------------------------------------
// Route trimming (so arrowheads aren't covered by the node rectangle)
// ---------------------------------------------------------------------------

/// Strict containment test (open rectangle interior).
fn point_inside_rect(p: Point, center: Point, half_w: i32, half_h: i32) -> bool {
    p.x > center.x - half_w
        && p.x < center.x + half_w
        && p.y > center.y - half_h
        && p.y < center.y + half_h
}

/// Given a segment whose `inside` endpoint sits inside an axis-aligned
/// rectangle and `outside` endpoint sits outside it, return the point at
/// which the segment crosses the rectangle boundary.
fn segment_rect_exit(
    inside: Point,
    outside: Point,
    center: Point,
    half_w: i32,
    half_h: i32,
) -> Point {
    let ax = inside.x as f64;
    let ay = inside.y as f64;
    let dx = (outside.x - inside.x) as f64;
    let dy = (outside.y - inside.y) as f64;

    let mut t_exit = 1.0_f64;

    if dx.abs() > 1e-9 {
        for &xb in &[(center.x - half_w) as f64, (center.x + half_w) as f64] {
            let t = (xb - ax) / dx;
            if t > 1e-9 && t < t_exit {
                t_exit = t;
            }
        }
    }
    if dy.abs() > 1e-9 {
        for &yb in &[(center.y - half_h) as f64, (center.y + half_h) as f64] {
            let t = (yb - ay) / dy;
            if t > 1e-9 && t < t_exit {
                t_exit = t;
            }
        }
    }

    Point::new(
        (ax + t_exit * dx).round() as i32,
        (ay + t_exit * dy).round() as i32,
    )
}

/// Trim a routed polyline so its first and last points lie on the
/// boundaries of the source and target node rectangles respectively.
///
/// The router emits routes that start at the source's port cell (a fixed
/// 30 px from node centre) and finish at the target's port cell. For nodes
/// whose rectangles are wider or taller than the port offset (because the
/// label needed extra padding), those endpoints actually live *inside* the
/// node rectangle. Since the rectangle is drawn on top of the edge in
/// z-order, any arrowhead at the route's last point would be hidden.
///
/// This function clips the head and tail of the route against the node
/// rectangles so the visible polyline ends exactly at the rectangle edge.
fn trim_route_to_node_boundaries(
    route: &[Point],
    src: Option<&Node>,
    tgt: Option<&Node>,
) -> Vec<Point> {
    if route.len() < 2 {
        return route.to_vec();
    }

    let mut points: Vec<Point> = route.to_vec();

    // ---- Source side: trim leading points that sit inside source rect. ----
    if let Some(node) = src {
        if let (Some(pos), Some(size)) = (node.position, node.node_size) {
            let hw = size.half_w();
            let hh = size.half_h();
            // First index with a point outside the rectangle.
            let first_out = points
                .iter()
                .position(|p| !point_inside_rect(*p, pos, hw, hh));
            if let Some(i) = first_out {
                if i > 0 {
                    let exit = segment_rect_exit(points[i - 1], points[i], pos, hw, hh);
                    let mut new_pts = Vec::with_capacity(points.len() - i + 1);
                    new_pts.push(exit);
                    new_pts.extend_from_slice(&points[i..]);
                    points = new_pts;
                }
            }
        }
    }

    // ---- Target side: clip at the first point that enters the target rect. ----
    if let Some(node) = tgt {
        if let (Some(pos), Some(size)) = (node.position, node.node_size) {
            let hw = size.half_w();
            let hh = size.half_h();
            let first_in = points
                .iter()
                .position(|p| point_inside_rect(*p, pos, hw, hh));
            if let Some(i) = first_in {
                if i > 0 {
                    let entry = segment_rect_exit(points[i], points[i - 1], pos, hw, hh);
                    points.truncate(i);
                    points.push(entry);
                }
            }
        }
    }

    points
}

/// Escape XML special characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------------------------------------------------------------------------
// Main serialiser
// ---------------------------------------------------------------------------

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
    svg.push_str(&build_defs(&colors));
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
        svg.push_str(&edge_path(i, &edge.route, edge.is_cyclic, src, tgt, &colors));
        svg.push('\n');
    }

    // Edge labels (between edges and nodes)
    for (i, edge) in diagram.edges.iter().enumerate() {
        if let Some(ref label) = edge.label {
            svg.push_str(&edge_label(i, label, &edge.route, &colors));
            svg.push('\n');
        }
    }

    // Nodes
    for node in &diagram.nodes {
        if let Some(pos) = node.position {
            let ns = node.node_size.unwrap_or(NodeSize { width: 60, height: 60 });
            svg.push_str(&node_rect(&node.id, pos, &ns, &colors));
            svg.push('\n');

            // Node label (always centred inside node)
            svg.push_str(&node_label_inside(&node.id, &node.label, pos, &colors));
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
    use super::*;
    use crate::ast::{Background, Diagram, Node, NodeSize, TextExtents, ThemeColors, Viewport};
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
        let mut d = Diagram {
            nodes: vec![
                Node {
                    id: "X".into(),
                    label: "A & B < C > D".into(),
                    label_extents: Some(TextExtents { width: 80.0, height: 16.0 }),
                    node_size: Some(NodeSize { width: 104, height: 40 }),
                    position: Some(Point::new(100, 100)),
                    spanning_index: Some(0),
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
        let light = ThemeColors::from_str("light").unwrap();
        d.theme = light;
        measure_diagram(&mut d);
        layout_backbone(&mut d);
        route_all_edges(&mut d);
        let svg = render_svg(&d);
        assert!(svg.contains("#F5F5F0"), "light bg (label halo)");
        assert!(svg.contains("#2C2C2E"), "light label color");
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
        let mut d_h = parse_dsl("A -> B : go\n").unwrap();
        measure_diagram(&mut d_h);
        layout_backbone(&mut d_h);
        route_all_edges(&mut d_h);
        let svg_h = render_svg(&d_h);

        let label_line = svg_h
            .lines()
            .find(|l| l.contains("elabel-0"))
            .expect("edge label missing");
        assert!(
            label_line.contains(r#"text-anchor="middle""#),
            "horizontal edge label should anchor middle: {label_line}"
        );

        let mut d_v = parse_dsl(
            "Draft -> Review : submit\n\
             Review -> Approved : approve\n\
             Review -> Revisions : revise\n\
             Revisions -> Draft : redraft\n\
             Revisions -> Review : resubmit\n\
             Approved -> Published : publish\n",
        )
        .unwrap();
        d_v.viewport = Viewport { width: 900, height: 800 };
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
            "expected at least one vertical edge label using text-anchor=start"
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
