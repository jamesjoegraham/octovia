# octovia 🦓

**A bespoke, DOM-free state-diagram rendering engine in pure Rust.**  
Takes a minimalist DSL or JSON input and outputs SVG in the style of a London
Underground transit map — octilinear routing, no bezier chaos, decoupled labels,
and a dark theme.

No Node.js, no headless Chrome, no OS fonts. Text measurement is done at
compile time with an embedded Noto Sans font via `cosmic-text`, and the entire
engine compiles to **WebAssembly** for in-browser use.

---

## Quick Start

```bash
# Render a DSL file to SVG
echo "title: Quick Demo
Idle -> Active : wake
Active -> Done : finish" | cargo run -- --stdout > demo.svg

# Or with a file
cargo run -- input.dsl
cargo run -- input.json -o output.svg --width 800 --height 600

# Generate 12 example SVGs + gallery page
cargo run --example generate_all
open temp/index.html
```

## DSL Syntax

```
# Comments start with #
title: My State Machine

# Sequence-first: declare the happy path
Idle -> Active : recheck
Active -> Processing : submit
Processing -> Done : complete

# Back-edges and cycles just reference existing states
Done -> Idle : reset
Processing -> Error : timeout
```

## CLI

```text
Usage: octovia [OPTIONS] [INPUT]

Arguments:
  [INPUT]  Input file (.dsl or .json). Omit to read from stdin

Options:
  -o, --output <OUTPUT>  Output SVG file (default: input path with .svg ext)
      --width <WIDTH>    Viewport width in pixels  [default: 1200]
      --height <HEIGHT>  Viewport height in pixels [default: 800]
      --json             Force JSON format (otherwise .json extension auto-detected)
  -s, --stdout           Print SVG to stdout
  -h, --help             Print help
```

## Architecture

The engine processes diagram descriptions through a **5-phase pipeline**:

```
  DSL/JSON
     │
     ▼
┌─────────────────┐
│  1. Parse       │  nom-based parser → AST (Node, Edge, Point)
│                 │  Accepts text DSL or JSON
└────────┬────────┘
         ▼
┌─────────────────┐
│  2. Measure     │  cosmic-text pre-flight layout for all labels
│                 │  Embedded Noto Sans, text-wrapping at MAX_NODE_WIDTH
└────────┬────────┘
         ▼
┌─────────────────┐
│  3. Backbone    │  Spanning tree extraction + boustrophedon grid
│     Layout      │  placement (serpentine row-filling within viewport)
└────────┬────────┘
         ▼
┌─────────────────┐
│  4. Cyclic      │  A* pathfinding with transit-map cost function:
│     Routing     │  f(n) = g(n) + h(n) + P_turn + P_cross
│                 │  Forward edges → East/West ports
│                 │  Cyclic edges  → North/South ports
└────────┬────────┘
         ▼
┌─────────────────┐
│  5. Label       │  Collision-free anchor-slot placement (8 slots)
│     Placement   │  Picks the slot with fewest edge overlaps
└────────┬────────┘
         ▼
┌─────────────────┐
│  6. SVG Output  │  Dark-theme SVG with <rect>, <path>, <text>
└─────────────────┘
```

## Modules

| Module | Purpose |
|--------|---------|
| `ast` | Graph types: `Node`, `Edge`, `Diagram`, `Point`, ports |
| `parser` | DSL text parser (nom) + JSON parser (serde) |
| `measure` | `cosmic-text` text measurement with embedded font |
| `layout` | Spanning tree extraction, boustrophedon grid placement |
| `routing` | A* pathfinding with transit-map heuristics |
| `label` | 8-slot collision-aware label placement |
| `svg_output` | SVG serialisation (dark theme, transit-map aesthetic) |
| `wasm` | `wasm-bindgen` entry points for WASM targets |
| `main` | CLI binary (clap-based) |

## Examples

```bash
cargo run --example generate_all
```

Generates 12 SVGs into `temp/` covering:
1. **Linear chain** — 10-node pipeline
2. **Simple cycle** — triangular A→B→C→A
3. **Diamond pattern** — fork/join with feedback
4. **Multi-cycle mesh** — error handling with retry paths
5. **Wide fan-out** — event bus with 8 parallel services
6. **Long labels** — stress test for text wrapping
7. **Tight viewport** — 500×300 aggressive wrapping
8. **Self-loop stress** — multiple self-referencing edges
9. **Deep nested** — 14 levels of depth
10. **Tiny dense** — 12-node graph in 400×350
11. **Crossing paths** — diagonal/boustrophedon interaction
12. **JSON input** — alternate format via parser

## Build

```bash
cargo build              # dev profile
cargo build --release    # release (fastest, for CLI use)
cargo test               # 73 unit + integration tests
```

## WASM

The engine compiles to WebAssembly for in-browser use:

```rust
// JavaScript
import { render_from_dsl } from './octovia';

const svg = render_from_dsl(
  'Idle -> Active : check\nActive -> Done : finish',
  800, 600
);
document.body.innerHTML = svg;
```

## License

MIT — do what you want with it.
