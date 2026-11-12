<script lang="ts">
  // 安装 / 重新安装前选资源的版本（只列适合目标游戏版本和加载器的）
  import { fade, scale } from "svelte/transition";
  import { X, LoaderCircle, RefreshCw, Check, Download as DownloadIcon, Info } from "@lucide/svelte";
  import { api, type Hit, type ModVersion, type ResKindApi } from "$lib/api";
  import { fmtCount } from "$lib/state.svelte";

  let {
    hit,
    kind,
    mc,
    loader,
    target,
    installedVersion = null,
    onclose,
    onpick,
  }: {
    hit: Hit;
    kind: ResKindApi;
    mc: string;
    loader: string | null;
    /** 显示用：安装到哪个版本 */
    target: string;
    /** 已安装的版本号；没装时为 null */
    installedVersion?: string | null;
    onclose: () => void;
    onpick: (v: ModVersion) => void;
  } = $props();

  const TYPE: Record<ModVersion["versionType"], [string, string]> = {
    release: ["正式版", "green"],
    beta: ["测试版", "amber"],
    alpha: ["开发版", "red"],
  };

  let list = $state<ModVersion[]>([]);
  let loading = $state(true);
  let err = $state("");
  let selected = $state<string | null>(null);
  let onlyRelease = $state(false);

  const reinstall = $derived(installedVersion != null);
  const shown = $derived(onlyRelease ? list.filter((v) => v.versionType === "release") : list);
  const pick = $derived(list.find((v) => v.id === selected) ?? null);
  const hasPre = $derived(list.some((v) => v.versionType !== "release"));

  // 只看正式版时，选中的版本被藏起来了就换成第一个
  $effect(() => {
    if (shown.length && !shown.some((v) => v.id === selected)) selected = shown[0].id;
  });

  async function load() {
    loading = true;
    err = "";
    try {
      list = await api.resourceVersions({ projectId: hit.id, kind, mc, loader });
      // 默认选最新的正式版，没有正式版就选最新的
      selected = (list.find((v) => v.versionType === "release") ?? list[0])?.id ?? null;
    } catch (e) {
      err = String(e);
    } finally {
      loading = false;
    }
  }
  load();

  const date = (s: string) => s.slice(0, 10);
  const size = (b: number) => (b >= 1048576 ? `${(b / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`);
  const same = (v: ModVersion) => !!installedVersion && v.versionNumber === installedVersion;

  function confirm() {
    if (pick) onpick(pick);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    } else if (e.key === "Enter" && pick && !(e.target instanceof HTMLButtonElement)) {
      confirm();
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && shown.length) {
      e.preventDefault();
      const i = shown.findIndex((v) => v.id === selected);
      const n = Math.max(0, Math.min(shown.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
      selected = shown[n].id;
      document.getElementById(`rv-${shown[n].id}`)?.scrollIntoView({ block: "nearest" });
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="mask" transition:fade={{ duration: 150 }} onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="rv-title"
    tabindex="-1"
    transition:scale={{ start: 0.96, duration: 180 }}
    onclick={(e) => e.stopPropagation()}
  >
    <header>
      {#if hit.icon}<img class="ic" src={hit.icon} alt="" />{:else}<span class="ic ph">{hit.title[0]}</span>{/if}
      <div class="ht">
        <h2 id="rv-title">{reinstall ? "重新安装" : "安装"} {hit.title}</h2>
        <p>装到 {target} · 只列出适合 {mc}{loader ? ` ${loader}` : ""} 的版本</p>
      </div>
      <button class="btn btn-sm btn-icon" onclick={onclose} aria-label="关闭"><X size={15} /></button>
    </header>

    {#if reinstall}
      <div class="installed">
        <Check size={14} />
        <span>已安装 <b>{installedVersion || "（版本未知）"}</b>。重新安装会用选中的版本替换它，旧文件会移到回收站。</span>
      </div>
    {/if}

    <div class="list" role="listbox" aria-label="选择版本">
      {#if loading}
        <p class="state"><LoaderCircle size={16} class="spin" />正在读取版本列表…</p>
      {:else if err}
        <div class="state">
          <span class="err-text">{err}</span>
          <button class="btn btn-sm" onclick={load}><RefreshCw size={14} />重试</button>
        </div>
      {:else if !shown.length}
        <p class="state">没有适合 {mc} 的版本</p>
      {:else}
        {#each shown as v (v.id)}
          {@const [tl, tc] = TYPE[v.versionType] ?? TYPE.release}
          <button
            id="rv-{v.id}"
            class="row"
            class:on={v.id === selected}
            role="option"
            aria-selected={v.id === selected}
            onclick={() => (selected = v.id)}
            ondblclick={() => ((selected = v.id), confirm())}
          >
            <span class="radio"></span>
            <span class="vn">
              <b>{v.versionNumber}</b>
              {#if v.name && v.name !== v.versionNumber}<small>{v.name}</small>{/if}
            </span>
            {#if same(v)}<span class="badge green">已安装</span>{/if}
            <span class="badge {tc}">{tl}</span>
            <span class="meta">{date(v.published)}</span>
            <span class="meta dl"><DownloadIcon size={11} />{fmtCount(v.downloads)}</span>
          </button>
        {/each}
      {/if}
    </div>

    <footer>
      {#if hasPre}
        <label class="only">
          <input type="checkbox" bind:checked={onlyRelease} />只看正式版
        </label>
      {:else if pick}
        <span class="faint"><Info size={12} />{pick.file} · {size(pick.size)}</span>
      {/if}
      <span class="sp"></span>
      <button class="btn btn-sm" onclick={onclose}>取消</button>
      <button class="btn btn-sm btn-primary" disabled={!pick} onclick={confirm}>
        {#if pick && same(pick)}重新安装这个版本{:else if reinstall}换成 {pick?.versionNumber ?? "…"}{:else}安装 {pick?.versionNumber ?? ""}{/if}
      </button>
    </footer>
  </div>
</div>

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
    width: min(600px, 100%);
    max-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.25);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ic {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    flex-shrink: 0;
    object-fit: cover;
    background: var(--hover);
  }
  .ph {
    display: grid;
    place-items: center;
    font-weight: 800;
    color: var(--text-2);
  }
  .ht {
    flex: 1;
    min-width: 0;
  }
  h2 {
    font-size: 16px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ht p {
    font-size: 12px;
    color: var(--text-2);
  }
  .installed {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 12px;
    border-radius: var(--r);
    background: var(--green-soft, var(--hover));
    color: var(--green);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .installed :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .installed span {
    color: var(--text-2);
  }
  .installed b {
    color: var(--text);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: min(340px, 50vh);
    min-height: 120px;
    overflow-y: auto;
    margin: 0 -6px;
    padding: 0 6px;
  }
  .state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 40px 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--r);
    border: 1px solid transparent;
    text-align: left;
    font-size: 12px;
    transition: background 0.12s, border-color 0.12s;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.on {
    background: var(--red-soft);
    border-color: var(--red);
  }
  .radio {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid var(--border-strong);
    flex-shrink: 0;
    transition: border 0.12s;
  }
  .row.on .radio {
    border: 4px solid var(--red);
  }
  .vn {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    overflow: hidden;
  }
  .vn b {
    font-size: 13px;
    font-family: var(--mono);
    white-space: nowrap;
  }
  .vn small {
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    color: var(--text-3);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .dl {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    min-width: 56px;
    justify-content: flex-end;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sp {
    flex: 1;
  }
  .only {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
    cursor: pointer;
  }
  .only input {
    accent-color: var(--red);
  }
  footer .faint {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
