<script lang="ts">
  import { onMount, type Component } from "svelte";
  import { smooth } from "$lib/motion";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import Tooltip from "$lib/components/Tooltip.svelte";
  import GameProblem from "$lib/components/GameProblem.svelte";
  import Backdrop from "$lib/components/Backdrop.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { onColor } from "$lib/theme";
  import Home from "$lib/pages/Home.svelte";
  import Versions from "$lib/pages/Versions.svelte";
  import Download from "$lib/pages/Download.svelte";
  import Mods from "$lib/pages/Mods.svelte";
  import Accounts from "$lib/pages/Accounts.svelte";
  import Settings from "$lib/pages/Settings.svelte";
  import { nav, settings, accounts, library, saveSettings, initBackend, appearance, isTauri, type PageId } from "$lib/state.svelte";

  // 取默认游戏目录、扫描已安装的版本
  initBackend();

  // 窗口一开始是隐藏的（tauri.conf.json 里 visible: false）：页面挂好、云母打开后再显示。
  // 不然会先看到空窗口，接着打开云母时系统重画整个窗口，正好压在进场动画上，看起来卡一下。
  // 隐藏的窗口不跑 requestAnimationFrame，所以这里只等挂载完成，不等画面帧；后端 3 秒后还会兜底显示
  onMount(() => {
    if (!isTauri()) return;
    const mica = settings.material === "mica";
    const applyMica = () => (mica ? invoke("set_backdrop", { mica: true }).catch(() => {}) : Promise.resolve());
    applyMica()
      .then(() => getCurrentWindow().show())
      // 显示窗口时底层会重写一次窗口样式，把云母模式下去掉的系统按钮样式加回来；再设一次（没变化的话什么都不做）
      .then(applyMica)
      .catch((e) => console.warn("显示窗口失败", e));
  });

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

  // 现在实际是不是暗色（跟随系统时听系统的）
  $effect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const sync = () => {
      appearance.dark = settings.theme === "dark" || (settings.theme === "system" && mq.matches);
      if (appearance.dark) document.documentElement.dataset.dark = "";
      else delete document.documentElement.dataset.dark;
    };
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  });

  // 主题色
  $effect(() => {
    const root = document.documentElement.style;
    root.setProperty("--red", settings.accent);
    root.setProperty("--on-red", onColor(settings.accent));
  });

  // 材质和背景
  $effect(() => {
    const root = document.documentElement;
    root.dataset.material = settings.material;
    if (appearance.backgroundKind !== "none") root.dataset.bg = appearance.backgroundKind;
    else delete root.dataset.bg;
  });

  // 卡片材质：不透明度和模糊；“云母”是一组固定的值（半透明 + 大模糊 + 提一点饱和度）
  $effect(() => {
    const c = appearance.card;
    const mica = c.kind === "mica";
    const a = mica ? 62 : c.opacity;
    const filter = mica ? "blur(40px) saturate(1.6)" : c.blur > 0 ? `blur(${c.blur}px)` : "none";
    // 弹出层至少 82% 不透明，不然字会看不清
    const pa = Math.max(a, 82);
    const s = document.documentElement.style;
    s.setProperty("--card-a", `${a}%`);
    s.setProperty("--card-filter", filter);
    s.setProperty("--pop-a", `${pa}%`);
    s.setProperty("--pop-filter", pa < 100 ? "blur(30px) saturate(1.4)" : "none");
  });

  // 窗口的亮色 / 暗色（云母的颜色跟着它走）；只在主题变化时设置
  $effect(() => {
    const theme = settings.theme === "system" ? null : settings.theme;
    if (isTauri()) getCurrentWindow().setTheme(theme).catch(() => {});
  });

  // 云母由系统绘制：后端直接调 Win11 的 DWM 接口（见 backdrop.rs）；云母的深浅跟着窗口主题（上面的 setTheme）。
  // 只负责打开，切回实色时不去关：关掉要把扩展到整个窗口的边框收回去，系统会重画整个窗口，看起来像浏览器刷新了一下；
  // 实色的页面（包括背景图）完全不透明，本来就把云母盖住了
  $effect(() => {
    const mica = settings.material === "mica";
    if (!isTauri() || !mica) return;
    invoke<number | null>("set_backdrop", { mica })
      .then((kind) => {
        if (mica && kind !== 2) console.warn("系统没有启用云母，读回的背景类型：", kind);
      })
      .catch((e) => console.warn("窗口背景设置失败", e));
  });

  // 设置变化时保存：停下来 300ms 再写（拖滑条时会连续变化）；关窗口前补存一次
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    JSON.stringify(settings);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(saveSettings, 300);
  });
  $effect(() => {
    const flush = () => {
      clearTimeout(saveTimer);
      saveSettings();
    };
    window.addEventListener("pagehide", flush);
    window.addEventListener("beforeunload", flush);
    return () => {
      window.removeEventListener("pagehide", flush);
      window.removeEventListener("beforeunload", flush);
    };
  });

  function isTyping(e: Event) {
    const t = e.target as HTMLElement | null;
    return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT");
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !isTyping(e)) nav.back();
    if (e.altKey && e.key === "ArrowLeft") nav.back();
  }

  // 去掉浏览器自带的右键菜单（刷新、检查之类的）；输入框和选中的文字保留，方便复制粘贴
  function onContextMenu(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    const editable = !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
    const selected = !!window.getSelection()?.toString();
    if (!editable && !selected) e.preventDefault();
  }

  // 鼠标侧键“后退”
  function onMouseUp(e: MouseEvent) {
    if (e.button === 3) {
      e.preventDefault();
      nav.back();
    }
  }

  // 固定页面创建时的层级：先按深度（更深的页面盖在上面），同一深度时后打开的盖在上面。
  // 不然快速“返回再打开”时，新页面会被还在滑走的旧页面挡住，看起来像动画没了
  let pageSeq = 0;
  function freezeZ(node: HTMLElement, depth: number) {
    pageSeq = (pageSeq % 999) + 1;
    node.style.zIndex = String(depth * 1000 + pageSeq);
  }

  /*
   * 页面切换：只用平移，不用淡入淡出。
   * 卡片有背景模糊（backdrop-filter），它的祖先在做 opacity 动画时浏览器不计算模糊，动画结束才突然出现；
   * 平移不影响模糊，所以整个过程中卡片的材质都是完整的。
   *
   * 两个页面会叠在一起（背景透明时能互相看见），所以下面那一页被裁到上面那一页的左边沿：
   * 裁剪靠 .page 的宽度 + overflow: hidden（不影响模糊），页面内容在 .page-body 里，宽度固定不跟着变，不会重新排版。
   *   前进：新页面（上层）从右边滑进来；旧页面（下层）往左挪 28%，同时被新页面的左边沿裁掉。
   *   返回：旧页面（上层）往右滑走；新页面（下层）从左边 28% 处挪回来，露出的部分跟着旧页面的左边沿变宽。
   *
   * 位置记在 pageX 里，每一帧由脚本设置，裁剪宽度按“上层页面此刻的真实位置”算，不靠假设。
   *
   * 用双向的 transition:（不是分开的 in: / out:）：动画被打断时，Svelte 会从当前进度原路倒放。
   * 快速“打开设置再返回”时：设置页沿着来的方向滑回右边，首页从它退到的位置滑回来。
   * （分开写的话是两个互不相干的动画，打断后进场动画还在跑，和离场动画每帧抢着设位置，页面会跳）
   */
  const PARALLAX = 0.28;
  const DURATION = 460;
  let stageEl = $state<HTMLElement>();
  /** 正在动的页面此刻的水平位置（px）；不在里面的就是 0 */
  const pageX = new Map<HTMLElement, number>();

  /** 每一页裁到压在它上面的页面的左边沿；上面没有页面的不裁 */
  function clipPages() {
    if (!stageEl) return;
    const w = stageEl.clientWidth;
    const pages = [...stageEl.querySelectorAll<HTMLElement>(":scope > .page")];
    for (const p of pages) {
      const z = Number(p.style.zIndex) || 0;
      const x = pageX.get(p) ?? 0;
      let right = Infinity;
      for (const q of pages) {
        if (q !== p && (Number(q.style.zIndex) || 0) > z) right = Math.min(right, pageX.get(q) ?? 0);
      }
      p.style.width = right === Infinity ? "" : `${Math.max(0, Math.min(w, right - x))}px`;
    }
  }

  function setX(node: HTMLElement, x: number | null) {
    if (x === null) {
      pageX.delete(node);
      node.style.transform = "";
    } else {
      pageX.set(node, x);
      node.style.transform = `translateX(${x}px)`;
    }
    clipPages();
  }

  /**
   * t = 1 在中间，t = 0 在一侧。哪一侧在动画第一次开始时定下来：
   *   进场：前进从右边来（+100%），返回从左边 28% 处来；
   *   离场：前进往左挪 28%，返回往右滑出去。
   * 被打断倒放时 Svelte 沿用同一份配置，路线不变，只是 t 往回走。
   * 双向 transition 拿到的 direction 永远是 "both"，所以拿“这一页是哪个页面”（创建时记下，之后不变）和当前页面比较，
   * 判断是进场还是离场
   */
  function page(node: HTMLElement, forward: boolean) {
    const w = stageEl?.clientWidth ?? node.offsetWidth;
    // 还没记录的就是刚创建、正要进场的页面（Svelte 不保证 use: 和 transition: 谁先跑）
    if (!pageOf.has(node)) pageOf.set(node, nav.current);
    const entering = pageOf.get(node) === nav.current;
    const side = entering ? (forward ? w : -PARALLAX * w) : forward ? -PARALLAX * w : w;
    // Svelte 的 t 只用来判断往哪走、走了多少：每次换方向或 t 跳变（被打断、倒放），
    // 都从页面此刻的真实位置接着走，不会跳回某个固定起点
    let seg: { t0: number; x0: number; up: boolean } | null = null;
    let lastT: number | null = null;
    return {
      duration: DURATION,
      easing: smooth,
      tick: (t: number) => {
        const moved = lastT === null ? 0 : t - lastT;
        const flipped = seg !== null && moved !== 0 && moved > 0 !== seg.up;
        if (seg === null || flipped || Math.abs(moved) > 0.25) {
          const x0 = pageX.get(node) ?? (entering ? side : 0);
          seg = { t0: t, x0, up: moved !== 0 ? moved > 0 : t < 0.5 };
        }
        lastT = t;
        const p = seg.up ? (seg.t0 >= 1 ? 1 : (t - seg.t0) / (1 - seg.t0)) : seg.t0 <= 0 ? 1 : (seg.t0 - t) / seg.t0;
        const k = Math.min(1, Math.max(0, p));
        const target = seg.up ? 0 : side;
        if (seg.up && k >= 1) setX(node, null);
        else setX(node, seg.x0 + (target - seg.x0) * k);
      },
    };
  }

  /** 每个页面元素是哪个页面：创建时记下，之后不随 nav.current 变 */
  const pageOf = new WeakMap<HTMLElement, PageId>();

  /** 记下页面是谁；页面被移除时清掉它的位置记录，别的页面不再被它裁 */
  function trackPage(node: HTMLElement, id: PageId) {
    if (!pageOf.has(node)) pageOf.set(node, id);
    return {
      destroy() {
        pageX.delete(node);
        queueMicrotask(clipPages);
      },
    };
  }
</script>

<svelte:window onkeydown={onKey} onmouseup={onMouseUp} oncontextmenu={onContextMenu} />

<Backdrop />
<div class="app">
  <TitleBar />
  <main class="stage" bind:this={stageEl}>
    {#key nav.current}
      {@const Page = PAGES[nav.current]}
      <div
        class="page"
        use:freezeZ={nav.stack.length}
        use:trackPage={nav.current}
        transition:page={nav.dir > 0}
        onoutrostart={(e) => (e.currentTarget.style.pointerEvents = "none")}
        onintrostart={(e) => (e.currentTarget.style.pointerEvents = "")}
      >
        <div class="page-body"><Page /></div>
      </div>
    {/key}
  </main>
  <Toast />
  <Tooltip />
  <GameProblem />
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--page);
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    /* 自成一层：页面的 z-index（1001、2002…）只在这里面比较，
       不会盖住挂在 body 上的下拉菜单、悬停提示和轻提示 */
    isolation: isolate;
    /* 页面内容按舞台宽度排版（100cqw），切换时 .page 变窄也不会重新排版 */
    container-type: inline-size;
  }
  .page {
    position: absolute;
    inset: 0;
    /* 切换时宽度会变小（被另一页裁掉），里面的内容宽度不变 */
    overflow: hidden;
  }
  .page-body {
    position: absolute;
    inset: 0 auto 0 0;
    width: 100cqw;
    /* 不再单独铺底色：两页不会叠在一起，底色由 .app 统一铺一层（铺两层的话，半透明底色会叠深） */
  }
</style>
