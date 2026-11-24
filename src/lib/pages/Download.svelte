<script lang="ts">
  import { smooth } from "$lib/motion";
  import { slide } from "svelte/transition";
  import { Box, FlaskConical, Search, Download as DownloadIcon, Check, X, LoaderCircle, RefreshCw, Info, History } from "@lucide/svelte";
  import Select from "$lib/components/Select.svelte";
  import { library, settings, installs, javas, cached, remember, nav, isPreload } from "$lib/state.svelte";
  import {
    api,
    checkVersionName,
    fmtBytes,
    type LoaderKind,
    type LoaderVersion,
    type ManifestEntry,
    type OptiFineVersion,
  } from "$lib/api";

  type Filter = "release" | "snapshot" | "old" | "all";
  const LOADERS: LoaderKind[] = ["Fabric", "Forge", "NeoForge", "Quilt"];
  const NO_OPTIFINE: LoaderKind[] = ["Fabric", "Quilt", "NeoForge"];
  const TYPE_LABEL: Record<string, string> = { release: "正式版", snapshot: "快照", old_beta: "远古 Beta", old_alpha: "远古 Alpha" };

  let filter = $state<Filter>("release");
  let query = $state("");
  let versions = $state<ManifestEntry[]>([]);
  let loading = $state(true);
  let loadErr = $state("");

  /** 版本列表：10 分钟内再打开直接用上次的，点“重试”才强制重新获取 */
  async function loadManifest(force = false) {
    const key = `manifest:${settings.source}`;
    const hit = force ? undefined : cached<ManifestEntry[]>(key, 10 * 60_000);
    if (hit) {
      versions = hit;
      loading = false;
      return;
    }
    loading = true;
    loadErr = "";
    try {
      const list = await api.manifest(settings.source);
      remember(key, list);
      // 列表有一百多行，等页面滑完再渲染，不然滑动会卡一下
      await nav.settled();
      versions = list;
    } catch (e) {
      loadErr = String(e);
    } finally {
      loading = false;
    }
  }
  // 预渲染时不联网取列表、不找 Java
  if (!isPreload()) {
    loadManifest();
    javas.load();
  }

  const list = $derived(
    versions.filter((v) => {
      const typeOk =
        filter === "all" ||
        (filter === "old" ? v.type.startsWith("old_") : v.type === filter);
      return typeOk && v.id.toLowerCase().includes(query.trim().toLowerCase());
    }),
  );
  const latest = $derived(versions.find((v) => v.type === "release")?.id);

  /* 版本列表分批渲染：全部版本有上千个元素，一下子建出来要 40ms+ 脚本、20ms 排版（实测，正好卡在点进来的那一下）。
     先只放一屏（16 行），页面滑完后趁空闲每次再加 80 行，直到放完；换筛选、搜索时从头来 */
  const FIRST = 16; // 一屏大概放得下十来行
  let limit = $state(FIRST);
  const visible = $derived(list.slice(0, limit));
  $effect(() => {
    const all = list;
    limit = FIRST;
    let stop = false;
    let h = 0;
    const idle = (cb: () => void) => ("requestIdleCallback" in window ? requestIdleCallback(cb, { timeout: 400 }) : setTimeout(cb, 50));
    const grow = () => {
      if (stop || limit >= all.length) return;
      limit += 80;
      h = idle(grow);
    };
    nav.settled().then(() => {
      if (!stop) h = idle(grow);
    });
    return () => {
      stop = true;
      if ("cancelIdleCallback" in window) cancelIdleCallback(h);
      else clearTimeout(h);
    };
  });

  /* ---------- 展开的安装面板 ---------- */
  let expanded = $state<string | null>(null);
  let loader = $state<LoaderKind | null>(null);
  let loaderList = $state<LoaderVersion[] | null>(null);
  let loaderErr = $state("");
  let loaderId = $state("");
  let ofOn = $state(false);
  let ofList = $state<OptiFineVersion[] | null>(null);
  let ofErr = $state("");
  let ofKey = $state("");
  let name = $state("");
  let nameTouched = $state(false);
  let submitErr = $state("");
  let reqSeq = 0;

  const existing = $derived(library.packs.map((p) => p.name));
  const ofSel = $derived(ofList?.find((o) => `${o.type}_${o.patch}` === ofKey));
  const loaderSel = $derived(loaderList?.find((l) => l.id === loaderId));
  const nameErr = $derived(checkVersionName(name, existing));
  const needsJava = $derived(loader === "Forge" || loader === "NeoForge" || ofOn);
  const noJava = $derived(needsJava && javas.found !== null && javas.all.length === 0);

  // OptiFine 搭配 Forge 时的版本要求
  const ofForgeReq = $derived.by(() => {
    if (loader !== "Forge" || !ofSel?.forge) return null;
    const v = ofSel.forge.replace(/^Forge\s*/i, "").trim();
    if (!v || /n\/a/i.test(v)) return { incompatible: true, version: "" };
    return { incompatible: false, version: v, match: loaderSel?.version === v, available: loaderList?.find((l) => l.version === v) };
  });

  function autoName(mc: string) {
    let base = mc + (loader ? `-${loader}` : "") + (ofOn ? "-OptiFine" : "");
    let n = base;
    for (let i = 2; existing.some((e) => e.toLowerCase() === n.toLowerCase()); i++) n = `${base}-${i}`;
    return n;
  }

  $effect(() => {
    if (expanded && !nameTouched) name = autoName(expanded);
  });

  function open(mc: string) {
    if (expanded === mc) {
      expanded = null;
      return;
    }
    expanded = mc;
    loader = null;
    loaderList = null;
    loaderErr = "";
    loaderId = "";
    ofOn = false;
    ofList = null;
    ofErr = "";
    ofKey = "";
    nameTouched = false;
    submitErr = "";
    name = autoName(mc);
  }

  async function pickLoader(k: LoaderKind | null) {
    loader = k;
    loaderList = null;
    loaderErr = "";
    loaderId = "";
    submitErr = "";
    if (k && NO_OPTIFINE.includes(k)) ofOn = false;
    if (!k || !expanded) return;
    const seq = ++reqSeq;
    const mc = expanded;
    try {
      const l = await api.loaderVersions(settings.source, k, mc);
      if (seq !== reqSeq) return;
      loaderList = l;
      loaderId = (l.find((x) => x.stable) ?? l[0])?.id ?? "";
    } catch (e) {
      if (seq === reqSeq) loaderErr = String(e);
    }
  }

  async function toggleOptiFine() {
    ofOn = !ofOn;
    submitErr = "";
    if (!ofOn || ofList || !expanded) return;
    try {
      const l = await api.optifineVersions(expanded);
      ofList = l;
      const first = l.find((x) => !x.preview) ?? l[0];
      ofKey = first ? `${first.type}_${first.patch}` : "";
    } catch (e) {
      ofErr = String(e);
    }
  }

  function useForge(v: LoaderVersion) {
    loaderId = v.id;
  }

  function submit(mc: string) {
    submitErr = "";
    if (nameErr) return (submitErr = nameErr);
    if (loader && !loaderSel) return (submitErr = loaderList?.length === 0 ? `${mc} 没有 ${loader}` : `先选一个 ${loader} 版本`);
    if (ofOn && !ofSel) return (submitErr = ofList?.length === 0 ? `${mc} 没有 OptiFine` : "先选一个 OptiFine 版本");
    if (noJava) return (submitErr = "没找到 Java，装不了 Forge / NeoForge / OptiFine。请先安装 Java 17 或 21。");
    if (ofForgeReq?.incompatible) return (submitErr = "这个 OptiFine 不兼容 Forge，换一个 OptiFine 版本或不装 Forge");
    installs.start({
      mc,
      name,
      loader: loader && loaderSel ? { kind: loader, id: loaderSel.id } : null,
      optifine: ofOn && ofSel ? { type: ofSel.type, patch: ofSel.patch } : null,
    });
    expanded = null;
  }
</script>

<div class="page-scroll">
  <div class="wrap">
    <div class="page-head">
      <div>
        <h1>下载游戏</h1>
        <p>选好版本和加载器就能安装</p>
      </div>
      {#if latest}<span class="badge green">最新正式版 {latest}</span>{/if}
    </div>

    <div class="toolbar">
      <div class="search">
        <Search size={16} />
        <input class="input" placeholder="搜索版本，比如 1.20.1" bind:value={query} />
      </div>
      <div class="seg">
        <button class:on={filter === "release"} onclick={() => (filter = "release")}>正式版</button>
        <button class:on={filter === "snapshot"} onclick={() => (filter = "snapshot")}>快照</button>
        <button class:on={filter === "old"} onclick={() => (filter = "old")}>远古版</button>
        <button class:on={filter === "all"} onclick={() => (filter = "all")}>全部</button>
      </div>
    </div>

    {#if loading}
      <div class="list" aria-label="正在获取版本列表">
        {#each Array(8) as _, k (k)}
          <div class="row-wrap sk-row" style:--i={k}>
            <span class="skel" style:width="{70 + ((k * 23) % 50)}px"></span>
            <span class="skel sk-tag"></span>
            <span class="sk-sp"></span>
            <span class="skel sk-date"></span>
          </div>
        {/each}
      </div>
    {:else if loadErr}
      <div class="state err">
        <p>版本列表获取失败：{loadErr}</p>
        <button class="btn btn-sm" onclick={() => loadManifest(true)}><RefreshCw size={14} />重试</button>
      </div>
    {:else}
      <div class="list">
        {#each visible as v, idx (v.id)}
          {@const owned = library.packs.filter((pk) => pk.mc === v.id).length}
          {@const task = installs.forMc(v.id)}
          <div class="row-wrap" class:open={expanded === v.id} style:--i={idx}>
            <div class="row">
              <span class="vic" class:snap={v.type === "snapshot"} class:old={v.type.startsWith("old_")}>
                {#if v.type === "release"}<Box size={18} />{:else if v.type === "snapshot"}<FlaskConical size={18} />{:else}<History size={18} />{/if}
              </span>
              <div class="vt">
                <b>{v.id}</b>
                <span>{TYPE_LABEL[v.type] ?? v.type} · {v.releaseTime.slice(0, 10)}</span>
              </div>
              <div class="va">
                {#if task}
                  {#if task.status === "error"}
                    <span class="task-err" title={task.error}>安装失败</span>
                    <button class="btn btn-sm btn-icon" onclick={() => installs.cancel(task.id)} aria-label="关闭" title="关闭"><X size={14} /></button>
                  {:else}
                    <div class="prog">
                      <div class="prog-top">
                        <span>({task.stageIndex}/{task.stageCount}) {task.status === "cancelling" ? "正在取消" : task.stage}</span>
                        <span class="num">
                          {#if task.total > 0}{task.done}/{task.total}{/if}{#if task.speed > 1024}{task.total > 0 ? " · " : ""}{fmtBytes(task.speed)}/s{/if}
                        </span>
                      </div>
                      <div class="progress" class:indet={task.total === 0}>
                        <div style:width="{task.total > 0 ? (task.done / task.total) * 100 : 30}%"></div>
                      </div>
                    </div>
                    <button class="btn btn-sm btn-icon" onclick={() => installs.cancel(task.id)} disabled={task.status === "cancelling"} aria-label="取消安装" title="取消安装"><X size={14} /></button>
                  {/if}
                {:else}
                  {#if owned}<span class="badge green"><Check size={12} />已有 {owned} 个</span>{/if}
                  <button class="btn btn-sm" onclick={() => open(v.id)}>
                    <DownloadIcon size={14} />{expanded === v.id ? "收起" : "安装"}
                  </button>
                {/if}
              </div>
            </div>
            {#if task?.status === "error"}
              <p class="err-full">{task.error}</p>
            {/if}

            {#if expanded === v.id && !task}
              <div class="opts" transition:slide={{ duration: 260, easing: smooth }}>
                <div class="opt">
                  <span class="lbl">加载器</span>
                  <div class="ctl">
                    <div class="seg">
                      <button class:on={loader === null} onclick={() => pickLoader(null)}>不装</button>
                      {#each LOADERS as k}
                        <button class:on={loader === k} onclick={() => pickLoader(k)}>{k}</button>
                      {/each}
                    </div>
                    {#if loader}
                      {#if loaderErr}
                        <span class="hint err">获取失败：{loaderErr}</span>
                      {:else if loaderList === null}
                        <span class="hint"><LoaderCircle size={14} class="spin" />正在获取 {loader} 版本…</span>
                      {:else if loaderList.length === 0}
                        <span class="hint warn">{v.id} 没有 {loader}</span>
                      {:else}
                        <Select
                          bind:value={loaderId}
                          size="sm"
                          width="200px"
                          ariaLabel="{loader} 版本"
                          options={loaderList.map((lv) => ({
                            value: lv.id,
                            label: lv.version,
                            hint: lv.stable ? (loader === "Forge" ? "推荐" : "") : "测试版",
                          }))}
                        />
                      {/if}
                    {/if}
                  </div>
                </div>

                <div class="opt">
                  <span class="lbl">OptiFine</span>
                  <div class="ctl">
                    <button
                      class="switch"
                      class:on={ofOn}
                      role="switch"
                      aria-checked={ofOn}
                      aria-label="安装 OptiFine"
                      disabled={!!loader && NO_OPTIFINE.includes(loader)}
                      onclick={toggleOptiFine}
                    ></button>
                    {#if loader && NO_OPTIFINE.includes(loader)}
                      <span class="hint">{loader} 不能装 OptiFine，推荐用 Sodium + Iris（在 Mod 页下载）</span>
                    {:else if ofOn}
                      {#if ofErr}
                        <span class="hint err">获取失败：{ofErr}</span>
                      {:else if ofList === null}
                        <span class="hint"><LoaderCircle size={14} class="spin" />正在获取 OptiFine 版本…</span>
                      {:else if ofList.length === 0}
                        <span class="hint warn">{v.id} 没有 OptiFine</span>
                      {:else}
                        <Select
                          bind:value={ofKey}
                          size="sm"
                          width="200px"
                          ariaLabel="OptiFine 版本"
                          options={ofList.map((o) => ({
                            value: `${o.type}_${o.patch}`,
                            label: `${o.type.replace("_", " ")} ${o.patch}`,
                            hint: o.preview ? "预览版" : o.forge && o.forge !== "Forge N/A" ? o.forge : "",
                          }))}
                        />
                      {/if}
                    {/if}
                  </div>
                </div>

                {#if ofForgeReq && (ofForgeReq.incompatible || !ofForgeReq.match)}
                  <div class="note" class:bad={ofForgeReq.incompatible}>
                    <Info size={14} />
                    {#if ofForgeReq.incompatible}
                      这个 OptiFine 不兼容 Forge
                    {:else if !ofForgeReq.match}
                      这个 OptiFine 建议搭配 Forge {ofForgeReq.version}，版本不一致可能进不了游戏
                      {#if ofForgeReq.available}
                        <button class="btn btn-sm" onclick={() => useForge(ofForgeReq.available!)}>改用 {ofForgeReq.version}</button>
                      {/if}
                    {/if}
                  </div>
                {/if}



                <div class="opt">
                  <span class="lbl">版本名称</span>
                  <div class="ctl col">
                    <input
                      class="input name"
                      class:invalid={!!nameErr}
                      bind:value={name}
                      oninput={() => {
                        nameTouched = true;
                        submitErr = "";
                      }}
                      maxlength="64"
                      aria-label="版本名称"
                    />
                    {#if nameErr}<p class="err-text">{nameErr}</p>{/if}
                  </div>
                </div>

                <div class="foot">
                  {#if submitErr && submitErr !== nameErr}<span class="err-text">{submitErr}</span>{/if}
                  <button class="btn btn-primary" onclick={() => submit(v.id)}><DownloadIcon size={14} />开始安装</button>
                </div>
              </div>
            {/if}
          </div>
        {:else}
          <p class="state">没找到「{query}」，换个关键词试试</p>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .wrap {
    max-width: 860px;
    margin: 0 auto;
  }
  .page-head .badge {
    flex-shrink: 0;
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
  .state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 48px 0;
    color: var(--text-3);
    font-size: 13px;
    flex-direction: column;
  }
  .state.err {
    color: var(--red);
  }
  p.state {
    flex-direction: row;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .sk-row {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 54px;
    padding: 0 16px;
  }
  .sk-row .skel {
    height: 13px;
  }
  .sk-tag {
    width: 42px;
  }
  .sk-date {
    width: 80px;
  }
  .sk-sp {
    flex: 1;
  }
  .row-wrap {
    border-radius: var(--r-lg);
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    transition: border-color 0.2s var(--ease);
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
  .vic.old {
    background: var(--hover);
    color: var(--text-2);
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
    width: 260px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    color: var(--text-2);
  }
  .prog-top {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    white-space: nowrap;
  }
  .prog-top span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .num {
    font-variant-numeric: tabular-nums;
    color: var(--text-3);
  }
  .progress.indet > div {
    animation: indet 1.2s ease-in-out infinite;
  }
  @keyframes indet {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .task-err {
    font-size: 12px;
    color: var(--red);
    font-weight: 600;
  }
  .err-full {
    margin: 0 12px 10px 60px;
    padding: 8px 10px;
    border-radius: var(--r);
    background: var(--red-soft);
    color: var(--red);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }

  .opts {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 4px 14px 14px 60px;
  }
  .opt {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    font-size: 13px;
  }
  .lbl {
    width: 64px;
    flex-shrink: 0;
    color: var(--text-2);
    line-height: 34px;
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    min-height: 34px;
    flex: 1;
    min-width: 0;
  }
  .ctl.col {
    flex-direction: column;
    align-items: stretch;
    gap: 0;
  }
  .name {
    max-width: 320px;
  }
  .hint {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-3);
  }
  .hint.warn {
    color: var(--amber);
  }
  .hint.err {
    color: var(--red);
  }
  .note {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-left: 76px;
    padding: 6px 10px;
    border-radius: var(--r);
    background: var(--amber-soft);
    color: var(--amber);
    font-size: 12px;
  }
  .note.bad {
    background: var(--red-soft);
    color: var(--red);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
  }
  .foot .err-text {
    margin: 0;
  }
</style>
