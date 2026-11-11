<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { TriangleAlert, FolderOpen, Copy, X } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { game, toast, nav } from "$lib/state.svelte";

  const p = $derived(game.problem);
  let logEl = $state<HTMLPreElement>();

  // 打开时滚到日志最底部（最后几行最有用）
  $effect(() => {
    if (p?.tail && logEl) logEl.scrollTop = logEl.scrollHeight;
  });

  function close() {
    game.problem = null;
  }

  async function openLog() {
    if (!p?.log) return;
    try {
      await revealItemInDir(p.log);
    } catch (e) {
      toast.show(`打不开：${e}`);
    }
  }

  async function copy() {
    if (!p) return;
    const text = [p.title, p.detail, ...(p.tail ?? [])].join("\n");
    try {
      await navigator.clipboard.writeText(text);
      toast.show("已复制，可以发给朋友帮忙看看");
    } catch {
      toast.show("复制失败");
    }
  }

  function onKey(e: KeyboardEvent) {
    if (p && e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

{#if p}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="mask" transition:fade={{ duration: 150 }} onclick={close}>
    <div
      class="dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="gp-title"
      tabindex="-1"
      transition:scale={{ start: 0.96, duration: 180 }}
      onclick={(e) => e.stopPropagation()}
    >
      <header>
        <span class="ic"><TriangleAlert size={18} /></span>
        <h2 id="gp-title">{p.title}</h2>
        <button class="btn btn-sm btn-icon" onclick={close} aria-label="关闭"><X size={15} /></button>
      </header>
      <p class="detail">{p.detail}</p>
      {#if p.tail?.length}
        <pre bind:this={logEl}>{p.tail.join("\n")}</pre>
      {/if}
      <footer>
        {#if p.log}<button class="btn btn-sm" onclick={openLog}><FolderOpen size={14} />打开日志文件</button>{/if}
        <button class="btn btn-sm" onclick={copy}><Copy size={14} />复制信息</button>
        <span class="sp"></span>
        {#if !p.tail}
          <button class="btn btn-sm" onclick={() => (close(), nav.go("versions"))}>去版本设置</button>
        {/if}
        <button class="btn btn-sm btn-primary" onclick={close}>知道了</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .mask {
    position: fixed;
    inset: 0;
    z-index: 1500;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }
  .dialog {
    width: min(640px, 100%);
    max-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 18px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.25);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .ic {
    width: 32px;
    height: 32px;
    border-radius: 9px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--red-soft);
    color: var(--red);
    flex-shrink: 0;
  }
  h2 {
    flex: 1;
    font-size: 16px;
    font-weight: 700;
  }
  .detail {
    font-size: 13px;
    color: var(--text-2);
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  pre {
    margin: 0;
    min-height: 120px;
    max-height: 300px;
    overflow: auto;
    padding: 10px 12px;
    border-radius: var(--r);
    background: var(--bg);
    border: 1px solid var(--border);
    font-family: var(--mono);
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--text-2);
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .sp {
    flex: 1;
  }
</style>
