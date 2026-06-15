# Octovia Render Pipeline — Architecture Document

> **Principle:** Easy to read, easy to write. Every graph is beautiful because every node connects through exactly 8 ports.

---

## Pipeline Overview

The engine processes a DSL description through **6 phases**, each one transforming the AST toward the final SVG:

```
  DSL/JSON
     │
     ▼
┌─────────────────┐
│  1. Parse       │  nom-based line parser → AST (Node, Edge, Point)
│                 │  Accepts text DSL or JSON
└────────┬────────┘
         ▼
┌─────────────────┐
│  2. Measure     │  cosmic-text pre-flight layout for all labels
│                 │  Embedded JetBrains Mono, text-wrapping at MAX_NODE_WIDTH
└────────┬────────┘
         ▼
┌─────────────────┐
│  3. Backbone    │  BFS spanning tree + boustrophedon grid
│     Layout      │  placement → node.position, is_cyclic tagging
└────────┬────────┘
         ▼
┌─────────────────┐
│  4. Edge        │  Forward: East→West straight lines
│     Routing     │  Cyclic:  A* with octilinear transit-map cost
└────────┬────────┘
         ▼
┌─────────────────┐
│  5. Label       │  8-slot anchor collision-free placement
│     Placement   │  Edge labels at route midpoint, node labels centred
└────────┬────────┘
         ▼
┌─────────────────┐
│  6. SVG Output  │  Auto-bounding-box viewBox, themed colours
└─────────────────┘
```

## Octavia Diagrams of the Pipeline Itself

The following Octovia DSL renders the pipeline architecture diagram above. Write it in the playground:

```
title: Octovia Render Pipeline
theme: transit

Parse -> Measure : text_extents
Measure -> Backbone_Layout : node_size
Backbone_Layout -> Edge_Routing : positions + is_cyclic
Edge_Routing -> Label_Placement : routes
Label_Placement -> SVG_Output : anchors
```

## Phase 1 — Parse (`src/parser.rs`)

### DSL Grammar (text format)

```
# Comment
title: My Machine
theme: ember

Idle -> Active : recheck
Active -> Processing : submit
Processing -> Done : complete
Done -> Idle : reset
```

**Parsing rules:**
- Lines starting with `#` are comments (skipped)
- `title: <string>` sets the diagram title
- `theme: <name>` sets the colour theme
- `Source -> Target : label` adds a directed edge
- Nodes are created implicitly the first time their ID is seen
- Also accepts **JSON input** via `parse_json()` for structured data inputs

### AST Types (`src/ast.rs`)

```rust
struct Diagram {
    nodes: Vec<Node>,       // ordered by insertion
    edges: Vec<Edge>,       // ordered by declaration
    title: Option<String>,
    viewport: Viewport,     // { width: u32, height: u32 }
    theme: Theme,           // Transit | Ember | Forest | Light | Monochrome
}

struct Node {
    id: String,
    label: String,
    label_extents: Option<TextExtents>,   // ← Phase 2
    node_size: Option<NodeSize>,           // ← Phase 2
    position: Option<Point>,               // ← Phase 3
    spanning_index: Option<usize>,         // ← Phase 3
}

struct Edge {
    from: String,
    to: String,
    label: Option<String>,
    label_extents: Option<TextExtents>,   // ← Phase 2
    is_cyclic: bool,                       // ← Phase 3
    route: Vec<Point>,                     // ← Phase 4
}
```

**Geometry primitives:**
```rust
struct Point { x: i32, y: i32 }
struct NodeSize { width: i32, height: i32 }

const MIN_NODE_SIDE: i32 = 60;
const NODE_PADDING: i32 = 24;
```

## Phase 2 — Measure (`src/measure.rs`)

Uses **cosmic-text** to measure every label. A global `FontSystem` is initialised once via `OnceLock<Mutex<FontSystem>>` with JetBrains Mono embedded via `include_bytes!`.

```
                 ┌─────────────────┐
                 │  FontSystem      │  OnceLock<Mutex<FontSystem>>
                 │  (global, lazy)  │  JetBrains Mono @ 14px
                 └────────┬────────┘
                          │
                    ┌─────┴──────────┬──────────────┐
                    ▼                ▼              ▼
                 ┌────────┐      ┌────────┐     ┌─────────┐
                 │ Node 1 │      │ Node 2 │ ... │ Edge N  │
                 │ text   │      │ text   │     │ label   │
                 │ → exts │      │ → exts │     │ → exts  │
                 └────────┘      └────────┘     └─────────┘
```

**Key constants:**

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_NODE_WIDTH` | `180.0` | Text wraps at this width |
| `FONT_SIZE` | `14.0` px | Body text size |
| `LINE_HEIGHT` | `1.35` | Line spacing ratio |
| `NODE_PADDING` | `24` | Padding inside node rect |
| `MIN_NODE_SIDE` | `60` | Smallest node dimension |

**Node size formula:**
```
width  = max(extents.width + NODE_PADDING, MIN_NODE_SIDE)
height = max(extents.height + NODE_PADDING, MIN_NODE_SIDE)
width  = max(width, height)  // Ensure proportional, minimum width
```

**Output:** each node gets `label_extents` + `node_size`; each edge gets `label_extents`.

---

## Phase 3 — Backbone Layout (`src/layout.rs`)

This is where the **spanning tree** (happy path) is extracted and nodes are placed on the grid using the boustrophedon (ox-ploughing) serpentine pattern.

### 3a: Spanning Tree Extraction

Octovia diagram of a spanning tree with a cyclic back-edge:

```
theme: transit
title: Spanning Tree + Back-Edge

Spanning_Root -> A : bfs
A -> B : bfs
B -> C : bfs
C -> D : bfs_back
```

**Algorithm:**
1. Build forward and reverse adjacency maps from all edges
2. Find the **root** — the first node with zero incoming edges (if none, use first node in list)
3. BFS from root, visiting forward neighbours → produces the spanning order
4. Disconnected stragglers (nodes with no edges or isolated subgraphs) are appended to the end

**Cyclic edge detection** (tagged in `is_cyclic`):
- `spanning_index(to) >= spanning_index(from)` → back-edge
- Target not in spanning order → cyclic

### 3b: Boustrophedon Grid Placement

The spanning tree is laid out in a serpentine row-filling pattern:

```
Row 0 (L→R):  A───B───C───D───E
                            │
Row 1 (R→L):  J───I───H───G───F
              │
Row 2 (L→R):  K───L───M
```

**Key constants:**
| Constant | Value | Purpose |
|----------|-------|---------|
| `GRID_SPACING_X` | `200` | Horizontal centre-to-centre |
| `GRID_SPACING_Y` | `150` | Vertical row-to-row |
| `NODE_RADIUS` | `30` | Fallback half-size |
| `MIN_VIEWPORT` | `400` | Minimum viewport edge |

**Per-row capacity:**
```
usable_width = viewport_width - 4 × node_half_w
per_row = max(1, usable_width / GRID_SPACING_X)
```

Where `node_half_w = (MAX_NODE_WIDTH + 24) / 2 ≈ 102`.

**Boustrophedon rule:**
- Even rows: increasing X (left→right)
- Odd rows: decreasing X (right→left)
- Y increases by `GRID_SPACING_Y` per row

---

## Phase 4 — Edge Routing (`src/routing.rs`)

This is where the **8-port-per-node constraint** creates beautiful graphs.

### The 8-Port System

Every node has exactly **8 connection points**:

```
              TopLeft    Top    TopRight
                   ●──────●──────●
                   │              │
           Left ●──┼── [ NODE ] ──┼──● Right
                   │              │
                   ●──────●──────●
             BottomLeft  Bottom  BottomRight
```

> The `AnchorSlot` enum and `direction()`/`is_turn()` helpers already support all 8 octilinear directions. The current port-to-edge assignment uses only 4 cardinal ports (see below), but diagonals are wired in and ready for the router to use when cardinal ports are blocked.

### Current Port Assignment

| Edge type | Source port | Target port | Path type |
|-----------|-------------|-------------|-----------|
| **Forward** | **East** (→) | **West** (←) | **Straight horizontal line** |
| **Cyclic** | **North** (↑) | **South** (↓) | **A*-routed octilinear path** |

### Two-Pass Routing Algorithm

**Pass 1 — Forward edges** (East→West straight lines):

Every forward edge exits the source node's **East port** and enters the target node's **West port** via a single straight horizontal line. The Y-coordinate is fixed at the source node's row Y. This is the core beauty of the boustrophedon layout: nodes on the same row always connect with perfectly straight, level edges.

Forward edge route example (A→B in row 0):
```
  ┌──────┐  ●────────────────────────●  ┌──────┐
  │  A   │  East────────West─────────→  │  B   │
  └──────┘                              └──────┘
```

**Pass 2 — Cyclic edges** (A* octilinear pathfinding):

Cyclic edges use the **North** (source) → **South** (target) port pair. The A* pathfinder navigates the 10px grid, avoiding node blocks and previously-routed edge cells.

Octovia diagram of a cyclic return edge:

```
theme: ember
title: Cyclic Routing (A*)
A -> B : forward
B -> C : forward
C -> A : cyclic_back
```

### A* Cost Function

```
f(n) = g(n) + h(n) + T(n) + C(n)

g(n) = movement cost from start
h(n) = octile distance to goal
T(n) = turn_penalty * number_of_direction_changes (≈200 each)
C(n) = cross_penalty * number_of_occupied_cells_crossed (≈500 each)
```

**Movement costs:**
| Direction | Cost |
|-----------|------|
| Axis-aligned (E, W, N, S) | 10 |
| Diagonal (NE, NW, SE, SW) | 14 |

**Octile heuristic** (admissible for octilinear movement):
```rust
fn octile_distance(a, b) {
    let d = min(|dx|, |dy|);
    (|dx| + |dy| - d) * 10 + d * 14
}
```

**Why A* only for cyclic edges?** Forward edges are deterministic (always straight East→West), so pathfinding is unnecessary for the happy path. Cyclic edges are the ones that need to navigate around existing content.

### Grid Occupancy System

```
  Cell grid (10px resolution):
  ┌────┬────┬────┬────┬────┬────┐
  │    │    │    │    │    │    │
  ├────┼────┼────┼────┼────┼────┤
  │    │ ██ │ ██ │ ██ │    │    │  ██ = node occupied
  ├────┼────┼────┼────┼────┼────┤
  │    │ ██ │ ██ │ ██ │ ━━ │    │  ━━ = edge occupied
  ├────┼────┼────┼────┼────┼────┤
  │    │    │    │ ━━ │    │    │
  ├────┼────┼────┼────┼────┼────┤
  │    │    │    │    │    │    │
  └────┴────┴────┴────┴────┴────┘
```

- **Node cells:** 3-cell-radius square around each node centre (~60px radius at 10px/cell)
- **Edge cells:** All cells along a routed path
- **A* constraint:** Only free cells (neither node nor edge occupied) are traversable

---

## Phase 5 — Label Placement (`src/svg_output.rs`)

### Current Implementation

- **Node labels:** Always centred inside the node rectangle
- **Edge labels:** Placed at the midpoint of the edge route, offset 12px above
- **Opacity:** Edge labels get `opacity="0.7"` for visual hierarchy

### The 8 Anchor Slots (Future Work)

The `AnchorSlot` enum defines 8 positions around each node:

| Slot | Position | Use case |
|------|----------|----------|
| `Right` | 3 o'clock | Forward edge destination |
| `Left` | 9 o'clock | Forward edge source |
| `Top` | 12 o'clock | Cyclic edge source |
| `Bottom` | 6 o'clock | Cyclic edge destination |
| `TopLeft` | 10:30 | Diagonal cyclic source |
| `TopRight` | 1:30 | Diagonal cyclic source |
| `BottomLeft` | 7:30 | Diagonal cyclic destination |
| `BottomRight` | 4:30 | Diagonal cyclic destination |

An anchor-based label placement system would:
1. Compute candidate positions for each of the 8 slots around the node
2. Score each slot by how many edge paths intersect it (fewer = better)
3. Assign the label to the slot with the fewest overlaps

---

## Phase 6 — SVG Output (`src/svg_output.rs`)

### Z-Order (bottom to top)

```
1. Background fill (viewport rectangle)
2. Title text (if any)
3. Edge paths (stroke-only, no fill)
4. Edge labels (text with opacity)
5. Node rectangles (filled + stroked)
6. Node labels (centred text)
```

### Auto-Bounding Box

`compute_bounds()` scans every node position and edge route point to create a tight `viewBox`:

```rust
fn compute_bounds(diagram) -> (min_x, min_y, max_x, max_y) {
    for each node → expand by node half-size + 20px margin
    for each edge → expand by 10px per route point
    return bounds + 40px padding in viewBox
}
```

This means the SVG always fits its content exactly — no wasted space, no clipping.

### Theme System

Five colour themes control every visual property:

| Part | Transit 🚇 | Ember 🔥 | Forest 🌲 | Light ☀️ | Mono ⚫ |
|------|-----------|---------|---------|---------|--------|
| Background | `#1A1A2E` | `#1C1410` | `#0F1A14` | `#F5F5F0` | `#111112` |
| Node fill | `#16213E` | `#2A1D16` | `#16251D` | `#FFFFFF` | `#1C1C1E` |
| Node stroke | `#4A90D9` | `#D4803A` | `#3D9B6B` | `#4A6FA5` | `#888899` |
| Forward edge | `#4A90D9` | `#D4803A` | `#3D9B6B` | `#4A6FA5` | `#888899` |
| Cyclic edge | `#E67E22` | `#E8A838` | `#7CC49E` | `#C06030` | `#BBBBC8` |
| Label | `#E0E0E0` | `#E8D5C0` | `#CDE0D5` | `#2C2C2E` | `#D0D0D6` |

Cyclic edges always render with `stroke-dasharray="6,4"` (dashed).

---

## The 8-Port Constraint — Why It Makes Beautiful Graphs

The constraint of **exactly 8 connection points per node** is the secret:

```
       1. Forces planarity     —  8 ports = bounded edge density
       2. Enforces ordering     —  forward uses E/W, cyclic uses N/S
       3. Prevents chaos        —  only octilinear angles (0°/45°/90°/135°/etc.)
       4. Guarantees            —  A* finds clean, separable paths for cycles
          separability
```

**Full 8-port routing (current vs future):**

The engine currently uses 4 ports for edges but defines 8 in the AST. A natural evolution would be:

1. First assign forward edges to East/West
2. Assign the first few cyclic edges to North/South
3. When N/S ports are exhausted or blocked, fall through to TopLeft/TopRight/BottomLeft/BottomRight diagonals
4. The A* pathfinder already supports all 8 octilinear directions — it just needs the port selector to offer diagonal starts

---

## Module Dependency Graph

```
lib.rs (orchestrates the 6-phase pipeline)
  ├── ast.rs             (types, enums, colour themes — zero dependencies)
  ├── parser.rs          (depends on ast + nom)
  ├── measure.rs         (depends on ast + cosmic-text)
  ├── layout.rs          (depends on ast + measure constants)
  ├── routing.rs         (depends on ast + layout + pathfinding)
  ├── svg_output.rs      (depends on ast — pure serialisation)
  └── wasm.rs            (depends on all modules + wasm-bindgen)
```

Each module is independently testable. The crate has **80 total tests** covering every phase.

---

## Playground Examples

Enter these DSL snippets in the Octovia playground to visualise the pipeline:

### Simple chain (forward edges only)
```
theme: transit
title: Forward Edges — Happy Path

Idle -> Active : recheck
Active -> Processing : submit
Processing -> Done : complete
```

### Triangle cycle (one cyclic back-edge)
```
theme: ember
title: Cyclic Edge — Back-Edge

A -> B : forward_1
B -> C : forward_2
C -> A : cycle_back
```

### Diamond with feedback (multiple cycle types)
```
theme: forest
title: Diamond + Feedback

Entry -> Fetch : begin
Entry -> Cache : check
Fetch -> Parse : raw_ok
Cache -> Parse : cached
Parse -> Store : parsed
Store -> Entry : feedback_loop
```
