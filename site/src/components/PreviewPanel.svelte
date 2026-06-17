<script lang="ts">
  import CheckIcon from './icons/CheckIcon.svelte';
  import CloseIcon from './icons/CloseIcon.svelte';
  import CopyIcon from './icons/CopyIcon.svelte';
  import DownloadIcon from './icons/DownloadIcon.svelte';

  let {
    svg,
    err,
    renderTimeStr,
    lightBackground,
  }: {
    svg: string;
    err: string;
    renderTimeStr: string | null;
    lightBackground: boolean;
  } = $props();

  let showRawSvg = $state(false);
  let copyState = $state<'idle' | 'copied' | 'downloaded' | 'error'>('idle');
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  const canCopyToClipboard =
    typeof navigator !== 'undefined' &&
    !!navigator.clipboard &&
    typeof (navigator.clipboard as Clipboard).write === 'function' &&
    typeof window !== 'undefined' &&
    typeof window.ClipboardItem !== 'undefined';

  function flashCopyState(next: 'copied' | 'downloaded' | 'error') {
    copyState = next;
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copyState = 'idle';
      copyTimer = null;
    }, 1500);
  }

  function parseSvgSize(source: string): { width: number; height: number } {
    const widthMatch = source.match(/<svg[^>]*\swidth="([\d.]+)"/);
    const heightMatch = source.match(/<svg[^>]*\sheight="([\d.]+)"/);
    let width = widthMatch ? parseFloat(widthMatch[1]) : 0;
    let height = heightMatch ? parseFloat(heightMatch[1]) : 0;

    if (!width || !height) {
      const viewBoxMatch = source.match(/<svg[^>]*\sviewBox="([\d.\s-]+)"/);
      if (viewBoxMatch) {
        const parts = viewBoxMatch[1].trim().split(/\s+/).map(parseFloat);
        if (parts.length === 4) {
          width = width || parts[2];
          height = height || parts[3];
        }
      }
    }

    return {
      width: width || 1100,
      height: height || 750,
    };
  }

  async function rasterize(scale = 2): Promise<Blob> {
    const { width, height } = parseSvgSize(svg);
    const blob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    try {
      const img = new Image();
      img.src = url;
      await img.decode();
      const canvas = document.createElement('canvas');
      canvas.width = Math.max(1, Math.round(width * scale));
      canvas.height = Math.max(1, Math.round(height * scale));
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('Canvas 2D context unavailable');
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      return await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob((b) => {
          if (b) resolve(b);
          else reject(new Error('Failed to encode PNG'));
        }, 'image/png');
      });
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  function downloadBlob(blob: Blob, filename: string) {
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 0);
  }

  async function copyImage() {
    if (!svg) return;
    try {
      const png = await rasterize();
      if (canCopyToClipboard) {
        await navigator.clipboard.write([
          new ClipboardItem({ 'image/png': png }),
        ]);
        flashCopyState('copied');
      } else {
        downloadBlob(png, 'octovia-diagram.png');
        flashCopyState('downloaded');
      }
    } catch (e) {
      console.error('Copy image failed:', e);
      try {
        const png = await rasterize();
        downloadBlob(png, 'octovia-diagram.png');
        flashCopyState('downloaded');
      } catch {
        flashCopyState('error');
      }
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

</script>

<section class="flex-1 flex flex-col min-w-0 bg-base-200 overflow-hidden">
  {#if err}
    <div class="alert alert-error m-3 p-3 text-sm">{err}</div>
  {:else if svg}
    <div
      class="flex-1 flex items-center justify-center p-4 overflow-auto relative"
      class:bg-base-300={!lightBackground}
      class:bg-white={lightBackground}
    >
      <div class="svg-wrap max-w-full max-h-full">
        {@html svg}
      </div>

      <button
        class="btn btn-sm btn-square absolute top-3 right-3 shadow-md"
        class:btn-success={copyState === 'copied' || copyState === 'downloaded'}
        class:btn-error={copyState === 'error'}
        title={copyState === 'copied'
          ? 'Copied!'
          : copyState === 'downloaded'
            ? 'Downloaded (clipboard unavailable)'
            : copyState === 'error'
              ? 'Failed'
              : canCopyToClipboard
                ? 'Copy image to clipboard (PNG)'
                : 'Download image (PNG) — clipboard requires HTTPS'}
        aria-label={canCopyToClipboard ? 'Copy image to clipboard' : 'Download image'}
        onclick={copyImage}
      >
        {#if copyState === 'copied'}
          <CheckIcon />
        {:else if copyState === 'downloaded'}
          <CheckIcon />
        {:else if copyState === 'error'}
          <CloseIcon />
        {:else if canCopyToClipboard}
          <CopyIcon />
        {:else}
          <DownloadIcon />
        {/if}
      </button>
    </div>

    
  {:else}
    <div class="flex-1 flex items-center justify-center text-base-content/40 text-sm gap-2">
      Enter DSL above, then
      <kbd class="kbd kbd-sm">⌘⏎</kbd>
    </div>
  {/if}

  {#if svg}
    <div class="border-t border-base-300 bg-base-100">
      <button
        class="btn btn-ghost btn-xs w-full justify-start gap-2 rounded-none"
        onclick={() => (showRawSvg = !showRawSvg)}
      >
        {showRawSvg ? '▾' : '▸'} Raw SVG
      </button>
      {#if showRawSvg}
        <pre class="p-3 text-[10px] leading-relaxed overflow-auto max-h-40 bg-base-300 text-base-content/70 font-mono">{svg}</pre>
      {/if}
    </div>
  {/if}

  <!-- Latency / pending indicator (bottom-right) -->
  <div
    class="pointer-events-none absolute bottom-1.5 right-2 text-[10px] font-mono tabular-nums"
    style="color: {lightBackground ? 'rgba(0,0,0,0.45)' : 'rgba(255,255,255,0.45)'}"
  >
    {#if !svg}
      waiting…
    {:else if renderTimeStr !== null}
      {renderTimeStr}
    {/if}
  </div>

</section>

<style>
  :global(.svg-wrap svg) {
    max-width: 100%;
    max-height: 100%;
    border-radius: 8px;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.4);
  }
</style>
