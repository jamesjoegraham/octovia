//! octovia — A bespoke, DOM-free state diagram rendering engine.
//!
//! Architecture
//! ============
//!
//! The engine processes diagram descriptions through a 5-phase pipeline:
//!
//!   1. **Parse**: Text DSL or JSON → AST
//!   2. **Measure**: cosmic-text pre-flight layout for all labels
//!   3. **Backbone Layout**: Spanning tree + boustrophedon grid placement
//!   4. **Cyclic Routing**: A* pathfinding with transit-map cost function
//!   5. **SVG Output**: Clean SVG string with transit-map aesthetic
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
