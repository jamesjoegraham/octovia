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
  let page = $state<Page>('home');

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
      svg = renderFn(dsl, w, h);
    } catch (e: any) {
      err = `Render error: ${e.message ?? e}`;
      svg = '';
    }
  }

  function openPlayground() {
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
    <HomePage {ready} renderSvg={renderFn} onOpenPlayground={openPlayground} />
  {:else}
    <div class="flex flex-1 overflow-hidden gap-px bg-base-300">
      <EditorPanel bind:dsl bind:w bind:h {ready} {render} />
      <PreviewPanel {svg} {err} lightBackground={theme === 'light'} />
    </div>
  {/if}
</div>
