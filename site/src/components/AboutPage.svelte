<div class="p-8 max-w-5xl mx-auto">
  <div class="flex items-center justify-between gap-4 mb-10">
    <div>
      <h1 class="text-3xl font-semibold tracking-tight text-base-content">Octovia</h1>
      <p class="text-sm text-base-content/60">A bespoke, DOM-free state diagram rendering engine in pure Rust</p>
    </div>
    <img src="./favicon.svg" alt="octovia" class="w-14 h-14" />
  </div>

  <div class="flex flex-col gap-4">
    
    <div class="card bg-base-100 border border-base-300 shadow-sm">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">Why Octovia?</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          Standard diagramming tools (like Mermaid) are incredibly feature-rich, but their layout engines are heavily tethered to the browser DOM. Running them in a CI/CD pipeline, a CLI tool, or a server-side doc generator often requires spinning up a headless Chromium instance just to measure text. 
        </p>
        <p class="text-sm text-base-content/70 leading-relaxed mt-2">
          Octovia was built to solve this. It is a scalpel, not a Swiss Army knife. By focusing strictly on state machines and ditching the DOM entirely, it provides a lightning-fast, highly portable layout engine that never draws "bezier spaghetti."
        </p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">What it is</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          <strong class="text-base-content">Octovia</strong> compiles a tiny text DSL into a crisp, scalable SVG diagram using a London Underground transit-map style. The entire engine — parser, text measurement, layout, edge routing, and SVG serialization — is a single, self-contained Rust crate. Zero DOM dependencies, zero external rendering runtimes.
        </p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">The 8-side rule</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          To guarantee legibility, <strong class="text-base-content">every node connects through exactly eight ports</strong>: the four sides (N, E, S, W) and the four corners (NE, NW, SE, SW). This eliminates arbitrary angles and overlapping chaos.
        </p>
        <pre class="bg-base-300 p-4 rounded-box overflow-x-auto text-[10px] leading-tight my-3 text-base-content/70"><code>      NW    N    NE
        ●────●────●
        │         │
   W ●──┤  NODE  ├──● E
        │         │
        ●────●────●
      SW    S    SE</code></pre>
        <p class="text-sm text-base-content/70 leading-relaxed">
          Edges are strictly classified to maintain flow:
        </p>
        <ul class="text-sm text-base-content/70 leading-relaxed mt-2 space-y-1 list-disc list-inside">
          <li><strong class="text-base-content">Forward edges</strong> — exit <code class="text-base-content/80">East</code>, enter <code class="text-base-content/80">West</code>. Drawn as straight horizontal lines. Zero turns, zero crossings.</li>
          <li><strong class="text-base-content">Cyclic edges</strong> (back-edges) — exit <code class="text-base-content/80">North</code>, enter <code class="text-base-content/80">South</code>. Routed by A* on an octilinear grid (0°/45°/90°/135° turns) with strict turn and crossing penalties.</li>
        </ul>
        <p class="text-sm text-base-content/70 leading-relaxed mt-2">
          The result: forward flow always reads left-to-right, feedback always loops cleanly over the top, and nothing overlaps.
        </p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">The pipeline</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          A diagram passes through six deterministic phases:
        </p>
        <ol class="text-sm text-base-content/70 leading-relaxed mt-2 space-y-1 list-decimal list-inside">
          <li><strong class="text-base-content">Parse</strong> — DSL or JSON → AST of nodes and edges.</li>
          <li><strong class="text-base-content">Measure</strong> — JetBrains Mono glyph metrics accurately size each node, wrapping text at 180px without a browser context.</li>
          <li><strong class="text-base-content">Layout</strong> — BFS extracts a spanning tree; nodes are placed on a serpentine (boustrophedon) grid filling left-to-right, then right-to-left.</li>
          <li><strong class="text-base-content">Route</strong> — Forward edges snap directly; cyclic edges are A*-routed around an occupancy grid aware of all nodes and prior edges.</li>
          <li><strong class="text-base-content">Label</strong> — Node labels are centered; edge labels float safely at the route midpoint.</li>
          <li><strong class="text-base-content">SVG</strong> — Auto-bounded viewBox, themed strokes, and serialized output.</li>
        </ol>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">Run anywhere</h2>
        <p class="text-sm text-base-content/70 leading-relaxed">
          Because the engine avoids the DOM, layout costs are dominated entirely by text measurement and A* math — microsecond-scale operations for small graphs. The core crate compiles to:
        </p>
        <ul class="text-sm text-base-content/70 leading-relaxed mt-2 space-y-2 list-none">
          <li>📦 <strong class="text-base-content">WebAssembly:</strong> Runs completely client-side. This playground re-renders the full pipeline on every keystroke with zero server latency.</li>
          <li>💻 <strong class="text-base-content">Native CLI:</strong> A standalone binary for CI/CD, piping <code class="text-base-content/80">.dsl</code> to SVG without external dependencies.</li>
          <li>🔌 <strong class="text-base-content">FFI Bindings (Roadmap):</strong> Native Python (<code class="text-base-content/80">pyo3</code>) and Node targets to bring fast, SSR-friendly layout to backend doc generators.</li>
        </ul>
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
        <p class="text-xs text-base-content/50">Each line is <code class="text-base-content/70">Source -> Target : label</code>. Optionally set a <code class="text-base-content/70">theme:</code> directive — <code class="text-base-content/70">transit</code>, <code class="text-base-content/70">ember</code>, <code class="text-base-content/70">forest</code>, <code class="text-base-content/70">light</code>, or <code class="text-base-content/70">monochrome</code>.</p>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300 border-dashed">
      <div class="card-body p-5">
        <h2 class="card-title text-sm">Built with</h2>
        <div class="flex flex-wrap gap-2 mt-2">
          <span class="badge badge-outline badge-sm">Rust</span>
          <span class="badge badge-outline badge-sm">wasm-bindgen</span>
          <span class="badge badge-outline badge-sm">cosmic-text</span>
          <span class="badge badge-outline badge-sm">pathfinding (A*)</span>
          <span class="badge badge-outline badge-sm">Svelte 5</span>
          <span class="badge badge-outline badge-sm">Tailwind CSS</span>
        </div>
      </div>
    </div>
  </div>
</div>