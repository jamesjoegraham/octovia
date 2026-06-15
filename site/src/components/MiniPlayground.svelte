<script lang="ts">
  import { onDestroy } from 'svelte';
  import { EXAMPLES, type ExampleName } from '../lib/examples';
  import { THEMES, THEME_LABELS, type Theme } from '../lib/themes';
  import { currentTheme, withTheme } from '../lib/dsl';

  type RenderFn = (d: string, w?: number | null, h?: number | null) => string;

  let {
    ready,
    renderSvg,
    onOpenPlayground,
  }: {
    ready: boolean;
    renderSvg: RenderFn | null;
    onOpenPlayground: () => void;
  } = $props();

  // Three curated examples for the homepage taster.
  const HOME_EXAMPLES: ExampleName[] = ['Simple Chain', 'Branching', 'Order Lifecycle'];

  // Debounce window for textarea edits: render once typing pauses.
  const DEBOUNCE_MS = 2000;

  let selected = $state<ExampleName>('Simple Chain');
  let dsl = $state<string>(EXAMPLES['Simple Chain']);
  let svg = $state('');
  let err = $state('');
  let lastMs = $state<number | null>(null);
  let pending = $state(false);

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const theme = $derived(currentTheme(dsl));
  const lightBackground = $derived(theme === 'light');

  function clearDebounce() {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  function doRender() {
    clearDebounce();
    pending = false;
    if (!renderSvg) return;
    try {
      const t0 =
        typeof performance !== 'undefined' && typeof performance.now === 'function'
          ? performance.now()
          : Date.now();
      const out = renderSvg(dsl, null, null);
      const t1 =
        typeof performance !== 'undefined' && typeof performance.now === 'function'
          ? performance.now()
          : Date.now();
      svg = out;
      lastMs = t1 - t0;
      err = '';
    } catch (e: any) {
      err = `Render error: ${e?.message ?? e}`;
      svg = '';
      lastMs = null;
    }
  }

  function scheduleRender() {
    if (!ready || !renderSvg) return;
    pending = true;
    clearDebounce();
    debounceTimer = setTimeout(doRender, DEBOUNCE_MS);
  }

  // Initial render once WASM is ready.
  $effect(() => {
    if (ready && renderSvg && !svg && !err) {
      doRender();
    }
  });

  onDestroy(clearDebounce);

  function selectExample(name: ExampleName) {
    selected = name;
    const hasExplicitTheme = /^\s*theme\s*:/im.test(dsl);
    const chosenTheme = hasExplicitTheme ? currentTheme(dsl) : null;
    let next: string = EXAMPLES[name];
    if (chosenTheme) next = withTheme(next, chosenTheme);
    dsl = next;
    doRender();
  }

  function setTheme(id: Theme) {
    dsl = withTheme(dsl, id);
    doRender();
  }

  function formatLatency(ms: number): string {
    // Some browsers (notably Firefox with default timer-precision clamping)
    // round `performance.now()` to the nearest 1 ms, so a fast render can
    // measure as exactly 0. Fall back to a "< 1 ms" display rather than "0 ms".
    if (ms <= 0) return '< 1 ms';
    if (ms < 1) {
      const us = ms * 1000;
      if (us < 10) return `${us.toFixed(1)} µs`;
      return `${Math.round(us)} µs`;
    }
    if (ms < 10) return `${ms.toFixed(1)} ms`;
    return `${Math.round(ms)} ms`;
  }
</script>

<div class="rounded-box border border-base-300 bg-base-100 overflow-hidden shadow-sm">
  <!-- Header: examples + theme + open-full-playground -->
  <div class="flex flex-wrap items-center gap-2 px-3 py-2 border-b border-base-300 bg-base-100">
    <div class="join">
      {#each HOME_EXAMPLES as name (name)}
        <button
          type="button"
          onclick={() => selectExample(name)}
          class="join-item btn btn-xs {selected === name ? 'btn-active' : 'btn-ghost'}"
        >{name}</button>
      {/each}
    </div>

    <div class="join ml-auto">
      {#each THEMES as t (t)}
        <button
          type="button"
          onclick={() => setTheme(t)}
          title={THEME_LABELS[t]}
          class="join-item btn btn-xs {theme === t ? 'btn-active' : 'btn-ghost'}"
        >{THEME_LABELS[t]}</button>
      {/each}
    </div>
  </div>

  <!-- Editor + Preview -->
  <div class="grid grid-cols-1 md:grid-cols-2 gap-px bg-base-300">
    <textarea
      bind:value={dsl}
      oninput={scheduleRender}
      spellcheck={false}
      class="bg-base-200 text-base-content p-3 font-mono text-xs leading-relaxed resize-none outline-none min-h-[260px] md:min-h-[340px] focus:bg-base-300"
    ></textarea>

    <div
      class="relative min-h-[260px] md:min-h-[340px] overflow-hidden"
      class:bg-base-200={!lightBackground}
      class:bg-white={lightBackground}
    >
      {#if err}
        <div class="absolute inset-0 flex items-center justify-center p-3">
          <div class="alert alert-error text-xs p-2">{err}</div>
        </div>
      {:else if svg}
        <div class="mini-svg absolute inset-0">{@html svg}</div>
      {:else if !ready}
        <div class="absolute inset-0 flex items-center justify-center">
          <span class="loading loading-spinner loading-sm text-base-content/40"></span>
        </div>
      {/if}

      <!-- Latency / pending indicator (bottom-right) -->
      <div
        class="pointer-events-none absolute bottom-1.5 right-2 text-[10px] font-mono tabular-nums"
        style="color: {lightBackground ? 'rgba(0,0,0,0.45)' : 'rgba(255,255,255,0.45)'}"
      >
        {#if pending}
          waiting…
        {:else if lastMs !== null}
          {formatLatency(lastMs)}
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  :global(.mini-svg svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
