/// Master runner: generates ALL example SVGs at once into ~/code/octovia/temp/
///
/// Usage: cargo run --example generate_all
///
/// Each example renders to:  temp/NN_description.svg
/// A summary index file is also written: temp/index.html  (open in browser)

use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("temp");
    fs::create_dir_all(&out_dir).expect("create temp dir");

    let examples: Vec<(&str, &str, fn() -> String)> = vec![
        ("01", "linear_chain", linear_chain),
        ("02", "simple_cycle", simple_cycle),
        ("03", "diamond_pattern", diamond_pattern),
        ("04", "multi_cycle_mesh", multi_cycle_mesh),
        ("05", "wide_fanout", wide_fanout),
        ("06", "long_labels", long_labels),
        ("07", "tight_viewport", tight_viewport),
        ("08", "self_loop_stress", self_loop_stress),
        ("09", "deep_nested", deep_nested),
        ("10", "tiny_dense", tiny_dense),
        ("11", "crossing_paths", crossing_paths),
        ("12", "json_input", json_input),
    ];

    let mut summary = String::from(
        "<html><head><title>octovia — Example Gallery</title>
        <style>
          body { font-family: system-ui, sans-serif; background: #111; color: #ddd; margin: 2rem; }
          h1 { color: #4A90D9; }
          h2 { margin-top: 2rem; }
          svg { max-width: 100%; border: 1px solid #333; border-radius: 8px; margin: 0.5rem 0; }
          .stats { color: #888; font-size: 0.9rem; }
          hr { border: none; border-top: 1px solid #333; margin: 1rem 0; }
        </style></head><body>
        <h1>🦓 octovia — SVG Gallery</h1>
        <p class=\"stats\">Generated from 12 examples</p>\n",
    );

    let mut ok = 0u32;

    for (num, name, render_fn) in &examples {
        let filename = format!("{}_{}.svg", num, name);
        let path = out_dir.join(&filename);

        print!("  {filename} ... ");
        let _ = std::io::stdout().flush();

        let svg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(render_fn));
        match svg {
            Ok(svg_str) => {
                let size_kb = svg_str.len() as f64 / 1024.0;
                fs::write(&path, &svg_str).expect("write svg");
                println!("OK  ({size_kb:.1} KB)");

                let node_count = svg_str.matches("node-").count();
                let edge_count = svg_str.matches("edge-").count();
                let display_name = name.replace('_', " ");

                summary.push_str(&format!(
                    "<h2 id=\"{num}\">{num}. {display_name}</h2>\n\
                     <p class=\"stats\">{svg_str} bytes — ~{size_kb:.1} KB — {node_count} nodes — {edge_count} edges</p>\n\
                     <object data=\"{filename}\" type=\"image/svg+xml\" width=\"100%\" height=\"400\"></object>\n<hr>\n",
                ));
                ok += 1;
            }
            Err(e) => {
                let msg = match e.downcast_ref::<String>() {
                    Some(s) => s.clone(),
                    None => match e.downcast_ref::<&str>() {
                        Some(s) => s.to_string(),
                        None => "unknown panic".into(),
                    },
                };
                fs::write(out_dir.join(&filename), &format!("<!-- PANICKED: {msg} -->"))
                    .expect("write error svg");
                println!("PANICKED: {msg}");

                summary.push_str(&format!(
                    "<h2 style=\"color: #E67E22;\">{num}. {name} — ERROR</h2><pre>{msg}</pre><hr>\n",
                ));
            }
        }
    }

    summary.push_str("</body></html>");
    fs::write(out_dir.join("index.html"), &summary).expect("write index");
    println!("\nDone. {ok}/{} examples generated in {}/", examples.len(), out_dir.display());
    println!("Open temp/index.html in your browser to view the gallery.");
}

// ---------------------------------------------------------------------------
// Example 01: Linear Chain
// ---------------------------------------------------------------------------
fn linear_chain() -> String {
    let dsl = "\
theme: transit
title: Linear Pipeline
State_1 -> State_2 : init
State_2 -> State_3 : load
State_3 -> State_4 : process
State_4 -> State_5 : validate
State_5 -> State_6 : transform
State_6 -> State_7 : enrich
State_7 -> State_8 : persist
State_8 -> State_9 : notify
State_9 -> State_10 : complete
";
    octovia::octo_render(dsl, None).expect("linear_chain")
}

// ---------------------------------------------------------------------------
// Example 02: Simple Cycle
// ---------------------------------------------------------------------------
fn simple_cycle() -> String {
    let dsl = "\
theme: ember
title: Triangular Cycle
A -> B : step_1
B -> C : step_2
C -> A : back
";
    octovia::octo_render(dsl, None).expect("simple_cycle")
}

// ---------------------------------------------------------------------------
// Example 03: Diamond Pattern
// ---------------------------------------------------------------------------
fn diamond_pattern() -> String {
    let dsl = "\
theme: forest
title: Diamond with Feedback
A -> B : fork_left
A -> C : fork_right
B -> D : join_left
C -> D : join_right
D -> A : feedback
D -> Home : settle
Home -> End : finish
";
    octovia::octo_render(dsl, None).expect("diamond_pattern")
}

// ---------------------------------------------------------------------------
// Example 04: Multi-Cycle Mesh
// ---------------------------------------------------------------------------
fn multi_cycle_mesh() -> String {
    let dsl = "\
title: Multi-Cycle Mesh
Start -> Fetch : begin
Fetch -> Parse : raw_ok
Fetch -> Error : timeout
Parse -> Validate : parsed
Parse -> Error : bad_format
Validate -> Transform : valid
Validate -> Error : invalid
Transform -> Store : transformed
Store -> Notify : saved
Notify -> Start : reset
Error -> Retry : recover
Retry -> Fetch : retry
Retry -> Abort : give_up
";
    octovia::octo_render(dsl, None).expect("multi_cycle_mesh")
}

// ---------------------------------------------------------------------------
// Example 05: Wide Fan-Out
// ---------------------------------------------------------------------------
fn wide_fanout() -> String {
    let dsl = "\
theme: monochrome
title: Event Fan-Out
EventBus -> Service_A : route_a
EventBus -> Service_B : route_b
EventBus -> Service_C : route_c
EventBus -> Service_D : route_d
EventBus -> Service_E : route_e
EventBus -> Service_F : route_f
EventBus -> Service_G : route_g
EventBus -> Service_H : route_h
Service_A -> Logger : log_a
Service_B -> Logger : log_b
Service_C -> Logger : log_c
Service_D -> Logger : log_d
Service_E -> Logger : log_e
Service_F -> Logger : log_f
Service_G -> Logger : log_h
Service_H -> Logger : log_h
Logger -> Sink : batch_write
";
    octovia::octo_render(dsl, None).expect("wide_fanout")
}

// ---------------------------------------------------------------------------
// Example 06: Long Labels
// ---------------------------------------------------------------------------
fn long_labels() -> String {
    let dsl = "\
title: Long Labels Stress Test
InitializationAndConfiguration -> DataAcquisitionAndPreprocessing : start_long_running_process
DataAcquisitionAndPreprocessing -> FeatureEngineeringAndExtraction : data_ready_and_validated
FeatureEngineeringAndExtraction -> ModelTrainingAndValidation : features_extracted_successfully
ModelTrainingAndValidation -> EvaluationAndDeployment : model_trained_and_verified
EvaluationAndDeployment -> MonitoringAndFeedbackLoop : deployed_to_production
MonitoringAndFeedbackLoop -> DataAcquisitionAndPreprocessing : retrigger_with_new_data
ModelTrainingAndValidation -> ErrorHandlingAndLogging : training_failed_exception
";
    octovia::octo_render(dsl, None).expect("long_labels")
}

// ---------------------------------------------------------------------------
// Example 07: Tight Viewport
// ---------------------------------------------------------------------------
fn tight_viewport() -> String {
    let dsl = "\
theme: light
title: Tight Viewport (500x300)
Step_01 -> Step_02 : pass
Step_02 -> Step_03 : pass
Step_03 -> Step_04 : pass
Step_04 -> Step_05 : pass
Step_05 -> Step_06 : pass
Step_06 -> Step_07 : pass
Step_07 -> Step_08 : pass
Step_08 -> Step_09 : pass
Step_09 -> Step_10 : pass
Step_10 -> Step_11 : pass
Step_11 -> Step_12 : pass
Step_12 -> Step_01 : loop_back
";
    let vp = octovia::ast::Viewport {
        width: 500,
        height: 300,
    };
    octovia::octo_render(dsl, Some(vp)).expect("tight_viewport")
}

// ---------------------------------------------------------------------------
// Example 08: Self-Loop Stress
// ---------------------------------------------------------------------------
fn self_loop_stress() -> String {
    let dsl = "\
title: Self-Loop Stress
Idle -> Active : start
Active -> Active : refresh
Active -> Active : poll
Active -> Active : heartbeat
Active -> Paused : suspend
Paused -> Active : resume
Paused -> Paused : wait
Active -> Done : finish
Done -> Idle : reset
Idle -> Idle : noop
";
    octovia::octo_render(dsl, None).expect("self_loop_stress")
}

// ---------------------------------------------------------------------------
// Example 09: Deep Nested States
// ---------------------------------------------------------------------------
fn deep_nested() -> String {
    let dsl = "\
title: Deep Nested States
Root -> Level1_A : enter
Level1_A -> Level2_A : deeper
Level2_A -> Level3_A : even_deeper
Level3_A -> Level4_A : deepest
Level4_A -> Level5_A : final_depth
Level5_A -> Level5_B : branch
Level5_B -> Level4_B : ascend
Level4_B -> Level3_B : ascend
Level3_B -> Level2_B : ascend
Level2_B -> Level1_B : ascend
Level1_B -> Root : return
Root -> Level1_C : fork_other
Level1_C -> Level2_C : deeper
Level2_C -> Root : early_return
";
    octovia::octo_render(dsl, None).expect("deep_nested")
}

// ---------------------------------------------------------------------------
// Example 10: Tiny Dense Graph (400x350)
// ---------------------------------------------------------------------------
fn tiny_dense() -> String {
    let dsl = "\
title: Tiny Dense Graph
A -> B : x
A -> C : y
B -> D : z
C -> D : w
D -> E : a
E -> F : b
F -> G : c
G -> H : d
H -> A : back
B -> E : alt
C -> F : alt2
E -> A : reset
";
    let vp = octovia::ast::Viewport {
        width: 400,
        height: 350,
    };
    octovia::octo_render(dsl, Some(vp)).expect("tiny_dense")
}

// ---------------------------------------------------------------------------
// Example 11: Crossing Paths
// ---------------------------------------------------------------------------
fn crossing_paths() -> String {
    let dsl = "\
title: Crossing Paths
Left_Top -> Mid_Top : across
Left_Top -> Mid_Bot : diagonal
Left_Bot -> Mid_Bot : straight
Left_Bot -> Mid_Top : diagonal
Mid_Top -> Right_Top : finish_top
Mid_Bot -> Right_Bot : finish_bot
Right_Top -> Left_Bot : feedback
Right_Bot -> Left_Top : feedback2
";
    octovia::octo_render(dsl, None).expect("crossing_paths")
}

// ---------------------------------------------------------------------------
// Example 12: JSON Input
// ---------------------------------------------------------------------------
fn json_input() -> String {
    let json = r#"{
        "title": "JSON-Powered Diagram",
        "states": ["Draft", "Review", "Approved", "Published", "Archived"],
        "transitions": [
            {"from": "Draft", "to": "Review", "label": "submit_for_review"},
            {"from": "Review", "to": "Draft", "label": "request_changes"},
            {"from": "Review", "to": "Approved", "label": "approve"},
            {"from": "Approved", "to": "Published", "label": "publish"},
            {"from": "Published", "to": "Archived", "label": "archive"},
            {"from": "Archived", "to": "Draft", "label": "revive"},
            {"from": "Review", "to": "Rejected", "label": "reject"}
        ],
        "viewport": {"width": 1000, "height": 600}
    }"#;
    let mut diagram = octovia::parser::parse_json(json).expect("json_input");
    // Run full pipeline
    octovia::measure::measure_diagram(&mut diagram);
    octovia::layout::layout_backbone(&mut diagram);
    octovia::routing::route_all_edges(&mut diagram);
    octovia::svg_output::render_svg(&diagram)
}
