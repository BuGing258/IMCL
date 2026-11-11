<script lang="ts" module>
  export interface SelectOption {
    value: string;
    label: string;
    /** 选项右侧的灰色小字 */
    hint?: string;
    disabled?: boolean;
    /** 分组标题，和上一个选项不同时显示 */
    group?: string;
  }
  /** 列表底部的额外操作，比如“添加文件夹…” */
  export interface SelectAction {
    label: string;
    onclick: () => void;
  }
</script>

<script lang="ts">
  import { ChevronDown, Check, Plus } from "@lucide/svelte";
  import { tick } from "svelte";

  interface Props {
    value: string;
    options: SelectOption[];
    actions?: SelectAction[];
    placeholder?: string;
    /** 触发按钮宽度，比如 "220px" / "100%" */
    width?: string;
    size?: "sm" | "md";
    mono?: boolean;
    disabled?: boolean;
    invalid?: boolean;
    ariaLabel?: string;
    onchange?: (value: string) => void;
  }

  let {
    value = $bindable(),
    options,
    actions = [],
    placeholder = "请选择",
    width,
    size = "md",
    mono = false,
    disabled = false,
    invalid = false,
    ariaLabel,
    onchange,
  }: Props = $props();

  const uid = `sel-${Math.random().toString(36).slice(2, 8)}`;
  let open = $state(false);
  let active = $state(-1);
  let trigger = $state<HTMLButtonElement>();
  let list = $state<HTMLDivElement>();
  let pos = $state({ left: 0, top: 0, width: 0, maxHeight: 280, up: false });

  const selected = $derived(options.find((o) => o.value === value));
  /** 键盘可以走到的条目：选项 + 底部操作 */
  const count = $derived(options.length + actions.length);
  const enabledAt = (i: number) => i >= options.length || !options[i].disabled;

  function place() {
    if (!trigger) return;
    const r = trigger.getBoundingClientRect();
    const below = window.innerHeight - r.bottom - 8;
    const above = r.top - 8;
    const want = Math.min(320, count * 34 + 12 + (actions.length ? 9 : 0));
    const up = below < Math.min(want, 180) && above > below;
    pos = {
      left: Math.min(r.left, window.innerWidth - Math.max(r.width, 160) - 8),
      top: up ? r.top - 4 : r.bottom + 4,
      width: r.width,
      maxHeight: Math.max(120, Math.min(320, up ? above : below)),
      up,
    };
  }

  async function show() {
    if (disabled || open) return;
    place();
    open = true;
    active = Math.max(0, options.findIndex((o) => o.value === value));
    await tick();
    scrollToActive();
  }

  function hide(focus = true) {
    if (!open) return;
    open = false;
    if (focus) trigger?.focus();
  }

  function choose(i: number) {
    if (i >= options.length) {
      hide();
      actions[i - options.length].onclick();
      return;
    }
    const o = options[i];
    if (!o || o.disabled) return;
    hide();
    if (o.value !== value) {
      value = o.value;
      onchange?.(o.value);
    }
  }

  function move(delta: number) {
    if (!count) return;
    let i = active;
    for (let n = 0; n < count; n++) {
      i = (i + delta + count) % count;
      if (enabledAt(i)) break;
    }
    active = i;
    scrollToActive();
  }

  function scrollToActive() {
    list?.querySelector<HTMLElement>(`[data-i="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onKey(e: KeyboardEvent) {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "Home":
        e.preventDefault();
        active = -1;
        move(1);
        break;
      case "End":
        e.preventDefault();
        active = count;
        move(-1);
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        if (active >= 0) choose(active);
        break;
      case "Escape":
        // 不要让页面把 Esc 当成“返回”
        e.preventDefault();
        e.stopPropagation();
        hide();
        break;
      case "Tab":
        hide(false);
        break;
    }
  }

  /** 把弹出层挂到 body 上，不被父元素的 overflow 裁掉 */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  // 打开时：点外面、滚动页面、改变窗口大小都会关掉
  $effect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!list?.contains(t) && !trigger?.contains(t)) hide(false);
    };
    const onScroll = (e: Event) => {
      if (!list?.contains(e.target as Node)) hide(false);
    };
    const onResize = () => hide(false);
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<button
  bind:this={trigger}
  type="button"
  class="trigger {size}"
  class:open
  class:invalid
  class:mono
  style:width
  {disabled}
  role="combobox"
  aria-label={ariaLabel}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={uid}
  aria-activedescendant={open && active >= 0 ? `${uid}-${active}` : undefined}
  title={selected ? selected.label + (selected.hint ? ` ${selected.hint}` : "") : undefined}
  onclick={() => (open ? hide() : show())}
  onkeydown={onKey}
>
  <span class="val" class:ph={!selected}>
    {selected?.label ?? placeholder}
    {#if selected?.hint}<span class="hint">{selected.hint}</span>{/if}
  </span>
  <ChevronDown size={size === "sm" ? 14 : 15} class="chev" />
</button>

{#if open}
  <div
    use:portal
    bind:this={list}
    id={uid}
    class="pop"
    class:up={pos.up}
    class:mono
    role="listbox"
    tabindex="-1"
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:min-width="{pos.width}px"
    style:max-height="{pos.maxHeight}px"
  >
    {#each options as o, i (o.value)}
      {#if o.group && o.group !== options[i - 1]?.group}
        <div class="group">{o.group}</div>
      {/if}
      <div
        id="{uid}-{i}"
        data-i={i}
        class="opt"
        class:active={active === i}
        class:sel={o.value === value}
        class:dis={o.disabled}
        role="option"
        tabindex="-1"
        aria-selected={o.value === value}
        aria-disabled={o.disabled}
        onpointermove={() => !o.disabled && (active = i)}
        onclick={() => choose(i)}
        onkeydown={() => {}}
      >
        <span class="tick">{#if o.value === value}<Check size={14} />{/if}</span>
        <span class="ol">{o.label}</span>
        {#if o.hint}<span class="hint">{o.hint}</span>{/if}
      </div>
    {:else}
      {#if !actions.length}<div class="empty">没有可选的项</div>{/if}
    {/each}
    {#if actions.length}
      {#if options.length}<div class="sep"></div>{/if}
      {#each actions as a, k}
        {@const i = options.length + k}
        <div
          data-i={i}
          class="opt act"
          class:active={active === i}
          role="option"
          tabindex="-1"
          aria-selected="false"
          onpointermove={() => (active = i)}
          onclick={() => choose(i)}
          onkeydown={() => {}}
        >
          <span class="tick"><Plus size={14} /></span>
          <span class="ol">{a.label}</span>
        </div>
      {/each}
    {/if}
  </div>
{/if}

<style>
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    min-width: 0;
    padding: 0 10px 0 12px;
    border-radius: var(--r);
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--text);
    font-size: 13px;
    text-align: left;
    transition: border-color 0.15s, box-shadow 0.15s, background 0.15s;
  }
  .trigger.sm {
    height: 32px;
    font-size: 13px;
  }
  .trigger:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .trigger:focus-visible,
  .trigger.open {
    outline: none;
    border-color: var(--red);
    box-shadow: 0 0 0 3px var(--red-soft);
  }
  .trigger.invalid {
    border-color: var(--red);
  }
  .trigger:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .val {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .val.ph {
    color: var(--text-3);
  }
  .mono .val,
  .pop.mono .ol {
    font-family: var(--mono);
    font-size: 12px;
  }
  .trigger :global(.chev) {
    flex-shrink: 0;
    color: var(--text-3);
    transition: transform 0.2s var(--ease);
  }
  .trigger.open :global(.chev) {
    transform: rotate(180deg);
    color: var(--red);
  }
  .hint {
    margin-left: 6px;
    font-size: 12px;
    color: var(--text-3);
  }

  .pop {
    position: fixed;
    z-index: 1000;
    max-width: min(480px, calc(100vw - 16px));
    overflow-y: auto;
    padding: 5px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    box-shadow: 0 10px 32px rgba(0, 0, 0, 0.16), 0 2px 6px rgba(0, 0, 0, 0.08);
    font-family: var(--font);
    color: var(--text);
    transform-origin: top center;
    animation: pop-in 0.16s var(--ease);
  }
  .pop.up {
    transform: translateY(-100%);
    transform-origin: bottom center;
    animation-name: pop-in-up;
  }
  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.98);
    }
  }
  @keyframes pop-in-up {
    from {
      opacity: 0;
      transform: translateY(calc(-100% + 4px)) scale(0.98);
    }
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 10px 0 6px;
    border-radius: 7px;
    font-size: 13px;
    white-space: nowrap;
    cursor: default;
  }
  .opt.active {
    background: var(--hover);
  }
  .opt.sel {
    color: var(--red);
    font-weight: 600;
  }
  .opt.sel.active {
    background: var(--red-soft);
  }
  .opt.dis {
    opacity: 0.4;
  }
  .opt.act {
    color: var(--text-2);
  }
  .opt.act.active {
    color: var(--text);
  }
  .tick {
    width: 16px;
    display: inline-flex;
    justify-content: center;
    flex-shrink: 0;
  }
  .ol {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .opt .hint {
    margin-left: 16px;
    font-weight: 400;
  }
  .group {
    padding: 8px 10px 4px 28px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
    letter-spacing: 0.5px;
  }
  .group:first-child {
    padding-top: 4px;
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
  .empty {
    padding: 10px 12px;
    font-size: 12px;
    color: var(--text-3);
  }
</style>
