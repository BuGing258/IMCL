<script lang="ts">
  // 全局悬停提示：接管所有元素的 title 属性，换成和界面一致的样式。
  // 用法不变：给按钮写 title="..." 即可；也可以写 data-tip="..."。
  import { onMount } from "svelte";

  let text = $state("");
  let visible = $state(false);
  let pos = $state({ x: 0, y: 0, below: true });
  let bubble = $state<HTMLDivElement>();

  const DELAY = 450;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let target: HTMLElement | null = null;
  /** 有提示时才监听元素被移除（onMount 里设置） */
  let watchRemoval: (on: boolean) => void = () => {};

  /** 把原生 title 挪到 data-tip，避免系统自带的提示框也冒出来 */
  function tipOf(el: HTMLElement) {
    const t = el.getAttribute("title");
    if (t !== null) {
      el.removeAttribute("title");
      if (t) el.dataset.tip = t;
    }
    return el.dataset.tip ?? "";
  }

  function hide() {
    clearTimeout(timer);
    visible = false;
    if (target) watchRemoval(false);
    target = null;
  }

  async function show(el: HTMLElement) {
    if (!el.isConnected || target !== el) return;
    const tip = el.dataset.tip;
    if (!tip) return;
    text = tip;
    const r = el.getBoundingClientRect();
    const below = r.bottom + 40 < window.innerHeight;
    pos = { x: r.left + r.width / 2, y: below ? r.bottom + 6 : r.top - 6, below };
    visible = true;
    // 量出宽度后，别超出窗口左右边
    await Promise.resolve();
    if (!bubble) return;
    const w = bubble.offsetWidth;
    pos.x = Math.min(Math.max(pos.x, w / 2 + 8), window.innerWidth - w / 2 - 8);
  }

  onMount(() => {
    const over = (e: PointerEvent) => {
      const el = (e.target as Element | null)?.closest?.<HTMLElement>("[title], [data-tip]");
      if (!el || el === target) return;
      hide();
      if (!tipOf(el)) return;
      target = el;
      watchRemoval(true);
      timer = setTimeout(() => show(el), DELAY);
    };
    const out = (e: PointerEvent) => {
      if (target && !target.contains(e.relatedTarget as Node | null)) hide();
    };
    window.addEventListener("pointerover", over);
    window.addEventListener("pointerout", out);
    window.addEventListener("pointerdown", hide, true);
    window.addEventListener("wheel", hide, { passive: true });
    window.addEventListener("keydown", hide, true);
    window.addEventListener("blur", hide);
    // 元素被移除（比如切换页面）时也要收起；只在有提示时才盯着，平时不占资源
    const mo = new MutationObserver(() => target && !target.isConnected && hide());
    watchRemoval = (on) => (on ? mo.observe(document.body, { childList: true, subtree: true }) : mo.disconnect());
    return () => {
      window.removeEventListener("pointerover", over);
      window.removeEventListener("pointerout", out);
      window.removeEventListener("pointerdown", hide, true);
      window.removeEventListener("wheel", hide);
      window.removeEventListener("keydown", hide, true);
      window.removeEventListener("blur", hide);
      mo.disconnect();
    };
  });
</script>

{#if visible}
  <div bind:this={bubble} class="tip" class:above={!pos.below} style:left="{pos.x}px" style:top="{pos.y}px" role="tooltip">
    {text}
  </div>
{/if}

<style>
  .tip {
    position: fixed;
    z-index: 2000;
    transform: translateX(-50%);
    max-width: 320px;
    padding: 5px 10px;
    border-radius: 8px;
    background: var(--pop-bg);
    backdrop-filter: var(--pop-filter);
    color: var(--text);
    border: 1px solid var(--border-strong);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.14), 0 1px 3px rgba(0, 0, 0, 0.08);
    font-size: 12px;
    line-height: 1.45;
    white-space: pre-line;
    word-break: break-all;
    pointer-events: none;
    animation: tip-in 0.14s var(--ease);
  }
  .tip.above {
    transform: translate(-50%, -100%);
    animation-name: tip-in-above;
  }
  /* 只移动，不淡入（淡入会让背景模糊在动画期间消失） */
  @keyframes tip-in {
    from {
      transform: translate(-50%, -3px);
    }
  }
  @keyframes tip-in-above {
    from {
      transform: translate(-50%, calc(-100% + 3px));
    }
  }
</style>
