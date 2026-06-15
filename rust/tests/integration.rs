//! End-to-end integration tests for the full octovia pipeline.
//!
//! These exercise the public API only (`octo_render`, `octo_render_with_theme`)
//! so any accidental loss of `pub` visibility on a pipeline stage shows up here.

use octovia::ast::{ThemeColors, Viewport};
use octovia::{octo_render, octo_render_with_theme};

#[test]
fn test_end_to_end_linear() {
    let dsl = "Idle -> Active : recheck\nActive -> Processing : submit\nProcessing -> Done : complete";
    let svg = octo_render(dsl, None).unwrap();
    assert!(svg.contains("node-Idle"));
    assert!(svg.contains("node-Done"));
    assert!(svg.contains("edge-0"));
    assert!(svg.contains("edge-2"));
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("M "));
}

#[test]
fn test_end_to_end_with_cycle() {
    let dsl = "A -> B : first\nB -> C : second\nC -> A : loop";
    let svg = octo_render(dsl, None).unwrap();
    assert!(svg.contains("node-A"));
    assert!(svg.contains("node-B"));
    assert!(svg.contains("node-C"));
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    // The cyclic edge C->A should produce a dashed path
    assert!(svg.contains("stroke-dasharray"));
}

#[test]
fn test_octo_render_empty_dsl() {
    let svg = octo_render("", None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn test_octo_render_invalid_dsl() {
    let result = octo_render("this is garbage", None);
    assert!(result.is_err());
}

#[test]
fn test_octo_render_custom_viewport() {
    let vp = Viewport { width: 400, height: 300 };
    let svg = octo_render("A -> B\n", Some(vp)).unwrap();
    assert!(svg.starts_with("<svg"));
    // With a small viewport, nodes should still be placed
    assert!(svg.contains("node-A"));
    assert!(svg.contains("node-B"));
}

#[test]
fn test_octo_render_branching_fanout() {
    let dsl = "A -> B\nA -> C\nA -> D\nB -> E\nC -> E\nD -> E\n";
    let svg = octo_render(dsl, None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("node-A"));
    assert!(svg.contains("node-B"));
    assert!(svg.contains("node-C"));
    assert!(svg.contains("node-D"));
    assert!(svg.contains("node-E"));
    // All 6 edges should have paths
    assert!(svg.contains("edge-0"));
    assert!(svg.contains("edge-5"));
}

#[test]
fn test_octo_render_self_loop() {
    let dsl = "A -> A : loop\n";
    let svg = octo_render(dsl, None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("node-A"));
}

#[test]
fn test_octo_render_large_graph() {
    // Create a large linear chain
    let mut dsl = String::new();
    for i in 0..20 {
        dsl.push_str(&format!("S{i} -> S{}\n", i + 1));
    }
    let svg = octo_render(&dsl, None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("node-S0"));
    assert!(svg.contains("node-S20"));
    // All 20 edges should be there
    for i in 0..20 {
        assert!(svg.contains(&format!("edge-{i}")), "missing edge-{i}");
    }
}

#[test]
fn test_octo_render_with_theme() {
    // Use a labelled edge so each theme's `bg` colour appears in the
    // edge-label halo stroke (the SVG canvas itself defaults to
    // `transparent` and is independent of the theme).
    let dsl = "A -> B : go\n";
    // Default theme (transit)
    let svg_default = octo_render(dsl, None).unwrap();
    assert!(svg_default.contains("#1A1A2E"));

    // Ember theme
    let ember = ThemeColors::from_str("ember").unwrap();
    let svg_ember = octo_render_with_theme(dsl, None, Some(ember)).unwrap();
    assert!(svg_ember.contains("#1C1410"));
    assert!(svg_ember.contains("#D4803A"));

    // Sage theme
    let sage = ThemeColors::from_str("sage").unwrap();
    let svg_sage = octo_render_with_theme(dsl, None, Some(sage)).unwrap();
    assert!(svg_sage.contains("#121412"));
    assert!(svg_sage.contains("#789070"));
}

#[test]
fn test_octo_render_jetbrains_mono() {
    let svg = octo_render("A -> B\n", None).unwrap();
    assert!(svg.contains("JetBrains Mono"));
}

/// Determinism contract: identical input must produce byte-identical SVG
/// output across runs. Exercises a graph with branching, multiple sources,
/// disconnected components, and a back-edge — every code path that has
/// historically depended on `HashMap` iteration order.
#[test]
fn test_octo_render_is_deterministic() {
    let dsl = "\
title: Determinism Probe
theme: ember

# Main happy path with a fan-out
Idle -> Active : recheck
Active -> Processing : submit
Active -> Cancelled : abort
Processing -> Done : complete
Processing -> Error : fail

# Back-edge (cyclic)
Done -> Idle : reset

# A second source with its own subgraph (disconnected from Idle)
Bootstrap -> Loading : start
Loading -> Bootstrap : retry
";

    let baseline = octo_render(dsl, None).unwrap();
    for i in 1..50 {
        let svg = octo_render(dsl, None).unwrap();
        assert_eq!(
            svg, baseline,
            "octo_render produced different SVG on run #{i} — determinism contract violated"
        );
    }
}
