<script lang="ts">
  // Mod 页的「联机」标签：内置的联机 Mod（Remote Play）。第一屏像海报：图标、名字、安装按钮；往下是介绍。
  // jar 编在启动器里（见 online_mod.rs），安装不用联网
  import { ChevronDown, Check, LoaderCircle, RefreshCw, Info, Download, LayoutList, House, Hash, UserPlus, Server, ExternalLink, ArrowRight } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import OnlineModIcon from "./OnlineModIcon.svelte";
  import { library, settings, toast, nav, game, isPreload } from "$lib/state.svelte";
  import { api, ONLINE_MOD_BUILDS, ONLINE_MOD_VERSION, type OnlineModStatus } from "$lib/api";
  import type { Pack } from "$lib/mock";

  let { pack }: { pack: Pack | undefined } = $props();

  const SOURCE = "https://github.com/BuGing258/MinecraftOnline";
  const preload = isPreload();

  let status = $state<OnlineModStatus | null>(null);
  let busy = $state(false);
  let addon = $state(true);

  // 换版本时重新看一下装没装
  $effect(() => {
    const p = pack;
    status = null;
    if (!p || preload) return;
    let live = true;
    api
      .onlineModStatus(settings.gameDir, p.name, p.mc, p.loader)
      .then((s) => {
        if (!live) return;
        status = s;
        // 已经装过的，按当时有没有装部署工具来勾
        if (s.installed) addon = !!s.addon;
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  });

  const supported = $derived(!!pack && (status?.supported ?? true));
  const running = $derived(game.status !== "idle");
  /** 已装的版本号（从文件名末尾取，比如 mconline-fabric-26.1.2-1.2.0.jar → 1.2.0） */
  const installedVer = $derived(status?.installed?.match(/-(\d+\.\d+\.\d+)\.jar(\.disabled)?$/i)?.[1] ?? null);
  /** 装的是旧版本：按钮变成「更新」 */
  const outdated = $derived(!!status && !!installedVer && installedVer !== status.version);

  async function install() {
    const p = pack;
    if (!p || busy) return;
    busy = true;
    try {
      const was = status?.installed ? (outdated ? "update" : "again") : "new";
      status = await api.installOnlineMod(settings.gameDir, p.name, p.mc, p.loader, addon);
      toast.show(was === "update" ? `已更新到 ${status.version}` : was === "again" ? `已重新安装到「${p.name}」` : `已安装到「${p.name}」`);
      // 版本管理里的 Mod 列表也更新一下
      if (p.real) library.loadFiles(p, true).catch(() => {});
    } catch (e) {
      toast.show(`安装失败：${e}`);
    } finally {
      busy = false;
    }
  }

  let intro = $state<HTMLElement>();
  const scrollDown = () => intro?.scrollIntoView({ behavior: "smooth", block: "start" });

  const ROUTES = [
    { name: "IPv6 直连", desc: "双方网络都支持 IPv6 时直接连上，延迟最低" },
    { name: "P2P 直连", desc: "通过服务器牵线，玩家之间直接打通连接" },
    { name: "FRP 中转", desc: "前两种都不通时，走房主配置的 FRP 隧道" },
  ];
  const FEATURES = [
    { icon: LayoutList, title: "联机大厅", text: "公开的房间都列在这里，可以按房间名、房主或房间码搜索，要密码的房间会标出来。" },
    {
      icon: House,
      title: "创建房间",
      text: "选一个存档开房，也可以先复制成联机存档再开，原来的存档不受影响。房间名、密码、最大人数、游戏模式、作弊、正版验证都能设。",
    },
    { icon: Hash, title: "房间码加入", text: "朋友把 6 位房间码发给你，输进去就能加入；房间有密码的话再填上密码。" },
    { icon: UserPlus, title: "邀请好友", text: "在房间里直接邀请同一服务器上的在线好友，对方会收到邀请，按 Y 接受、按 X 拒绝。" },
  ];
</script>

<div class="poster">
  <!-- 第一屏 -->
  <section class="hero">
    <div class="glow" aria-hidden="true"></div>
    <div class="icon"><OnlineModIcon size={124} /></div>
    <h2>Remote Play</h2>
    <p class="sub">远程联机 · v{status?.version ?? ONLINE_MOD_VERSION}</p>
    <p class="tagline">和远方的好友一起玩，就像在同一个局域网里</p>

    <div class="action">
      {#if !pack}
        <button class="btn btn-primary big" onclick={() => nav.go("download")}><Download size={18} />先去下载一个游戏版本</button>
      {:else if !supported}
        <button class="btn btn-primary big" disabled>这个版本装不了</button>
        <p class="hint warn-text">
          <Info size={14} />「{pack.name}」是 {pack.mc} {pack.loader}，联机 Mod 支持 1.20.1 Fabric / Forge，1.21.1、1.21.10、1.21.11、26.1.2、26.3 的 Fabric / NeoForge。可以在右上角换一个版本。
        </p>
      {:else}
        <button class="btn btn-primary big" class:again={!!status?.installed && !outdated} onclick={install} disabled={busy || running || !status}>
          {#if busy}
            <LoaderCircle size={18} class="spin" />正在安装…
          {:else if outdated}
            <Download size={18} />更新到 {status?.version}
          {:else if status?.installed}
            <RefreshCw size={17} />重新安装
          {:else}
            <Download size={18} />安装
          {/if}
        </button>
        {#if outdated && !busy && !running}
          <p class="hint">「{pack.name}」里装的是 {installedVer}，有新版本</p>
        {:else if status?.installed && !busy}
          <p class="hint ok-text"><Check size={14} />已安装到「{pack.name}」</p>
        {:else if running}
          <p class="hint">游戏运行中，关掉游戏后再安装</p>
        {:else}
          <p class="hint">装到「{pack.name}」· {pack.mc} {pack.loader} · 不用联网</p>
        {/if}
        <label class="addon">
          <input type="checkbox" bind:checked={addon} disabled={busy} />同时安装服务器部署工具
        </label>
      {/if}
    </div>

    <button class="more" onclick={scrollDown}>往下看看它能做什么<ChevronDown size={16} /></button>
  </section>

  <!-- 介绍 -->
  <section class="block" bind:this={intro}>
    <h3>三条线路，自动挑能通的</h3>
    <p class="lead">默认「自动」模式按 IPv6 → P2P → FRP 的顺序尝试，哪条通走哪条；也可以在游戏里手动指定。</p>
    <div class="routes">
      {#each ROUTES as r, i}
        {#if i}<ArrowRight size={18} class="arrow" />{/if}
        <div class="card route">
          <span class="step">{i + 1}</span>
          <b>{r.name}</b>
          <p>{r.desc}</p>
        </div>
      {/each}
    </div>
  </section>

  <section class="block">
    <h3>能做什么</h3>
    <div class="features">
      {#each FEATURES as f}
        <div class="card feature">
          <span class="f-icon"><f.icon size={20} /></span>
          <div>
            <b>{f.title}</b>
            <p>{f.text}</p>
          </div>
        </div>
      {/each}
    </div>
    <p class="note"><Info size={13} />房间公开又没设密码时，其他玩家能看到房主的 IP。</p>
  </section>

  <section class="block">
    <h3>怎么用</h3>
    <ol class="steps">
      <li class="card">
        <span class="num">1</span>
        <div><b>装好后启动游戏</b><p>点上面的「安装」，回首页启动这个版本。你和朋友都要装。</p></div>
      </li>
      <li class="card">
        <span class="num">2</span>
        <div><b>打开「多人游戏」</b><p>会先问你要「局域网联机」还是「远程联机」，选远程联机。</p></div>
      </li>
      <li class="card">
        <span class="num">3</span>
        <div>
          <b>开房或加入</b>
          <p>房主：在「创建房间」里选存档；或者进入单人世界后按 Esc，选「开放到远程联机」。<br />朋友：在联机大厅找到房间，或者输入房间码。</p>
        </div>
      </li>
    </ol>
  </section>

  <section class="block">
    <div class="card kit">
      <span class="f-icon"><Server size={20} /></span>
      <div>
        <b>服务器部署工具（附属）</b>
        <p>
          联机要通过一台联机服务器来建房间、交换连接信息，可以在游戏里的「服务器配置」添加自己的服务器（阿里云、腾讯云或自定义地址）。
          装上这个附属后会多一个「部署服务器」按钮，一步步引导你把联机服务器部署好。只是加入朋友房间的话用不到。
        </p>
      </div>
    </div>
  </section>

  <section class="block">
    <h3>支持的版本</h3>
    <div class="card table">
      {#each ONLINE_MOD_BUILDS as b}
        {@const cur = !!pack && pack.mc === b.mc && pack.loader.toLowerCase() === b.loader.toLowerCase()}
        <div class="row" class:cur>
          <b>{b.mc}</b>
          <span class="loader">{b.loader}</span>
          <span class="faint">{b.note}</span>
          {#if cur}<span class="tag">当前版本</span>{/if}
        </div>
      {/each}
    </div>
    <p class="note"><Info size={13} />只装在玩家这边，服务器不用装。Fabric 版已经自带需要的 Fabric API 模块，不用另外装。</p>
  </section>

  <footer class="foot">
    <button class="btn btn-sm" onclick={() => openUrl(SOURCE).catch(() => toast.show("打不开链接"))}><ExternalLink size={13} />查看源代码</button>
    <p>开源项目 · GPL-3.0</p>
    <p class="faint">Remote Play 不是 Minecraft 官方产品，与 Mojang、Microsoft 无关。</p>
  </footer>
</div>

<style>
  .poster {
    /* 窗口高度减去标题栏、页头和标签栏，正好一屏 */
    --hero-h: max(400px, calc(100vh - 186px));
  }
  /* 第一屏：占满可见区域，内容居中 */
  .hero {
    position: relative;
    min-height: var(--hero-h);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 8px 0 44px;
  }
  .glow {
    position: absolute;
    left: 50%;
    top: 42%;
    width: 420px;
    height: 320px;
    transform: translate(-50%, -50%);
    background: radial-gradient(closest-side, color-mix(in srgb, var(--red) 22%, transparent), transparent);
    pointer-events: none;
    z-index: -1;
  }
  .icon {
    filter: drop-shadow(0 14px 28px color-mix(in srgb, var(--red) 35%, transparent));
    margin-bottom: 18px;
  }
  .icon :global(svg) {
    display: block;
  }
  h2 {
    font-size: 34px;
    font-weight: 800;
    letter-spacing: 0.5px;
    line-height: 1.1;
  }
  .sub {
    margin-top: 6px;
    font-size: 13px;
    color: var(--text-2);
    letter-spacing: 1px;
  }
  .tagline {
    margin-top: 14px;
    font-size: 15px;
  }
  .action {
    margin-top: 26px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }
  .big {
    height: 44px;
    min-width: 180px;
    padding: 0 26px;
    border-radius: 999px;
    font-size: 15px;
    gap: 8px;
  }
  /* 已安装：不再是醒目的红色，换成普通按钮 */
  .big.again {
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border-color: var(--border-strong);
    color: var(--text);
    font-weight: 500;
  }
  .big.again:hover:not(:disabled) {
    background: var(--hover);
  }
  .hint {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
    max-width: 440px;
    text-align: left;
  }
  .hint :global(svg) {
    flex-shrink: 0;
  }
  .ok-text {
    color: var(--green, #2e9e5b);
  }
  .warn-text {
    align-items: flex-start;
    line-height: 1.6;
  }
  .warn-text :global(svg) {
    margin-top: 3px;
  }
  .addon {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    color: var(--text-2);
    cursor: pointer;
  }
  .more {
    position: absolute;
    bottom: 6px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-3);
    padding: 6px 10px;
    border-radius: 999px;
    transition: color 0.15s var(--ease), background 0.15s var(--ease);
  }
  .more:hover {
    color: var(--text);
    background: var(--hover);
  }

  /* 介绍 */
  .block {
    padding-top: 30px;
  }
  h3 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 6px;
  }
  .lead {
    font-size: 13px;
    color: var(--text-2);
    margin-bottom: 14px;
  }
  .block > h3 + .features,
  .block > h3 + .steps,
  .block > h3 + .table {
    margin-top: 12px;
  }
  .routes {
    display: flex;
    align-items: stretch;
    gap: 8px;
  }
  .routes :global(.arrow) {
    flex-shrink: 0;
    align-self: center;
    color: var(--text-3);
  }
  .route {
    flex: 1;
    min-width: 0;
    padding: 14px 16px;
    position: relative;
  }
  .step {
    position: absolute;
    top: 12px;
    right: 14px;
    font-size: 22px;
    font-weight: 800;
    color: var(--red);
    opacity: 0.25;
  }
  .route b,
  .feature b,
  .steps b,
  .kit b {
    font-size: 14px;
    font-weight: 600;
  }
  .route p,
  .feature p,
  .steps p,
  .kit p {
    margin-top: 4px;
    font-size: 12.5px;
    line-height: 1.65;
    color: var(--text-2);
  }
  .features {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .feature,
  .kit {
    display: flex;
    gap: 12px;
    padding: 14px 16px;
  }
  .f-icon {
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--red-soft);
    color: var(--red);
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
    font-size: 12px;
    color: var(--text-3);
  }
  .note :global(svg) {
    flex-shrink: 0;
  }
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .steps li {
    display: flex;
    gap: 14px;
    padding: 14px 16px;
  }
  .num {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--red);
    color: var(--on-red);
    font-size: 13px;
    font-weight: 700;
  }
  .table {
    overflow: hidden;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 11px 16px;
    font-size: 13px;
  }
  .row + .row {
    border-top: 1px solid var(--border);
  }
  .row b {
    width: 56px;
    font-weight: 600;
  }
  .loader {
    width: 72px;
  }
  .row .faint {
    flex: 1;
    min-width: 0;
    font-size: 12px;
  }
  .row.cur {
    background: var(--red-soft);
  }
  .tag {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--red);
    color: var(--on-red);
  }
  .foot {
    margin-top: 36px;
    padding: 22px 0 8px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
    text-align: center;
  }
  .foot .btn {
    margin-bottom: 4px;
  }

  @container (max-width: 720px) {
    .routes {
      flex-direction: column;
      align-items: stretch;
    }
    .routes :global(.arrow) {
      align-self: center;
      transform: rotate(90deg);
    }
    .features {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
