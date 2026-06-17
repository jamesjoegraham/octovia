<script lang="ts">
  import { onMount } from 'svelte';
  import AboutPage from './components/AboutPage.svelte';
  import HomePage from './components/HomePage.svelte';
  import Navbar from './components/Navbar.svelte';
  import EditorPanel from './components/EditorPanel.svelte';
  import PreviewPanel from './components/PreviewPanel.svelte';
  import { EXAMPLES } from './lib/examples';
  import { currentTheme } from './lib/dsl';

  type RenderFn = (d: string, w?: number | null, h?: number | null) => string;
  type Page = 'home' | 'playground' | 'about';

  let dsl = $state(EXAMPLES['Pizza Order']);
  let svg = $state('');
  let err = $state('');
  let ready = $state(false);
  let w = $state(1100);
  let h = $state(750);
  let renderTimeStr = $state<string | null>(null);
  let page = $state<Page>('home');

  let lightBackground = $derived(() => {
    const lightThemes = ['paper', 'arctic', 'mono-light', 'sepia'];
    return lightThemes.includes(currentTheme(dsl));
  });

  let renderFn: RenderFn | null = $state(null);

  const theme = $derived(currentTheme(dsl));

  onMount(async () => {
    try {
      const mod = await import('./octovia/octovia.js');
      await mod.default();

      renderFn = mod.render_from_dsl;
      ready = true;
      render();

      
    } catch (e: any) {
      err = `WASM init failed: ${e.message ?? e}`;
    }
  });

  function render() {
    if (!renderFn) return;
    err = '';
    try {
      const t0 =
        typeof performance !== 'undefined' && typeof performance.now === 'function'
          ? performance.now()
          : Date.now();
      svg = renderFn(dsl, w, h);
      const t1 =
        typeof performance !== 'undefined' && typeof performance.now === 'function'
          ? performance.now()
          : Date.now();
      renderTimeStr = formatLatency(t1 - t0);
    } catch (e: any) {
      err = `Render error: ${e.message ?? e}`;
      svg = '';
    }
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

  function onOpenPlayground() {
    page = 'playground';
  }
</script>

<div
  class="flex flex-col bg-base-300"
  class:h-screen={page === 'playground'}
  class:min-h-screen={page !== 'playground'}
>
  <Navbar bind:page />

  {#if page === 'about'}
    <AboutPage />
  {:else if page === 'home'}
    <HomePage {ready} {render} bind:dsl={dsl} {svg} {renderTimeStr} {onOpenPlayground} />
  {:else}
    <div class="flex flex-col md:flex-row flex-1 overflow-hidden gap-px bg-base-300">
      <EditorPanel bind:dsl bind:w bind:h {ready} {render} />
      <PreviewPanel {svg} {err} {renderTimeStr} lightBackground={lightBackground()} />
    </div>
  {/if}
</div>