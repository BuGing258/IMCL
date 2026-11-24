<script lang="ts">
  import { Play, Square, Plus, FolderOpen, PackageOpen, Puzzle, Check, X, Sun, Image, RefreshCw, Layers, SlidersHorizontal, FolderX, Search, Wrench, CircleArrowUp, LoaderCircle, Trash2, Ban, CircleCheck, Download, FolderInput } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { SvelteSet } from "svelte/reactivity";
  import { fly } from "svelte/transition";
  import { smooth } from "$lib/motion";
  import PackCover from "$lib/components/PackCover.svelte";
  import Select from "$lib/components/Select.svelte";
  import VersionSettings from "$lib/components/VersionSettings.svelte";
  import ExportPackDialog from "$lib/components/ExportPackDialog.svelte";
  import ChangeLoaderDialog from "$lib/components/ChangeLoaderDialog.svelte";
  import ImportPackDialog from "$lib/components/ImportPackDialog.svelte";
  import Popover from "$lib/components/Popover.svelte";
  import { library, game, nav, toast, ui, settings, gameDirs, installs, cached, remember, isPreload } from "$lib/state.svelte";
  import { api, fmtRelative, type ModUpdate } from "$lib/api";
  import { marketName, RES_LABELS, type Pack, type ResKind } from "$lib/mock";

  type Tab = ResKind | "settings";
  const TABS: ResKind[] = ["mod", "shader", "resourcepack"];

  let selId = $state(library.currentId);
  /** 正在导出整合包的版本 */
  let exporting = $state<Pack | null>(null);
  /** 从文件导入整合包：选好文件后弹出导入对话框（和 Mod 页的一样），进度在标题栏的下载面板里 */
  let importFile = $state<string | null>(null);
  async function pickPackFile() {
    const f = await api.pickMrpack();
    if (f) importFile = f;
  }
  /** 正在选新加载器的版本 */
  let changing = $state<Pack | null>(null);
  let tab = $state<Tab>("mod");
  let filesErr = $state("");

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
  const shownMods = $derived(!sel ? [] : needle ? sel.mods.filter((m) => hitText(m.name, m.id, m.version, sel.projects?.[`mods/${m.id}`]?.title)) : sel.mods);
  function shownRes(kind: ResKind, ids: string[]) {
    return needle ? ids.filter((id) => hitText(resName(kind, id), id)) : ids;
  }

  const listKey = (k: ResKind) => (k === "shader" ? "shaders" : "resourcepacks") as "shaders" | "resourcepacks";
  const dirName = (k: ResKind) => (k === "mod" ? "mods" : k === "shader" ? "shaderpacks" : "resourcepacks");

  // 每次打开版本管理都重新扫一遍：在文件夹里直接删掉、加进来或改过的版本马上就能看到。
  // 已经读过的 Mod / 光影 / 材质包也作废：当前选中的版本立刻重读，其他版本等选中时再读
  // 等页面滑完再扫：扫描结果要重新排版整页，放在滑动途中会卡一下
  // 预渲染时不扫描、不查更新（见 isPreload）
  const preload = isPreload();
  if (!preload)
    nav.settled().then(() => library.refresh()).then(() => {
      for (const p of library.packs) if (p.real && p !== sel) p.filesLoaded = false;
      if (sel?.real && sel.filesLoaded) library.loadFiles(sel, true).catch((e) => (filesErr = String(e)));
    });

  // 选中真实版本时读取它文件夹里的内容
  $effect(() => {
    const p = sel;
    if (preload || !p?.real || p.filesLoaded) return;
    filesErr = "";
    nav.settled().then(() => library.loadFiles(p)).catch((e) => (filesErr = String(e)));
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

  /* ---------- 更换加载器的进度 ---------- */
  const loaderTask = $derived(sel ? Object.values(installs.tasks).find((t) => t.loaderFor === sel.name) : undefined);

  /* ---------- 删除版本（加载器按钮右边）----------
   * 和版本设置里的一样：点一次变成“确认删除”，3 秒内再点才把整个版本文件夹移到回收站；
   * 这个版本正在运行、正在换加载器时不能删
   */
  let delArmed = $state(false);
  let deletingVer = $state(false);
  let delTimer: ReturnType<typeof setTimeout> | undefined;
  const delBlocked = $derived(mine ? "游戏正在运行，关掉后才能删除" : loaderTask && loaderTask.status !== "error" ? "正在更换加载器，完成后才能删除" : "");
  // 换了选中的版本，没确认的删除作废
  $effect(() => {
    void sel?.id;
    delArmed = false;
    clearTimeout(delTimer);
  });
  async function deleteSel() {
    const p = sel;
    if (!p || deletingVer || delBlocked) return;
    if (!delArmed) {
      delArmed = true;
      clearTimeout(delTimer);
      delTimer = setTimeout(() => (delArmed = false), 3000);
      return;
    }
    clearTimeout(delTimer);
    delArmed = false;
    deletingVer = true;
    try {
      await library.deleteVersion(p);
      toast.show(`已把「${p.name}」移到回收站`);
    } catch (e) {
      toast.show(String(e));
    } finally {
      deletingVer = false;
    }
  }
  const loaderPct = $derived(
    loaderTask ? Math.min(99, Math.floor(((Math.max(1, loaderTask.stageIndex) - 1 + (loaderTask.total ? loaderTask.done / loaderTask.total : 0)) / Math.max(1, loaderTask.stageCount)) * 100)) : 0,
  );

  /* ---------- Mod 更新 ---------- */
  let updates = $state<Record<string, ModUpdate>>({});
  let picked = $state<Record<string, boolean>>({});
  let checking = $state(false);
  let checked = $state(false);
  let updating = $state<{ done: number; total: number } | null>(null);
  // 换版本时清空
  $effect(() => {
    void sel?.id;
    updates = {};
    picked = {};
    checked = false;
  });
  const updList = $derived(Object.values(updates));
  const pickedList = $derived(updList.filter((u) => picked[u.file]));
  const allPicked = $derived(updList.length > 0 && pickedList.length === updList.length);
  const updOf = (m: Pack["mods"][number]) => updates[`mods/${m.id}`];

  const updKey = (p: Pack) => `updates:${settings.gameDir}:${p.name}`;

  /** 检查 Mod 更新。auto = 打开 Mod 标签时自动检查：10 分钟内用上次的结果，没更新或失败都不弹提示 */
  async function checkUpdates(auto = false) {
    if (!sel || checking) return;
    const p = sel;
    checking = true;
    try {
      const hit = auto ? cached<ModUpdate[]>(updKey(p), 10 * 60_000) : undefined;
      const list = hit ?? (await api.resourceUpdates(settings.gameDir, p.name, "mod", p.mc, p.loader === "原版" ? null : p.loader));
      remember(updKey(p), list);
      if (sel?.id !== p.id) return;
      updates = Object.fromEntries(list.map((u) => [u.file, u]));
      // 默认全选，用户可以取消不想更新的
      picked = Object.fromEntries(list.map((u) => [u.file, true]));
      if (!list.length && !auto) toast.show("所有 Mod 都是最新的");
    } catch (e) {
      if (!auto) toast.show(`检查更新失败：${e}`);
    } finally {
      // 自动检查失败也算查过，不会一直重试
      if (sel?.id === p.id) checked = true;
      checking = false;
    }
  }

  // 打开（或切到）Mod 标签时自动检查一次；Mod 列表读出来之后再查
  $effect(() => {
    if (preload || tab !== "mod" || !sel || sel.loader === "原版" || !sel.mods.length || checked || checking || updating) return;
    if (sel.real && !sel.filesLoaded) return;
    checkUpdates(true);
  });

  function toggleAll() {
    const v = !allPicked;
    picked = Object.fromEntries(updList.map((u) => [u.file, v]));
  }

  async function updateSelected() {
    if (!sel || updating || !pickedList.length) return;
    const p = sel;
    const todo = [...pickedList];
    updating = { done: 0, total: todo.length };
    const failed: string[] = [];
    for (const u of todo) {
      try {
        await api.installResource({
          gameDir: settings.gameDir,
          version: p.name,
          kind: "mod",
          projectId: u.projectId,
          mc: p.mc,
          loader: p.loader,
          versionId: u.versionId,
          reinstall: true,
          withDeps: true,
        });
        delete updates[u.file];
        delete picked[u.file];
        if (!p.real) {
          const m = p.mods.find((x) => `mods/${x.id}` === u.file);
          if (m) m.version = u.versionNumber;
        }
      } catch {
        failed.push(u.title || u.file);
      }
      updating = { done: updating!.done + 1, total: todo.length };
    }
    updating = null;
    remember(updKey(p), Object.values(updates));
    if (p.real) await library.loadFiles(p, true).catch(() => {});
    const ok = todo.length - failed.length;
    toast.show(failed.length ? `更新了 ${ok} 个，${failed.join("、")} 更新失败` : `已更新 ${ok} 个 Mod`);
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

  /* ---------- 批量选择：Mod、光影、材质包 ----------
   * 点一行选中，再点取消；Shift 连选；Ctrl+A 全选当前列表；Esc 取消选择。
   * 选中后底部出现操作条：启用 / 禁用（只有 Mod）、删除（移到回收站，要点两下确认）、取消
   */
  const chosen = new SvelteSet<string>();
  let anchor: string | null = null;
  let armed = $state(false);
  let armTimer: ReturnType<typeof setTimeout> | undefined;
  let batchBusy = $state<{ done: number; total: number } | null>(null);

  /** 当前标签里显示出来的文件（搜索过滤后的），顺序和列表一致 */
  const visibleIds = $derived(!sel || tab === "settings" ? [] : tab === "mod" ? shownMods.map((m) => m.id) : shownRes(tab, sel[listKey(tab)]));
  /** 只对看得见的生效：搜索把选中的项过滤掉了，就不会被误删 */
  const chosenList = $derived(visibleIds.filter((id) => chosen.has(id)));
  const chosenMods = $derived(tab === "mod" && sel ? sel.mods.filter((m) => chosen.has(m.id) && visibleIds.includes(m.id)) : []);

  function clearChosen() {
    chosen.clear();
    anchor = null;
    armed = false;
  }
  // 换版本、换标签时清空选择
  $effect(() => {
    void [sel?.id, tab];
    clearChosen();
  });

  function pick(id: string, e: MouseEvent) {
    // 行里的开关、更新勾选框自己处理，不算选行
    if ((e.target as HTMLElement).closest("button, input, a") || batchBusy) return;
    if (e.shiftKey && anchor && visibleIds.includes(anchor)) {
      const a = visibleIds.indexOf(anchor);
      const b = visibleIds.indexOf(id);
      for (const x of visibleIds.slice(Math.min(a, b), Math.max(a, b) + 1)) chosen.add(x);
    } else if (chosen.has(id)) chosen.delete(id);
    else chosen.add(id);
    anchor = id;
    armed = false;
  }

  function onKey(e: KeyboardEvent) {
    if (preload || tab === "settings" || changing || exporting || batchBusy) return;
    const t = e.target as HTMLElement;
    const typing = t?.closest?.("input, textarea, [contenteditable='true']");
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a" && !typing && visibleIds.length) {
      e.preventDefault();
      for (const id of visibleIds) chosen.add(id);
      armed = false;
    } else if (e.key === "Escape" && chosenList.length && !typing) {
      // 先取消选择，不返回上一页（捕获阶段，拦在全局的 Esc 前面）
      e.stopPropagation();
      clearChosen();
    }
  }

  /** 批量启用或禁用选中的 Mod */
  async function setChosenEnabled(on: boolean) {
    const p = sel;
    if (!p || batchBusy) return;
    const todo = chosenMods.filter((m) => m.enabled !== on);
    batchBusy = { done: 0, total: todo.length };
    const failed: string[] = [];
    for (const m of todo) {
      try {
        const old = m.id;
        if (p.real) m.id = await api.setModEnabled(settings.gameDir, p.name, m.id, on);
        m.enabled = on;
        // 启用 / 禁用会改文件名（加上或去掉 .disabled），选择跟着换
        chosen.delete(old);
        chosen.add(m.id);
      } catch {
        failed.push(m.name);
      }
      batchBusy = { done: batchBusy!.done + 1, total: todo.length };
    }
    batchBusy = null;
    armed = false;
    if (failed.length) toast.show(`${failed.join("、")} ${on ? "启用" : "禁用"}失败（游戏是不是还开着？）`);
  }

  /** 删除选中的文件：第一次点变成“确认”，3 秒内再点才删；移到回收站，可以还原 */
  async function deleteChosen() {
    const p = sel;
    if (!p || batchBusy || tab === "settings") return;
    if (!armed) {
      armed = true;
      clearTimeout(armTimer);
      armTimer = setTimeout(() => (armed = false), 3000);
      return;
    }
    armed = false;
    const kind = tab;
    const todo = [...chosenList];
    batchBusy = { done: 0, total: todo.length };
    const gone = new Set<string>();
    const failed: string[] = [];
    for (const id of todo) {
      try {
        if (p.real) await api.trashFile(settings.gameDir, p.name, dirName(kind), id);
        gone.add(id);
      } catch {
        failed.push(kind === "mod" ? (p.mods.find((m) => m.id === id)?.name ?? id) : resName(kind, id));
      }
      batchBusy = { done: batchBusy!.done + 1, total: todo.length };
    }
    if (kind === "mod") {
      p.mods = p.mods.filter((m) => !gone.has(m.id));
      // 删掉的 Mod 也不再提示更新
      for (const id of gone) {
        delete updates[`mods/${id}`];
        delete picked[`mods/${id}`];
      }
    } else {
      const k = listKey(kind);
      p[k] = p[k].filter((x) => !gone.has(x));
    }
    for (const id of gone) chosen.delete(id);
    batchBusy = null;
    const what = kind === "mod" ? " Mod" : RES_LABELS[kind];
    if (failed.length) toast.show(`${failed.join("、")} 删除失败（游戏是不是还开着？）`);
    else toast.show(p.real ? `已把 ${gone.size} 个${what}移到回收站` : `已移除 ${gone.size} 个${what}`);
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="versions">
  <aside class="list">
    <div class="list-head">
      <h2>版本管理</h2>
      <!-- 安装：下载新的游戏版本，或者从文件导入整合包 -->
      <Popover align="start" width="236px" label="安装">
        {#snippet trigger({ open, toggle })}
          <button class="btn btn-sm" class:on={open} onclick={toggle} aria-haspopup="menu" aria-expanded={open}><Plus size={14} />安装</button>
        {/snippet}
        {#snippet children({ close })}
          <button class="menu-item" role="menuitem" onclick={() => (close(), nav.go("download"))}>
            <span class="mi-ic"><Download size={16} /></span>
            <span class="mi-text">安装游戏版本<small>原版 · 加载器 · OptiFine</small></span>
          </button>
          <button class="menu-item" role="menuitem" onclick={() => (close(), pickPackFile())}>
            <span class="mi-ic"><FolderInput size={16} /></span>
            <span class="mi-text">导入整合包<small>从 .mrpack 文件</small></span>
          </button>
        {/snippet}
      </Popover>
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
          <div class="sk-detail" aria-label="正在读取已安装的版本">
            <span class="skel" style:width="38%" style:height="22px"></span>
            <span class="skel" style:width="62%"></span>
            <span class="skel" style:width="100%" style:height="120px"></span>
          </div>
        {/if}
      </div>
    {:else}
      {#key sel.id}
        <div class="detail-inner">
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
                  disabled={(game.status !== "idle" && !mine) || (!!loaderTask && loaderTask.status !== "error")}
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
                <button class="btn btn-pill" onclick={() => (exporting = sel ?? null)} title="导出整合包"><PackageOpen size={14} />导出</button>
                <button class="btn btn-pill" onclick={() => (changing = sel ?? null)} disabled={!!loaderTask && loaderTask.status !== "error"} title="更换加载器或加载器版本">
                  <Wrench size={14} />加载器
                </button>
                <button
                  class="btn btn-pill del-ver"
                  class:armed={delArmed}
                  onclick={deleteSel}
                  disabled={deletingVer || !!delBlocked}
                  title={delBlocked || (sel.isolated === false ? "版本文件夹移到回收站，共用的 Mod 和存档不受影响" : "整个版本文件夹移到回收站，包括它自己的 Mod、存档和配置")}
                >
                  {#if deletingVer}<LoaderCircle size={14} class="spin" />正在删除{:else}<Trash2 size={14} />{delArmed ? "确认删除" : "删除"}{/if}
                </button>
              </div>
              {#if loaderTask}
                <div class="lt" class:bad={loaderTask.status === "error"}>
                  {#if loaderTask.status === "error"}
                    <span class="lt-text" title={loaderTask.error}>更换加载器失败：{loaderTask.error}</span>
                  {:else}
                    <LoaderCircle size={13} class="spin" />
                    <span class="lt-text">正在更换加载器 · {loaderTask.stage}</span>
                    <span class="lt-bar"><i style:width="{loaderPct}%"></i></span>
                    <span class="lt-pct">{loaderPct}%</span>
                  {/if}
                  <button class="btn btn-sm btn-icon" onclick={() => installs.cancel(loaderTask.id)} disabled={loaderTask.status === "cancelling"} aria-label={loaderTask.status === "error" ? "关闭" : "取消"}><X size={13} /></button>
                </div>
              {/if}
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
                {#if tab === "mod" && sel.loader !== "原版" && sel.mods.length}
                  <button class="btn btn-sm" onclick={() => checkUpdates()} disabled={checking || !!updating}>
                    {#if checking}<LoaderCircle size={14} class="spin" />检查中{:else}<CircleArrowUp size={14} />检查更新{/if}
                  </button>
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
            <p class="shared-note">和其他版本共用（没开版本隔离）</p>
          {/if}

          {#if tab === "settings"}
            <VersionSettings pack={sel} />
          {:else if tab === "mod"}
            {#if updList.length}
              <div class="upd-bar">
                <CircleArrowUp size={15} />
                <span><b>{updList.length}</b> 个 Mod 有新版本</span>
                <span class="sp"></span>
                {#if updating}
                  <span class="faint"><LoaderCircle size={13} class="spin" />正在更新 {updating.done}/{updating.total}</span>
                {:else}
                  <button class="btn btn-sm btn-primary" onclick={updateSelected} disabled={!pickedList.length}>更新所选（{pickedList.length}）</button>
                {/if}
              </div>
            {/if}
            {#if sel.mods.length}
              <div class="table">
                <div class="tr th">
                  {#if updList.length}
                    <input type="checkbox" class="ck" checked={allPicked} indeterminate={pickedList.length > 0 && !allPicked} onchange={toggleAll} disabled={!!updating} aria-label="全选有更新的 Mod" title="全选" />
                  {:else}
                    <span>#</span>
                  {/if}
                  <span>Mod</span><span>版本</span><span>大小</span><span class="r">启用 {enabled}/{sel.mods.length}</span>
                </div>
                {#each shownMods as m, k (m.id)}
                  {@const up = updOf(m)}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <div
                    class="tr pickable"
                    class:off={!m.enabled}
                    class:has-up={!!up}
                    class:chosen={chosen.has(m.id)}
                    role="option"
                    tabindex="-1"
                    aria-selected={chosen.has(m.id)}
                    onclick={(e) => pick(m.id, e)}
                  >
                    {#if up}
                      <input type="checkbox" class="ck" bind:checked={picked[up.file]} disabled={!!updating} aria-label="更新 {m.name}" />
                    {:else}
                      <span class="faint">{k + 1}</span>
                    {/if}
                    <span class="name" title={m.id}>{m.name}</span>
                    {#if up}
                      <span class="ver-up mono" title="{up.current} → {up.versionNumber}"><span class="muted">{m.version || up.current}</span><span class="arrow">→</span><b>{up.versionNumber}</b></span>
                    {:else}
                      <span class="muted mono">{m.version || "—"}</span>
                    {/if}
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
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <div
                    class="res pickable"
                    class:chosen={chosen.has(id)}
                    role="option"
                    tabindex="-1"
                    aria-selected={chosen.has(id)}
                    onclick={(e) => pick(id, e)}
                  >
                    <span class="res-ic">{#if tab === "shader"}<Sun size={16} />{:else}<Image size={16} />{/if}</span>
                    <span class="name" title={id}>{resName(tab as ResKind, id)}</span>
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
          {#if visibleIds.length}
            <p class="pick-tip">点击选中，可以多选（Shift 连选，Ctrl+A 全选），然后{tab === "mod" ? "批量启用、禁用或" : ""}删除</p>
          {/if}

          <!-- 选中后的操作条：贴在面板底部，滚动时也一直在 -->
          <div class="batch-dock">
            {#if chosenList.length}
              <div class="batch-pos">
                <div class="batch" role="toolbar" aria-label="选中的文件" transition:fly={{ y: 16, duration: 200, easing: smooth }}>
                  {#if batchBusy}
                    <span class="b-count"><LoaderCircle size={14} class="spin" />正在处理 {batchBusy.done}/{batchBusy.total}</span>
                  {:else}
                    <span class="b-count">已选 {chosenList.length} 个</span>
                    {#if chosenMods.some((m) => !m.enabled)}
                      <button class="btn btn-sm btn-pill" onclick={() => setChosenEnabled(true)}><CircleCheck size={13} />启用</button>
                    {/if}
                    {#if chosenMods.some((m) => m.enabled)}
                      <button class="btn btn-sm btn-pill" onclick={() => setChosenEnabled(false)}><Ban size={13} />禁用</button>
                    {/if}
                    <button class="btn btn-sm btn-pill b-del" class:armed onclick={deleteChosen} title={sel.real ? "移到回收站，可以还原" : "移除"}>
                      <Trash2 size={13} />{armed ? `确认删除 ${chosenList.length} 个` : "删除"}
                    </button>
                    <button class="btn btn-sm btn-pill" onclick={clearChosen}>取消</button>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </div>
      {/key}
    {/if}
  </section>
</div>


{#if changing}
  <ChangeLoaderDialog pack={changing} onclose={() => (changing = null)} />
{/if}

{#if exporting}
  <ExportPackDialog pack={exporting} onclose={() => (exporting = null)} />
{/if}

{#if importFile}
  <ImportPackDialog file={importFile} onclose={() => (importFile = null)} />
{/if}

<style>
  .empty-page {
    max-width: 460px;
    margin: 72px auto 24px;
    padding: 32px 28px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-3);
    text-align: center;
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
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
  /* 文字都放在卡片材质上：设了自定义背景图时也看得清 */
  .list {
    margin: 12px 0 12px 12px;
    padding: 6px 10px 12px;
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
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
    transition: background 0.15s var(--ease);
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
    /* 至少占满面板高度，底部的操作条（batch-dock）才能贴在面板底部 */
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }
  .hero {
    display: flex;
    align-items: flex-end;
    gap: 22px;
    margin-bottom: 14px;
    padding: 16px;
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
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
  .lt {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    max-width: 460px;
    font-size: 12px;
    color: var(--text-2);
  }
  .lt.bad {
    color: var(--red);
  }
  .lt-text {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .lt-bar {
    flex: 1;
    min-width: 60px;
    height: 5px;
    border-radius: 3px;
    background: var(--active);
    overflow: hidden;
  }
  .lt-bar i {
    display: block;
    height: 100%;
    background: var(--red);
    transition: width 0.35s var(--ease);
  }
  .lt-pct {
    font-variant-numeric: tabular-nums;
    min-width: 30px;
    text-align: right;
  }
  .upd-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 8px 12px;
    margin-bottom: 8px;
    border-radius: var(--r);
    background: linear-gradient(var(--red-soft), var(--red-soft)), var(--surface);
    backdrop-filter: var(--card-filter);
    color: var(--red);
    font-size: 12.5px;
  }
  .upd-bar > span:not(.sp) {
    color: var(--text);
  }
  .upd-bar .faint {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .upd-bar .sp {
    flex: 1;
  }
  .ver-up {
    display: flex;
    align-items: baseline;
    gap: 4px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  .ver-up .arrow {
    color: var(--text-3);
  }
  .ver-up b {
    color: var(--red);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tabs-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    /* 窗口窄的时候右边的按钮换到下一行，不会被挤出去 */
    flex-wrap: wrap;
    gap: 10px 12px;
    margin-bottom: 10px;
  }
  /* 右边这组按钮按 0 宽度参与换行判断、再占满剩下的空间：搜索框先缩短，
     窗口最小时也和标签放在同一行；真放不下（比最小窗口还窄）才换行 */
  .tabs-actions {
    margin-left: auto;
    flex: 1 1 0;
    justify-content: flex-end;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .find {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-3);
    flex: 0 1 200px;
    min-width: 110px;
  }
  .find > :global(svg) {
    position: absolute;
    left: 9px;
    pointer-events: none;
  }
  .find .input {
    width: 100%;
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
  /* 标签切换的底板也用卡片材质；选中的那一格比底板亮一点，和别处的分段按钮一样 */
  .tabs-row .seg {
    background: linear-gradient(var(--hover), var(--hover)), var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
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
    background: linear-gradient(var(--amber-soft), var(--amber-soft)), var(--surface);
    backdrop-filter: var(--card-filter);
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
    /* 行与行之间留 2px：连着选中几行时，红色高亮一行一块，不会糊成一整片 */
    gap: 2px;
    padding: 4px 6px 6px;
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
  }
  .tr {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) minmax(100px, auto) 80px 80px;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    height: 40px;
    border-radius: var(--r);
    font-size: 13px;
    transition: background 0.15s var(--ease), opacity 0.2s var(--ease);
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
    backdrop-filter: var(--card-filter);
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
  /* 可以点选的行 */
  .pickable {
    cursor: pointer;
    user-select: none;
  }
  .tr.chosen,
  .tr.chosen:hover,
  .res.chosen {
    background: var(--red-soft);
  }
  .res.chosen {
    border-color: color-mix(in srgb, var(--red) 45%, transparent);
  }
  .res.pickable:hover:not(.chosen) {
    background: var(--hover);
  }
  .pick-tip {
    margin-top: 10px;
    font-size: 12px;
    color: var(--text-3);
    text-align: center;
  }

  /* 操作条：sticky 的零高度容器贴在面板底部，条本身绝对定位在它上面 */
  .batch-dock {
    position: sticky;
    bottom: 0;
    margin-top: auto;
    height: 0;
    z-index: 5;
  }
  .batch-pos {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 14px;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }
  .batch {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    /* 右边比上下多 2px：两头是半圆，按钮离右沿看起来会比离上下沿近，几何上相等反而显得右边挤 */
    padding: 5px 7px 5px 16px;
    border-radius: 999px;
    background: var(--pop-bg);
    backdrop-filter: var(--pop-filter);
    border: 1px solid var(--border-strong);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.18);
    font-size: 13px;
  }
  .b-count {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-right: 6px;
    font-weight: 500;
    white-space: nowrap;
  }
  .b-del {
    color: var(--red);
  }
  .b-del.armed {
    background: var(--red);
    border-color: var(--red);
    color: var(--on-red);
  }
  .b-del.armed:hover {
    background: var(--red-hover);
  }
  /* 删除版本：平时和旁边的按钮一样，悬停变红；点了一次（等确认）整个变红 */
  .del-ver:not(:disabled):hover {
    border-color: var(--red);
    color: var(--red);
  }
  .del-ver.armed,
  .del-ver.armed:hover {
    background: var(--red);
    border-color: var(--red);
    color: var(--on-red);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 40px 16px;
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    color: var(--text-3);
    font-size: 13px;
    text-align: center;
  }
  .sk-detail {
    width: 100%;
    max-width: 520px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
