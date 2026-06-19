//! octovia CLI — Render state-diagram DSL to SVG.
//!
//! Usage:
//!   octovia input.dsl                          → writes input.svg
//!   octovia input.dsl -o output.svg
//!   octovia input.dsl --width 800 --height 600
//!   octovia input.dsl --theme ocean
//!   octovia input.dsl --theme ocean
//!   octovia input.json --json                  # JSON-format input
//!   octovia input.dsl --png                     # render to PNG (1x)
//!   octovia input.dsl --png --scale 2           # Retina PNG
//!   octovia input.dsl --jpeg --quality 90       # JPEG at quality 90
//!   cat input.dsl | octovia                    # pipe mode

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use clap::Parser;

/// The CLI argument structure.
#[derive(Parser)]
#[command(
    name = "octovia",
    about = "Render state-diagram DSL/JSON files to SVG (transit-map aesthetic)",
    version,
)]
struct Cli {
    /// Input file (.dsl or .json). Omit to read from stdin.
    input: Option<PathBuf>,

    /// Output SVG file (default: input path with .svg extension, or stdout).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Viewport width in pixels (default: 1200).
    #[arg(long, default_value_t = 1200)]
    width: u32,

    /// Viewport height in pixels (default: 800).
    #[arg(long, default_value_t = 800)]
    height: u32,

    /// Force JSON format regardless of file extension.
    #[arg(long)]
    json: bool,

    /// Print the SVG to stdout instead of writing to a file.
    #[arg(short, long)]
    stdout: bool,

    /// Colour theme (default: transit). Run `octovia --list-themes` to see all.
    #[arg(long, default_value = "transit")]
    theme: String,

    /// SVG canvas background (`transparent`, `theme`, or any CSS colour).
    /// Defaults to `transparent` when omitted; overrides any `background`
    /// directive in the input file.
    #[arg(long)]
    background: Option<String>,

    /// List all available themes and exit.
    #[arg(long)]
    list_themes: bool,

    /// Render to PNG instead of SVG. (default scale: 1x)
    #[arg(long)]
    png: bool,

    /// Render to JPEG instead of SVG.
    #[arg(long)]
    jpeg: bool,

    /// Output scale factor for PNG/JPEG (0.1 – 10.0, default: 1.0).
    #[arg(long, default_value_t = 1.0)]
    scale: f32,

    /// JPEG quality (1–100, default: 85). Only used with --jpeg.
    #[arg(long, default_value_t = 85)]
    quality: u8,
}

fn main() {
    let cli = Cli::parse();

    // Handle --list-themes
    if cli.list_themes {
        let themes = octovia::ast::list_themes();
        println!("Available themes:");
        for (id, name) in &themes {
            println!("  {id:20} {name}");
        }
        println!("\n{} themes total.", themes.len());
        return;
    }

    // --- Read input ---
    let (input_text, input_name): (String, Option<String>) = if let Some(ref path) = cli.input {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|e| {
                eprintln!("error: cannot read '{}': {e}", path.display());
                std::process::exit(1);
            });
        (text, Some(path.to_string_lossy().to_string()))
    } else {
        // stdin
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .unwrap_or_else(|e| {
                eprintln!("error: reading stdin: {e}");
                std::process::exit(1);
            });
        if buf.trim().is_empty() {
            eprintln!("error: empty input from stdin");
            std::process::exit(1);
        }
        (buf, None)
    };

    // --- Detect format ---
    let is_json = cli.json
        || input_name
            .as_deref()
            .map(|n| n.ends_with(".json"))
            .unwrap_or(false);

    // --- Parse ---
    let mut diagram = if is_json {
        octovia::parser::parse_json(&input_text).unwrap_or_else(|e| {
            eprintln!("error: JSON parse failed: {e}");
            std::process::exit(1);
        })
    } else {
        octovia::parser::parse_dsl(&input_text).unwrap_or_else(|e| {
            eprintln!("error: DSL parse failed: {e}");
            std::process::exit(1);
        })
    };

    // --- Set viewport ---
    diagram.viewport = octovia::ast::Viewport {
        width: cli.width,
        height: cli.height,
    };

    // --- Set theme ---
    diagram.theme = match octovia::ast::resolve_theme(&cli.theme) {
        Some(t) => t,
        None => {
            eprintln!("error: unknown theme '{}'. Use --list-themes to see all options.", cli.theme);
            std::process::exit(1);
        }
    };

    // --- Set background (CLI overrides any directive in the input) ---
    if let Some(bg) = cli.background {
        diagram.background = octovia::ast::Background::parse_value(&bg);
    }

    // --- Run pipeline ---
    octovia::measure::measure_diagram(&mut diagram);
    octovia::layout::layout_backbone(&mut diagram);
    octovia::routing::route_all_edges(&mut diagram);

    // --- Handle PNG/JPEG output ---
    if cli.png || cli.jpeg {
        let svg = octovia::svg_output::render_svg(&diagram);

        let ext = if cli.jpeg { "jpg" } else { "png" };

        let output_path: PathBuf = cli.output.unwrap_or_else(|| {
            if let Some(ref name) = input_name {
                let p = PathBuf::from(name);
                let stem = p.file_stem().unwrap_or(p.as_os_str());
                let mut out = p.with_file_name(stem);
                out.set_extension(ext);
                out
            } else {
                eprintln!("error: --{} requires an input file path or -o to name the output", ext);
                std::process::exit(1);
            }
        });

        if cli.jpeg {
            octovia::png_export::render_svg_to_jpeg_bytes(&svg, cli.scale, cli.quality)
                .and_then(|bytes| {
                    fs::write(&output_path, &bytes)
                        .map_err(|e| format!("cannot write JPEG: {e}"))
                })
                .unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                });
            eprintln!("wrote {} (JPEG, scale={}, quality={})", output_path.display(), cli.scale, cli.quality);
        } else {
            octovia::png_export::render_svg_to_png_bytes(&svg, cli.scale)
                .and_then(|bytes| {
                    fs::write(&output_path, &bytes)
                        .map_err(|e| format!("cannot write PNG: {e}"))
                })
                .unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                });
            eprintln!("wrote {} (PNG, scale={})", output_path.display(), cli.scale);
        }
        return;
    }

    // --- SVG output ---
    let svg = octovia::svg_output::render_svg(&diagram);

    if cli.stdout {
        print!("{svg}");
        return;
    }

    let output_path: PathBuf = cli.output.unwrap_or_else(|| {
        if let Some(ref name) = input_name {
            let p = PathBuf::from(name);
            let stem = p.file_stem().unwrap_or(p.as_os_str());
            let mut out = p.with_file_name(stem);
            out.set_extension("svg");
            out
        } else {
            eprintln!("note: no input file path or -o given; writing to stdout");
            print!("{svg}");
            std::process::exit(0);
        }
    });

    fs::write(&output_path, &svg).unwrap_or_else(|e| {
        eprintln!("error: writing '{}': {e}", output_path.display());
        std::process::exit(1);
    });

    eprintln!("wrote {} bytes to {}", svg.len(), output_path.display());
}
