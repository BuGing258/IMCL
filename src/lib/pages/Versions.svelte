<script lang="ts">
  import { fade } from "svelte/transition";
  import { Play, Square, Plus, FolderOpen, PackageOpen, Puzzle, Check, X, Sun, Image } from "@lucide/svelte";
  import PackCover from "$lib/components/PackCover.svelte";
  import { library, game, nav, toast, ui } from "$lib/state.svelte";
  import { marketName, RES_LABELS, type ResKind } from "$lib/mock";

  const TABS: ResKind[] = ["mod", "shader", "resourcepack"];

  let selId = $state(library.currentId);
  let tab = $state<ResKind>("mod");

  const sel = $derived(library.packs.find((p) => p.id === selId) ?? library.packs[0]);
  const isCurrent = $derived(sel.id === library.currentId);
  const enabled = $derived(sel.mods.filter((m) => m.enabled).length);
  const counts = $derived<Record<ResKind, number>>({
    mod: sel.mods.length,
    shader: sel.shaders.length,
    resourcepack: sel.resourcepacks.length,
  });
  const listKey = (k: ResKind) => (k === "shader" ? "shaders" : "resourcepacks") as "shaders" | "resourcepacks";

  function launch() {
    if (game.status !== "idle") {
      if (isCurrent) game.stop();
      return;
    }
    library.currentId = sel.id;
    game.start();
    toast.show(`正在启动 ${sel.name}`);
  }

  function goDownload(kind: ResKind) {
    library.currentId = sel.id;
    ui.resKind = kind;
    ui.resQuery = "";
    nav.go("resources");
  }

  function removeRes(kind: "shader" | "resourcepack", id: string) {
    const key = listKey(kind);
    sel[key] = sel[key].filter((x) => x !== id);
    toast.show(`已移除 ${marketName(id)}`);
  }
</script>

<div class="versions">
  <aside class="list">
    <div class="list-head">
      <h2>版本管理</h2>
      <button class="btn btn-sm" onclick={() => nav.go("download")}><Plus size={14} />安装</button>
    </div>
    <div class="items">
      {#each library.packs as p, idx (p.id)}
        <button class="item" class:on={p.id === selId} style:--i={idx} onclick={() => (selId = p.id)}>
          <PackCover pack={p} size={40} />
          <div class="it-text">
            <b>{p.name}</b>
            <span>{p.mc} · {p.loader}</span>
          </div>
          {#if p.id === library.currentId}<span class="cur" title="当前版本"></span>{/if}
        </button>
      {/each}
    </div>
  </aside>

  <section class="detail">
    {#key selId}
      <div class="detail-inner" in:fade={{ duration: 180 }}>
        <div class="hero">
          <PackCover pack={sel} size={128} />
          <div class="hero-text">
            <p class="label">{isCurrent ? "当前版本" : "版本"}</p>
            <h1>{sel.name}</h1>
            <p class="meta">
              Minecraft {sel.mc} · {sel.loader} · 上次游玩 {sel.lastPlayed}
            </p>
            <div class="actions">
              <button class="btn btn-primary btn-pill" onclick={launch} disabled={game.status !== "idle" && !isCurrent}>
                {#if game.status !== "idle" && isCurrent}<Square size={14} fill="currentColor" />停止{:else}<Play size={14} fill="currentColor" />启动{/if}
              </button>
              {#if !isCurrent}
                <button class="btn btn-pill" onclick={() => (library.currentId = sel.id)} disabled={game.status !== "idle"}>
                  <Check size={14} />设为当前
                </button>
              {/if}
              <button class="btn btn-pill" onclick={() => toast.show("打开版本文件夹（界面演示）")}><FolderOpen size={14} />文件夹</button>
              <button class="btn btn-pill" onclick={() => toast.show(`已导出 ${sel.name}.zip（界面演示）`)}><PackageOpen size={14} />导出</button>
            </div>
          </div>
        </div>

        <div class="tabs-row">
          <div class="seg">
            {#each TABS as k}
              <button class:on={tab === k} onclick={() => (tab = k)}>{RES_LABELS[k]} <span class="cnt">{counts[k]}</span></button>
            {/each}
          </div>
          {#if !(tab === "mod" && sel.loader === "原版")}
            <button class="btn btn-sm" onclick={() => goDownload(tab)}><Plus size={14} />添加{RES_LABELS[tab]}</button>
          {/if}
        </div>

        {#if tab === "mod"}
          {#if sel.mods.length}
            <div class="table">
              <div class="tr th">
                <span>#</span><span>Mod</span><span>版本</span><span>大小</span><span class="r">启用 {enabled}/{sel.mods.length}</span>
              </div>
              {#each sel.mods as m, k (m.id)}
                <div class="tr" class:off={!m.enabled}>
                  <span class="faint">{k + 1}</span>
                  <span class="name">{m.name}</span>
                  <span class="muted mono">{m.version}</span>
                  <span class="muted">{m.size}</span>
                  <span class="r">
                    <button
                      class="switch"
                      class:on={m.enabled}
                      role="switch"
                      aria-checked={m.enabled}
                      aria-label="启用 {m.name}"
                      onclick={() => (m.enabled = !m.enabled)}
                    ></button>
                  </span>
                </div>
              {/each}
            </div>
          {:else}
            <div class="empty">
              <Puzzle size={32} strokeWidth={1.5} />
              <p>{sel.loader === "原版" ? "原版不能安装 Mod，需要装 Fabric、Forge 等加载器的版本" : "这个版本还没有 Mod"}</p>
              {#if sel.loader !== "原版"}
                <button class="btn btn-sm" onclick={() => goDownload("mod")}>去下载 Mod</button>
              {:else}
                <button class="btn btn-sm" onclick={() => nav.go("download")}>去安装带加载器的版本</button>
              {/if}
            </div>
          {/if}
        {:else}
          {@const key = listKey(tab)}
          {#if sel[key].length}
            <div class="res-list">
              {#each sel[key] as id (id)}
                <div class="res">
                  <span class="res-ic">{#if tab === "shader"}<Sun size={16} />{:else}<Image size={16} />{/if}</span>
                  <span class="name">{marketName(id)}</span>
                  <button class="btn btn-sm btn-icon" aria-label="移除 {marketName(id)}" title="移除" onclick={() => removeRes(tab as "shader" | "resourcepack", id)}>
                    <X size={14} />
                  </button>
                </div>
              {/each}
            </div>
          {:else}
            <div class="empty">
              {#if tab === "shader"}<Sun size={32} strokeWidth={1.5} />{:else}<Image size={32} strokeWidth={1.5} />{/if}
              <p>还没有{RES_LABELS[tab]}</p>
              <button class="btn btn-sm" onclick={() => goDownload(tab)}>去下载{RES_LABELS[tab]}</button>
            </div>
          {/if}
        {/if}
      </div>
    {/key}
  </section>
</div>

<style>
  .versions {
    height: 100%;
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr);
  }
  .list {
    border-right: 1px solid var(--border);
    padding: 8px 12px 16px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px 10px;
  }
  .list-head h2 {
    font-size: 18px;
    font-weight: 700;
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: var(--r);
    transition: background 0.15s;
    animation: rise 0.45s var(--ease) backwards;
    animation-delay: calc(var(--i) * 40ms + 60ms);
  }
  .item:hover {
    background: var(--hover);
  }
  .item.on {
    background: var(--active);
  }
  .it-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .it-text b {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .it-text span {
    font-size: 12px;
    color: var(--text-2);
  }
  .cur {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--red);
    flex-shrink: 0;
  }

  .detail {
    min-height: 0;
    overflow-y: auto;
  }
  .detail-inner {
    padding: 12px 28px 28px;
  }
  .hero {
    display: flex;
    align-items: flex-end;
    gap: 22px;
    margin-bottom: 20px;
  }
  .hero-text {
    min-width: 0;
  }
  .label {
    font-size: 12px;
    color: var(--text-2);
  }
  .hero h1 {
    font-size: 30px;
    font-weight: 800;
    line-height: 1.2;
    margin: 2px 0 4px;
  }
  .meta {
    font-size: 12px;
    color: var(--text-2);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
    flex-wrap: wrap;
  }

  .tabs-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }
  .cnt {
    font-size: 11px;
    color: var(--text-3);
    margin-left: 2px;
  }

  .table {
    display: flex;
    flex-direction: column;
  }
  .tr {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) 100px 80px 80px;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    height: 40px;
    border-radius: var(--r);
    font-size: 13px;
    transition: background 0.15s, opacity 0.2s;
  }
  .tr:not(.th):hover {
    background: var(--hover);
  }
  .tr.th {
    height: 30px;
    font-size: 12px;
    color: var(--text-3);
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    margin-bottom: 4px;
  }
  .tr.off .name,
  .tr.off .muted {
    opacity: 0.45;
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
    flex: 1;
    min-width: 0;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  .r {
    display: flex;
    justify-content: flex-end;
  }

  .res-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .res {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: 13px;
  }
  .res-ic {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--red-soft);
    color: var(--red);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 40px 0;
    color: var(--text-3);
    font-size: 13px;
    text-align: center;
  }
</style>
