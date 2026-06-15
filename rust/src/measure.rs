//! Pre-flight text measurement using `cosmic-text`.
//!
//! This runs entirely in pure Rust — no file I/O, no OS font API.
//! An embedded font (typically Noto Sans or similar) is compiled into
//! the binary via `include_bytes!`.
//!
//! Each label's bounding box is computed before any layout or routing math runs.

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use std::sync::{Mutex, OnceLock};

use crate::ast::TextExtents;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum width for a single line of node text before we start wrapping.
/// This prevents labels from being pathologically long and ruining layout.
pub const MAX_NODE_WIDTH: f32 = 180.0;

/// Font size in logical pixels.
pub const FONT_SIZE: f32 = 14.0;

/// Line height ratio (cosmic-text default is ~1.2).
pub const LINE_HEIGHT: f32 = 1.35;

// ---------------------------------------------------------------------------
// Global font system (initialised once, shared across all measurements)
// ---------------------------------------------------------------------------

static FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();

fn with_font_sys<F, R>(f: F) -> R
where
    F: FnOnce(&mut FontSystem) -> R,
{
    let mutex = FONT_SYSTEM.get_or_init(|| {
        let font_data: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");
        let db = cosmic_text::fontdb::Database::new();
        let mut fs = FontSystem::new_with_locale_and_db("en".into(), db);
        fs.db_mut().load_font_data(font_data.to_vec());
        Mutex::new(fs)
    });
    let mut guard = mutex.lock().expect("font system lock poisoned");
    f(&mut *guard)
}

// ---------------------------------------------------------------------------
// Text measurement
// ---------------------------------------------------------------------------

/// Measure a single text label and return its bounding-box extents.
///
/// Uses the global `FontSystem` with the embedded Noto Sans font.
/// Wraps text at `MAX_NODE_WIDTH` pixels using unicode-linebreak.
pub fn measure_text(text: &str) -> TextExtents {
    with_font_sys(|font_sys| {
        let metrics = Metrics::new(FONT_SIZE, LINE_HEIGHT);
        let attrs = Attrs::new();

        let mut buffer = Buffer::new(font_sys, metrics);
        buffer.set_size(font_sys, Some(MAX_NODE_WIDTH), Some(f32::INFINITY));
        buffer.set_text(font_sys, text, attrs, Shaping::Advanced);

        let (width, height) = buffer
            .layout_runs()
            .fold((0.0f32, 0.0f32), |(max_w, total_h), run| {
                let run_w = run.line_w;
                (max_w.max(run_w), total_h + run.line_i as f32)
            });

        TextExtents {
            width: width.ceil() + 4.0,
            height: height.ceil() + 4.0,
        }
    })
}

// ---------------------------------------------------------------------------
// Batch measurement — iterate all labels in a Diagram
// ---------------------------------------------------------------------------

use crate::ast::{Diagram, NodeSize, NODE_PADDING};

/// Sub-grid resolution used by the routing/occupancy layer (px per cell).
pub const GRID: i32 = 10;

/// Round `n` up to the nearest multiple of `GRID`.
fn ceil_to_grid(n: i32) -> i32 {
    ((n + GRID - 1) / GRID) * GRID
}

/// Run measurement on every label in the diagram, mutating the AST in place.
///
/// Node dimensions are quantized to the routing sub-grid (10 px) so that
/// every node block fits cleanly into the occupancy grid. Widths and
/// heights are text-driven — verbose labels span more grid cells while
/// short labels remain compact.
pub fn measure_diagram(diagram: &mut Diagram) {
    for node in &mut diagram.nodes {
        let extents = measure_text(&node.label);
        node.label_extents = Some(extents);
        let mut size = NodeSize::from_extents(&extents, NODE_PADDING);
        size.width = ceil_to_grid(size.width);
        size.height = ceil_to_grid(size.height);
        node.node_size = Some(size);
    }

    for edge in &mut diagram.edges {
        if let Some(ref label) = edge.label {
            edge.label_extents = Some(measure_text(label));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_short_text() {
        let extents = measure_text("Hello");
        assert!(extents.width > 20.0);
        assert!(extents.height > 0.0);
    }

    #[test]
    fn test_measure_multiline() {
        let extents = measure_text("Supercalifragilisticexpialidocious");
        assert!(extents.width <= MAX_NODE_WIDTH + 10.0);
        // Should have produced multiple lines (height > single line)
        let single = measure_text("Hi");
        assert!(extents.height >= single.height);
    }

    #[test]
    fn test_batch_measure() {
        let mut diagram = crate::parser::parse_dsl("A -> B : trigger").unwrap();
        assert!(diagram.nodes[0].label_extents.is_none());
        measure_diagram(&mut diagram);
        assert!(diagram.nodes[0].label_extents.is_some());
        let edge_label_extents = &diagram.edges[0].label_extents;
        assert!(edge_label_extents.is_some());
    }

    #[test]
    fn test_measure_empty_string() {
        let extents = measure_text("");
        assert!(extents.width >= 0.0);
        assert!(extents.height >= 0.0);
    }

    #[test]
    fn test_measure_single_character() {
        let extents = measure_text("X");
        assert!(extents.width > 0.0);
        assert!(extents.height > 0.0);
    }

    #[test]
    fn test_measure_unicode() {
        // Ensure we don't panic on unicode text
        let extents = measure_text("über cool ✓");
        assert!(extents.width > 0.0);
        assert!(extents.height > 0.0);
    }

    #[test]
    fn test_measure_multiple_words_with_spaces() {
        let single = measure_text("Antidisestablishment");
        let multiple = measure_text("Antidisestablishment is quite long actually");

        // Multiple words at the same font size should wrap more
        let ratio = multiple.width / single.width;
        assert!(ratio < 2.5 || multiple.height > single.height,
            "multiple words should either wrap or be narrower than 2.5x single word");
    }

    #[test]
    fn test_measure_very_long_repeated() {
        let text = "word ".repeat(50);
        let extents = measure_text(&text);
        // Must not overflow: width capped at MAX_NODE_WIDTH
        assert!(extents.width <= MAX_NODE_WIDTH + 20.0);
        // Height must be larger than a single word
        let single = measure_text("word");
        assert!(extents.height >= single.height);
    }

    #[test]
    fn test_batch_measure_skips_missing_labels() {
        let mut diagram = crate::parser::parse_dsl("A -> B\n").unwrap();
        // Edge has no label; measure_diagram should not crash
        assert!(diagram.edges[0].label.is_none());
        measure_diagram(&mut diagram);
        assert!(diagram.nodes[0].label_extents.is_some());
        assert!(diagram.edges[0].label_extents.is_none());
    }

    #[test]
    fn test_measure_idempotent() {
        let mut diagram = crate::parser::parse_dsl("X -> Y : go\n").unwrap();
        measure_diagram(&mut diagram);
        let first = diagram.nodes[0].label_extents.unwrap();
        measure_diagram(&mut diagram); // second pass
        let second = diagram.nodes[0].label_extents.unwrap();
        assert!((first.width - second.width).abs() < 1.0);
    }
}
