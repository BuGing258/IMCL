<script lang="ts">
  // 所有弹窗共用的外壳：遮罩、动画、Esc 关闭、点外面关闭
  import type { Snippet } from "svelte";
  import { fade } from "svelte/transition";
  import { smooth, accel, pop, materialPop } from "$lib/motion";

  let {
    onclose,
    width = 600,
    labelledby,
    role = "dialog",
    closable = true,
    active = true,
    children,
  }: {
    onclose: () => void;
    width?: number;
    labelledby?: string;
    role?: "dialog" | "alertdialog";
    /** 忙的时候（比如正在导出）不能关 */
    closable?: boolean;
    /** 叠了好几层时，只有最上面那层响应 Esc */
    active?: boolean;
    children: Snippet;
  } = $props();

  function onKey(e: KeyboardEvent) {
    if (active && e.key === "Escape") {
      e.stopPropagation();
      if (closable) onclose();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<!-- 遮罩和弹窗是兄弟，不是父子：遮罩淡入淡出用的 opacity 会让弹窗的背景模糊在动画期间消失 -->
<div class="layer">
  <div class="mask" in:fade={{ duration: 180, easing: smooth }} out:fade={{ duration: 140, easing: accel }} onclick={() => closable && onclose()}></div>
  <div
    class="dialog"
    {role}
    aria-modal="true"
    aria-labelledby={labelledby}
    tabindex="-1"
    style:width="min({width}px, 100%)"
    in:materialPop={{ start: 0.94, duration: 320, easing: pop }} out:materialPop={{ start: 0.97, duration: 140, easing: accel }}
  >
    {@render children()}
  </div>
</div>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 1500;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }
  .mask {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
  }
  .dialog {
    position: relative;
    isolation: isolate;
    max-height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    border-radius: var(--r-lg);
    background: var(--pop-bg);
    backdrop-filter: var(--pop-filter);
    border: 1px solid var(--border-strong);
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.25);
  }

</style>
