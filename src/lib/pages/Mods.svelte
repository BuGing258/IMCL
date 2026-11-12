<script lang="ts">
  import { Search, Download as DownloadIcon, Check, LoaderCircle, Info, Puzzle, Sun, Image, ExternalLink, RefreshCw } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Select from "$lib/components/Select.svelte";
  import { library, toast, ui, nav, fmtCount, isTauri, settings } from "$lib/state.svelte";
  import { RES_LABELS, type ResKind } from "$lib/mock";
  import { api, type Hit } from "$lib/api";

  const KINDS: { k: ResKind; icon: typeof Puzzle }[] = [
    { k: "mod", icon: Puzzle },
    { k: "shader", icon: Sun },
    { k: "resourcepack", icon: Image },
  ];

  /** Modrinth 分类 → 中文 */
  const CATS: Record<ResKind, [string, string][]> = {
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
  /** 刚装上的（识别结果回来之前先当作已安装） */
  let justInstalled = $state<Record<string, boolean>>({});

  const pack = $derived(library.current);
  const isMod = $derived(ui.resKind === "mod");
  const hasIris = $derived(!!pack && pack.mods.some((m) => m.enabled && /iris/i.test(m.id + m.name)));
  const shaderReady = $derived(!!pack && (!!pack.optifine || hasIris));
  const canInstall = $derived(!!pack && (!isMod || pack.loader !== "原版"));

  /** 光影按 Iris / OptiFine 过滤；Mod 按加载器过滤 */
  const loaderFilter = $derived.by(() => {
    if (!pack) return null;
    if (ui.resKind === "mod") return pack.loader === "原版" ? null : pack.loader.toLowerCase();
    if (ui.resKind === "shader") return hasIris ? "iris" : pack.optifine ? "optifine" : null;
    return null;
  });

  const installedIds = $derived(new Set(Object.values(pack?.projects ?? {}).map((p) => p.projectId)));
  function has(h: Hit) {
    if (justInstalled[`${pack?.id}:${h.id}`]) return true;
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
    loading = true;
    loadErr = "";
    try {
      const r = await api.searchResources({
        kind: ui.resKind,
        query: translate(ui.resQuery),
        mc: pack?.mc ?? null,
        loader: loaderFilter,
        category: cat || null,
        sort,
        offset: append ? hits.length : 0,
      });
      if (my !== seq) return;
      hits = append ? [...hits, ...r.hits] : r.hits;
      total = r.total;
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

  function setKind(k: ResKind) {
    if (ui.resKind === k) return;
    ui.resKind = k;
    cat = "";
    hits = [];
  }

  async function install(h: Hit) {
    if (!pack || !canInstall || installing[h.id] || has(h)) return;
    const target = pack;
    const kind = ui.resKind;
    installing[h.id] = true;
    try {
      const r = await api.installResource({
        gameDir: settings.gameDir,
        version: target.name,
        kind,
        projectId: h.id,
        mc: target.mc,
        loader: kind === "mod" ? target.loader : loaderFilter,
      });
      justInstalled[`${target.id}:${h.id}`] = true;
      let msg = `已把 ${h.title} 装进 ${target.name}`;
      if (r.deps.length) msg += `，并装上前置 ${r.deps.join("、")}`;
      toast.show(msg);
      if (r.missing.length) setTimeout(() => toast.show(`有前置没找到适合的版本：${r.missing.join("、")}`), 2000);
      if (target.real) await library.loadFiles(target, true);
      else if (kind === "mod") target.mods.push({ id: h.slug, name: h.title, version: "最新", size: "—", enabled: true });
      else (kind === "shader" ? target.shaders : target.resourcepacks).push(h.slug);
    } catch (e) {
      toast.show(`安装失败：${e}`);
    } finally {
      delete installing[h.id];
    }
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

<div class="page-scroll">
  <div class="wrap">
    <div class="page-head">
      <div>
        <h1>Mod</h1>
        <p>Mod、光影、材质包都从 Modrinth 下载，会自动装上需要的前置</p>
      </div>
      {#if pack}
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

    {#if !pack}
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
        <input class="input" placeholder="搜索{RES_LABELS[ui.resKind]}（英文名更准，常用中文也能搜）" bind:value={ui.resQuery} />
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

    {#if pack}
      <p class="filter-note">
        只显示适合 {pack.mc}{isMod && pack.loader !== "原版" ? ` ${pack.loader}` : ""}{loaderFilter && !isMod
          ? `（${loaderFilter === "iris" ? "Iris" : "OptiFine"}）`
          : ""} 的{isMod ? " Mod" : RES_LABELS[ui.resKind]}{total ? ` · 共 ${fmtCount(total)} 个` : ""}
      </p>
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
                {#each h.categories.filter((c) => CAT_ZH[c]).slice(0, 2) as c}
                  <span class="badge gray">{CAT_ZH[c]}</span>
                {/each}
                <span class="faint dl"><DownloadIcon size={12} /> {fmtCount(h.downloads)}</span>
              </div>
            </div>
            <div class="ma">
              {#if installed}
                <span class="badge green"><Check size={12} />已安装</span>
              {:else if installing[h.id]}
                <button class="btn btn-sm" disabled><LoaderCircle size={14} class="spin" />安装中</button>
              {:else}
                <button class="btn btn-sm" disabled={!canInstall} onclick={() => install(h)}>安装</button>
              {/if}
            </div>
          </div>
        {:else}
          {#if !loading}
            <p class="none">
              {ui.resQuery ? `没找到「${ui.resQuery}」，试试英文名` : `没有适合这个版本的${RES_LABELS[ui.resKind]}`}
            </p>
          {/if}
        {/each}
      </div>
      {#if loading}
        <p class="state faint"><LoaderCircle size={16} class="spin" />正在搜索…</p>
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
    grid-template-columns: repeat(3, minmax(0, 1fr));
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
    border: 1px solid var(--border);
    font-size: 14px;
    font-weight: 600;
    color: var(--text-2);
    transition: border-color 0.2s, color 0.2s, background 0.2s;
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
    transition: opacity 0.2s;
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
    border: 1px solid var(--border);
    transition: border-color 0.2s;
    animation: rise 0.4s var(--ease) backwards;
    animation-delay: calc(min(var(--i), 12) * 25ms);
    min-width: 0;
  }
  .mod:hover {
    border-color: var(--border-strong);
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
    transition: opacity 0.15s;
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
  }
  .dl {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .ma {
    flex-shrink: 0;
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
