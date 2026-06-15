//! octovia — A bespoke, DOM-free state diagram rendering engine.
//!
//! Architecture
//! ============
//!
//! The engine processes diagram descriptions through a 5-phase pipeline:
//!
//!   1. **Parse**: Text DSL or JSON → AST.
//!   2. **Measure**: cosmic-text pre-flight layout snaps each node's box
//!      to the 10 px sub-grid (no global width unification).
//!   3. **Layered Layout**: classify back-edges (iterative DFS), assign
//!      every forward node a depth via a longest-path topological sort
//!      (Kahn / Sugiyama L1), then place layers as columns and stack
//!      same-layer nodes vertically.
//!   4. **Unified Routing + Labelling**: a single sequential loop drives
//!      every edge through one A* pass over a [`routing::GridOccupancy`]
//!      grid. After each route is committed, a label anchor is searched
//!      in the local neighbourhood and the label's bounding box is also
//!      reserved, so subsequent edges and labels treat both routes and
//!      labels as impassable terrain.
//!   5. **SVG Output**: deterministic SVG string with the transit-map
//!      aesthetic (octilinear strokes, end-cap arrows, halo'd labels).
//!
//! Compiles to WebAssembly (no DOM, no OS fonts, no headless browser).

pub mod ast;
pub mod label_placement;
pub mod layout;
pub mod measure;
pub mod parser;
pub mod routing;
pub mod svg_output;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// ---------------------------------------------------------------------------
// Top-level convenience API
// ---------------------------------------------------------------------------

use ast::{ThemeColors, Viewport};
use layout::layout_backbone;
use measure::measure_diagram;
use parser::parse_dsl;
use routing::route_all_edges;
use svg_output::render_svg;

/// One-shot convenience: parse DSL → full pipeline → SVG string.
///
/// This is the quickest way to use octovia from Rust.
/// For WASM targets, use the `wasm` module functions instead.
pub fn octo_render(dsl: &str, viewport: Option<Viewport>) -> Result<String, String> {
    octo_render_with_theme(dsl, viewport, None)
}

/// One-shot convenience with optional theme override.
pub fn octo_render_with_theme(
    dsl: &str,
    viewport: Option<Viewport>,
    theme: Option<ThemeColors>,
) -> Result<String, String> {
    let mut diagram = parse_dsl(dsl)?;
    if let Some(vp) = viewport {
        diagram.viewport = vp;
    }
    if let Some(t) = theme {
        diagram.theme = t;
    }

    // Phase 1
    measure_diagram(&mut diagram);

    // Phase 2
    layout_backbone(&mut diagram);

    // Phase 3
    route_all_edges(&mut diagram);

    // Phase 4
    Ok(render_svg(&diagram))
}
