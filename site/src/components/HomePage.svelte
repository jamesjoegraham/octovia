<script lang="ts">
  import MiniPlayground from './MiniPlayground.svelte';
  import CopyIcon from './icons/CopyIcon.svelte';
  import CheckIcon from './icons/CheckIcon.svelte';
  import CloseIcon from './icons/CloseIcon.svelte';
  import GithubIcon from './icons/GithubIcon.svelte';

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
    
    <header class="flex items-start justify-between gap-6 mb-8">
      <div class="min-w-0">
        <div class="flex items-center gap-3 mb-2">
          <h1 class="text-4xl md:text-5xl font-semibold tracking-tight text-base-content">
            Octovia
          </h1>
          <span class="badge badge-primary badge-outline badge-sm mt-2 hidden sm:inline-flex font-mono">
            v0.1.0
          </span>
        </div>
        
        <p class="mt-4 text-base md:text-lg text-base-content/80 max-w-2xl leading-relaxed">
          A bespoke, DOM-free state diagram rendering engine.
        </p>
        <p class="mt-2 text-sm md:text-base text-base-content/60 max-w-2xl leading-relaxed">
          Write a tiny text DSL, get a crisp, transit-map style SVG. No headless browsers, no bezier spaghetti. The entire engine is a single Rust crate built for CI pipelines, static site generators, and the browser.
        </p>
      </div>
      <img
        src="./favicon.svg"
        alt="octovia logo"
        class="hidden sm:block w-20 h-20 md:w-24 md:h-24 shrink-0"
      />
    </header>

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
                'px-3 py-1.5 text-xs font-mono rounded-t-md border-t border-l border-r transition-colors',
                activeKey === pkg.key
                  ? 'bg-base-200 border-base-300 text-base-content'
                  : 'bg-transparent border-transparent text-base-content/50 hover:text-base-content hover:bg-base-200/50',
              ]}
              onclick={() => selectPackage(pkg.key)}
            >{pkg.label}</button>
          {/each}
        </div>

        <div
          id="install-command"
          role="tabpanel"
          class="flex items-stretch rounded-b-lg rounded-tr-lg border border-base-300 bg-base-200 overflow-hidden font-mono text-sm shadow-sm"
        >
          <div
            class="flex items-center px-4 select-none text-base-content/40 border-r border-base-300 bg-base-300/30"
            aria-hidden="true"
          >$</div>
          <code
            class="flex-1 sm:flex-none px-4 py-3 overflow-x-auto whitespace-nowrap text-base-content"
          >{active.command}</code>
          <button
            type="button"
            class="btn btn-sm btn-ghost rounded-none border-l border-base-300 px-4 h-auto min-h-0 py-3 transition-colors"
            class:text-success={copyState === 'copied'}
            class:text-error={copyState === 'error'}
            class:hover:bg-base-300={copyState === 'idle'}
            title={copyState === 'copied' ? 'Copied!' : copyState === 'error' ? 'Copy failed' : 'Copy install command'}
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

      <span class="text-xs text-base-content/50 sm:ml-auto sm:pb-3">
        distributed on
        <a
          href={active.registryHref}
          target="_blank"
          rel="noopener noreferrer"
          class="link link-hover text-base-content/70 font-medium"
        >{active.registryLabel}</a>
      </span>
    </section>

    <div class="shadow-sm border border-base-300 rounded-xl overflow-hidden mb-4">
      <MiniPlayground {ready} {renderSvg} {onOpenPlayground} />
    </div>

    <div class="flex items-center justify-between mt-4 gap-2">
      <p class="text-xs text-base-content/50 max-w-md">
        Diagrams re-render locally via WebAssembly on every keystroke. 
      </p>
      <div class="flex items-center gap-2">
        <a
          href="https://github.com/jamesjoegraham/octovia"
          target="_blank"
          rel="noopener noreferrer"
          class="btn btn-sm btn-ghost gap-2"
        >
          <GithubIcon size={16} />
          GitHub
        </a>
        <button
          type="button"
          onclick={onOpenPlayground}
          class="btn btn-sm btn-neutral"
        >
          Open full playground →
        </button>
      </div>
    </div>
  </div>
</div>