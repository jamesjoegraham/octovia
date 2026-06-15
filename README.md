<p align="center">
  <img src="site/public/favicon.svg" alt="octovia" width="128" height="128">
</p>

<h1 align="center">octovia</h1>

<p align="center"><strong>A pure-Rust, DOM-free state-diagram rendering engine.</strong><br>
Text in. Transit-map SVG out. No Node, no headless Chrome, no OS fonts.</p>

```
title: octovia
DSL -> Engine : parse
Engine -> SVG : render
SVG -> Anywhere : ship
Anywhere -> DSL : iterate
```

---

## Why

Every other diagramming tool in this category — Mermaid, PlantUML, D2, Graphviz — ultimately depends on **a browser, a JVM, a native binary, or a server process** to produce a final image. That dependency is a tax. It blocks CI pipelines, breaks in sandboxed runners, fights with container images, and makes "just render this diagram" a half-page of setup in any LLM tool chain.

octovia removes the tax. The engine is a single Rust crate that links into a CLI binary or a ~400 KB WebAssembly module. Text measurement is done in-process via `cosmic-text` against an embedded font. Layout, routing, and SVG serialisation are pure functions over an AST. There is **no DOM**, **no Chromium**, **no font cache to warm**, **no network at render time**.

The output is opinionated: octilinear routes on a 10 px sub-grid, dashed cycles, halo'd labels, transit-map palette. There is one correct way for a diagram to look, and the engine produces it deterministically — the same input bytes always produce the same output bytes.

---

## Features

- **Pure Rust core.** Zero unsafe, zero C dependencies, zero runtime DOM. Compiles cleanly to `x86_64`, `aarch64`, and `wasm32-unknown-unknown`.
- **Two surface formats.** A line-oriented narrative DSL for humans and structured JSON for tool calls and adapters. Both produce the same `Diagram` AST.
- **Embedded typography.** `cosmic-text` plus an embedded font give exact label metrics with no OS font lookup and no fallback chain to manage.
- **Layered topological layout.** Sugiyama L1 — iterative-DFS back-edge classification followed by a longest-path Kahn topological sort. Forward edges always go right.
- **Unified A\* routing.** Every edge — forward, cyclic, self-loop — is routed by one A\* pass over a shared sub-grid `GridOccupancy`. Nodes, prior routes, and prior labels are all impassable terrain.
- **Co-routed labels.** Label anchors are searched against the *post-routing* occupancy and reserved into it, so no label overlaps a node, an edge, or another label.
- **Deterministic by construction.** `BTreeMap` iteration, fixed neighbour order, declaration-order DFS seeding. Byte-for-byte reproducibility is verified by an integration test.
- **16 curated themes.** Transit, ink, noir, paper, mono-light, arctic, slate, nord, sage, storm, midnight, cobalt, jade, ember, copper, sepia. Pick one in a single line of DSL.
- **Two ship targets.** A `clap`-based CLI binary and a `wasm-bindgen` module with a two-function JS API (`render_from_dsl`, `render_from_json`).
- **112 tests.** 101 unit + 11 integration, all green and deterministic.

---

## Install

### Rust / CLI

```bash
# As a library
cargo add octovia

# Or build the CLI from source
git clone https://github.com/jamesjoegraham/octovia
cd octovia/rust
cargo install --path .
```

### JavaScript / WASM

```bash
npm install octovia
```

```ts
import init, { render_from_dsl } from 'octovia';

await init();
const svg = render_from_dsl(
  'theme: nord\nIdle -> Active : wake\nActive -> Done : finish',
  1200, 800,
);
```

No bundler plugins, no `headless: true`, no `puppeteer.launch()`. The module is self-contained.

---

## Quickstart

### CLI

```bash
# Render a file
octovia diagram.dsl -o diagram.svg

# Pipe from stdin
echo "title: Demo
theme: ember
Idle -> Active : wake
Active -> Done : finish" | octovia --stdout > demo.svg

# Inspect themes
octovia --list-themes
```

### Library (Rust)

```rust
use octovia::{octo_render_with_theme, ast::{resolve_theme, Viewport}};

let dsl = "Idle -> Active : check\nActive -> Done : finish";
let svg = octo_render_with_theme(
    dsl,
    Some(Viewport { width: 1200, height: 800 }),
    resolve_theme("nord"),
)?;
```

---

## DSL

A diagram is a sequence of directed transitions. Nodes are created implicitly on first mention. Declaration order is load-bearing for layout.

```text
# Top-of-file directives (all optional)
title: Order Processing
theme: nord
background: theme

# Sequence-first: declare the happy path
Idle      -> Validate   : submit
Validate  -> Charge     : ok
Charge    -> Fulfil     : authorised
Fulfil    -> Done       : shipped

# Back-edges and cycles just reference existing nodes
Validate  -> Idle       : invalid
Charge    -> Idle       : declined
Fulfil    -> Charge     : retry
```

- Lines starting with `#` are comments.
- `title:`, `theme:`, and `background:` are directives at the top.
- `Source -> Target : label` adds an edge; the label is optional.
- The full theme set is selectable by id (`transit`, `ink`, `noir`, `paper`, `mono-light`, `arctic`, `slate`, `nord`, `sage`, `storm`, `midnight`, `cobalt`, `jade`, `ember`, `copper`, `sepia`) or by alias (e.g. `dark`, `blueprint`, `printer`).

A JSON form is also accepted for programmatic callers — same schema, same pipeline.

---

## Designed for agents

State diagrams are one of the highest-leverage things an LLM can produce. octovia is built to be the rendering primitive at the end of that tool call.

- **Single-string contract.** Every entry point — `render_from_dsl`, `render_from_json`, `octo_render` — takes a string and returns a string. No file handles, no callbacks, no streams, no async.
- **No environment to provision.** The WASM build runs in a Worker, a Cloudflare/Vercel edge function, a Deno isolate, or any Node process. The CLI runs in a scratch container. There is nothing to install, nothing to mount, nothing to seccomp-allow.
- **Deterministic, byte-stable output.** Caching and diffing diagrams just works. Two identical tool calls produce two identical SVGs; a one-character DSL change produces a minimal, reviewable byte diff.
- **Tight, validated schema.** JSON input maps directly to the `Diagram` type; malformed inputs return precise errors a model can re-attempt from. Themes are a closed enum, not free-text CSS.
- **Aesthetic discipline.** There is no `style:` attribute, no inline colour overrides, no custom shapes. A model cannot produce an ugly diagram by accident — the design surface is intentionally narrow, and every theme is curated.
- **No prompt-injection vector at render time.** The DSL has no `include`, no `exec`, no template language. Input is parsed into a typed AST and never evaluated.

The result: a `render_diagram(dsl: string) -> svg: string` tool that an agent can call as freely as it calls `str.upper()`.

---

## Architecture

Five phases, each a pure transformation of the AST:

```
DSL / JSON
   │
   ▼
1. Parse              text or JSON → Diagram
2. Measure            cosmic-text → per-node sub-grid box
3. Layered Layout     back-edges → layers (Kahn) → grid placement
4. Routing + Labels   one A* per edge over shared GridOccupancy,
                      labels searched against post-route occupancy
5. SVG Output         trim to node boundaries, emit themed SVG
```

Full details, invariants, and the mathematical formulation are in [rust/ARCHITECTURE.md](rust/ARCHITECTURE.md).

---

## Repository layout

| Path | Contents |
|------|----------|
| [rust/](rust/) | The engine — `octovia` crate, CLI binary, WASM build. |
| [rust/ARCHITECTURE.md](rust/ARCHITECTURE.md) | Pipeline reference: phases, types, invariants, determinism. |
| [rust/examples/](rust/examples/) | `demo.rs`, `generate_all.rs` (12-diagram gallery). |
| [rust/tests/](rust/tests/) | Integration tests, including the byte-stability check. |
| [site/](site/) | Svelte + Vite playground that loads the WASM build. |
| [docs/](docs/) | Generated architecture documentation. |

---

## Building from source

```bash
# Native CLI
cd rust
cargo build --release
cargo test                       # 112 tests

# Gallery of 12 example SVGs
cargo run --example generate_all
open ../temp/index.html

# WASM
wasm-pack build --target web --release --out-dir pkg

# Playground
cd ../site
npm install
npm run dev
```

---

## License

MIT.
