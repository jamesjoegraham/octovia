<div class="p-8 max-w-2xl mx-auto">
  <!-- Hero -->
  <div class="flex items-center justify-between gap-4 mb-10">
    <div>
      <h1 class="text-3xl font-semibold tracking-tight text-base-content">Octovia</h1>
      <p class="text-sm text-base-content/60">A bespoke, DOM-free state diagram rendering engine in pure Rust</p>
    </div>
    <img src="./favicon.svg" alt="octovia" class="w-14 h-14" />
  </div>

  <!-- Cards -->
  <div class="flex flex-col gap-4">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">What it is</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          <strong class="text-base-content">Octovia</strong> compiles a tiny text DSL describing a state machine into a crisp, scalable SVG diagram in the London Underground transit-map style. The whole engine — parser, text measurement, layout, edge routing, SVG serialiser — is one self-contained Rust crate with no DOM dependency and no external rendering runtime.
        </p>
        <p class="text-sm text-base-content/70 leading-relaxed mt-2">
          The same crate compiles to a <strong class="text-base-content">native CLI binary</strong> and to a <strong class="text-base-content">WebAssembly module</strong>. This page is the WASM build running entirely in your browser: zero server calls, zero install, zero latency.
        </p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">The 8-side rule</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          The core idea that makes the output legible: <strong class="text-base-content">every node connects through exactly eight ports</strong> — the four sides (N, E, S, W) and the four corners (NE, NW, SE, SW). No arbitrary angles, no overlapping chaos, no bezier spaghetti.
        </p>
        <pre class="bg-base-300 p-4 rounded-box overflow-x-auto text-[10px] leading-tight my-3 text-base-content/70"><code>      NW    N    NE
        ●────●────●
        │         │
   W ●──┤  NODE  ├──● E
        │         │
        ●────●────●
      SW    S    SE</code></pre>
        <p class="text-sm text-base-content/70 leading-relaxed">
          Edges then split into two classes with strict rules:
        </p>
        <ul class="text-sm text-base-content/70 leading-relaxed mt-2 space-y-1 list-disc list-inside">
          <li><strong class="text-base-content">Forward edges</strong> — exit <code class="text-base-content/80">East</code>, enter <code class="text-base-content/80">West</code>, drawn as a straight horizontal line. Zero turns, zero crossings.</li>
          <li><strong class="text-base-content">Cyclic edges</strong> (back-edges) — exit <code class="text-base-content/80">North</code>, enter <code class="text-base-content/80">South</code>, routed by A* on an octilinear grid (only 0°/45°/90°/135° turns) with a turn penalty and a crossing penalty so paths stay clean.</li>
        </ul>
        <p class="text-sm text-base-content/70 leading-relaxed mt-2">
          The result: forward flow always reads left-to-right, feedback always loops over the top, and nothing overlaps.
        </p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">The pipeline</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          A diagram passes through six linear phases, each one feeding the next:
        </p>
        <ol class="text-sm text-base-content/70 leading-relaxed mt-2 space-y-1 list-decimal list-inside">
          <li><strong class="text-base-content">Parse</strong> — DSL or JSON → AST of nodes and edges.</li>
          <li><strong class="text-base-content">Measure</strong> — JetBrains Mono glyph metrics size each node, wrapping labels at 180px.</li>
          <li><strong class="text-base-content">Layout</strong> — BFS extracts a spanning tree from the root; back-edges are tagged. Nodes are placed on a serpentine (boustrophedon) grid that fills left-to-right, then right-to-left.</li>
          <li><strong class="text-base-content">Route</strong> — forward edges drawn directly; cyclic edges A*-routed around an occupancy grid that knows where every node and previously-routed edge sits.</li>
          <li><strong class="text-base-content">Label</strong> — node labels centred in their box; edge labels float at the route midpoint.</li>
          <li><strong class="text-base-content">SVG</strong> — auto-bounded viewBox, themed strokes and fills, dashed lines for cyclic edges.</li>
        </ol>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">Rust speed, two targets</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          Because the engine is plain Rust with no DOM, layout cost is dominated by text measurement and A* — both of which are microsecond-scale on small graphs. The same source compiles to:
        </p>
        <ul class="text-sm text-base-content/70 leading-relaxed mt-2 space-y-1 list-disc list-inside">
          <li><strong class="text-base-content">Native</strong> — a single static <code class="text-base-content/80">octovia</code> binary that reads <code class="text-base-content/80">.dsl</code> or <code class="text-base-content/80">.json</code> and writes an SVG file. Pipe-friendly, scriptable, suitable for CI and docs build steps.</li>
          <li><strong class="text-base-content">WebAssembly</strong> — a wasm-bindgen module that exposes the same render function to JavaScript. The editor on this site re-runs the full pipeline on every keystroke without breaking a sweat.</li>
        </ul>
        <p class="text-sm text-base-content/70 leading-relaxed mt-2">
          Output is identical between targets — the WASM build is just the same compiled engine running on a different ISA.
        </p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">DSL quick reference</h2>
        <pre class="bg-base-300 p-4 rounded-box overflow-x-auto text-xs leading-relaxed my-2"><code># Comments
title: My Machine
theme: ember

Idle -> Active : recheck
Active -> Processing : submit
Processing -> Done : complete
Done -> Idle : reset</code></pre>
        <p class="text-xs text-base-content/50">Each line is <code class="text-base-content/70">Source -> Target : label</code>. Optionally set a <code class="text-base-content/70">theme:</code> directive — <code class="text-base-content/70">transit</code>, <code class="text-base-content/70">ember</code>, <code class="text-base-content/70">forest</code>, <code class="text-base-content/70">light</code>, or <code class="text-base-content/70">monochrome</code>. Everything runs client-side.</p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">Built with</h2>
        <div class="flex flex-wrap gap-2 mt-1">
          <span class="badge badge-outline">Rust</span>
          <span class="badge badge-outline">wasm-bindgen</span>
          <span class="badge badge-outline">cosmic-text</span>
          <span class="badge badge-outline">pathfinding (A*)</span>
          <span class="badge badge-outline">Svelte 5</span>
          <span class="badge badge-outline">DaisyUI</span>
          <span class="badge badge-outline">Tailwind CSS</span>
          <span class="badge badge-outline">Vite</span>
          <span class="badge badge-outline">SVG</span>
        </div>
      </div>
    </div>
  </div>
</div>
