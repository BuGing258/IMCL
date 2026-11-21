<script lang="ts">
  import type { Component, Snippet } from "svelte";

  let {
    icon: Icon,
    title,
    sub = "",
    class: cls = "",
    i = 0,
    onclick,
    children,
  }: {
    icon: Component<any>;
    title: string;
    sub?: string;
    class?: string;
    /** 入场动画的顺序 */
    i?: number;
    /** 不传就是一张只展示信息、不能点的卡片 */
    onclick?: () => void;
    children?: Snippet;
  } = $props();
</script>

<svelte:element this={onclick ? "button" : "div"} class="tile {cls}" class:static={!onclick} style:--i={i} {onclick} role={onclick ? undefined : "group"}>
  <span class="ic"><Icon size={20} /></span>
  <span class="wm" aria-hidden="true"><Icon size={108} strokeWidth={1.25} /></span>
  <div class="body">
    <div class="head">
      <b>{title}</b>
      {#if sub}<span class="sub">{sub}</span>{/if}
    </div>
    {#if children}{@render children()}{/if}
  </div>
</svelte:element>

<style>
  .tile {
    position: relative;
    overflow: hidden;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 14px 16px;
    border-radius: var(--r-lg);
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    text-align: left;
    transition: transform 0.25s var(--ease), border-color 0.2s var(--ease), box-shadow 0.25s var(--ease);
  }
  .tile:not(.static):hover {
    transform: translateY(-3px);
    border-color: var(--red);
    box-shadow: var(--shadow);
  }
  .tile:not(.static):active {
    transform: translateY(-1px) scale(0.99);
  }
  .ic {
    color: var(--text-2);
    display: flex;
    transition: color 0.2s var(--ease);
    position: relative;
  }
  .tile:not(.static):hover .ic {
    color: var(--red);
  }
  .wm {
    position: absolute;
    right: -14px;
    bottom: -22px;
    color: var(--wm);
    transform: rotate(-10deg);
    transition: transform 0.5s var(--ease), color 0.3s var(--ease);
    pointer-events: none;
    display: flex;
  }
  .tile:not(.static):hover .wm {
    transform: rotate(-4deg) scale(1.06);
    color: var(--red-soft);
  }
  .body {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  b {
    font-size: 16px;
    font-weight: 700;
  }
  .sub {
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .c2 {
    grid-column: span 2;
  }
  .r2 {
    grid-row: span 2;
  }
</style>
