<script lang="ts">
  import { slide } from "svelte/transition";
  import { Box, FlaskConical, Search, Download as DownloadIcon, Check } from "@lucide/svelte";
  import { library, settings, toast } from "$lib/state.svelte";
  import { mockVersions, type Loader } from "$lib/mock";

  let filter = $state<"release" | "snapshot" | "all">("release");
  let query = $state("");
  let expanded = $state<string | null>(null);
  let loader = $state<Loader>("Fabric");
  let progress = $state<Record<string, number>>({});

  const LOADERS: Loader[] = ["原版", "Fabric", "Forge", "NeoForge", "Quilt"];

  const list = $derived(
    mockVersions.filter(
      (v) => (filter === "all" || v.type === filter) && v.id.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );

  const COLORS = ["#1D9E75", "#7F77DD", "#C98A2B", "#378ADD", "#D85A30", "#D4537E", "#639922"];

  function install(id: string) {
    const chosen = loader;
    expanded = null;
    progress[id] = 0;
    const t = setInterval(() => {
      progress[id] = Math.min(100, progress[id] + 2.5);
      if (progress[id] >= 100) {
        clearInterval(t);
        delete progress[id];
        const name = chosen === "原版" ? `原版 ${id}` : `${id} ${chosen}`;
        library.packs.push({
          id: `v-${Date.now()}`,
          name,
          mc: id,
          loader: chosen,
          color: COLORS[library.packs.length % COLORS.length],
          icon: chosen === "原版" ? "leaf" : "cube",
          lastPlayed: "从未",
          mods: [],
          shaders: [],
          resourcepacks: [],
        });
        toast.show(`已安装 ${name}，可以在版本管理里找到`);
      }
    }, 50);
  }
</script>

<div class="page-scroll">
  <div class="wrap">
    <div class="page-head">
      <div>
        <h1>下载游戏</h1>
        <p>选好版本和加载器，装好后会出现在版本管理里 · 下载源：{settings.source === "bmcl" ? "BMCLAPI 镜像" : "官方源"}</p>
      </div>
    </div>

    <div class="toolbar">
      <div class="search">
        <Search size={16} />
        <input class="input" placeholder="搜索版本，比如 1.20.1" bind:value={query} />
      </div>
      <div class="seg">
        <button class:on={filter === "release"} onclick={() => (filter = "release")}>正式版</button>
        <button class:on={filter === "snapshot"} onclick={() => (filter = "snapshot")}>快照</button>
        <button class:on={filter === "all"} onclick={() => (filter = "all")}>全部</button>
      </div>
    </div>

    <div class="list">
      {#each list as v, idx (v.id)}
        {@const owned = library.packs.filter((pk) => pk.mc === v.id).length}
        {@const p = progress[v.id]}
        <div class="row-wrap" class:open={expanded === v.id} style:--i={idx}>
          <div class="row">
            <span class="vic" class:snap={v.type === "snapshot"}>
              {#if v.type === "release"}<Box size={18} />{:else}<FlaskConical size={18} />{/if}
            </span>
            <div class="vt">
              <b>{v.id}</b>
              <span>{v.type === "release" ? "正式版" : "快照"} · {v.date}</span>
            </div>
            <div class="va">
              {#if owned && p === undefined}
                <span class="badge green"><Check size={12} />已有 {owned} 个</span>
              {/if}
              {#if p !== undefined}
                <div class="prog">
                  <div class="progress"><div style:width="{p}%"></div></div>
                  <span>{Math.round(p)}%</span>
                </div>
              {:else}
                <button class="btn btn-sm" onclick={() => (expanded = expanded === v.id ? null : v.id)}>
                  <DownloadIcon size={14} />{expanded === v.id ? "收起" : "安装"}
                </button>
              {/if}
            </div>
          </div>
          {#if expanded === v.id}
            <div class="opts" transition:slide={{ duration: 200 }}>
              <span class="muted">加载器</span>
              <div class="seg">
                {#each LOADERS as l}
                  <button class:on={loader === l} onclick={() => (loader = l)}>{l}</button>
                {/each}
              </div>
              <button class="btn btn-primary btn-sm" onclick={() => install(v.id)}>开始安装</button>
            </div>
          {/if}
        </div>
      {:else}
        <p class="none">没找到「{query}」，换个关键词试试</p>
      {/each}
    </div>
  </div>
</div>

<style>
  .wrap {
    max-width: 820px;
    margin: 0 auto;
  }
  .toolbar {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-bottom: 12px;
  }
  .search {
    flex: 1;
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-3);
  }
  .search :global(svg) {
    position: absolute;
    left: 12px;
    pointer-events: none;
  }
  .search .input {
    width: 100%;
    padding-left: 36px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row-wrap {
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--border);
    transition: border-color 0.2s;
    animation: rise 0.4s var(--ease) backwards;
    animation-delay: calc(min(var(--i), 10) * 25ms);
  }
  .row-wrap:hover,
  .row-wrap.open {
    border-color: var(--border-strong);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
  }
  .vic {
    width: 36px;
    height: 36px;
    border-radius: var(--r);
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--green-soft);
    color: var(--green);
    flex-shrink: 0;
  }
  .vic.snap {
    background: var(--amber-soft);
    color: var(--amber);
  }
  .vt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .vt b {
    font-size: 14px;
  }
  .vt span {
    font-size: 12px;
    color: var(--text-2);
  }
  .va {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .prog {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 160px;
    font-size: 12px;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
  .prog .progress {
    flex: 1;
  }
  .opts {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 12px 12px 60px;
    font-size: 13px;
    flex-wrap: wrap;
  }
  .opts .btn-primary {
    margin-left: auto;
  }
  .none {
    text-align: center;
    color: var(--text-3);
    padding: 40px 0;
    font-size: 13px;
  }
</style>
