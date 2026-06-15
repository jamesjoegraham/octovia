# Octovia Render Pipeline — Architecture

> **Principle:** Easy to read, easy to write. Every diagram is laid out as a *layered topological grid* and every edge is routed through a single, occupancy-aware A* pass — including its label.

---

## Pipeline Overview

The engine processes a DSL or JSON description through **5 phases**. Every phase transforms the AST in place and feeds the next.

```
  DSL / JSON
       │
       ▼
┌────────────────────────────┐
│ 1. Parse                   │  text DSL or JSON  → AST (Node, Edge)
└──────────────┬─────────────┘
               ▼
┌────────────────────────────┐
│ 2. Measure                 │  cosmic-text → label_extents
│                            │  → grid-quantised node_size
└──────────────┬─────────────┘
               ▼
┌────────────────────────────┐
│ 3. Layered Layout          │  classify back-edges (iterative DFS)
│   (Sugiyama L1)            │  longest-path topological sort  (Kahn)
│                            │  → node.position, node.layer,
│                            │    edge.is_cyclic
└──────────────┬─────────────┘
               ▼
┌────────────────────────────┐
│ 4. Unified Routing +       │  one A* pass per edge over a shared
│    Labelling               │  GridOccupancy.  After every route is
│                            │  committed, search the local
│                            │  neighbourhood for a free label
│                            │  anchor and reserve its bbox too.
└──────────────┬─────────────┘
               ▼
┌────────────────────────────┐
│ 5. SVG Output              │  trim polylines to node rectangles,
│                            │  emit themed SVG with auto viewBox
└────────────────────────────┘
```

The same pipeline can be expressed *as* an Octovia diagram:

```
title: Octovia Render Pipeline
theme: transit

Parse -> Measure : text
Measure -> Layout : node_size
Layout -> Routing : positions + layers
Routing -> SVG_Output : routes + label_anchors
```

---

## Phase 1 — Parse (`src/parser/`)

Two surface formats produce the same `Diagram`:

* **DSL** (`src/parser/dsl.rs`) — line-oriented, narrative.
* **JSON** (`src/parser/json.rs`) — structured input for adapters and tool calls.

```
title: My Machine
theme: ember

Idle -> Active : recheck
Active -> Processing : submit
Processing -> Done : complete
Done -> Idle : reset
```

Rules:
* Lines beginning with `#` are comments.
* `title: <string>`, `theme: <name>`, `background: <colour|theme|transparent>` are directives.
* `Source -> Target : label` adds a directed edge (label optional).
* Nodes are created implicitly the first time they appear.
* Insertion order is preserved — it is load-bearing for deterministic layout.

### AST Types (`src/ast/`)

```rust
struct Diagram {
    nodes: Vec<Node>,        // insertion-ordered
    edges: Vec<Edge>,        // declaration-ordered
    title: Option<String>,
    viewport: Viewport,
    theme: ThemeColors,
    background: Background,
}

struct Node {
    id: String,
    label: String,
    label_extents: Option<TextExtents>,   // ← Phase 2
    node_size:     Option<NodeSize>,       // ← Phase 2 (grid-quantised)
    position:      Option<Point>,          // ← Phase 3
    layer:         Option<usize>,          // ← Phase 3 (depth in topo order)
}

struct Edge {
    from: String,
    to:   String,
    label:         Option<String>,
    label_extents: Option<TextExtents>,   // ← Phase 2
    is_cyclic:     bool,                   // ← Phase 3 (back-edge)
    route:         Vec<Point>,             // ← Phase 4
    label_anchor:  Option<EdgeLabelAnchor>,// ← Phase 4
}

struct EdgeLabelAnchor { x: i32, y: i32, anchor: &'static str /* start | middle | end */ }
```

Geometry primitives (`src/ast/geom.rs`):

```rust
struct Point     { x: i32, y: i32 }
struct NodeSize  { width: i32, height: i32 }
struct Viewport  { width: u32, height: u32 }

const MIN_NODE_SIDE: i32 = 60;
const NODE_PADDING:  i32 = 24;
```

---

## Phase 2 — Measure (`src/measure.rs`)

Uses **cosmic-text** with embedded JetBrains Mono to size every label without any DOM or OS font lookup.

A single global `FontSystem` is built lazily via `OnceLock<Mutex<FontSystem>>`.

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_NODE_WIDTH` | `180.0` px | Soft wrap target for long labels |
| `FONT_SIZE`      | `14.0` px | Body text |
| `LINE_HEIGHT`    | `1.35`    | Line spacing ratio |
| `NODE_PADDING`   | `24` px   | Padding inside node rect |
| `MIN_NODE_SIDE`  | `60` px   | Smallest node dimension |
| `GRID`           | `10` px   | Sub-grid all dimensions snap to |

**Per-node sizing (no global unification):**

```
ext         = measure(label, MAX_NODE_WIDTH)
raw_w       = max(ext.width  + NODE_PADDING, MIN_NODE_SIDE)
raw_h       = max(ext.height + NODE_PADDING, MIN_NODE_SIDE)
node_size.w = ceil_to_grid(raw_w)        // multiple of GRID
node_size.h = ceil_to_grid(raw_h)        // multiple of GRID
```

Every node is sized for *its own* label. Earlier versions unified all widths to the single largest node; that has been removed. Quantising to the 10 px sub-grid lets the routing phase reason about node footprints exactly in cell coordinates.

---

## Phase 3 — Layered Layout (`src/layout.rs`)

A lightweight **Sugiyama L1**: classify back-edges, then assign every forward node a depth via a *longest-path* topological sort.

### 3a — Back-edge classification

`classify_back_edges(diagram) -> HashSet<usize>` runs an iterative DFS with three-colour bookkeeping:

```
WHITE  — unvisited
GRAY   — currently on the DFS stack
BLACK  — fully processed
```

An edge `u → v` is a back-edge iff:
* `v` is currently `GRAY` (closes a cycle), **or**
* `v == u` (self-loop).

DFS roots are taken in node insertion order, so the result is deterministic.

### 3b — Longest-path topology

`compute_layers(diagram, back_set)` runs Kahn's algorithm using only forward edges (back-edges and self-edges are skipped). For every node `v`:

$$
L(v) \;=\; \max_{u \to v \in E_{\text{fwd}}} \bigl(L(u) + 1\bigr), \qquad L(\text{source}) = 0
$$

Equivalently: `L(v)` is the length of the longest forward path from any source to `v`. This guarantees layered acyclicity — every forward edge points strictly *right* (`L(to) > L(from)`), every back-edge points *left or flat*.

The seeding queue is initialised with sources in input order, so two diagrams with the same logical structure produce identical layer assignments.

### 3c — Grid placement

Layers become **columns**, layers' nodes stack into **rows**:

```
layer:        0          1          2          3
            ┌────┐                ┌────┐    ┌────┐
            │ A  │ ───────────▶  │ C  │ ──▶│ E  │
            └────┘                └────┘    └────┘
                                  ┌────┐
                                  │ D  │
            ┌────┐                └────┘
            │ B  │
            └────┘
```

Constants:

| Constant       | Value | Purpose |
|----------------|-------|---------|
| `LAYER_GUTTER` | `90`  | Pixels between layer columns |
| `ROW_GUTTER`   | `70`  | Pixels between rows |
| `MARGIN`       | `50`  | Outer margin |

For each layer `l` we compute the layer's column width = `max(node.width)` over its members; for each row `r` we compute the row height = `max(node.height)` over all layers at that row. A cumulative cursor places columns and rows with the gutters above. Each node's `position` is its centre; `layer` records its column.

`is_cyclic` is finally written into every edge from the back-edge set.

---

## Phase 4 — Unified Routing + Labelling (`src/routing/`)

This is the heart of the new pipeline. **Every** edge — forward or back — flows through one A* router operating over a shared **GridOccupancy** that is mutated as routes and labels are committed.

### 4a — Sub-grid model

Pixel coordinates `(x, y)` map to grid cells `(x / 10, y / 10)`. Three disjoint cell sets live inside `GridOccupancy`:

* `node_cells`  — the rectangular block of every placed node, padded by `NODE_BLOCK_MARGIN = 1` cell.
* `edge_cells`  — every cell along an already-committed polyline.
* `label_cells` — the bounding box of every already-placed edge label.

`is_free(cell)` is the conjunction over all three; the search treats labels as impassable terrain just like nodes and prior edges.

A **world bounding box** is also derived from the union of node blocks plus a fixed `WORLD_PAD_CELLS = 12` margin. A* neighbour expansion is bounded by this rectangle so the search is always finite.

### 4b — Port selection (`src/routing/ports.rs`)

```
                  (port = node_centre + (½ size + margin + 1) cells)

forward edge        cyclic / back-edge
   src → tgt:           src ↺ tgt:

  +───+    +───+         +───+        +───+
  │ A │ ─▶ │ B │         │ A │ ──┐    │ B │
  +───+    +───+         +───+   │    +───+
                                 │      ▲
                            (S ↓)│      │(N ↑)
                                 └──────┘
```

* **Forward edges** — `forward_port_candidates(from, to)` returns three ranked (src, tgt) pairs: the dominant-axis primary plus two perpendicular alternates that share the natural target port. The router runs A* for each and keeps the cheapest path.
* **Back-edges** — `back_port_candidates(from, to)` returns three ranked pairs all entering the target via **North**, exiting either **South** (canonical wrap), **East**, or **West** depending on which side of the source the target sits on.

Ports are placed one cell *outside* the node block. To prevent diagonal A* moves from snapping into a node port at 45°, the search itself runs between **stalk cells** — one further cell along the port's outward axis — and the rendered polyline prepends/appends the port cell, giving every route a 1-cell orthogonal entry and exit.

### 4c — A* (`src/routing/astar.rs`)

Plain A* with the **octile distance** heuristic and 8-direction movement:

| Direction | Cost |
|-----------|------|
| E, W, N, S      | 10 (orthogonal) |
| NE, NW, SE, SW  | 14 (diagonal, ≈ √2 × 10) |

$$
h(a, b) \;=\; (|\Delta x| + |\Delta y| - d) \cdot 10 + d \cdot 14, \qquad d = \min(|\Delta x|, |\Delta y|)
$$

Neighbours are emitted in a fixed order (E, W, S, N, NE, NW, SE, SW) so ties are broken deterministically. The endpoints `start` and `end` always satisfy `is_free`, regardless of node/edge/label state, so a route always exists from a port to its goal. `astar_cells` returns both the path and its accumulated cost so the routing loop can compare candidate port pairs.

### 4d — The unified loop (`src/routing/mod.rs`)

```rust
let mut occupancy = GridOccupancy::new(diagram);

// Pass 1 — route every edge, lowest-cost candidate wins.
for ei in forward_edges_then_back_edges {
    let candidates = if edge.is_cyclic {
        back_port_candidates(from_centre, to_centre)
    } else {
        forward_port_candidates(from_centre, to_centre)
    };
    let cells = best_route(from_centre, to_centre, src_size, tgt_size,
                           &candidates, &occupancy);

    let route = [from_centre]
        ++ cells.iter().map(|(cx,cy)| Point::new(cx*10, cy*10))
        ++ [to_centre];
    edge.route = route;
    occupancy.occupy_path(&cells);
}

// Pass 2 — spread genuinely co-linear forward edges.
assign_parallel_lanes(diagram);

// Pass 3 — labels run last, against a fresh occupancy rebuilt from
// the post-lane geometry so anchors track the final polyline.
let mut label_occupancy = GridOccupancy::new(diagram);
for edge in &diagram.edges {
    label_occupancy.occupy_path(&cells_along_polyline(&edge.route));
}
for edge in &mut diagram.edges {
    if let Some(extents) = edge.label_extents {
        edge.label_anchor = seek_label_anchor(&edge.route, extents, &label_occupancy);
        if let Some(a) = edge.label_anchor {
            label_occupancy.occupy_label(a, extents);
        }
    }
}
```

`best_route` runs A* between the **stalk** cells of each candidate port pair (one cell beyond the port along its outward axis) and returns the cell sequence — including the port cells themselves — for the cheapest candidate. Forward edges are processed before back-edges so back-edges naturally route around already-committed forward routes.

Routing labels *after* the lane spreading pass is critical: the lane pass shifts straight segments by one grid cell, and if labels were placed first they would no longer track the edge they belong to. Pass 3 rebuilds the occupancy grid from the actual final polylines so each label avoids both nodes and the (possibly shifted) edges, then writes its bounding box into `label_cells` so subsequent labels avoid each other.

The **lanes** post-pass (`src/routing/lanes.rs`) handles the rare case where two or more forward edges end up perfectly co-linear (same fixed `x` or `y` over an overlapping span); each is shifted one cell perpendicular with 45° connectors at the ends.

### 4e — Occupancy invariant

At any point during the loop, the following holds for the partial diagram:

$$
\texttt{is\_free}(c) \;\Longleftrightarrow\; c \notin (\texttt{nodes} \cup \texttt{committed routes} \cup \texttt{committed labels})
$$

A* search always respects this set, so by induction no two committed elements share a cell (modulo deliberate path crossings — the router accepts those because forbidding them would make some diagrams unroutable).

---

## Phase 5 — SVG Output (`src/svg_output/`)

### Z-order (bottom to top)

```
1. Background fill (themed or transparent)
2. Title text (if any)
3. Edge paths (themed strokes, dashed for cyclic)
4. Edge labels (haloed text using the precomputed label_anchor)
5. Node rectangles (filled + stroked)
6. Node labels (centred text)
```

### Trimming (`src/svg_output/trim.rs`)

Edge polylines are bookended with the source and target node centres. `trim_route_to_node_boundaries` walks the segments at each end and finds the intersection with the rectangle's edge; the polyline is rewritten to start and end exactly on those rectangle boundaries. The arrowhead marker is then emitted at the (already trimmed) tail.

### Auto bounding box (`compute_bounds`)

Every node rectangle and every route point contributes to `(x_min, y_min, x_max, y_max)`; the result is padded into a tight `viewBox`. The SVG always fits its content with no clipping and no wasted whitespace.

### Themes

Five colour themes drive every visual property:

| Part         | Transit  | Ember    | Forest   | Light    | Mono     |
|--------------|----------|----------|----------|----------|----------|
| Background   | `#1A1A2E`| `#1C1410`| `#0F1A14`| `#F5F5F0`| `#111112`|
| Node fill    | `#16213E`| `#2A1D16`| `#16251D`| `#FFFFFF`| `#1C1C1E`|
| Node stroke  | `#4A90D9`| `#D4803A`| `#3D9B6B`| `#4A6FA5`| `#888899`|
| Forward edge | `#4A90D9`| `#D4803A`| `#3D9B6B`| `#4A6FA5`| `#888899`|
| Cyclic edge  | `#E67E22`| `#E8A838`| `#7CC49E`| `#C06030`| `#BBBBC8`|
| Label        | `#E0E0E0`| `#E8D5C0`| `#CDE0D5`| `#2C2C2E`| `#D0D0D6`|

Cyclic edges always render with `stroke-dasharray="6,4"`.

---

## Mathematical summary

Let $G = (V, E)$ be a directed multigraph.

1. **Back-edge set** $B \subseteq E$ from iterative DFS:

   $$ B = \{\,(u,v) \in E \mid v \text{ is on the DFS stack when } u\to v \text{ is traversed}\,\} \;\cup\; \{\,(v,v) \in E\,\} $$

2. **Layer function** $L : V \to \mathbb{Z}_{\geq 0}$ from Kahn's longest-path topo sort over $E \setminus B$:

   $$ L(v) = \max_{u \to v \in E \setminus B}\bigl(L(u) + 1\bigr), \quad L(v) = 0 \text{ if } v \text{ has no forward predecessors} $$

3. **Geometric placement** $p : V \to \mathbb{R}^2$ from layered grid: column index = $L(v)$, row index = the node's order within its layer (input order).

4. **Routing**: for every edge $e = (u, v) \in E$ in a fixed schedule (forwards first), find a path

   $$ \pi_e : \{0, \ldots, n_e\} \to \mathbb{Z}^2, \qquad \pi_e(0) = \text{port}(u, e),\; \pi_e(n_e) = \text{port}(v, e) $$

   minimising octile cost subject to $\pi_e(k) \in \text{Free}_{<e}$ for $0 < k < n_e$, where $\text{Free}_{<e}$ is the occupancy complement *at the moment $e$ is routed*.

5. **Label placement**: choose anchor $a_e \in \text{Free}_{<e}^{\text{after-route}}$ minimising distance to a midpoint candidate set; reserve $\text{bbox}(a_e, \text{extents}_e)$ into the occupancy for later edges.

6. **SVG**: trim, bookend, emit. End.

---

## Module dependency graph

```
lib.rs (orchestrates the 5-phase pipeline)
  ├── ast/                 (types, themes, geometry — zero deps)
  ├── parser/              (dsl + json → Diagram)
  ├── measure.rs           (cosmic-text → label_extents + node_size)
  ├── layout.rs            (back-edges → layers → positions)
  ├── label_placement.rs   (free-anchor search around a polyline)
  ├── routing/             (occupancy + ports + astar + lanes)
  ├── svg_output/          (defs + elements + trim + render)
  └── wasm.rs              (wasm-bindgen entry points)
```

Each module is independently testable. The crate ships **101 unit tests** plus **11 integration tests**, all green and deterministic.

---

## Determinism

Octovia is deterministic by design:

* `BTreeMap` iteration in `compute_layers`.
* DFS roots seeded from `nodes` in input order.
* A* neighbour order is a fixed array of 8 directions.
* `pathfinding::astar` ties broken by insertion order in the open set.
* Routing schedule is `[forward_edges_in_input_order, back_edges_in_input_order]`.
* Label anchor search probes waypoints in a fixed mid-out spiral.

The `test_octo_render_is_deterministic` integration test renders the same DSL twice and asserts byte-for-byte equality.

---

## Playground examples

Drop these into the playground to see the pipeline in action.

### Linear chain
```
theme: transit
title: Forward Edges — Happy Path

Idle -> Active : recheck
Active -> Processing : submit
Processing -> Done : complete
```

### Triangular cycle
```
theme: ember
title: Back-Edge

A -> B : forward_1
B -> C : forward_2
C -> A : cycle_back
```

### Diamond with feedback
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
