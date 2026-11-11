<script lang="ts">
  import type { Component } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import Home from "$lib/pages/Home.svelte";
  import Versions from "$lib/pages/Versions.svelte";
  import Download from "$lib/pages/Download.svelte";
  import Mods from "$lib/pages/Mods.svelte";
  import Accounts from "$lib/pages/Accounts.svelte";
  import Settings from "$lib/pages/Settings.svelte";
  import { nav, settings, accounts, library, saveSettings, initBackend, type PageId } from "$lib/state.svelte";

  // 取默认游戏目录、扫描已安装的版本
  initBackend();

  // 记住首页选中的版本
  $effect(() => {
    if (library.loaded && library.currentId) settings.selectedVersion = library.currentId;
  });

  const PAGES: Record<PageId, Component> = {
    home: Home,
    versions: Versions,
    download: Download,
    resources: Mods,
    accounts: Accounts,
    settings: Settings,
  };

  // 账户变化时保存
  $effect(() => {
    JSON.stringify(accounts.list);
    void accounts.currentId;
    accounts.persist();
  });

  // 主题
  $effect(() => {
    const root = document.documentElement;
    if (settings.theme === "system") delete root.dataset.theme;
    else root.dataset.theme = settings.theme;
  });

  // 设置变化时保存
  $effect(() => {
    JSON.stringify(settings);
    saveSettings();
  });

  function isTyping(e: Event) {
    const t = e.target as HTMLElement | null;
    return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT");
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !isTyping(e)) nav.back();
    if (e.altKey && e.key === "ArrowLeft") nav.back();
  }

  // 鼠标侧键“后退”
  function onMouseUp(e: MouseEvent) {
    if (e.button === 3) {
      e.preventDefault();
      nav.back();
    }
  }

  // 固定页面创建时的层级，保证“更深”的页面始终盖在上面
  function freezeZ(node: HTMLElement, z: number) {
    node.style.zIndex = String(z);
  }

  const inParams = () =>
    nav.dir > 0
      ? { x: "100%", opacity: 1, duration: 460, easing: cubicOut }
      : { x: "-28%", opacity: 0, duration: 460, easing: cubicOut };
  const outParams = () =>
    nav.dir > 0
      ? { x: "-28%", opacity: 0, duration: 460, easing: cubicOut }
      : { x: "100%", opacity: 1, duration: 460, easing: cubicOut };
</script>

<svelte:window onkeydown={onKey} onmouseup={onMouseUp} />

<div class="app">
  <TitleBar />
  <main class="stage">
    {#key nav.current}
      {@const Page = PAGES[nav.current]}
      <div
        class="page"
        use:freezeZ={nav.stack.length}
        in:fly={inParams()}
        out:fly={outParams()}
        onoutrostart={(e) => (e.currentTarget.style.pointerEvents = "none")}
        onintrostart={(e) => (e.currentTarget.style.pointerEvents = "")}
      >
        <Page />
      </div>
    {/key}
  </main>
  <Toast />
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .page {
    position: absolute;
    inset: 0;
    background: var(--bg);
  }
</style>
