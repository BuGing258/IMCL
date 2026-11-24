<script lang="ts">
  // 点一个按钮弹出的一层（操作菜单、下载进度面板）。和下拉框（Select）用同一套：
  // 挂在 body 上、分层（阴影 / 裁剪 / 材质）、WAAPI 滑进滑出（motion.ts 的 dropSheet）、
  // 背景和控件都是云母时在下面挖洞透出云母（micaHole.ts）；控件材质是自定义时按自定义的不透明度和模糊。
  // 点外面、按 Esc、改窗口大小都会关掉；Esc 不会顺带让页面返回。
  import { tick, type Snippet } from "svelte";
  import { dropSheet } from "$lib/motion";
  import { micaHole, portal } from "$lib/micaHole";

  interface Props {
    /** 和按钮左边对齐（start）还是右边对齐（end） */
    align?: "start" | "end";
    /** 弹出层宽度，比如 "360px"；不给就按内容 */
    width?: string;
    /** 弹出层里的内边距 */
    pad?: string;
    label?: string;
    role?: "menu" | "dialog";
    open?: boolean;
    trigger: Snippet<[{ open: boolean; toggle: () => void }]>;
    children: Snippet<[{ close: () => void }]>;
  }
  let { align = "start", width, pad = "6px", label, role = "menu", open = $bindable(false), trigger, children }: Props = $props();

  let anchor = $state<HTMLSpanElement>();
  let popEl = $state<HTMLDivElement>();
  /** 在不在页面上：关的时候 open 先变 false，滑走以后才拿掉 */
  let shown = $state(false);
  let pos = $state({ left: 0, top: 0, up: false, maxHeight: 480 });

  function place() {
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    const below = window.innerHeight - r.bottom - 12;
    const above = r.top - 12;
    const up = below < 200 && above > below;
    pos = { left: r.left, top: up ? r.top - 6 : r.bottom + 6, up, maxHeight: Math.max(160, Math.min(560, up ? above : below)) };
  }

  /** 量到真宽度后再定左右：右对齐、伸出窗口时挪回来。在第一帧画出来之前做 */
  function fit() {
    if (!popEl || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const w = popEl.offsetWidth;
    let left = align === "end" ? r.right - w : r.left;
    left = Math.max(8, Math.min(left, window.innerWidth - 8 - w));
    pos.left = left;
  }

  async function show() {
    place();
    open = shown = true;
    await tick();
    fit();
  }
  function close() {
    open = false;
  }
  function toggle() {
    if (open) close();
    else show();
  }

  // 外面改了 open（比如别处让它关掉）
  $effect(() => {
    if (open && !shown) show();
  });

  $effect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!popEl?.contains(t) && !anchor?.contains(t)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // 捕获阶段拦下：只关弹出层，不让页面返回
      e.preventDefault();
      e.stopPropagation();
      close();
    };
    const onResize = () => close();
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<span class="anchor" bind:this={anchor}>{@render trigger({ open, toggle })}</span>

{#if shown}
  <div
    use:portal
    use:dropSheet={{ open, up: pos.up, onclosed: () => (shown = false) }}
    bind:this={popEl}
    inert={!open}
    class="pop"
    class:up={pos.up}
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:width
  >
    <div class="shade-clip"><div class="shade"></div></div>
    <div class="clip" use:micaHole>
      <div class="sheet" {role} aria-label={label} style:padding={pad} style:max-height="{pos.maxHeight}px">
        {@render children({ close })}
      </div>
    </div>
  </div>
{/if}

<style>
  .anchor {
    display: inline-flex;
  }
  .pop {
    position: fixed;
    z-index: 1000;
    max-width: calc(100vw - 16px);
    font-family: var(--font);
    color: var(--text);
  }
  .pop.up {
    transform: translateY(-100%);
  }
  /* 阴影层：和 sheet 一起滑，套在不动的 shade-clip 里（原因见 motion.ts） */
  .shade-clip {
    position: absolute;
    inset: 0 -60px -60px -60px;
    overflow: hidden;
    pointer-events: none;
  }
  .pop.up .shade-clip {
    inset: -60px -60px 0 -60px;
  }
  .shade {
    position: absolute;
    top: 0;
    left: 60px;
    right: 60px;
    bottom: 60px;
    border-radius: var(--r-lg);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.18), 0 2px 6px rgba(0, 0, 0, 0.08);
  }
  .pop.up .shade {
    top: 60px;
    bottom: 0;
  }
  .clip {
    position: relative;
    border-radius: var(--r-lg);
    overflow: hidden;
  }
  /* 控件材质：和卡片一样的不透明度、模糊 */
  .sheet {
    overflow-y: auto;
    overscroll-behavior: contain;
    border-radius: var(--r-lg);
    background: var(--surface);
    backdrop-filter: var(--float-filter);
    border: 1px solid var(--border-strong);
  }
  :global(:root[data-pop="hole"]) .sheet {
    backdrop-filter: none;
  }

  /* 菜单项：用在 role="menu" 的弹出层里 */
  .sheet :global(.menu-item) {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 8px;
    font-size: 13px;
    text-align: left;
    white-space: nowrap;
    color: var(--text);
    transition: background 0.12s var(--ease);
  }
  .sheet :global(.menu-item:hover),
  .sheet :global(.menu-item:focus-visible) {
    background: var(--hover);
    outline: none;
  }
  .sheet :global(.menu-item .mi-ic) {
    flex-shrink: 0;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--red-soft);
    color: var(--red);
  }
  .sheet :global(.menu-item .mi-text) {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .sheet :global(.menu-item .mi-text small) {
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
