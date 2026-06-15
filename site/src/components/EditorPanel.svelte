<script lang="ts">
  import { tick } from 'svelte';
  import { EXAMPLES, type ExampleName } from '../lib/examples';
  import { THEMES, THEME_LABELS } from '../lib/themes';
  import { currentTheme, withTheme } from '../lib/dsl';
  import HelpIcon from './icons/HelpIcon.svelte';

  const BLANK = 'Blank' as const;
  type Selection = typeof BLANK | ExampleName;

  let {
    dsl = $bindable(),
    w = $bindable(),
    h = $bindable(),
    ready,
    render,
  }: {
    dsl: string;
    w: number;
    h: number;
    ready: boolean;
    render: () => void;
  } = $props();

  let selected = $state<Selection>('Simple Chain');
  const theme = $derived(currentTheme(dsl));

  const exampleNames = Object.keys(EXAMPLES) as ExampleName[];
  const allOptions: Selection[] = [BLANK, ...exampleNames];

  const isMac =
    typeof navigator !== 'undefined' &&
    /Mac|iPhone|iPad|iPod/i.test(
      (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
        navigator.platform ??
        navigator.userAgent,
    );
  const renderShortcut = isMac ? '⌘⏎' : 'Ctrl+⏎';

  // Switch between scrollable tab strip and dropdown based on the panel's
  // own width, so this responds to the editor pane (not just the viewport).
  const TABS_BREAKPOINT = 480;
  let barWidth = $state(0);
  const showTabs = $derived(barWidth >= TABS_BREAKPOINT);

  let scrollEl = $state<HTMLDivElement | null>(null);
  let canScrollLeft = $state(false);
  let canScrollRight = $state(false);
  
  let cheatsheetModal: HTMLDialogElement;

  function updateScrollState() {
    const el = scrollEl;
    if (!el) {
      canScrollLeft = false;
      canScrollRight = false;
      return;
    }
    canScrollLeft = el.scrollLeft > 1;
    canScrollRight = el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
  }

  function scrollByDelta(dx: number) {
    scrollEl?.scrollBy({ left: dx, behavior: 'smooth' });
  }

  $effect(() => {
    if (!showTabs || !scrollEl) {
      updateScrollState();
      return;
    }
    updateScrollState();
    const ro = new ResizeObserver(updateScrollState);
    ro.observe(scrollEl);
    for (const child of Array.from(scrollEl.children)) ro.observe(child);
    return () => ro.disconnect();
  });

  // When the active tab changes, keep it visible.
  $effect(() => {
    void selected;
    if (!showTabs) return;
    tick().then(() => {
      const el = scrollEl?.querySelector<HTMLElement>('[data-active="true"]');
      el?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' });
    });
  });

  function selectExample(name: Selection) {
    selected = name;
    const hasExplicitTheme = /^\s*theme\s*:/im.test(dsl);
    const chosenTheme = hasExplicitTheme ? currentTheme(dsl) : null;
    let next = name === BLANK ? '' : EXAMPLES[name];
    if (chosenTheme) next = withTheme(next, chosenTheme);
    dsl = next;
    render();
  }

  function setTheme(id: string) {
    dsl = withTheme(dsl, id);
    render();
  }

  function onKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      render();
    }
  }
</script>

<section class="flex-1 flex flex-col min-w-0 bg-base-200 overflow-hidden">
  <!-- Example picker -->
  <div
    class="shrink-0 bg-base-100 border-b border-base-300"
    bind:clientWidth={barWidth}
  >
    {#if showTabs}
      <div class="relative">
        <div
          bind:this={scrollEl}
          onscroll={updateScrollState}
          class="flex items-stretch gap-1 overflow-x-auto scroll-smooth px-2 py-1.5 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
        >
          {#each allOptions as name (name)}
            {@const active = selected === name}
            <button
              type="button"
              data-active={active}
              onclick={() => selectExample(name)}
              class="shrink-0 px-3 py-1 text-xs rounded-md whitespace-nowrap transition-colors
                     {active
                       ? 'bg-base-300 text-base-content'
                       : 'text-base-content/60 hover:text-base-content hover:bg-base-200'}"
            >{name}</button>
          {/each}
        </div>

        {#if canScrollLeft}
          <button
            type="button"
            aria-label="Scroll examples left"
            onclick={() => scrollByDelta(-160)}
            class="absolute left-0 top-0 bottom-0 flex items-center justify-center w-7
                   bg-gradient-to-r from-base-100 via-base-100/90 to-transparent
                   text-base-content/70 hover:text-base-content"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <polyline points="15 18 9 12 15 6" />
            </svg>
          </button>
        {/if}
        {#if canScrollRight}
          <button
            type="button"
            aria-label="Scroll examples right"
            onclick={() => scrollByDelta(160)}
            class="absolute right-0 top-0 bottom-0 flex items-center justify-center w-7
                   bg-gradient-to-l from-base-100 via-base-100/90 to-transparent
                   text-base-content/70 hover:text-base-content"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <polyline points="9 18 15 12 9 6" />
            </svg>
          </button>
        {/if}
      </div>
    {:else}
      <label class="flex items-center gap-2 text-xs text-base-content/70 p-2">
        <span class="shrink-0">Example</span>
        <select
          class="select select-xs select-bordered flex-1 max-w-xs"
          value={selected}
          onchange={(e) => selectExample(e.currentTarget.value as Selection)}
        >
          <option value={BLANK}>{BLANK}</option>
          {#each exampleNames as name (name)}
            <option value={name}>{name}</option>
          {/each}
        </select>
      </label>
    {/if}
  </div>

  <textarea
    bind:value={dsl}
    onkeydown={onKeydown}
    spellcheck={false}
    class="flex-1 m-2 p-3 bg-base-300 text-base-content border border-base-300 rounded-box font-mono text-sm leading-relaxed resize-none outline-none focus:border-base-content/30"
  ></textarea>

  <!-- Toolbar -->
  <div class="flex items-center gap-2 p-2 shrink-0 flex-wrap bg-base-100 border-t border-base-300">
    <label class="flex items-center gap-1 text-xs text-base-content/70">
      W
      <input type="number" bind:value={w} min="200" max="3000" class="input input-xs input-bordered w-16 text-center" />
    </label>
    <label class="flex items-center gap-1 text-xs text-base-content/70">
      H
      <input type="number" bind:value={h} min="200" max="3000" class="input input-xs input-bordered w-16 text-center" />
    </label>

    <div class="divider divider-horizontal mx-0"></div>

    <!-- Theme dropdown -->
    <div class="flex items-center gap-1">
      <span class="text-[10px] font-medium text-base-content/50 hidden sm:inline">Theme</span>
      <select
        class="select select-xs select-bordered max-w-[120px]"
        value={theme}
        onchange={(e) => setTheme(e.currentTarget.value)}
      >
        {#each THEMES as t (t)}
          <option value={t}>{THEME_LABELS[t]}</option>
        {/each}
      </select>
    </div>

    <!-- Help & Render Buttons -->
    <div class="ml-auto flex items-center gap-2">
      <button
        type="button"
        class="btn btn-sm btn-ghost btn-square text-base-content/60 hover:text-base-content"
        onclick={() => cheatsheetModal.showModal()}
        title="Syntax Cheatsheet"
        aria-label="Open Syntax Cheatsheet"
      >
        <HelpIcon size={18} />
      </button>
      
      <button onclick={render} disabled={!ready} class="btn btn-neutral btn-sm">
        {#if ready}
          Render {renderShortcut}
        {:else}
          Loading…
        {/if}
      </button>
    </div>
  </div>
</section>

<!-- Syntax Cheatsheet Modal -->
<dialog bind:this={cheatsheetModal} class="modal backdrop-blur-sm">
  <div class="modal-box bg-base-100 border border-base-300 shadow-xl max-w-2xl p-0 overflow-hidden">
    <div class="p-4 border-b border-base-300 bg-base-200/50 flex justify-between items-center">
      <h3 class="font-semibold text-base-content">Syntax Guide</h3>
      <form method="dialog">
        <button class="btn btn-sm btn-circle btn-ghost">✕</button>
      </form>
    </div>

    <div class="p-6 overflow-y-auto max-h-[60vh]">
      <div class="space-y-6 text-sm text-base-content/80">
        <div>
          <h4 class="font-semibold text-base-content mb-2 border-b border-base-200 pb-1">Nodes & Edges</h4>
          <p class="mb-2">Define paths using the <code class="font-mono bg-base-200 px-1 rounded text-xs">-></code> operator. Labels go after a colon.</p>
          <pre class="bg-base-200 p-3 rounded-md font-mono text-xs text-base-content/70"><code>Source -> Target : edge label</code></pre>
        </div>

        <div>
          <h4 class="font-semibold text-base-content mb-2 border-b border-base-200 pb-1">Properties</h4>
          <p class="mb-2">Global properties are set with a colon at the top of the file.</p>
            <pre class="bg-base-200 p-3 rounded-md font-mono text-xs text-base-content/70 flex flex-col">
              <code>title: My Diagram</code>
              <code>theme: transit | ember | monochrome | light | forest</code>
            </pre>
        </div>

      </div>
    </div>
  </div>
  
  <form method="dialog" class="modal-backdrop">
    <button type="button" onclick={() => cheatsheetModal.close()} class="cursor-default">close</button>
  </form>
</dialog>