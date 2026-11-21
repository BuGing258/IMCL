<script lang="ts">
  import { FolderOpen, X, Check, Download as DownloadIcon, ExternalLink, ImagePlus, Pipette } from "@lucide/svelte";
  import { ACCENTS, fill } from "$lib/theme";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getVersion } from "@tauri-apps/api/app";
  import { appDataDir } from "@tauri-apps/api/path";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import Select from "$lib/components/Select.svelte";
  import { settings, gameDirs, javas, javaOptions, toast, isTauri, fmtMem, installs, appearance, memory, library, type InstallTask } from "$lib/state.svelte";

  const isPreset = $derived(ACCENTS.some((a) => a.color.toLowerCase() === settings.accent.toLowerCase()));
  /** 当前背景材质对应的卡片材质 */
  const card = $derived(appearance.card);

  /* 内存分配：自动时按首页选中的版本算给多少；手动最多给到总内存减 2 GB */
  const plan = $derived(memory.plan(library.current?.name ?? null));
  const memMax = $derived(plan?.maxMb ?? 16384);
  const autoMem = $derived(settings.memoryMode === "auto");
  const memShown = $derived(autoMem ? (plan?.autoMb ?? 0) : Math.min(settings.memory, memMax));
  // 换到内存更小的电脑上时，手动值不能超过上限
  $effect(() => {
    if (plan && settings.memory > plan.maxMb) settings.memory = plan.maxMb;
  });

  if (!javas.offers) javas.loadOffers();

  let version = $state("0.1.0");
  if (isTauri()) getVersion().then((v) => (version = v)).catch(() => {});

  function link(url: string) {
    if (isTauri()) openUrl(url).catch((e) => toast.show(String(e)));
    else window.open(url, "_blank");
  }

  /** 启动器自己的数据：账户、下载的 Java、皮肤缓存等 */
  async function openDataDir() {
    if (!isTauri()) return toast.show("浏览器预览里打不开文件夹");
    try {
      await revealItemInDir(await appDataDir());
    } catch (e) {
      toast.show(`打不开：${e}`);
    }
  }

  /** 下载进度（0~100） */
  const pct = (t: InstallTask) => Math.min(99, Math.floor(((Math.max(1, t.stageIndex) - 1 + (t.total ? t.done / t.total : 0)) / Math.max(1, t.stageCount)) * 100));

  const dirOptions = $derived(settings.gameDirs.map((d) => ({ value: d.path, label: d.name, hint: d.path })));

  async function openDir() {
    if (!isTauri()) return toast.show("浏览器预览里打不开文件夹");
    try {
      await revealItemInDir(settings.gameDir);
    } catch (e) {
      toast.show(`打不开：${e}（文件夹可能还不存在，装一个版本后就有了）`);
    }
  }

  async function browseJava() {
    const p = await javas.browse();
    if (p) settings.java = p;
  }
</script>

<div class="page-scroll">
  <div class="wrap">
    <div class="page-head">
      <div>
        <h1>设置</h1>
        <p>修改后自动保存</p>
      </div>
    </div>

    <section class="group" style:--i={1}>
      <h2>游戏</h2>
      <div class="row">
        <div class="lbl"><b>游戏目录</b></div>
        <div class="ctl">
          <Select
            bind:value={settings.gameDir}
            options={dirOptions}
            actions={[{ label: "添加游戏文件夹…", onclick: () => gameDirs.add() }]}
            width="220px"
            ariaLabel="游戏目录"
            onchange={(v) => gameDirs.switchTo(v)}
          />
          <button class="btn btn-icon" onclick={openDir} aria-label="打开游戏目录" title="打开"><FolderOpen size={15} /></button>
          {#if !gameDirs.isDefault(settings.gameDir)}
            <button class="btn btn-icon" onclick={() => gameDirs.remove(settings.gameDir)} aria-label="从列表移除" title="从列表移除（不删除文件）"><X size={15} /></button>
          {/if}
        </div>
      </div>
      <div class="row">
        <div class="lbl"><b>内存分配</b></div>
        <div class="ctl">
          <div class="seg">
            <button class:on={autoMem} onclick={() => (settings.memoryMode = "auto")}>自动分配</button>
            <button class:on={!autoMem} onclick={() => (settings.memoryMode = "manual")}>手动分配</button>
          </div>
        </div>
      </div>
      <div class="row sub">
        <div class="lbl">{#if plan}<span>电脑内存 {fmtMem(plan.totalMb)}</span>{/if}</div>
        <div class="ctl">
          {#if autoMem}
            <input type="range" class="locked" min="1024" max={memMax} step="512" value={memShown} use:fill={[memShown, memMax]} disabled aria-label="内存占用" />
          {:else}
            <input type="range" min="1024" max={memMax} step="512" bind:value={settings.memory} use:fill={[settings.memory, memMax]} aria-label="内存占用" />
          {/if}
          <span class="val">{memShown ? fmtMem(memShown) : "…"}</span>
        </div>
      </div>
      <div class="row">
        <div class="lbl">
          <b>Java</b>
          {#if javas.found !== null && !javas.all.length}<span>电脑上没有 Java，启动时会自动下载</span>{/if}
        </div>
        <div class="ctl">
          <Select
            bind:value={settings.java}
            options={javaOptions()}
            actions={[{ label: "浏览 java.exe…", onclick: browseJava }]}
            width="260px"
            ariaLabel="Java"
          />
        </div>
      </div>
      <div class="row col">
        <div class="lbl"><b>下载 Java</b></div>
        <div class="jgrid">
          {#each javas.offers ?? [] as o (o.component)}
            {@const t = javas.taskOf(o.component)}
            <div class="jcard" class:ok={!!o.installed && !t}>
              <div class="jt">
                <b>Java {o.major}</b>
                <span>{t && t.status !== "error" ? t.stage : o.forGames}</span>
              </div>
              {#if t?.status === "error"}
                <button class="btn btn-sm fail" title={t.error} onclick={() => (installs.cancel(t.id), javas.download(o))}>失败，重试</button>
              {:else if t}
                <span class="jrun">
                  <span class="jbar"><i style:width="{pct(t)}%"></i></span>
                  <span class="jpct">{pct(t)}%</span>
                  <button class="btn btn-sm btn-icon" onclick={() => installs.cancel(t.id)} disabled={t.status === "cancelling"} aria-label="取消下载" title="取消下载"><X size={13} /></button>
                </span>
              {:else if o.installed}
                <span class="badge green" title={o.installed.path}><Check size={11} />已下载 {o.installed.version}</span>
              {:else}
                <button class="btn btn-sm" onclick={() => javas.download(o)}><DownloadIcon size={13} />下载</button>
              {/if}
            </div>
          {:else}
            <p class="faint">正在读取…</p>
          {/each}
        </div>
      </div>
      <div class="row">
        <div class="lbl"><b>JVM 参数</b><span>不懂可以留空</span></div>
        <div class="ctl">
          <input class="input mono" bind:value={settings.jvmArgs} placeholder="例如 -XX:+UseG1GC" spellcheck="false" aria-label="JVM 参数" />
        </div>
      </div>
      <div class="row">
        <div class="lbl"><b>游戏窗口</b></div>
        <div class="ctl">
          <div class="seg">
            <button class:on={settings.windowMode === "default"} onclick={() => (settings.windowMode = "default")}>默认</button>
            <button class:on={settings.windowMode === "720p"} onclick={() => (settings.windowMode = "720p")}>1280×720</button>
            <button class:on={settings.windowMode === "fullscreen"} onclick={() => (settings.windowMode = "fullscreen")}>全屏</button>
          </div>
        </div>
      </div>
    </section>

    <section class="group" style:--i={2}>
      <h2>外观</h2>
      <div class="row">
        <div class="lbl"><b>主题</b></div>
        <div class="ctl">
          <div class="seg">
            <button class:on={settings.theme === "system"} onclick={() => (settings.theme = "system")}>跟随系统</button>
            <button class:on={settings.theme === "light"} onclick={() => (settings.theme = "light")}>亮色</button>
            <button class:on={settings.theme === "dark"} onclick={() => (settings.theme = "dark")}>暗色</button>
          </div>
        </div>
      </div>
      <div class="row">
        <div class="lbl"><b>主题色</b></div>
        <div class="ctl swatches">
          {#each ACCENTS as a (a.color)}
            <button
              class="sw"
              class:on={a.color.toLowerCase() === settings.accent.toLowerCase()}
              style:--c={a.color}
              title={a.name}
              aria-label="主题色：{a.name}"
              onclick={() => (settings.accent = a.color)}
            ></button>
          {/each}
          <label class="sw custom" class:on={!isPreset} style:--c={isPreset ? "transparent" : settings.accent} title="自定义颜色">
            <input type="color" bind:value={settings.accent} aria-label="自定义主题色" />
            {#if isPreset}<Pipette size={13} />{/if}
          </label>
        </div>
      </div>
      <div class="row">
        <div class="lbl"><b>背景材质</b></div>
        <div class="ctl">
          <div class="seg">
            <button class:on={settings.material === "solid"} onclick={() => (settings.material = "solid")}>实色</button>
            <button
              class:on={settings.material === "mica"}
              disabled={!appearance.win11}
              title={appearance.win11 ? undefined : "需要 Win11"}
              onclick={() => (settings.material = "mica")}>云母</button
            >
          </div>
        </div>
      </div>
      {#if settings.material === "solid"}
        <div class="row sub">
          <div class="lbl"><b>背景图片</b></div>
          <div class="ctl bg-ctl">
            {#if settings.background === "image" && appearance.bgImage}
              <span class="thumb" style:background-image="url({appearance.bgImage})"></span>
              <button class="btn btn-sm" onclick={() => appearance.pickImage()}><ImagePlus size={14} />换一张</button>
            {/if}
            <div class="seg">
              <button class:on={appearance.backgroundKind === "none"} onclick={() => (settings.background = "none")}>无</button>
              <button class:on={appearance.backgroundKind === "image"} onclick={() => (appearance.bgImage ? (settings.background = "image") : appearance.pickImage())}>图片</button>
            </div>
          </div>
        </div>
        {#if appearance.backgroundKind === "image"}
          <div class="row sub">
            <div class="lbl"><b>模糊</b></div>
            <div class="ctl">
              <input type="range" min="0" max="20" step="1" bind:value={settings.bgBlur} use:fill={settings.bgBlur} aria-label="背景图模糊" />
              <span class="val">{settings.bgBlur}</span>
            </div>
          </div>
        {/if}
      {/if}
      <div class="row">
        <div class="lbl"><b>卡片材质</b></div>
        <div class="ctl">
          <div class="seg">
            <button class:on={card.kind === "custom"} onclick={() => (card.kind = "custom")}>自定义</button>
            <button class:on={card.kind === "mica"} onclick={() => (card.kind = "mica")}>云母</button>
          </div>
        </div>
      </div>
      {#if card.kind === "custom"}
        <div class="row sub">
          <div class="lbl"><b>不透明度</b></div>
          <div class="ctl">
            <input type="range" min="0" max="100" step="5" bind:value={card.opacity} use:fill={card.opacity} aria-label="卡片不透明度" />
            <span class="val">{card.opacity}%</span>
          </div>
        </div>
        <div class="row sub">
          <div class="lbl"><b>模糊</b></div>
          <div class="ctl">
            <input type="range" min="0" max="40" step="2" bind:value={card.blur} use:fill={card.blur} aria-label="卡片模糊" />
            <span class="val">{card.blur}</span>
          </div>
        </div>
      {/if}
    </section>

    <section class="group" style:--i={3}>
      <h2>下载</h2>
      <div class="row">
        <div class="lbl"><b>下载源</b><span>国内建议用 BMCLAPI 镜像</span></div>
        <div class="ctl">
          <div class="seg">
            <button class:on={settings.source === "bmcl"} onclick={() => (settings.source = "bmcl")}>BMCLAPI 镜像</button>
            <button class:on={settings.source === "official"} onclick={() => (settings.source = "official")}>官方源</button>
          </div>
        </div>
      </div>
      <div class="row">
        <div class="lbl"><b>同时下载数</b></div>
        <div class="ctl">
          <input type="range" min="8" max="128" step="8" bind:value={settings.threads} use:fill={settings.threads} aria-label="同时下载数" />
          <span class="val">{settings.threads}</span>
        </div>
      </div>
    </section>

    <section class="group" style:--i={4}>
      <h2>关于</h2>
      <div class="row">
        <div class="about">
          <span class="logo">IMCL</span>
          <div class="lbl"><b>IMCL {version}</b><span>Tauri 开发的 Minecraft Java 版启动器</span></div>
        </div>
        <div class="ctl">
          <button class="btn btn-sm" onclick={openDataDir} title="账户、下载的 Java、皮肤缓存都在这里"><FolderOpen size={14} />数据文件夹</button>
        </div>
      </div>
      <div class="row">
        <div class="lbl">
          <b>能做什么</b>
          <span>安装原版、Fabric、Forge、NeoForge、Quilt 和 OptiFine；下载 Mod、光影、材质包和整合包；离线和微软正版登录；缺 Java 时自动下载</span>
        </div>
      </div>
      <div class="row">
        <div class="lbl">
          <b>特别鸣谢</b>
          <span>
            游戏文件来自 Mojang 官方和
            <button class="lnk" onclick={() => link("https://bmclapidoc.bangbang93.com")}>BMCLAPI<ExternalLink size={10} /></button>
            镜像，Mod 和整合包来自
            <button class="lnk" onclick={() => link("https://modrinth.com")}>Modrinth<ExternalLink size={10} /></button>
            和
            <button class="lnk" onclick={() => link("https://github.com/mcmod-info-mirror/mcim")}>MCIM<ExternalLink size={10} /></button>
            镜像
          </span>
        </div>
      </div>
      
      <div class="row">
        <div class="lbl"><span class="faint">IMCL 不是 Mojang 或微软的官方产品，Minecraft 是 Mojang Studios 的商标</span></div>
      </div>
    </section>
  </div>
</div>

<style>
  .wrap {
    max-width: 720px;
    margin: 0 auto;
  }
  /* 分组是卡片（.row）的祖先，只能用 lift，见 app.css */
  .group {
    margin-bottom: 18px;
  }
  h2 {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-3);
    letter-spacing: 1px;
    margin: 0 4px 6px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 12px 16px;
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    border-bottom-width: 0;
  }
  .row:first-of-type {
    border-radius: var(--r-lg) var(--r-lg) 0 0;
  }
  .row:last-child {
    border-bottom-width: 1px;
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  .row:first-of-type:last-child {
    border-radius: var(--r-lg);
  }
  .about {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .logo {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--red);
    color: #fff;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 1px;
  }
  .lnk {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--red);
    font-size: inherit;
  }
  .lnk:hover {
    text-decoration: underline;
  }
  .swatches {
    gap: 8px;
  }
  .sw {
    position: relative;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 1px var(--border-strong);
    display: grid;
    place-items: center;
    color: var(--text-2);
    cursor: pointer;
    transition: transform 0.15s var(--ease), box-shadow 0.15s var(--ease);
  }
  .sw:hover {
    transform: scale(1.12);
  }
  .sw.on {
    box-shadow:
      0 0 0 2px var(--surface-solid),
      0 0 0 4px var(--c);
  }
  .sw.custom {
    background: conic-gradient(from 0deg, #ef323d, #f0b21f, #1fa463, #109fa8, #2f7cf6, #7c5cf0, #e4508e, #ef323d);
  }
  .sw.custom.on {
    background: var(--c);
    box-shadow:
      0 0 0 2px var(--surface-solid),
      0 0 0 4px var(--c);
  }
  .sw.custom :global(svg) {
    color: #fff;
    filter: drop-shadow(0 0 2px rgba(0, 0, 0, 0.6));
  }
  .sw.custom input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .bg-ctl {
    gap: 10px;
  }
  .thumb {
    width: 64px;
    height: 36px;
    border-radius: 6px;
    background-size: cover;
    background-position: center;
    box-shadow: 0 0 0 1px var(--border-strong);
  }
  .row.col {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }
  .jgrid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 8px;
  }
  .jcard {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    padding: 8px 10px 8px 12px;
    border-radius: var(--r);
    border: 1px solid var(--border);
    background: var(--bg);
  }
  .jcard.ok {
    border-color: color-mix(in srgb, var(--green) 35%, var(--border));
  }
  .jt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .jt b {
    font-size: 13px;
  }
  .jt span {
    font-size: 11.5px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .jrun {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .jbar {
    width: 56px;
    height: 5px;
    border-radius: 3px;
    background: var(--hover);
    overflow: hidden;
  }
  .jbar i {
    display: block;
    height: 100%;
    background: var(--red);
    transition: width 0.3s var(--ease);
  }
  .jpct {
    font-size: 12px;
    color: var(--text-2);
    min-width: 30px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .fail {
    color: var(--red);
    border-color: var(--red);
  }
  .lbl {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .lbl b {
    font-size: 14px;
    font-weight: 600;
  }
  .lbl span {
    font-size: 12px;
    color: var(--text-2);
  }
  /* 从属于上一行的设置：标题缩进、字小一点 */
  .row.sub .lbl {
    padding-left: 14px;
  }
  .row.sub .lbl b {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-2);
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .ctl .input {
    width: 200px;
  }
  .ctl input[type="range"] {
    width: 180px;
  }
  .ctl {
    gap: 6px;
  }
  .ctl .input.mono {
    width: 260px;
    font-family: var(--mono);
    font-size: 12px;
  }
  .val {
    min-width: 56px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-size: 13px;
    font-weight: 600;
  }
</style>
