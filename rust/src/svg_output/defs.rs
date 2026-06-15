//! `<defs>` block: filters, gradients, and arrow markers.
//!
//! Marker / filter IDs are also re-used by `elements.rs` to point edge
//! paths and node rectangles at the right defs entry, so they live here
//! and are imported via `super::defs::FOO_ID`.

use crate::ast::ThemeColors;

/// SVG marker id for forward (solid) edge arrowheads.
pub(super) const ARROW_FORWARD_ID: &str = "octovia-arrow-forward";
/// SVG marker id for cyclic (dashed) edge arrowheads.
pub(super) const ARROW_CYCLIC_ID: &str = "octovia-arrow-cyclic";

/// Glow filter id.
pub(super) const GLOW_FILTER_ID: &str = "octovia-glow";
/// Drop shadow filter id.
pub(super) const SHADOW_FILTER_ID: &str = "octovia-shadow";

/// Build the full `<defs>` block: filters, gradients, and arrow markers.
pub(super) fn build_defs(colors: &ThemeColors) -> String {
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
