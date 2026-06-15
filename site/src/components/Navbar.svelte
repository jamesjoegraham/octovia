<script lang="ts">
  type Page = 'home' | 'playground' | 'about';

  let {
    page = $bindable(),
    ready,
  }: {
    page: Page;
    ready: boolean;
  } = $props();
</script>

<div class="navbar bg-base-100 border-b border-base-300 min-h-12 px-3 gap-2">
  <button
    class="btn btn-lg gap-4 flex items-center bg-transparent border-0 shadow-none hover:bg-transparent hover:border-0 hover:shadow-none focus:bg-transparent focus:outline-none focus-visible:outline-none active:bg-transparent"
    onclick={() => (page = 'home')}
  >
    <img src="./favicon.svg" alt="octovia" class="w-12 h-12" />
    <span class="font-semibold tracking-tight">Octovia</span>
  </button>

  <div class="join gap-0 ml-3">
    <button
      class="btn btn-md join-item {page === 'home' ? 'btn-active' : 'btn-ghost'}"
      onclick={() => (page = 'home')}
    >Home</button>
    <button
      class="btn btn-md join-item {page === 'playground' ? 'btn-active' : 'btn-ghost'}"
      onclick={() => (page = 'playground')}
    >Playground</button>
    <button
      class="btn btn-md join-item {page === 'about' ? 'btn-active' : 'btn-ghost'}"
      onclick={() => (page = 'about')}
    >About</button>
  </div>

  {#if page === 'playground' || page === 'home'}
    <div class="ml-auto flex items-center gap-2">
      {#if ready}
        <div class="w-2 h-2 m-2 rounded-full bg-success shadow-[0_0_6px_oklch(var(--suc))]"></div>
      {:else}
        <span class="loading loading-spinner loading-xs text-base-content/50"></span>
      {/if}
    </div>
  {/if}
</div>
