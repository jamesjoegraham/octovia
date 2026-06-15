//! Per-element SVG builders: node rectangles, edge paths, labels.
//!
//! Each function returns a single SVG fragment (without trailing newline)
//! that `render_svg` stitches together. Shared font constants live here
//! and are reused for every text element.

use crate::ast::{Node, NodeSize, Point, ThemeColors};

use super::defs::{ARROW_CYCLIC_ID, ARROW_FORWARD_ID, GLOW_FILTER_ID};
use super::trim::trim_route_to_node_boundaries;

/// Font used for all text in the diagram.
pub(super) const LABEL_FONT_FAMILY: &str =
    "'JetBrains Mono', 'Fira Code', 'Cascadia Code', 'SF Mono', monospace";
pub(super) const LABEL_FONT_SIZE: f64 = 13.0;

/// Build a `<rect>` string for a node with gradient fill, glow filter, and
/// a subtle inner accent line.
pub(super) fn node_rect(
    node_id: &str,
    pos: Point,
    node_size: &NodeSize,
    colors: &ThemeColors,
) -> String {
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
pub(super) fn edge_path(
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
pub(super) fn node_label_inside(
    node_id: &str,
    text: &str,
    pos: Point,
    colors: &ThemeColors,
) -> String {
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
/// The anchor is decided during routing (see
/// [`crate::label_placement::seek_label_anchor`]) so this function only
/// formats the SVG (including the background-coloured halo stroke via
/// `paint-order`, which masks any line passing behind the label).
pub(super) fn edge_label(
    edge_id: usize,
    text: &str,
    edge: &crate::ast::Edge,
    colors: &ThemeColors,
) -> String {
    let anchor = match edge.label_anchor {
        Some(a) => a,
        None => match crate::label_placement::place_edge_label(&edge.route) {
            Some(a) => a,
            None => return String::new(),
        },
    };

    format!(
        r#"  <text id="elabel-{eid}" x="{lx}" y="{ly}" text-anchor="{anc}" dominant-baseline="central" fill="{fill}" font-family="{fam}" font-size="{size}" paint-order="stroke" stroke="{bg}" stroke-width="3" stroke-linejoin="round" stroke-linecap="round">{esc}</text>"#,
        eid = edge_id,
        lx = anchor.x,
        ly = anchor.y,
        anc = anchor.anchor,
        fill = colors.label_fill,
        fam = LABEL_FONT_FAMILY,
        size = LABEL_FONT_SIZE - 1.0,
        bg = colors.bg,
        esc = escape_xml(text),
    )
}

/// Escape XML special characters.
pub(super) fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
