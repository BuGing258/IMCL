<script lang="ts">
  import { FolderOpen, X } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import Select from "$lib/components/Select.svelte";
  import { settings, gameDirs, javas, javaOptions, toast, isTauri, fmtMem } from "$lib/state.svelte";

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
        <div class="lbl">
          <b>游戏目录</b>
          <span>可以添加其他启动器的 .minecraft，里面已装的版本会直接读出来；也可以在版本管理里切换</span>
        </div>
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
        <div class="lbl"><b>最大内存</b><span>建议不超过电脑内存的一半</span></div>
        <div class="ctl">
          <input type="range" min="1024" max="16384" step="512" bind:value={settings.memory} aria-label="最大内存" />
          <span class="val">{fmtMem(settings.memory)}</span>
        </div>
      </div>
      <div class="row">
        <div class="lbl">
          <b>Java</b>
          <span>
            {javas.found === null
              ? "正在查找电脑上的 Java…"
              : javas.all.length
                ? `找到 ${javas.all.length} 个；自动选择会按游戏版本挑合适的`
                : "没找到 Java，装 Forge / OptiFine 和启动游戏都需要它"}
          </span>
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
      <div class="row">
        <div class="lbl"><b>JVM 参数</b><span>追加在启动参数里，不懂可以留空</span></div>
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
        <div class="lbl"><b>同时下载数</b><span>网络不稳定时可以调低</span></div>
        <div class="ctl">
          <input type="range" min="8" max="128" step="8" bind:value={settings.threads} aria-label="同时下载数" />
          <span class="val">{settings.threads}</span>
        </div>
      </div>
    </section>

    <section class="group" style:--i={4}>
      <h2>关于</h2>
      <div class="row">
        <div class="lbl"><b>AMC Launcher 0.1.0</b><span>下载安装、版本管理已可用；启动游戏和 Mod 下载还在做</span></div>
      </div>
    </section>
  </div>
</div>

<style>
  .wrap {
    max-width: 720px;
    margin: 0 auto;
  }
  .group {
    margin-bottom: 18px;
    animation: rise 0.45s var(--ease) backwards;
    animation-delay: calc(var(--i) * 50ms + 40ms);
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
