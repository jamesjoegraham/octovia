<script lang="ts">
  import GithubIcon from './icons/GithubIcon.svelte';

  type Page = 'home' | 'playground' | 'about';

  let {
    page = $bindable(),
  }: {
    page: Page;
  } = $props();

  let menuOpen = $state(false);

  function go(p: Page) {
    page = p;
    menuOpen = false;
  }
</script>

<div class="navbar bg-base-100 border-b border-base-300 min-h-12 px-3 gap-2">
  <button
    class="btn btn-lg gap-4 flex items-center bg-transparent border-0 shadow-none hover:bg-transparent hover:border-0 hover:shadow-none focus:bg-transparent focus:outline-none focus-visible:outline-none active:bg-transparent"
    onclick={() => go('home')}
  >
    <img src="./favicon.svg" alt="octovia" class="w-12 h-12" />
    <span class="font-semibold tracking-tight">Octovia</span>
  </button>

  <div class="join gap-0 ml-3 hidden sm:inline-flex">
    <button
      class="btn btn-md join-item {page === 'home' ? 'btn-active' : 'btn-ghost'}"
      onclick={() => go('home')}
    >Home</button>
    <button
      class="btn btn-md join-item {page === 'about' ? 'btn-active' : 'btn-ghost'}"
      onclick={() => go('about')}
    >About</button>
    <button
      class="btn btn-md join-item {page === 'playground' ? 'btn-active' : 'btn-ghost'}"
      onclick={() => go('playground')}
    >Playground</button>
  </div>

  <div class="ml-auto flex items-center gap-2">
    <a
      href="https://github.com/jamesjoegraham/octovia"
      target="_blank"
      rel="noopener noreferrer"
      aria-label="View on GitHub"
      title="View on GitHub"
      class="btn btn-ghost btn-square hidden sm:inline-flex"
    >
      <GithubIcon />
    </a>

    <div class="sm:hidden relative">
      <button
        class="btn btn-ghost btn-square"
        aria-label="Open menu"
        aria-expanded={menuOpen}
        onclick={() => (menuOpen = !menuOpen)}
      >
        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>
      {#if menuOpen}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="fixed inset-0 z-40"
          onclick={() => (menuOpen = false)}
        ></div>
        <ul class="menu menu-sm bg-base-100 rounded-box shadow-lg border border-base-300 absolute right-0 mt-2 w-44 z-50">
          <li><button class={page === 'home' ? 'active' : ''} onclick={() => go('home')}>Home</button></li>
          <li><button class={page === 'about' ? 'active' : ''} onclick={() => go('about')}>About</button></li>
          <li><button class={page === 'playground' ? 'active' : ''} onclick={() => go('playground')}>Playground</button></li>
        </ul>
      {/if}
    </div>
  </div>
</div>
