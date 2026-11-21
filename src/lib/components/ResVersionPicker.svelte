<script lang="ts">
  import { scrollWithin } from "$lib/theme";
  // 安装 / 重新安装前选资源的版本。
  // Mod、光影、材质包：只列适合目标游戏版本的版本，Mod 还会列出前置。
  // 整合包：列出全部版本，装成一个新的游戏版本，所以要填版本名称。
  import Modal from "./Modal.svelte";
  import { X, LoaderCircle, RefreshCw, Check, Download as DownloadIcon, Info, Link2, TriangleAlert } from "@lucide/svelte";
  import { api, checkVersionName, type Hit, type ModVersion, type ResDep, type ResKindApi } from "$lib/api";
  import { fmtCount, library, settings } from "$lib/state.svelte";

  let {
    hit,
    kind,
    mc,
    loader,
    target,
    gameDir,
    installedVersion = null,
    installedOf = () => null,
    busyOf = () => false,
    active = true,
    onclose,
    onpick,
    onpickdep = () => {},
  }: {
    hit: Hit;
    kind: ResKindApi;
    /** 目标游戏版本；整合包为空 */
    mc: string;
    loader: string | null;
    /** 安装到哪个版本（版本名）；整合包为空 */
    target: string;
    gameDir: string;
    /** 已安装的版本号：空字符串 = 装了但版本未知，null = 没装 */
    installedVersion?: string | null;
    /** 某个项目在目标版本里装的版本号（同上） */
    installedOf?: (projectId: string) => string | null;
    /** 某个项目正在安装 */
    busyOf?: (projectId: string) => boolean;
    /** 叠了好几层时，只有最上面那层响应键盘 */
    active?: boolean;
    onclose: () => void;
    /** 选好了：withDeps 是否连前置一起装；name 是整合包新版本的名字 */
    onpick: (v: ModVersion, withDeps: boolean, name?: string) => void;
    /** 给某个前置选版本；installed 是它现在装的版本号（没装是 null） */
    onpickdep?: (d: ResDep, installed: string | null) => void;
  } = $props();

  const isPack = $derived(kind === "modpack");

  const TYPE: Record<ModVersion["versionType"], [string, string]> = {
    release: ["正式版", "green"],
    beta: ["测试版", "amber"],
    alpha: ["开发版", "red"],
  };
  const LOADER_ZH: Record<string, string> = { fabric: "Fabric", forge: "Forge", neoforge: "NeoForge", quilt: "Quilt" };

  let list = $state<ModVersion[]>([]);
  let loading = $state(true);
  let err = $state("");
  let selected = $state<string | null>(null);
  let onlyRelease = $state(false);

  const reinstall = $derived(!isPack && installedVersion != null);
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
  /** 整合包版本用的游戏版本和加载器，比如 “1.20.1 · Fabric” */
  const packInfo = (v: ModVersion) =>
    [v.gameVersions[0], v.loaders.map((l) => LOADER_ZH[l] ?? l).join(" / ")].filter(Boolean).join(" · ");

  /* ---------- 整合包：新版本名称 ---------- */
  let packName = $state("");
  let nameTouched = $state(false);
  const existing = $derived(library.packs.map((p) => p.name));
  const nameErr = $derived(isPack ? checkVersionName(packName, existing) : "");
  $effect(() => {
    if (!isPack || nameTouched || !pick) return;
    // 默认名字：整合包名 + 游戏版本，重名就加序号
    const base = `${hit.title} ${pick.gameVersions[0] ?? ""}`.replace(/[\\/:*?"<>|]/g, " ").replace(/\s+/g, " ").trim().slice(0, 60);
    let n = base;
    for (let i = 2; existing.includes(n); i++) n = `${base} (${i})`;
    packName = n;
  });

  /* ---------- 前置（只有 Mod 有） ---------- */
  let deps = $state<ResDep[]>([]);
  let depsLoading = $state(false);
  let depsErr = $state("");
  const depCache = new Map<string, ResDep[]>();
  let depSeq = 0;

  $effect(() => {
    const id = selected;
    if (!id || kind !== "mod") return;
    const hit = depCache.get(id);
    if (hit) {
      deps = hit;
      depsErr = "";
      return;
    }
    const my = ++depSeq;
    depsLoading = true;
    depsErr = "";
    // 上下键快速切换时别每个都去查
    const t = setTimeout(async () => {
      try {
        const r = await api.resourceDeps({ gameDir, version: target, kind, versionId: id, mc, loader });
        depCache.set(id, r);
        if (my === depSeq) deps = r;
      } catch (e) {
        if (my === depSeq) depsErr = String(e);
      } finally {
        if (my === depSeq) depsLoading = false;
      }
    }, 200);
    return () => clearTimeout(t);
  });

  /** 前置装的版本号（刚在这个窗口里装上的也算）；没装是 null */
  const depVersion = (d: ResDep) => installedOf(d.projectId) ?? (d.installed ? "" : null);
  const needDeps = $derived(deps.filter((d) => depVersion(d) == null));
  /** 安装时会一起装上的前置数量 */
  const withCount = $derived(settings.autoDeps && !depsLoading ? needDeps.filter((d) => d.available).length : 0);

  function confirm() {
    if (!pick) return;
    if (isPack) {
      nameTouched = true;
      if (nameErr) return;
      onpick(pick, true, packName);
    } else onpick(pick, settings.autoDeps);
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    if (e.key === "Enter" && pick && !(e.target instanceof HTMLButtonElement)) {
      confirm();
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && shown.length && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      const i = shown.findIndex((v) => v.id === selected);
      const n = Math.max(0, Math.min(shown.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
      selected = shown[n].id;
      const row = document.getElementById(`rv-${hit.id}-${shown[n].id}`);
      scrollWithin(row?.closest<HTMLElement>(".list"), row);
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<Modal {onclose} {active} width={600} labelledby="rv-title-{hit.id}">
    <header>
      {#if hit.icon}<img class="ic" src={hit.icon} alt="" />{:else}<span class="ic ph">{hit.title[0]}</span>{/if}
      <div class="ht">
        <h2 id="rv-title-{hit.id}">{isPack ? "安装整合包" : reinstall ? "重新安装" : "安装"} {hit.title}</h2>
        <p>
          {#if isPack}会新建一个游戏版本{:else}装到 {target}{/if}
        </p>
      </div>
      <button class="btn btn-sm btn-icon" onclick={onclose} aria-label="关闭"><X size={15} /></button>
    </header>

    {#if reinstall}
      <div class="installed">
        <Check size={14} />
        <span>已安装 <b>{installedVersion || "（版本未知）"}</b>，旧文件会移到回收站</span>
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
        <p class="state">{isPack ? "这个整合包还没有可以下载的版本" : `没有适合 ${mc} 的版本`}</p>
      {:else}
        {#each shown as v (v.id)}
          {@const [tl, tc] = TYPE[v.versionType] ?? TYPE.release}
          <button
            id="rv-{hit.id}-{v.id}"
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
              {#if isPack}<small>{packInfo(v)}</small>{:else if v.name && v.name !== v.versionNumber}<small>{v.name}</small>{/if}
            </span>
            {#if same(v)}<span class="badge green">已安装</span>{/if}
            <span class="badge {tc}">{tl}</span>
            <span class="meta">{date(v.published)}</span>
            <span class="meta dl"><DownloadIcon size={11} />{fmtCount(v.downloads)}</span>
          </button>
        {/each}
      {/if}
    </div>

    {#if isPack && pick}
      <section class="box">
        <label class="name-row">
          <span>版本名称</span>
          <input
            class="input"
            class:invalid={nameTouched && !!nameErr}
            bind:value={packName}
            oninput={() => (nameTouched = true)}
            maxlength="64"
            spellcheck="false"
          />
        </label>
        {#if nameTouched && nameErr}<p class="err-text">{nameErr}</p>{/if}
        <p class="faint">Minecraft {packInfo(pick)}</p>
      </section>
    {/if}

    {#if kind === "mod" && pick}
      <section class="box" aria-label="前置 Mod">
        <div class="deps-head">
          <Link2 size={14} />
          <b>前置 Mod</b>
          {#if depsLoading}
            <LoaderCircle size={13} class="spin" />
          {:else if !depsErr}
            <span class="faint">{deps.length ? `需要 ${deps.length} 个${needDeps.length ? `，还差 ${needDeps.length} 个` : "，都装好了"}` : "不需要前置"}</span>
          {/if}
          <span class="sp"></span>
          {#if deps.length}
            <label class="auto">
              <input type="checkbox" bind:checked={settings.autoDeps} />自动安装前置
            </label>
          {/if}
        </div>
        {#if depsErr}
          <p class="err-text">读取前置失败：{depsErr}</p>
        {:else if deps.length}
          <ul>
            {#each deps as d (d.projectId)}
              {@const dv = depVersion(d)}
              <li>
                {#if d.icon}<img src={d.icon} alt="" />{:else}<span class="dph">{d.title[0]}</span>{/if}
                <span class="dt">
                  <span class="dn">{d.title}</span>
                  {#if dv != null}<span class="dok"><Check size={11} />已安装{dv ? ` ${dv}` : ""}</span>{/if}
                </span>
                {#if busyOf(d.projectId)}
                  <button class="btn btn-sm" disabled><LoaderCircle size={13} class="spin" />安装中</button>
                {:else if dv != null}
                  <button class="btn btn-sm" onclick={() => onpickdep(d, dv)} title="换个版本或重新下载">重新安装</button>
                {:else if !d.available}
                  <span class="badge amber" title="没有适合 {mc} 的版本"><TriangleAlert size={11} />没有合适的版本</span>
                {:else if settings.autoDeps}
                  <span class="badge gray">会一起安装</span>
                {:else}
                  <button class="btn btn-sm" onclick={() => onpickdep(d, null)}>安装</button>
                {/if}
              </li>
            {/each}
          </ul>
          {#if !settings.autoDeps && needDeps.some((d) => d.available)}
            <p class="faint warn-line">没装前置的话，游戏很可能会启动失败。</p>
          {/if}
        {/if}
      </section>
    {/if}

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
      <button class="btn btn-sm btn-primary" disabled={!pick || (isPack && nameTouched && !!nameErr)} onclick={confirm}>
        {#if isPack}安装整合包{:else if pick && same(pick)}重新安装这个版本{:else if reinstall}换成 {pick?.versionNumber ?? "…"}{:else}安装 {pick?.versionNumber ?? ""}{/if}{#if withCount && !isPack}（含 {withCount} 个前置）{/if}
      </button>
    </footer>
  </Modal>

<style>
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
    max-height: min(320px, 44vh);
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
    transition:
      background 0.12s var(--ease),
      border-color 0.12s var(--ease);
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
    transition: border 0.12s var(--ease);
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
  .box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border-radius: var(--r);
    background: var(--bg);
    border: 1px solid var(--border);
  }
  .box > .faint {
    font-size: 11.5px;
  }
  .name-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    font-weight: 600;
  }
  .name-row .input {
    flex: 1;
    min-width: 0;
    font-weight: 400;
  }
  .deps-head {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    min-height: 22px;
  }
  .deps-head .faint {
    font-size: 12px;
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
    cursor: pointer;
  }
  .box ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 140px;
    overflow-y: auto;
  }
  .box li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 30px;
    font-size: 12.5px;
  }
  .box img,
  .dph {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    flex-shrink: 0;
    object-fit: cover;
    background: var(--hover);
  }
  .dph {
    display: grid;
    place-items: center;
    font-size: 11px;
    font-weight: 800;
    color: var(--text-2);
  }
  .dt {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    overflow: hidden;
  }
  .dn {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dok {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    max-width: 50%;
    font-size: 11.5px;
    color: var(--green);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dok :global(svg) {
    flex-shrink: 0;
    align-self: center;
  }
  .warn-line {
    font-size: 11.5px;
    color: var(--amber);
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
