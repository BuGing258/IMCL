<script lang="ts">
  import { fade } from "svelte/transition";
  import { Play, Square, Plus, FolderOpen, PackageOpen, Puzzle, Check, X, Sun, Image, RefreshCw, Layers, SlidersHorizontal, FolderX, Search } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import PackCover from "$lib/components/PackCover.svelte";
  import Select from "$lib/components/Select.svelte";
  import VersionSettings from "$lib/components/VersionSettings.svelte";
  import ExportPackDialog from "$lib/components/ExportPackDialog.svelte";
  import { library, game, nav, toast, ui, settings, gameDirs } from "$lib/state.svelte";
  import { api, fmtRelative } from "$lib/api";
  import { marketName, RES_LABELS, type Pack, type ResKind } from "$lib/mock";

  type Tab = ResKind | "settings";
  const TABS: ResKind[] = ["mod", "shader", "resourcepack"];

  let selId = $state(library.currentId);
  /** 正在导出整合包的版本 */
  let exporting = $state<Pack | null>(null);
  let tab = $state<Tab>("mod");
  let filesErr = $state("");
  let confirmKey = $state<string | null>(null);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  const sel = $derived<Pack | undefined>(library.packs.find((p) => p.id === selId) ?? library.packs[0]);
  const isCurrent = $derived(!!sel && sel.id === library.currentId);
  /** 正在启动 / 运行的就是选中的这个版本 */
  const mine = $derived(!!sel && game.status !== "idle" && game.packId === sel.id);
  const dirOptions = $derived(settings.gameDirs.map((d) => ({ value: d.path, label: d.name, hint: d.path })));
  const dirName_ = $derived(settings.gameDirs.find((d) => d.path === settings.gameDir)?.name ?? "游戏目录");
  const enabled = $derived(sel?.mods.filter((m) => m.enabled).length ?? 0);
  const counts = $derived<Record<ResKind, number>>({
    mod: sel?.mods.length ?? 0,
    shader: sel?.shaders.length ?? 0,
    resourcepack: sel?.resourcepacks.length ?? 0,
  });
  /** 光影 / 材质包的显示名：Modrinth 上认得出来就用正式名字 */
  function resName(kind: ResKind, id: string) {
    return sel?.projects?.[`${dirName(kind)}/${id}`]?.title ?? marketName(id).replace(/\.zip$/i, "");
  }
  /* ---------- 搜索当前标签里的 Mod / 光影 / 材质包 ---------- */
  let q = $state("");
  // 换版本或换标签时清空
  $effect(() => {
    void [sel?.id, tab];
    q = "";
  });
  const needle = $derived(q.trim().toLowerCase());
  const hitText = (...parts: (string | null | undefined)[]) => parts.some((s) => !!s && s.toLowerCase().includes(needle));
  const shownMods = $derived(!sel ? [] : needle ? sel.mods.filter((m) => hitText(m.name, m.id, m.version)) : sel.mods);
  function shownRes(kind: ResKind, ids: string[]) {
    return needle ? ids.filter((id) => hitText(resName(kind, id), id)) : ids;
  }

  const listKey = (k: ResKind) => (k === "shader" ? "shaders" : "resourcepacks") as "shaders" | "resourcepacks";
  const dirName = (k: ResKind) => (k === "mod" ? "mods" : k === "shader" ? "shaderpacks" : "resourcepacks");

  // 选中真实版本时读取它文件夹里的内容
  $effect(() => {
    const p = sel;
    if (!p?.real || p.filesLoaded) return;
    filesErr = "";
    library.loadFiles(p).catch((e) => (filesErr = String(e)));
  });

  function meta(p: Pack) {
    const parts = [`Minecraft ${p.mc}`];
    parts.push(p.loader === "原版" ? "原版" : `${p.loader}${p.loaderVersion ? ` ${p.loaderVersion.replace(`${p.mc}-`, "")}` : ""}`);
    if (p.optifine) parts.push(`OptiFine ${p.optifine.replace("_", " ")}`);
    parts.push(p.real && p.modified ? `更新于 ${fmtRelative(p.modified)}` : `上次游玩 ${p.lastPlayed}`);
    return parts.join(" · ");
  }

  function launch() {
    if (!sel) return;
    if (game.status !== "idle") {
      if (game.packId === sel.id) game.stop();
      return;
    }
    game.start(sel);
  }

  function goDownload(kind: ResKind) {
    if (sel) library.currentId = sel.id;
    ui.resKind = kind;
    ui.resQuery = "";
    nav.go("resources");
  }

  async function openFolder() {
    if (!sel?.dir) return toast.show("打开版本文件夹（演示数据没有真实文件夹）");
    try {
      await revealItemInDir(sel.dir);
    } catch (e) {
      toast.show(`打不开文件夹：${e}`);
    }
  }

  async function reload() {
    if (!sel?.real) return;
    filesErr = "";
    try {
      await library.loadFiles(sel, true);
    } catch (e) {
      filesErr = String(e);
    }
  }

  async function toggleMod(m: Pack["mods"][number]) {
    if (!sel) return;
    if (!sel.real) {
      m.enabled = !m.enabled;
      return;
    }
    try {
      m.id = await api.setModEnabled(settings.gameDir, sel.name, m.id, !m.enabled);
      m.enabled = !m.enabled;
    } catch (e) {
      toast.show(String(e));
    }
  }

  // 删除要点两次：第一次变成“确认”，3 秒内再点才删除
  async function removeRes(kind: "shader" | "resourcepack", id: string) {
    if (!sel) return;
    const key = `${kind}:${id}`;
    if (confirmKey !== key) {
      confirmKey = key;
      clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => (confirmKey = null), 3000);
      return;
    }
    confirmKey = null;
    const k = listKey(kind);
    try {
      if (sel.real) await api.trashFile(settings.gameDir, sel.name, dirName(kind), id);
      sel[k] = sel[k].filter((x) => x !== id);
      toast.show(sel.real ? `已把 ${resName(kind, id)} 移到回收站` : `已移除 ${resName(kind, id)}`);
    } catch (e) {
      toast.show(String(e));
    }
  }
</script>

<div class="versions">
  <aside class="list">
    <div class="list-head">
      <h2>版本管理</h2>
      <button class="btn btn-sm" onclick={() => nav.go("download")}><Plus size={14} />安装</button>
    </div>
    <div class="dir">
      <Select
        bind:value={settings.gameDir}
        options={dirOptions}
        actions={[{ label: "添加游戏文件夹…", onclick: () => gameDirs.add() }]}
        size="sm"
        width="100%"
        ariaLabel="游戏目录"
        onchange={(v) => gameDirs.switchTo(v)}
      />
      {#if !gameDirs.isDefault(settings.gameDir)}
        <button class="btn btn-sm btn-icon" onclick={() => gameDirs.remove(settings.gameDir)} aria-label="从列表移除这个文件夹" title="从列表移除（不删除文件）">
          <FolderX size={14} />
        </button>
      {/if}
    </div>
    <div class="items">
      {#each library.packs as p, idx (p.id)}
        <button class="item" class:on={p.id === sel?.id} style:--i={idx} onclick={() => (selId = p.id)}>
          <PackCover pack={p} size={40} />
          <div class="it-text">
            <b>{p.name}</b>
            <span>{p.mc} · {p.loader}{p.optifine ? " + OptiFine" : ""}</span>
          </div>
          {#if p.id === library.currentId}<span class="cur" title="当前版本"></span>{/if}
        </button>
      {:else}
        {#if library.loaded}<p class="list-empty">「{dirName_}」里没有版本</p>{/if}
      {/each}
    </div>
  </aside>

  <section class="detail">
    {#if !sel}
      <div class="empty-page">
        {#if library.loaded}
          <Layers size={40} strokeWidth={1.4} />
          <h2>这个文件夹里还没有版本</h2>
          <p>去“下载游戏”装一个，或者在左上角添加其他启动器的游戏文件夹</p>
          {#if library.error}<p class="err-text">{library.error}</p>{/if}
          <div class="empty-actions">
            <button class="btn btn-primary" onclick={() => nav.go("download")}><Plus size={14} />下载游戏</button>
            <button class="btn" onclick={() => gameDirs.add()}><FolderOpen size={14} />添加游戏文件夹</button>
          </div>
        {:else}
          <p class="faint">正在读取已安装的版本…</p>
        {/if}
      </div>
    {:else}
      {#key sel.id}
        <div class="detail-inner" in:fade={{ duration: 180 }}>
          <div class="hero">
            <PackCover pack={sel} size={128} />
            <div class="hero-text">
              <p class="label">{isCurrent ? "当前版本" : "版本"}</p>
              <h1>{sel.name}</h1>
              <p class="meta">{meta(sel)}</p>
              <div class="actions">
                <button
                  class="btn btn-primary btn-pill"
                  onclick={launch}
                  disabled={game.status !== "idle" && !mine}
                  title={game.status !== "idle" && !mine ? "已经有一个版本在运行" : undefined}
                >
                  {#if !mine}
                    <Play size={14} fill="currentColor" />启动
                  {:else if game.status === "preparing"}
                    <Square size={14} fill="currentColor" />取消启动 {Math.round(game.progress)}%
                  {:else if game.confirmStop}
                    <Square size={14} fill="currentColor" />再点一次强制关闭
                  {:else}
                    <Square size={14} fill="currentColor" />{game.ready ? "强制关闭" : "正在打开…"}
                  {/if}
                </button>
                {#if !isCurrent}
                  <button class="btn btn-pill" onclick={() => (library.currentId = sel.id)} disabled={game.status !== "idle"}>
                    <Check size={14} />设为当前
                  </button>
                {/if}
                <button class="btn btn-pill" onclick={openFolder}><FolderOpen size={14} />文件夹</button>
                <button class="btn btn-pill" onclick={() => (exporting = sel ?? null)} title="导出成整合包，发给朋友导入"><PackageOpen size={14} />导出</button>
              </div>
            </div>
          </div>

          <div class="tabs-row">
            <div class="seg">
              {#each TABS as k}
                <button class:on={tab === k} onclick={() => (tab = k)}>{RES_LABELS[k]} <span class="cnt">{counts[k]}</span></button>
              {/each}
              <button class:on={tab === "settings"} onclick={() => (tab = "settings")}>
                <SlidersHorizontal size={13} class="tab-ic" />设置{#if sel.vs && Object.keys(sel.vs).length}<span class="dot"></span>{/if}
              </button>
            </div>
            {#if tab !== "settings"}
              <div class="tabs-actions">
                {#if counts[tab] > 0}
                  <label class="find">
                    <Search size={14} />
                    <input class="input" placeholder="搜索{RES_LABELS[tab]}" bind:value={q} spellcheck="false" aria-label="搜索{RES_LABELS[tab]}" />
                    {#if q}<button class="clear" onclick={() => (q = "")} aria-label="清空搜索"><X size={12} /></button>{/if}
                  </label>
                {/if}
                {#if sel.real}
                  <button class="btn btn-sm btn-icon" onclick={reload} aria-label="重新读取" title="重新读取文件夹"><RefreshCw size={14} /></button>
                {/if}
                {#if !(tab === "mod" && sel.loader === "原版")}
                  <button class="btn btn-sm" onclick={() => goDownload(tab as ResKind)}><Plus size={14} />添加{RES_LABELS[tab]}</button>
                {/if}
              </div>
            {/if}
          </div>

          {#if filesErr && tab !== "settings"}<p class="err-text">读取文件夹失败：{filesErr}</p>{/if}
          {#if sel.isolated === false && tab !== "settings"}
            <p class="shared-note">没开版本隔离：这里显示的是游戏目录里共用的{tab === "mod" ? " Mod" : RES_LABELS[tab]}，可以在“设置”里打开隔离</p>
          {/if}

          {#if tab === "settings"}
            <VersionSettings pack={sel} />
          {:else if tab === "mod"}
            {#if sel.mods.length}
              <div class="table">
                <div class="tr th">
                  <span>#</span><span>Mod</span><span>版本</span><span>大小</span><span class="r">启用 {enabled}/{sel.mods.length}</span>
                </div>
                {#each shownMods as m, k (m.id)}
                  <div class="tr" class:off={!m.enabled}>
                    <span class="faint">{k + 1}</span>
                    <span class="name" title={m.id}>{m.name}</span>
                    <span class="muted mono">{m.version || "—"}</span>
                    <span class="muted">{m.size}</span>
                    <span class="r">
                      <button
                        class="switch"
                        class:on={m.enabled}
                        role="switch"
                        aria-checked={m.enabled}
                        aria-label="启用 {m.name}"
                        onclick={() => toggleMod(m)}
                      ></button>
                    </span>
                  </div>
                {:else}
                  <p class="no-match">没有名字里带「{q.trim()}」的 Mod</p>
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
                {#each shownRes(tab as ResKind, sel[key]) as id (id)}
                  {@const confirming = confirmKey === `${tab}:${id}`}
                  <div class="res">
                    <span class="res-ic">{#if tab === "shader"}<Sun size={16} />{:else}<Image size={16} />{/if}</span>
                    <span class="name" title={id}>{resName(tab as ResKind, id)}</span>
                    <button
                      class="btn btn-sm"
                      class:btn-icon={!confirming}
                      class:confirm={confirming}
                      aria-label="移除 {resName(tab as ResKind, id)}"
                      title={sel.real ? "移到回收站" : "移除"}
                      onclick={() => removeRes(tab as "shader" | "resourcepack", id)}
                    >
                      {#if confirming}{sel.real ? "确认移到回收站" : "确认移除"}{:else}<X size={14} />{/if}
                    </button>
                  </div>
                {:else}
                  <p class="no-match">没有名字里带「{q.trim()}」的{RES_LABELS[tab]}</p>
                {/each}
              </div>
            {:else}
              <div class="empty">
                {#if tab === "shader"}<Sun size={32} strokeWidth={1.5} />{:else}<Image size={32} strokeWidth={1.5} />{/if}
                <p>还没有{RES_LABELS[tab]}</p>
                <button class="btn btn-sm" onclick={() => goDownload(tab as ResKind)}>去下载{RES_LABELS[tab]}</button>
              </div>
            {/if}
          {/if}
        </div>
      {/key}
    {/if}
  </section>
</div>


{#if exporting}
  <ExportPackDialog pack={exporting} onclose={() => (exporting = null)} />
{/if}

<style>
  .empty-page {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-3);
    text-align: center;
    animation: rise 0.45s var(--ease) backwards;
  }
  .empty-page h2 {
    color: var(--text);
    font-size: 18px;
    font-weight: 700;
  }
  .empty-page p {
    font-size: 13px;
  }
  .empty-actions {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }
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
  .dir {
    display: flex;
    gap: 6px;
    padding: 0 2px 10px;
  }
  .list-empty {
    padding: 20px 8px;
    text-align: center;
    font-size: 12px;
    color: var(--text-3);
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
    animation-delay: calc(min(var(--i), 10) * 40ms + 60ms);
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
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
    word-break: break-all;
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
  .tabs-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .find {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-3);
  }
  .find > :global(svg) {
    position: absolute;
    left: 9px;
    pointer-events: none;
  }
  .find .input {
    width: 170px;
    height: 30px;
    padding-left: 30px;
    padding-right: 24px;
    font-size: 12.5px;
  }
  .clear {
    position: absolute;
    right: 6px;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--text-3);
  }
  .clear:hover {
    background: var(--hover);
    color: var(--text);
  }
  .no-match {
    padding: 24px 0;
    text-align: center;
    font-size: 13px;
    color: var(--text-3);
  }
  .seg :global(.tab-ic) {
    vertical-align: -2px;
    margin-right: 3px;
  }
  .dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-left: 4px;
    border-radius: 50%;
    background: var(--red);
    vertical-align: 2px;
  }
  .shared-note {
    font-size: 12px;
    color: var(--amber);
    background: var(--amber-soft);
    border-radius: var(--r);
    padding: 7px 12px;
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
    flex-shrink: 0;
  }
  .confirm {
    border-color: var(--red);
    color: var(--red);
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
