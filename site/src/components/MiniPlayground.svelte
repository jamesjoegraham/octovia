<script lang="ts">
  import { onDestroy } from 'svelte';
  import { EXAMPLES, type ExampleName } from '../lib/examples';
  import { THEMES, THEME_LABELS } from '../lib/themes';
  import { currentTheme, withTheme } from '../lib/dsl';
  
  let {
    ready,
    render,
    dsl = $bindable(),
    svg,
    renderTimeStr,
  }: {
    ready: boolean;
    render: () => void;
    dsl: string;
    svg: string;
    renderTimeStr: string | null;
  } = $props();

  // Three curated examples for the homepage taster.
  const HOME_EXAMPLES: ExampleName[] = ['Pizza Order', 'Pull Request', 'Order Lifecycle'];

  // Debounce window for textarea edits: render once typing pauses.
  const DEBOUNCE_MS = 2000;

  let selected = $state<ExampleName>('Pizza Order');
  let err = $state('');

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const theme = $derived(currentTheme(dsl));
  const lightBackground = $derived(theme === 'light');

  function clearDebounce() {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  function scheduleRender() {
    if (!ready || !render) return;
    clearDebounce();
    debounceTimer = setTimeout(render, DEBOUNCE_MS);
  }

  // Initial render once WASM is ready.
  $effect(() => {
    if (ready && !svg && !err) {
      dsl = EXAMPLES[selected];
      render();
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
    render();
  }

  function setTheme(id: string) {
    dsl = withTheme(dsl, id);
    render();
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

    <!-- Theme picker (dropdown for all 46 themes) -->
    <div class="flex items-center gap-1 ml-auto">
      <span class="text-[10px] font-medium text-base-content/50 hidden sm:inline">Theme</span>
      <select
        class="select select-xs select-bordered max-w-[110px]"
        value={theme}
        onchange={(e) => { dsl = withTheme(dsl, e.currentTarget.value); render(); }}
      >
        {#each THEMES as t (t)}
          <option value={t}>{THEME_LABELS[t]}</option>
        {/each}
      </select>
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
        {#if renderTimeStr === null}
          waiting…
        {:else}
          {renderTimeStr}
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
