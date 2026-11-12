<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { ChevronLeft, Minus, Square, Copy, X } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import { nav, PAGE_TITLES } from "$lib/state.svelte";

  // 在普通浏览器里预览时没有 Tauri，窗口按钮不做任何事
  const win = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window ? getCurrentWindow() : null;

  let maximized = $state(false);

  $effect(() => {
    if (!win) return;
    let unlisten: (() => void) | undefined;
    win.isMaximized().then((v) => (maximized = v));
    win
      .onResized(async () => {
        maximized = await win.isMaximized();
      })
      .then((f) => (unlisten = f));
    return () => unlisten?.();
  });
</script>

<header class="tb" data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    {#if nav.stack.length > 1}
      <button class="back" onclick={() => nav.back()} aria-label="返回" title="返回（Esc）" transition:fly={{ x: -8, duration: 180 }}>
        <ChevronLeft size={18} />
      </button>
    {/if}
    <button class="brand" onclick={() => nav.home()} title="回到首页">IMCL</button>
    {#key nav.current}
      {#if nav.current !== "home"}
        <span class="crumb" data-tauri-drag-region in:fly={{ x: 8, duration: 220 }}>
          <span class="sep" data-tauri-drag-region>/</span>{PAGE_TITLES[nav.current]}
        </span>
      {/if}
    {/key}
  </div>

  <div class="controls">
    <button class="wc" onclick={() => win?.minimize()} aria-label="最小化"><Minus size={16} /></button>
    <button class="wc" onclick={() => win?.toggleMaximize()} aria-label={maximized ? "还原" : "最大化"}>
      {#if maximized}<Copy size={13} />{:else}<Square size={13} />{/if}
    </button>
    <button class="wc close" onclick={() => win?.close()} aria-label="关闭"><X size={16} /></button>
  </div>
</header>

<style>
  .tb {
    height: 40px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 10px;
    position: relative;
    z-index: 50;
  }
  .left {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 100%;
    flex: 1;
    min-width: 0;
  }
  .back {
    width: 28px;
    height: 28px;
    border-radius: var(--r-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-2);
    transition: background 0.15s, color 0.15s;
  }
  .back:hover {
    background: var(--hover);
    color: var(--text);
  }
  .brand {
    font-weight: 800;
    font-size: 14px;
    letter-spacing: 3px;
    color: var(--red);
    padding: 2px 6px;
    border-radius: var(--r-sm);
  }
  .brand:hover {
    background: var(--red-soft);
  }
  .crumb {
    font-size: 13px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .sep {
    margin-right: 8px;
    color: var(--text-3);
  }
  .controls {
    display: flex;
    height: 100%;
  }
  .wc {
    width: 46px;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-2);
    transition: background 0.12s, color 0.12s;
  }
  .wc:hover {
    background: var(--hover);
    color: var(--text);
  }
  .wc.close:hover {
    background: #e81123;
    color: #fff;
  }
</style>
