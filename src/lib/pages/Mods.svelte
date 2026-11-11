<script lang="ts">
  import { Search, Download as DownloadIcon, Check, LoaderCircle, Info, Puzzle, Sun, Image } from "@lucide/svelte";
  import Select from "$lib/components/Select.svelte";
  import { library, toast, ui, nav, fmtCount } from "$lib/state.svelte";
  import { mockMarket, RES_CATEGORIES, RES_LABELS, type MarketItem, type ResKind } from "$lib/mock";

  const KINDS: { k: ResKind; icon: typeof Puzzle }[] = [
    { k: "mod", icon: Puzzle },
    { k: "shader", icon: Sun },
    { k: "resourcepack", icon: Image },
  ];

  let cat = $state("全部");
  let installing = $state<Record<string, boolean>>({});

  const pack = $derived(library.current);
  // 真实版本里 Mod 的 id 是文件名（比如 iris-1.7.0+mc1.20.1.jar），按包含关系判断
  const hasMod = (id: string) => !!pack?.mods.some((x) => x.id.toLowerCase().includes(id));
  const shaderReady = $derived(
    !!pack && (!!pack.optifine || pack.mods.some((m) => m.enabled && m.id.toLowerCase().includes("iris"))),
  );
  const cats = $derived(["全部", ...RES_CATEGORIES[ui.resKind]]);

  // Mod 需要加载器；光影和材质包任何版本都能装（光影要 Iris 或 OptiFine 才能生效，下面会提示）
  const canInstall = $derived(!!pack && (ui.resKind !== "mod" || pack.loader !== "原版"));

  // 真实版本先读一下文件夹，才知道哪些已经装了
  $effect(() => {
    if (pack?.real && !pack.filesLoaded) library.loadFiles(pack).catch(() => {});
  });

  const results = $derived(
    mockMarket.filter((m) => {
      if (m.kind !== ui.resKind) return false;
      const q = ui.resQuery.trim().toLowerCase();
      const hit = !q || m.name.toLowerCase().includes(q) || m.id.includes(q) || m.desc.includes(q);
      return hit && (cat === "全部" || m.category === cat);
    }),
  );

  function setKind(k: ResKind) {
    ui.resKind = k;
    cat = "全部";
  }

  function has(m: MarketItem) {
    if (!pack) return false;
    if (m.kind === "mod") return hasMod(m.id);
    return (m.kind === "shader" ? pack.shaders : pack.resourcepacks).some((x) => x.toLowerCase().includes(m.id));
  }

  function install(m: MarketItem) {
    if (!pack || !canInstall || installing[m.id] || has(m)) return;
    if (pack.real) {
      toast.show("Mod / 光影 / 材质包下载还没接入，下一步做");
      return;
    }
    const target = pack;
    installing[m.id] = true;
    setTimeout(() => {
      delete installing[m.id];
      if (m.kind === "mod") target.mods.push({ id: m.id, name: m.name, version: "最新", size: "—", enabled: true });
      else if (m.kind === "shader") target.shaders.push(m.id);
      else target.resourcepacks.push(m.id);
      toast.show(`已把 ${m.name} 装进 ${target.name}`);
    }, 900);
  }

  function findIris() {
    setKind("mod");
    ui.resQuery = "iris";
  }
</script>

<div class="page-scroll">
  <div class="wrap">
    <div class="page-head">
      <div>
        <h1>Mod</h1>
        <p>Mod、光影、材质包都在这里下载（列表目前是演示数据）</p>
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
            : pack.loader === "Fabric" || pack.loader === "Quilt"
              ? "当前版本还没装 Iris，光影装了也不会生效。"
              : "当前版本没有 OptiFine 或 Iris，光影装了也不会生效。"}
        </span>
        {#if pack.loader === "Fabric" || pack.loader === "Quilt"}<button class="btn btn-sm" onclick={findIris}>去装 Iris</button>{/if}
      </div>
    {/if}

    <div class="toolbar">
      <div class="search">
        <Search size={16} />
        <input class="input" placeholder="搜索{RES_LABELS[ui.resKind]}" bind:value={ui.resQuery} />
      </div>
      <div class="seg">
        {#each cats as c}
          <button class:on={cat === c} onclick={() => (cat = c)}>{c}</button>
        {/each}
      </div>
    </div>

    <div class="grid">
      {#each results as m, idx (m.id)}
        {@const installed = has(m)}
        <div class="mod" style:--i={idx}>
          <div class="mic" style:background={m.color}>{m.name[0]}</div>
          <div class="mt">
            <div class="mt-top">
              <b>{m.name}</b>
              <span class="faint">by {m.author}</span>
            </div>
            <p class="desc">{m.desc}</p>
            <div class="mt-meta">
              <span class="badge gray">{m.category}</span>
              <span class="faint"><DownloadIcon size={12} /> {fmtCount(m.downloads)}</span>
            </div>
          </div>
          <div class="ma">
            {#if installed}
              <span class="badge green"><Check size={12} />已安装</span>
            {:else if installing[m.id]}
              <button class="btn btn-sm" disabled><LoaderCircle size={14} class="spin" />安装中</button>
            {:else}
              <button class="btn btn-sm" disabled={!canInstall} onclick={() => install(m)}>安装</button>
            {/if}
          </div>
        </div>
      {:else}
        <p class="none">没找到「{ui.resQuery}」，换个关键词试试</p>
      {/each}
    </div>
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
    gap: 12px;
    align-items: center;
    margin-bottom: 12px;
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
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 8px;
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
  .mt-top b {
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mt-top .faint {
    white-space: nowrap;
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
    gap: 8px;
    margin-top: 4px;
    font-size: 12px;
  }
  .mt-meta .faint {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .none {
    grid-column: 1 / -1;
    text-align: center;
    color: var(--text-3);
    padding: 40px 0;
    font-size: 13px;
  }
</style>
