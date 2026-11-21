<script lang="ts">
  import { Search, Download as DownloadIcon, Check, LoaderCircle, Info, Puzzle, Sun, Image, ExternalLink, RefreshCw, Package, X, FolderInput } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Select from "$lib/components/Select.svelte";
  import ResVersionPicker from "$lib/components/ResVersionPicker.svelte";
  import ImportPackDialog from "$lib/components/ImportPackDialog.svelte";
  import { library, toast, ui, nav, fmtCount, isTauri, settings, installs, cached, remember } from "$lib/state.svelte";
  import { RES_LABELS, type MarketKind } from "$lib/mock";
  import { api, type Hit, type ModVersion, type ResDep } from "$lib/api";

  const KINDS: { k: MarketKind; icon: typeof Puzzle }[] = [
    { k: "mod", icon: Puzzle },
    { k: "shader", icon: Sun },
    { k: "resourcepack", icon: Image },
    { k: "modpack", icon: Package },
  ];

  /** Modrinth 分类 → 中文 */
  const CATS: Record<MarketKind, [string, string][]> = {
    modpack: [
      ["optimization", "优化"],
      ["adventure", "冒险"],
      ["technology", "科技"],
      ["magic", "魔法"],
      ["quests", "任务"],
      ["multiplayer", "多人"],
      ["lightweight", "轻量"],
      ["kitchen-sink", "大杂烩"],
      ["challenging", "挑战"],
      ["combat", "战斗"],
    ],
    mod: [
      ["optimization", "优化"],
      ["utility", "实用"],
      ["technology", "科技"],
      ["adventure", "冒险"],
      ["decoration", "装饰"],
      ["magic", "魔法"],
      ["worldgen", "世界生成"],
      ["equipment", "装备"],
      ["food", "食物"],
      ["mobs", "生物"],
      ["storage", "存储"],
      ["library", "前置库"],
    ],
    shader: [
      ["vanilla-like", "原版风"],
      ["semi-realistic", "半写实"],
      ["realistic", "写实"],
      ["fantasy", "奇幻"],
      ["cartoon", "卡通"],
      ["potato", "低配"],
    ],
    resourcepack: [
      ["vanilla-like", "原版风"],
      ["realistic", "写实"],
      ["simplistic", "简约"],
      ["tweaks", "微调"],
      ["themed", "主题"],
      ["decoration", "装饰"],
      ["combat", "战斗"],
      ["utility", "实用"],
    ],
  };
  const CAT_ZH: Record<string, string> = Object.fromEntries(Object.values(CATS).flat());

  /** Modrinth 只认英文，常用的中文关键词先翻译一下 */
  const ZH_QUERY: Record<string, string> = {
    钠: "sodium",
    锂: "lithium",
    光影: "iris",
    小地图: "minimap",
    地图: "map",
    物品管理器: "jei",
    合成表: "jei",
    机械动力: "create",
    农夫乐事: "farmer's delight",
    精妙背包: "sophisticated backpacks",
    背包: "backpack",
    匠魂: "tinkers construct",
    暮色森林: "twilight forest",
    植物魔法: "botania",
    应用能源: "applied energistics",
    工业: "industrial",
    优化: "optimization",
    连锁挖矿: "vein mine",
    一键整理: "inventory sorting",
    整理: "sorting",
    饱和度: "appleskin",
    显示信息: "jade",
    动画: "animations",
    高清: "32x",
    原版风: "vanilla",
    家具: "furniture",
    装饰: "decoration",
    皮肤: "skin",
    语音: "voice chat",
    语音聊天: "simple voice chat",
    帧率: "fps",
    内存: "memory",
  };
  const translate = (q: string) => ZH_QUERY[q.trim()] ?? q.trim();

  let cat = $state("");
  let sort = $state<"relevance" | "downloads" | "updated">("relevance");
  let hits = $state<Hit[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let loadErr = $state("");
  let installing = $state<Record<string, boolean>>({});
  /** 刚装上的（识别结果回来之前先当作已安装），值是版本号 */
  let justInstalled = $state<Record<string, string>>({});
  /** 打开的版本选择窗口（给前置选版本时会叠在上面） */
  let stack = $state<{ hit: Hit; kind: MarketKind; installed?: string | null }[]>([]);

  const pack = $derived(library.current);
  const isMod = $derived(ui.resKind === "mod");
  const isPack = $derived(ui.resKind === "modpack");
  const hasIris = $derived(!!pack && pack.mods.some((m) => m.enabled && /iris/i.test(m.id + m.name)));
  const shaderReady = $derived(!!pack && (!!pack.optifine || hasIris));
  const canInstall = $derived(isPack || (!!pack && (!isMod || pack.loader !== "原版")));

  /** 光影按 Iris / OptiFine 过滤；Mod 按加载器过滤 */
  const loaderFilter = $derived.by(() => {
    if (!pack) return null;
    if (ui.resKind === "mod") return pack.loader === "原版" ? null : pack.loader.toLowerCase();
    if (ui.resKind === "shader") return hasIris ? "iris" : pack.optifine ? "optifine" : null;
    return null;
  });

  const installedIds = $derived(new Set(Object.values(pack?.projects ?? {}).map((p) => p.projectId)));
  function has(h: Hit) {
    if (isPack) return false;
    if (justInstalled[`${pack?.id}:${h.id}`] != null) return true;
    if (installedIds.has(h.id)) return true;
    // 浏览器预览的演示数据按名字判断
    if (pack && !pack.real) {
      const list = ui.resKind === "mod" ? pack.mods.map((m) => m.id) : ui.resKind === "shader" ? pack.shaders : pack.resourcepacks;
      return list.some((x) => x.toLowerCase().includes(h.slug));
    }
    return false;
  }

  // 真实版本先读一下文件夹，才知道哪些已经装了
  $effect(() => {
    if (pack?.real && !pack.filesLoaded) library.loadFiles(pack).catch(() => {});
  });

  /* ---------- 搜索（输入停顿后再搜） ---------- */
  let seq = 0;
  async function runSearch(append = false) {
    const my = ++seq;
    const req = {
      kind: ui.resKind,
      query: translate(ui.resQuery),
      // 整合包自己决定游戏版本，不按当前版本过滤
      mc: isPack ? null : (pack?.mc ?? null),
      loader: isPack ? null : loaderFilter,
      category: cat || null,
      sort,
    };
    // 5 分钟内搜过一样的（比如切到别的页面又回来），直接用上次的结果
    const key = `search:${JSON.stringify(req)}`;
    const hit = append ? undefined : cached<{ hits: Hit[]; total: number }>(key, 5 * 60_000);
    if (hit) {
      hits = hit.hits;
      total = hit.total;
      loading = false;
      loadErr = "";
      return;
    }
    loading = true;
    loadErr = "";
    try {
      const r = await api.searchResources({ ...req, offset: append ? hits.length : 0 });
      if (my !== seq) return;
      hits = append ? [...hits, ...r.hits] : r.hits;
      total = r.total;
      remember(key, { hits, total });
    } catch (e) {
      if (my === seq) loadErr = String(e);
    } finally {
      if (my === seq) loading = false;
    }
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    // 依赖：类型、关键词、分类、排序、目标版本
    void [ui.resKind, ui.resQuery, cat, sort, pack?.id, loaderFilter];
    clearTimeout(timer);
    timer = setTimeout(() => runSearch(false), ui.resQuery ? 350 : 0);
    return () => clearTimeout(timer);
  });

  function setKind(k: MarketKind) {
    if (ui.resKind === k) return;
    ui.resKind = k;
    cat = "";
    hits = [];
  }

  /** 某个项目在当前版本里装的版本号：空字符串 = 装了但不知道版本，null = 没装 */
  function installedOf(projectId: string): string | null {
    const just = justInstalled[`${pack?.id}:${projectId}`];
    if (just != null) return just;
    const f = Object.values(pack?.projects ?? {}).find((p) => p.projectId === projectId);
    return f ? f.versionNumber : null;
  }

  /** 卡片上的资源装的版本号（演示数据按名字判断，版本未知） */
  function installedVersion(h: Hit): string | null {
    if (!has(h)) return null;
    return installedOf(h.id) ?? "";
  }

  const apiLoader = (kind: MarketKind) => (kind === "mod" ? (pack?.loader ?? null) : kind === "shader" ? loaderFilter : null);

  function openPicker(h: Hit, kind: MarketKind = ui.resKind, installed?: string | null) {
    if (installing[h.id] || stack.some((x) => x.hit.id === h.id)) return;
    if (kind !== "modpack" && (!pack || !canInstall)) return;
    const task = installs.forModpack(h.id);
    if (task?.status === "error") installs.cancel(task.id); // 失败的先清掉，重新来
    stack.push({ hit: h, kind, installed });
  }

  const closePicker = (id: string) => (stack = stack.filter((x) => x.hit.id !== id));

  /** 给前置选版本：在当前窗口上面再开一个 */
  function pickDep(d: ResDep, installed: string | null) {
    openPicker(
      { id: d.projectId, slug: d.projectId, title: d.title, author: "", description: "", downloads: 0, icon: d.icon, categories: [], color: null },
      "mod",
      installed,
    );
  }

  function installModpack(h: Hit, v: ModVersion, name: string) {
    closePicker(h.id);
    installs.startModpack({
      name,
      mc: v.gameVersions[0] ?? "",
      loader: v.loaders.find((l) => l !== "minecraft") ?? null,
      projectId: h.id,
      versionId: v.id,
      title: h.title,
    });
    toast.show(`开始安装 ${h.title}，完成后会出现在版本管理里`);
  }

  /* ---------- 从本地文件导入整合包 ---------- */
  let importFile = $state<string | null>(null);
  const fileTasks = $derived(Object.values(installs.tasks).filter((t) => t.modpack?.startsWith("file:")));

  async function pickPackFile() {
    const f = await api.pickMrpack();
    if (f) importFile = f;
  }

  /** 整合包安装进度（0~100） */
  function packPct(t: { stageIndex: number; stageCount: number; done: number; total: number }) {
    const part = t.total ? t.done / t.total : 0;
    return Math.min(99, Math.floor(((Math.max(1, t.stageIndex) - 1 + part) / Math.max(1, t.stageCount)) * 100));
  }

  async function install(h: Hit, v: ModVersion, withDeps = true, kind: MarketKind = ui.resKind) {
    closePicker(h.id);
    if (!pack || installing[h.id]) return;
    const target = pack;
    const reinstall = installedOf(h.id) != null || has(h);
    installing[h.id] = true;
    try {
      const r = await api.installResource({
        gameDir: settings.gameDir,
        version: target.name,
        kind,
        projectId: h.id,
        mc: target.mc,
        loader: apiLoader(kind),
        versionId: v.id,
        reinstall,
        withDeps,
      });
      justInstalled[`${target.id}:${h.id}`] = r.versionNumber;
      let msg = reinstall ? `已把 ${h.title} 重新安装为 ${r.versionNumber}` : `已把 ${h.title} ${r.versionNumber} 装进 ${target.name}`;
      if (r.deps.length) msg += `，并装上前置 ${r.deps.join("、")}`;
      toast.show(msg);
      if (r.missing.length) setTimeout(() => toast.show(`有前置没找到适合的版本：${r.missing.join("、")}`), 2000);
      if (target.real) await library.loadFiles(target, true);
      else if (!reinstall) {
        if (kind === "mod") target.mods.push({ id: h.slug, name: h.title, version: r.versionNumber, size: "—", enabled: true });
        else (kind === "shader" ? target.shaders : target.resourcepacks).push(h.slug);
      }
    } catch (e) {
      toast.show(`安装失败：${e}`);
    } finally {
      delete installing[h.id];
    }
  }

  /** 已安装标签展开后最多这么宽，版本号放不下就来回滚动 */
  const VER_MAX = 120;
  function measureVersion(e: MouseEvent) {
    const ver = (e.currentTarget as HTMLElement).querySelector<HTMLElement>(".ver");
    const text = ver?.firstElementChild as HTMLElement | null;
    if (!ver || !text) return;
    const over = text.scrollWidth - VER_MAX;
    ver.classList.toggle("over", over > 0);
    ver.style.setProperty("--shift", `${-Math.max(0, over)}px`);
    ver.style.setProperty("--dur", `${Math.max(1.2, over / 30)}s`);
  }

  function openPage(h: Hit) {
    const url = `https://modrinth.com/${ui.resKind}/${h.slug}`;
    if (isTauri()) openUrl(url).catch((e) => toast.show(String(e)));
    else window.open(url, "_blank");
  }

  function findIris() {
    setKind("mod");
    ui.resQuery = "iris";
  }

  const hex = (c: number | null) => (c == null ? "#888780" : `#${c.toString(16).padStart(6, "0")}`);
</script>

{#if importFile}
  <ImportPackDialog file={importFile} onclose={() => (importFile = null)} />
{/if}

{#each stack as { hit: h, kind, installed }, i (h.id)}
  {#if kind === "modpack"}
    <ResVersionPicker
      hit={h}
      {kind}
      mc=""
      loader={null}
      target=""
      gameDir={settings.gameDir}
      active={i === stack.length - 1}
      onclose={() => closePicker(h.id)}
      onpick={(v, _deps, name) => installModpack(h, v, name ?? h.title)}
    />
  {:else if pack}
    <ResVersionPicker
      hit={h}
      {kind}
      mc={pack.mc}
      loader={apiLoader(kind)}
      target={pack.name}
      gameDir={settings.gameDir}
      installedVersion={i === 0 ? installedVersion(h) : (installedOf(h.id) ?? installed ?? null)}
      {installedOf}
      busyOf={(id) => !!installing[id]}
      active={i === stack.length - 1}
      onclose={() => closePicker(h.id)}
      onpick={(v, withDeps) => install(h, v, withDeps, kind)}
      onpickdep={pickDep}
    />
  {/if}
{/each}

<div class="page-scroll">
  <div class="wrap">
    <div class="page-head">
      <div>
        <h1>Mod</h1>
        <p>下载 Mod、光影、材质包和整合包</p>
      </div>
      {#if pack && !isPack}
        <label class="target">
          <span class="muted">安装到</span>
          <Select
            bind:value={library.currentId}
            width="240px"
            ariaLabel="安装到哪个版本"
            options={library.packs.map((p) => ({ value: p.id, label: p.name, hint: `${p.mc} ${p.loader}` }))}
          />
        </label>
      {/if}
    </div>

    <div class="kinds">
      {#each KINDS as { k, icon: Icon }}
        <button class="kind" class:on={ui.resKind === k} onclick={() => setKind(k)}>
          <Icon size={18} />{RES_LABELS[k]}
        </button>
      {/each}
    </div>

    {#if isPack}
      <!-- 整合包会新建版本，不需要先选版本 -->
    {:else if !pack}
      <div class="warn">
        <Info size={16} />
        <span>还没有安装任何版本，先去下载一个游戏版本。</span>
        <button class="btn btn-sm" onclick={() => nav.go("download")}>下载游戏</button>
      </div>
    {:else if !canInstall}
      <div class="warn">
        <Info size={16} />
        <span>原版不能安装 Mod。请在右上角换成 Fabric、Forge 之类的版本。</span>
      </div>
    {:else if ui.resKind === "shader" && !shaderReady}
      <div class="warn">
        <Info size={16} />
        <span>
          {pack.loader === "原版"
            ? "原版用不了光影，需要装 OptiFine，或换成装了 Iris 的 Fabric 版本。"
            : pack.loader === "Forge"
              ? "当前版本没有 OptiFine 或 Iris，光影装了也不会生效。"
              : "当前版本还没装 Iris，光影装了也不会生效。"}
        </span>
        {#if pack.loader !== "原版" && pack.loader !== "Forge"}<button class="btn btn-sm" onclick={findIris}>去装 Iris</button>{/if}
      </div>
    {/if}

    <div class="toolbar">
      <div class="search">
        <Search size={16} />
        <input class="input" placeholder="搜索{RES_LABELS[ui.resKind]}（英文名更准）" bind:value={ui.resQuery} />
      </div>
      <Select
        bind:value={cat}
        width="130px"
        ariaLabel="分类"
        options={[{ value: "", label: "全部分类" }, ...CATS[ui.resKind].map(([value, label]) => ({ value, label }))]}
      />
      <Select
        bind:value={sort}
        width="120px"
        ariaLabel="排序"
        options={[
          { value: "relevance", label: "最相关" },
          { value: "downloads", label: "下载最多" },
          { value: "updated", label: "最近更新" },
        ]}
      />
    </div>

    {#if isPack}
      <div class="pack-bar">
        <p class="filter-note">{total ? `共 ${fmtCount(total)} 个` : ""}</p>
        <button class="btn btn-sm" onclick={pickPackFile} title="导入 .mrpack 整合包"><FolderInput size={14} />从文件导入</button>
      </div>
      {#each fileTasks as t (t.id)}
        <div class="file-task" class:bad={t.status === "error"}>
          <Package size={15} />
          <span class="ft-name">导入 {t.name}</span>
          {#if t.status === "error"}
            <span class="ft-err" title={t.error}>失败：{t.error}</span>
            <button class="btn btn-sm btn-icon" onclick={() => installs.cancel(t.id)} aria-label="关闭" title="关闭"><X size={13} /></button>
          {:else}
            <span class="ft-stage">{t.stage}</span>
            <span class="pk-bar"><i style:width="{packPct(t)}%"></i></span>
            <span class="pk-pct">{packPct(t)}%</span>
            <button class="btn btn-sm btn-icon" onclick={() => installs.cancel(t.id)} disabled={t.status === "cancelling"} aria-label="取消导入" title="取消导入"><X size={13} /></button>
          {/if}
        </div>
      {/each}
    {:else if pack && total}
      <p class="filter-note">共 {fmtCount(total)} 个</p>
    {/if}

    {#if loadErr}
      <div class="state">
        <p class="err-text">{loadErr}</p>
        <button class="btn btn-sm" onclick={() => runSearch(false)}><RefreshCw size={14} />重试</button>
      </div>
    {:else}
      <div class="grid" class:dim={loading && !hits.length}>
        {#each hits as h, idx (h.id)}
          {@const installed = has(h)}
          <div class="mod" style:--i={idx % 24}>
            {#if h.icon}
              <img class="mic" src={h.icon} alt="" loading="lazy" />
            {:else}
              <div class="mic" style:background={hex(h.color)}>{h.title[0]}</div>
            {/if}
            <div class="mt">
              <div class="mt-top">
                <button class="title" onclick={() => openPage(h)} title="在 Modrinth 上查看">{h.title}<ExternalLink size={11} /></button>
                <span class="faint">by {h.author}</span>
              </div>
              <p class="desc" title={h.description}>{h.description}</p>
              <div class="mt-meta">
                {#if installed}
                  {@const iv = installedVersion(h)}
                  <span class="badge green inst" role="note" aria-label={iv ? `已安装 ${iv}` : "已安装"} onmouseenter={measureVersion}>
                    <Check size={11} />已安装
                    {#if iv}<span class="ver"><span class="ver-t">{iv}</span></span>{/if}
                  </span>
                {/if}
                {#each h.categories.filter((c) => CAT_ZH[c]).slice(0, 2) as c}
                  <span class="badge gray">{CAT_ZH[c]}</span>
                {/each}
                <span class="faint dl"><DownloadIcon size={12} /> {fmtCount(h.downloads)}</span>
              </div>
            </div>
            <div class="ma">
              {#if isPack}
                {@const task = installs.forModpack(h.id)}
                {#if task?.status === "error"}
                  <button class="btn btn-sm fail" onclick={() => openPicker(h)} title={task.error}>安装失败，重试</button>
                {:else if task}
                  <span class="pk-run" title={task.stage}>
                    <span class="pk-bar"><i style:width="{packPct(task)}%"></i></span>
                    <span class="pk-pct">{packPct(task)}%</span>
                    <button class="btn btn-sm btn-icon" onclick={() => installs.cancel(task.id)} disabled={task.status === "cancelling"} aria-label="取消安装" title="取消安装"><X size={13} /></button>
                  </span>
                {:else}
                  <button class="btn btn-sm" onclick={() => openPicker(h)}>安装</button>
                {/if}
              {:else if installing[h.id]}
                <button class="btn btn-sm" disabled><LoaderCircle size={14} class="spin" />安装中</button>
              {:else if installed}
                <button class="btn btn-sm" disabled={!canInstall} onclick={() => openPicker(h)} title="换个版本或重新下载">重新安装</button>
              {:else}
                <button class="btn btn-sm" disabled={!canInstall} onclick={() => openPicker(h)}>安装</button>
              {/if}
            </div>
          </div>
        {:else}
          {#if loading}
            {#each Array(6) as _, k (k)}
              <div class="mod sk-card" aria-hidden="true">
                <span class="skel sk-ic"></span>
                <div class="mt">
                  <span class="skel" style:width="{40 + ((k * 17) % 30)}%"></span>
                  <span class="skel sk-line"></span>
                  <span class="skel sk-short"></span>
                </div>
              </div>
            {/each}
          {:else}
            <p class="none">
              {ui.resQuery ? `没找到「${ui.resQuery}」，试试英文名` : isPack ? "没有找到整合包" : `没有适合这个版本的${RES_LABELS[ui.resKind]}`}
            </p>
          {/if}
        {/each}
      </div>
      {#if loading && hits.length}
        <p class="state faint"><LoaderCircle size={16} class="spin" />正在加载…</p>
      {:else if hits.length < total}
        <div class="state"><button class="btn" onclick={() => runSearch(true)}>加载更多</button></div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .wrap {
    max-width: 880px;
    margin: 0 auto;
  }
  .target {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .kinds {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
    margin-bottom: 14px;
  }
  .kind {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 44px;
    border-radius: var(--r-lg);
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    font-size: 14px;
    font-weight: 600;
    color: var(--text-2);
    transition: border-color 0.2s var(--ease), color 0.2s var(--ease), background 0.2s var(--ease);
  }
  .kind:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }
  .kind.on {
    border-color: var(--red);
    background: var(--red-soft);
    color: var(--red);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 14px;
    min-height: 40px;
    margin-bottom: 12px;
    border-radius: var(--r);
    background: var(--amber-soft);
    color: var(--amber);
    font-size: 13px;
  }
  .warn span {
    flex: 1;
  }
  .toolbar {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-bottom: 6px;
  }
  .search {
    flex: 1;
    min-width: 160px;
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
  .filter-note {
    font-size: 12px;
    color: var(--text-3);
    margin: 0 2px 10px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 8px;
    transition: opacity 0.2s var(--ease);
  }
  .grid.dim {
    opacity: 0.5;
  }
  .mod {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    border-radius: var(--r-lg);
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    transition:
      border-color 0.2s var(--ease),
      transform 0.35s var(--ease-spring),
      box-shadow 0.3s var(--ease);
    min-width: 0;
  }
  .sk-card {
    pointer-events: none;
  }
  .sk-ic {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    flex-shrink: 0;
  }
  .sk-card .mt {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .sk-line {
    width: 85%;
  }
  .sk-short {
    width: 30%;
    height: 10px;
  }
  .mod:hover {
    border-color: var(--border-strong);
    transform: translateY(-2px);
    box-shadow: var(--shadow);
  }
  .mic {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    color: #fff;
    font-weight: 800;
    font-size: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    object-fit: cover;
    background: var(--hover);
  }
  .mt {
    flex: 1;
    min-width: 0;
  }
  .mt-top {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: 12px;
    min-width: 0;
  }
  .title {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    min-width: 0;
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title :global(svg) {
    flex-shrink: 0;
    color: var(--text-3);
    opacity: 0;
    transition: opacity 0.15s var(--ease);
  }
  .title:hover {
    color: var(--red);
  }
  .title:hover :global(svg) {
    opacity: 1;
  }
  .mt-top .faint {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .desc {
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mt-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    font-size: 12px;
    min-width: 0;
    overflow: hidden;
  }
  .dl {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .ma {
    flex-shrink: 0;
  }
  .fail {
    color: var(--red);
    border-color: var(--red);
  }
  .pack-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }
  .pack-bar .filter-note {
    margin-bottom: 0;
  }
  .file-task {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 8px 12px;
    margin-bottom: 8px;
    border-radius: var(--r);
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    font-size: 12.5px;
  }
  .file-task.bad {
    border-color: var(--red);
  }
  .ft-name {
    font-weight: 600;
    white-space: nowrap;
  }
  .ft-stage,
  .ft-err {
    flex: 1;
    min-width: 0;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ft-err {
    color: var(--red);
  }
  .pk-run {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pk-bar {
    width: 64px;
    height: 5px;
    border-radius: 3px;
    background: var(--hover);
    overflow: hidden;
  }
  .pk-bar i {
    display: block;
    height: 100%;
    background: var(--red);
    border-radius: 3px;
    transition: width 0.3s var(--ease);
  }
  .pk-pct {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    min-width: 30px;
    text-align: right;
  }
  .mt-meta .badge {
    flex-shrink: 0;
  }
  /* 已安装：平时只显示“已安装”，鼠标移上去展开版本号，太长就来回滚动 */
  .ver {
    display: inline-block;
    max-width: 0;
    margin-left: -4px;
    overflow: hidden;
    white-space: nowrap;
    font-family: var(--mono);
    font-weight: 500;
    transition:
      max-width 0.25s var(--ease),
      margin-left 0.25s var(--ease);
  }
  .inst:hover .ver {
    max-width: 120px;
    margin-left: 0;
  }
  .ver-t {
    display: inline-block;
  }
  .inst:hover .ver:global(.over) .ver-t {
    animation: ver-scroll var(--dur) ease-in-out 0.6s infinite alternate;
  }
  @keyframes ver-scroll {
    0%,
    15% {
      transform: translateX(0);
    }
    85%,
    100% {
      transform: translateX(var(--shift));
    }
  }
  .none {
    grid-column: 1 / -1;
    text-align: center;
    color: var(--text-3);
    padding: 40px 0;
    font-size: 13px;
  }
  .state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 16px 0;
    font-size: 13px;
  }
</style>
