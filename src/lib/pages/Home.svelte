<script lang="ts">
  import { fly } from "svelte/transition";
  import { tick } from "svelte";
  import {
    Layers,
    UserRound,
    Download,
    Puzzle,
    Settings as SettingsIcon,
    Activity,
    Play,
    Square,
    Pickaxe,
    ChevronsUpDown,
    Check,
  } from "@lucide/svelte";
  import Tile from "$lib/components/Tile.svelte";
  import PackCover from "$lib/components/PackCover.svelte";
  import PixelAvatar from "$lib/components/PixelAvatar.svelte";
  import { nav, library, game, settings, accounts, sysmon, installs, ACCOUNT_LABELS, fmtTime } from "$lib/state.svelte";
  import { api } from "$lib/api";

  let pickerOpen = $state(false);
  let active = $state(0);
  let listEl = $state<HTMLDivElement>();
  const canSwitch = $derived(game.status === "idle");

  const hour = new Date().getHours();
  const greet = hour < 6 ? "夜深了" : hour < 12 ? "早上好" : hour < 18 ? "下午好" : "晚上好";

  const pack = $derived(library.current);
  const account = $derived(accounts.current);

  // 最新正式版（后端有 10 分钟缓存，这里拿不到也不影响）
  let latest = $state("");
  api.manifest(settings.source)
    .then((m) => (latest = m.find((v) => v.type === "release")?.id ?? ""))
    .catch(() => {});

  function onStart() {
    if (!pack && game.status === "idle") nav.go("download");
    else game.toggle();
  }

  const gauges = $derived(
    sysmon.ready
      ? [
          { label: "CPU", pct: sysmon.cpu, text: `${Math.round(sysmon.cpu)}%` },
          {
            label: "内存",
            pct: (sysmon.mem / sysmon.memTotal) * 100,
            text: `${sysmon.mem.toFixed(1)}/${Math.round(sysmon.memTotal)} GB`,
          },
          sysmon.gpu === null
            ? { label: "GPU", pct: 0, text: "读不到" }
            : { label: "GPU", pct: sysmon.gpu, text: `${Math.round(sysmon.gpu)}%` },
        ]
      : [
          { label: "CPU", pct: 0, text: "…" },
          { label: "内存", pct: 0, text: "…" },
          { label: "GPU", pct: 0, text: "…" },
        ],
  );
  const monitorSub = $derived(
    !sysmon.live ? "浏览器预览 · 模拟数据" : game.status === "running" ? "游戏运行中 · 实时" : "实时 · 每秒刷新",
  );
  const level = (p: number) => (p >= 85 ? "hi" : p >= 60 ? "mid" : "lo");

  $effect(() => {
    sysmon.start();
    return () => sysmon.stop();
  });

  async function openPicker() {
    pickerOpen = true;
    active = Math.max(0, library.packs.findIndex((p) => p.id === library.currentId));
    await tick();
    scrollActive("center");
  }

  function scrollActive(block: ScrollLogicalPosition = "nearest") {
    listEl?.querySelector<HTMLElement>(`[data-i="${active}"]`)?.scrollIntoView({ block });
  }

  function choose(id: string) {
    if (!canSwitch) return;
    library.currentId = id;
    pickerOpen = false;
  }

  // 在版本卡片上滚动滚轮：直接切换上一个 / 下一个版本
  let wheelLock = 0;
  function onCardWheel(e: WheelEvent) {
    if (pickerOpen || !canSwitch || library.packs.length < 2) return;
    e.preventDefault();
    const now = performance.now();
    if (now - wheelLock < 120 || Math.abs(e.deltaY) < 4) return;
    wheelLock = now;
    const i = library.packs.findIndex((p) => p.id === library.currentId);
    const n = Math.min(library.packs.length - 1, Math.max(0, i + (e.deltaY > 0 ? 1 : -1)));
    library.currentId = library.packs[n].id;
  }

  function onPickerKey(e: KeyboardEvent) {
    if (!pickerOpen) return;
    const n = library.packs.length;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      active = (active + (e.key === "ArrowDown" ? 1 : -1) + n) % n;
      scrollActive();
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(library.packs[active].id);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      pickerOpen = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    // 只在没有聚焦任何控件时响应 Enter，避免和按钮自身的回车冲突
    if (e.key === "Enter" && e.target === document.body && !pickerOpen && pack) {
      e.preventDefault();
      game.toggle();
    }
  }
</script>

<svelte:window onkeydown={onKey} onclick={() => (pickerOpen = false)} />

<div class="home">
  <section class="left">
    <header class="hello">
      <p class="greet">{greet}，{account.name}</p>
      <p class="faint">今天玩点什么？</p>
    </header>

    <div class="picker-wrap">
      {#if pack}
        <button
          class="pack-card"
          class:open={pickerOpen}
          onclick={(e) => {
            e.stopPropagation();
            if (pickerOpen) pickerOpen = false;
            else openPicker();
          }}
          onkeydown={onPickerKey}
          onwheel={onCardWheel}
          aria-haspopup="listbox"
          aria-expanded={pickerOpen}
          title={canSwitch ? "点击选择版本，也可以在这里滚动滚轮切换" : "游戏运行中，不能切换版本"}
        >
          <PackCover {pack} size={40} />
          <div class="pc-text">
            <b>{pack.name}</b>
            <span>{pack.mc} {pack.loader}{pack.optifine ? " + OptiFine" : ""}{pack.mods.length ? ` · ${pack.mods.length} 个 Mod` : ""}</span>
          </div>
          {#if library.packs.length > 1}
            <span class="pc-count">{library.packs.findIndex((p) => p.id === pack.id) + 1}/{library.packs.length}</span>
          {/if}
          <ChevronsUpDown size={16} />
        </button>
      {:else}
        <button class="pack-card empty" onclick={() => nav.go("download")}>
          <span class="empty-ic"><Download size={18} /></span>
          <div class="pc-text">
            <b>{library.loaded ? "还没有安装版本" : "正在读取版本…"}</b>
            <span>{installs.running ? `正在安装 ${installs.running} 个版本` : "点这里去下载游戏"}</span>
          </div>
        </button>
      {/if}

      {#if pickerOpen && pack}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="picker" transition:fly={{ y: -6, duration: 160 }} onclick={(e) => e.stopPropagation()}>
          <div class="pk-head">
            <span>选择版本</span>
            <span class="faint">共 {library.packs.length} 个</span>
          </div>
          <div class="pk-list" bind:this={listEl} role="listbox" aria-label="选择版本">
            {#each library.packs as p, i (p.id)}
              <button
                class="pk"
                class:on={p.id === library.currentId}
                class:active={i === active}
                data-i={i}
                role="option"
                aria-selected={p.id === library.currentId}
                disabled={!canSwitch}
                onpointermove={() => (active = i)}
                onclick={() => choose(p.id)}
              >
                <PackCover pack={p} size={32} />
                <span class="pk-text">
                  <span class="pk-name">{p.name}</span>
                  <span class="pk-ver">{p.mc} · {p.loader}{p.optifine ? " + OptiFine" : ""}</span>
                </span>
                {#if p.id === library.currentId}<Check size={16} />{/if}
              </button>
            {/each}
          </div>
          <button class="pk more" onclick={() => nav.go("versions")}>管理版本…</button>
        </div>
      {/if}
    </div>

    <button
      class="start"
      class:preparing={game.status === "preparing"}
      class:running={game.status === "running"}
      onclick={onStart}
    >
      <span class="start-icon">
        {#if game.status === "idle" && !pack}
          <Download size={22} />
        {:else if game.status === "idle"}
          <Play size={22} fill="currentColor" />
        {:else}
          <Square size={16} fill="currentColor" />
        {/if}
      </span>
      <span class="start-wm" aria-hidden="true"><Pickaxe size={190} strokeWidth={1.4} /></span>
      <div class="start-text">
        {#if game.status === "idle" && !pack}
          <b>下载游戏</b>
          <span>先装一个版本，装好就能开始</span>
        {:else if game.status === "idle"}
          <b>开始游戏</b>
          <span>以 {account.name} 的身份进入 · 按 Enter 启动</span>
        {:else if game.status === "preparing"}
          <b>{Math.round(game.progress)}%</b>
          <span>{game.step}{game.total > 1 ? ` ${game.done}/${game.total}` : ""}… · 点击取消</span>
          <div class="bar"><div style:width="{game.progress}%"></div></div>
        {:else if game.confirmStop}
          <b>强制关闭？</b>
          <span>再点一次关闭游戏，没保存的进度可能会丢失</span>
        {:else if !game.ready}
          <b><span class="live"></span>正在打开</b>
          <span>游戏窗口马上出现…</span>
        {:else}
          <b><span class="live"></span>游戏运行中</b>
          <span>已游玩 {fmtTime(game.elapsed)} · 点击可强制关闭</span>
        {/if}
      </div>
    </button>
  </section>

  <section class="tiles">
    <Tile
      i={0}
      icon={Layers}
      title="版本管理"
      sub={library.packs.length ? `已安装 ${library.packs.length} 个版本` : "还没有版本"}
      class="c2"
      onclick={() => nav.go("versions")}
    />
    <Tile i={1} icon={UserRound} title="账户管理" sub="共 {accounts.list.length} 个账户" class="r2" onclick={() => nav.go("accounts")}>
      <div class="acc">
        <PixelAvatar name={account.name} size={48} />
        <div class="acc-text">
          <b>{account.name}</b>
          <span>{ACCOUNT_LABELS[account.type]}</span>
        </div>
      </div>
    </Tile>
    <Tile
      i={2}
      icon={Download}
      title="下载游戏"
      sub={installs.running ? `正在安装 ${installs.running} 个` : latest ? `最新 ${latest}` : "原版 · 加载器 · OptiFine"}
      onclick={() => nav.go("download")}
    />
    <Tile i={3} icon={Puzzle} title="Mod" sub="Mod · 光影 · 材质包" onclick={() => nav.go("resources")} />
    <Tile
      i={4}
      icon={SettingsIcon}
      title="设置"
      sub="{settings.memory / 1024} GB 内存"
      onclick={() => nav.go("settings")}
    />
    <Tile i={5} icon={Activity} title="系统状态" sub={monitorSub} class="c2">
      <div class="gauges">
        {#each gauges as g}
          <div class="gauge">
            <div class="g-top">
              <span class="g-label">{g.label}</span>
              <span class="g-val">{g.text}</span>
            </div>
            <div class="g-bar {level(g.pct)}"><div style:width="{g.pct}%"></div></div>
          </div>
        {/each}
      </div>
    </Tile>
  </section>
</div>

<style>
  .home {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.45fr);
    gap: 16px;
    padding: 8px 24px 24px;
  }

  /* ---------- 左栏 ---------- */
  .left {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }
  .hello {
    animation: rise 0.5s var(--ease) backwards;
    padding: 4px 2px 2px;
  }
  .greet {
    font-size: 22px;
    font-weight: 700;
  }
  .hello .faint {
    font-size: 13px;
  }

  .picker-wrap {
    position: relative;
    animation: rise 0.5s var(--ease) 40ms backwards;
  }
  .pack-card {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px 10px 10px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-2);
    transition: border-color 0.2s, box-shadow 0.2s;
  }
  .pack-card:hover,
  .pack-card.open {
    border-color: var(--red);
    box-shadow: var(--shadow);
  }
  .pc-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .pc-text b,
  .pc-text span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pack-card.empty {
    border-style: dashed;
  }
  .empty-ic {
    width: 40px;
    height: 40px;
    border-radius: 9px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--red-soft);
    color: var(--red);
    flex-shrink: 0;
  }
  .pc-text b {
    color: var(--text);
    font-size: 15px;
  }
  .pc-count {
    font-size: 11px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .pc-text span {
    font-size: 12px;
  }

  .picker {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 20;
    padding: 6px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.18);
  }
  .pk-head {
    display: flex;
    justify-content: space-between;
    padding: 4px 8px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .pk-list {
    max-height: min(300px, 46vh);
    overflow-y: auto;
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .pk {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: var(--r);
    font-size: 13px;
    color: var(--red);
    text-align: left;
    flex-shrink: 0;
  }
  .pk.active:not(:disabled) {
    background: var(--hover);
  }
  .pk.on {
    background: var(--red-soft);
  }
  .pk:disabled {
    opacity: 0.5;
  }
  .pk-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .pk-name {
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pk-ver {
    font-size: 11px;
    color: var(--text-3);
  }
  .pk.on .pk-name {
    font-weight: 600;
  }
  .pk.more {
    color: var(--text-2);
    justify-content: center;
    margin-top: 4px;
    border-top: 1px solid var(--border);
    border-radius: 0 0 var(--r) var(--r);
    padding-top: 8px;
  }
  .pk.more:hover {
    background: var(--hover);
  }

  /* ---------- 开始游戏 ---------- */
  .start {
    flex: 1;
    min-height: 0;
    position: relative;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 20px 22px;
    border-radius: var(--r-lg);
    background: var(--red);
    color: #fff;
    box-shadow: 0 10px 28px rgba(239, 50, 61, 0.3);
    transition: background 0.2s, transform 0.25s var(--ease), box-shadow 0.25s;
    animation: rise 0.55s var(--ease) 80ms backwards;
  }
  .start:hover {
    background: var(--red-hover);
    transform: translateY(-2px);
    box-shadow: 0 14px 34px rgba(239, 50, 61, 0.38);
  }
  .start:active {
    transform: scale(0.985);
  }
  .start-icon {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    z-index: 1;
    transition: transform 0.25s var(--ease);
  }
  .start:hover .start-icon {
    transform: scale(1.08);
  }
  .start-wm {
    position: absolute;
    right: -36px;
    bottom: -40px;
    color: rgba(255, 255, 255, 0.16);
    transform: rotate(-8deg);
    transform-origin: 70% 70%;
    transition: transform 0.5s var(--ease);
    display: flex;
    pointer-events: none;
  }
  .start:hover .start-wm {
    transform: rotate(-18deg) scale(1.04);
  }
  .preparing .start-wm {
    animation: swing 0.7s ease-in-out infinite alternate;
  }
  @keyframes swing {
    from {
      transform: rotate(-30deg);
    }
    to {
      transform: rotate(6deg);
    }
  }
  .start-text {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .start-text b {
    font-size: 32px;
    font-weight: 800;
    letter-spacing: 1px;
    line-height: 1.15;
    display: flex;
    align-items: center;
    gap: 10px;
    font-variant-numeric: tabular-nums;
  }
  .start-text > span {
    font-size: 13px;
    opacity: 0.88;
    font-variant-numeric: tabular-nums;
  }
  .bar {
    margin-top: 10px;
    height: 5px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.25);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: #fff;
    border-radius: 3px;
    transition: width 0.1s linear;
  }
  .live {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #fff;
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
      transform: scale(0.8);
    }
  }

  /* ---------- 右侧卡片 ---------- */
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    grid-template-rows: repeat(3, minmax(0, 1fr));
    gap: 12px;
    min-height: 0;
  }

  .acc {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 4px;
    border-top: 1px solid var(--border);
  }
  .acc :global(.px) {
    margin-top: 8px;
  }
  .acc-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .acc-text b {
    font-size: 15px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .acc-text span {
    font-size: 12px;
    color: var(--green);
  }

  .gauges {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }
  .gauge {
    min-width: 0;
  }
  .g-top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 4px;
    margin-bottom: 5px;
  }
  .g-label {
    font-size: 12px;
    color: var(--text-2);
  }
  .g-val {
    font-size: 12px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .g-bar {
    height: 5px;
    border-radius: 3px;
    background: var(--hover);
    overflow: hidden;
  }
  .g-bar div {
    height: 100%;
    border-radius: 3px;
    transition: width 0.9s var(--ease), background 0.4s;
  }
  .g-bar.lo div {
    background: var(--green);
  }
  .g-bar.mid div {
    background: var(--amber);
  }
  .g-bar.hi div {
    background: var(--red);
  }
</style>
