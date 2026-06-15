//! WASM entry point for the octovia engine.
//!
//! Exposes a clean JS API:
//!   - `render_from_dsl(dsl: &str, viewport_width?: u32, viewport_height?: u32) -> String`
//!   - `render_from_json(json: &str) -> String`
//!
//! Both return an SVG string directly.

use wasm_bindgen::prelude::*;

use crate::ast::{Theme, Viewport};
use crate::layout::layout_backbone;
use crate::measure::measure_diagram;
use crate::parser::{parse_dsl, parse_json};
use crate::routing::route_all_edges;
use crate::svg_output::render_svg;

/// Render a state diagram from the text DSL.
///
/// # Arguments
/// * `dsl` - The text DSL string (sequence-first syntax).
/// * `viewport_width` - Optional viewport width in pixels (default: 1200).
/// * `viewport_height` - Optional viewport height in pixels (default: 800).
/// * `theme` - Optional theme string: "transit", "ember", "forest", "light", "monochrome".
///
/// # Returns
/// An SVG string, or a JS error.
#[wasm_bindgen]
pub fn render_from_dsl(
    dsl: &str,
    viewport_width: Option<u32>,
    viewport_height: Option<u32>,
    theme: Option<String>,
) -> Result<String, JsError> {
    let mut diagram = parse_dsl(dsl).map_err(|e| JsError::new(&e))?;

    // Override viewport if provided
    if let Some(w) = viewport_width {
        diagram.viewport.width = w;
    }
    if let Some(h) = viewport_height {
        diagram.viewport.height = h;
    }

    // Override theme if provided
    if let Some(ref t) = theme {
        diagram.theme = Theme::from_str(t)
            .ok_or_else(|| JsError::new(&format!("Unknown theme: '{t}'. Options: transit, ember, forest, light, monochrome")))?;
    }

    run_pipeline(&mut diagram)
}

/// Render a state diagram from a JSON description.
///
/// # Returns
/// An SVG string, or a JS error.
#[wasm_bindgen]
pub fn render_from_json(json: &str) -> Result<String, JsError> {
    let mut diagram = parse_json(json).map_err(|e| JsError::new(&e))?;
    run_pipeline(&mut diagram)
}

/// Run the full rendering pipeline on a diagram.
fn run_pipeline(diagram: &mut crate::ast::Diagram) -> Result<String, JsError> {
    // Phase 1: Measure all text labels
    measure_diagram(diagram);

    // Phase 2: Backbone layout (spanning tree + boustrophedon placement)
    layout_backbone(diagram);

    // Phase 3: Route all edges (forward edges + A* for cyclic)
    route_all_edges(diagram);

    // Phase 4: Render SVG
    Ok(render_svg(diagram))
}
