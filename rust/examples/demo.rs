/// Demo: render a state diagram to SVG.
///
/// Run with: cargo run --example demo > diagram.svg

fn main() {
    let dsl = r#"
title: Payment Workflow
Idle -> Active : recheck
Active -> Processing : submit
Processing -> Authorizing : charge
Processing -> Error : timeout
Authorizing -> Captured : success
Authorizing -> Declined : fail
Captured -> Settled : batch
Settled -> Complete : finalize
Complete -> Idle : reset
Processing -> Voided : cancel
"#;

    let svg = octovia::octo_render(dsl, None).expect("render failed");
    println!("{svg}");
}
