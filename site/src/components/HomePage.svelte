<script lang="ts">
  import MiniPlayground from './MiniPlayground.svelte';
  import CopyIcon from './icons/CopyIcon.svelte';
  import CheckIcon from './icons/CheckIcon.svelte';
  import CloseIcon from './icons/CloseIcon.svelte';

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

  type PkgKey = 'npm' | 'cargo' | 'pip';

  type PkgInfo = {
    key: PkgKey;
    label: string;
    command: string;
    registryLabel: string;
    registryHref: string;
  };

  const packages: PkgInfo[] = [
    {
      key: 'npm',
      label: 'npm',
      command: 'npm install @octovia/octovia',
      registryLabel: '@octovia/octovia',
      registryHref: 'https://www.npmjs.com/package/@octovia/octovia',
    },
    {
      key: 'cargo',
      label: 'cargo',
      command: 'cargo add octovia',
      registryLabel: 'octovia',
      registryHref: 'https://crates.io/crates/octovia',
    },
    {
      key: 'pip',
      label: 'pip',
      command: 'pip install octovia',
      registryLabel: 'octovia',
      registryHref: 'https://pypi.org/project/octovia/',
    },
  ];

  let activeKey: PkgKey = $state('npm');
  let active = $derived(packages.find((p) => p.key === activeKey) ?? packages[0]);

  let copyState: 'idle' | 'copied' | 'error' = $state('idle');
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  function flashCopyState(state: 'copied' | 'error') {
    copyState = state;
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copyState = 'idle';
      copyTimer = null;
    }, 1500);
  }

  function selectPackage(key: PkgKey) {
    if (activeKey === key) return;
    activeKey = key;
    // reset feedback so the new command's button starts clean
    if (copyTimer) {
      clearTimeout(copyTimer);
      copyTimer = null;
    }
    copyState = 'idle';
  }

  async function copyInstall() {
    const cmd = active.command;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(cmd);
      } else {
        const ta = document.createElement('textarea');
        ta.value = cmd;
        ta.setAttribute('readonly', '');
        ta.style.position = 'absolute';
        ta.style.left = '-9999px';
        document.body.appendChild(ta);
        ta.select();
        const ok = document.execCommand('copy');
        ta.remove();
        if (!ok) throw new Error('copy command failed');
      }
      flashCopyState('copied');
    } catch (e) {
      console.error('Copy install command failed:', e);
      flashCopyState('error');
    }
  }
</script>

<div class="flex-1 overflow-auto">
  <div class="max-w-5xl mx-auto px-6 py-10 md:py-14">
    <!-- Hero -->
    <header class="flex items-start justify-between gap-6 mb-6">
      <div class="min-w-0">
        <h1 class="text-4xl md:text-5xl font-semibold tracking-tight text-base-content">
          Octovia
        </h1>
        <p class="mt-3 text-base md:text-lg text-base-content/70 max-w-2xl leading-relaxed">
          A tiny text DSL becomes a crisp, transit-map style state diagram. The whole
          engine — parser, layout, edge routing, SVG output — is one self-contained Rust
          crate. This page runs the WebAssembly build entirely in your browser.
        </p>
      </div>
      <img
        src="./favicon.svg"
        alt="octovia"
        class="hidden sm:block w-20 h-20 md:w-24 md:h-24 shrink-0"
      />
    </header>

    <!-- Install command with package-manager tabs -->
    <section
      class="mb-10 flex flex-col sm:flex-row sm:items-end gap-3"
      aria-label="Install Octovia"
    >
      <div class="min-w-0 sm:w-auto">
        <div role="tablist" aria-label="Package manager" class="flex gap-1 mb-1.5">
          {#each packages as pkg (pkg.key)}
            <button
              type="button"
              role="tab"
              aria-selected={activeKey === pkg.key}
              aria-controls="install-command"
              class={[
                'px-2.5 py-1 text-xs font-mono rounded-md border transition-colors',
                activeKey === pkg.key
                  ? 'bg-base-300 border-base-content/30 text-base-content'
                  : 'bg-base-200 border-base-300 text-base-content/60 hover:bg-base-300 hover:text-base-content',
              ]}
              onclick={() => selectPackage(pkg.key)}
            >{pkg.label}</button>
          {/each}
        </div>

        <div
          id="install-command"
          role="tabpanel"
          class="flex items-stretch rounded-lg border border-base-300 bg-base-200 overflow-hidden font-mono text-sm shadow-sm"
        >
          <div
            class="flex items-center px-3 select-none text-base-content/40 border-r border-base-300 bg-base-300/40"
            aria-hidden="true"
          >$</div>
          <code
            class="flex-1 sm:flex-none px-3 py-2 overflow-x-auto whitespace-nowrap text-base-content"
          >{active.command}</code>
          <button
            type="button"
            class="btn btn-sm btn-ghost rounded-none border-l border-base-300 px-3 h-auto min-h-0 py-2"
            class:text-success={copyState === 'copied'}
            class:text-error={copyState === 'error'}
            title={copyState === 'copied'
              ? 'Copied!'
              : copyState === 'error'
                ? 'Copy failed'
                : 'Copy install command'}
            aria-label="Copy install command"
            onclick={copyInstall}
          >
            {#if copyState === 'copied'}
              <CheckIcon />
            {:else if copyState === 'error'}
              <CloseIcon />
            {:else}
              <CopyIcon />
            {/if}
          </button>
        </div>
      </div>

      <span class="text-xs text-base-content/50 sm:ml-auto sm:pb-1">
        on {active.label} —
        <a
          href={active.registryHref}
          target="_blank"
          rel="noopener noreferrer"
          class="link link-hover"
        >{active.registryLabel}</a>
      </span>
    </section>

    <!-- Mini playground taster -->
    <MiniPlayground {ready} {renderSvg} {onOpenPlayground} />

    <div class="flex justify-end mt-4">

      <button
      type="button"
      onclick={onOpenPlayground}
      class="btn btn-sm btn-neutral self-start sm:self-auto"
    >Open the full playground →</button>

    </div>
    <p class="mt-3 text-xs text-base-content/50">
      Edit the DSL above — diagrams re-render live. For width/height, downloads, and the
      full example library, head over to the
      <button
        type="button"
        class="link link-hover text-base-content/70"
        onclick={onOpenPlayground}
      >Playground</button>.
    </p>
  </div>
</div>
