import { getContext } from "svelte";
import { invoke } from "@tauri-apps/api/core";
// 全局界面状态（Svelte 5 runes）。
// 已接后端：系统状态、下载安装、已安装版本。其余（Mod 市场、启动游戏、账户登录）仍是前端模拟。
import { listen } from "@tauri-apps/api/event";
import { api, isTauri, type JavaOffer, type GameExit, type InstallRequest, type MsAccountInfo, type JavaInfo, type LoaderKind, type ProgressEvent, type VersionSettings, type MemoryPlan } from "./api";
import { mockPacks, packLook, type Loader, type Pack, type ResKind, type MarketKind } from "./mock";

export { isTauri };

/* ---------- 页面导航 ---------- */
export type PageId = "home" | "versions" | "download" | "resources" | "accounts" | "settings";

export const PAGE_TITLES: Record<PageId, string> = {
  home: "首页",
  versions: "版本管理",
  download: "下载游戏",
  resources: "Mod",
  accounts: "账户管理",
  settings: "设置",
};

/** 页面切换滑动的时长（毫秒），+page.svelte 里的动画也用这个 */
export const PAGE_SLIDE_MS = 460;

/**
 * 预渲染：启动后空闲时把各页在看不见的地方画一遍（见 +page.svelte），
 * 让浏览器提前排好、画好里面的中文，第一次真正打开时就不卡。预渲染的页面不加载任何数据
 */
export const PRELOAD = Symbol("preload");
/** 当前页面是不是预渲染出来的（只能在组件初始化时调用） */
export const isPreload = () => getContext(PRELOAD) === true;

class Nav {
  stack = $state<PageId[]>(["home"]);
  /** 1 = 前进（新页面从右往左滑入），-1 = 返回 */
  dir = $state<1 | -1>(1);
  current = $derived(this.stack[this.stack.length - 1]);
  /** 最近一次切换页面的时间 */
  #movedAt = 0;

  go(page: PageId) {
    if (page === this.current) return;
    this.dir = 1;
    this.#movedAt = performance.now();
    this.stack = [...this.stack, page];
  }

  back() {
    if (this.stack.length <= 1) return;
    this.dir = -1;
    this.#movedAt = performance.now();
    this.stack = this.stack.slice(0, -1);
  }

  home() {
    if (this.stack.length <= 1) return;
    this.dir = -1;
    this.#movedAt = performance.now();
    this.stack = ["home"];
  }

  /**
   * 等页面滑动结束（没在切换就马上返回）。
   * 进页面时异步拿到的数据（版本列表、搜索结果、刷新结果）如果在滑动途中渲染，
   * 那一下的脚本和排版会把动画卡住几十毫秒；等滑完再放上去，静止时多花这点时间看不出来
   */
  async settled() {
    for (;;) {
      const left = this.#movedAt + PAGE_SLIDE_MS + 20 - performance.now();
      if (left <= 0) return;
      await new Promise((r) => setTimeout(r, left));
    }
  }
}
export const nav = new Nav();

/** 跨页面共享的小状态，比如从版本管理跳到 Mod 页时预先选好“光影”标签 */
/** resOnline：Mod 页选的是「联机」标签（内置联机 Mod），这时不搜索 */
export const ui = $state<{ resKind: MarketKind; resQuery: string; resOnline: boolean }>({ resKind: "mod", resQuery: "", resOnline: false });

/* ---------- localStorage 小工具 ---------- */
function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (raw) return { ...fallback, ...JSON.parse(raw) };
  } catch {
    /* 读不到就用默认值 */
  }
  return fallback;
}

function save(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* 忽略 */
  }
}

/* ---------- 设置 ---------- */
export interface Settings {
  /** 内存分配：自动 / 手动 */
  memoryMode: "auto" | "manual";
  /** 手动分配的内存（MB） */
  memory: number;
  java: string;
  windowMode: "default" | "720p" | "fullscreen";
  theme: "system" | "light" | "dark";
  source: "bmcl" | "official";
  threads: number;
  /** 当前游戏目录；空字符串表示还没取到默认值 */
  gameDir: string;
  /** 添加过的游戏目录（第一个是默认目录） */
  gameDirs: GameDir[];
  /** 首页选中的版本 */
  selectedVersion: string;
  /** 额外 JVM 参数 */
  jvmArgs: string;
  /** 手动添加的 Java（自动查找找不到的） */
  extraJavas: JavaInfo[];
  /** 微软登录用的 Azure 应用 Client ID */
  msClientId: string;
  /** 安装 Mod 时自动装前置 */
  autoDeps: boolean;
  /** 主题色 */
  accent: string;
  /** 背景材质：实色 / 云母 */
  material: Material;
  /** 实色时的背景：无 / 自己的图片（图片存在应用数据目录里） */
  background: "none" | "image";
  /** 背景图模糊（px） */
  bgBlur: number;
  /** 卡片材质，实色和云母背景各存一份 */
  cards: Record<Material, CardMaterial>;
}

export type Material = "solid" | "mica";

export interface CardMaterial {
  /** 自定义不透明度和模糊，或者直接用云母 */
  kind: "custom" | "mica";
  /** 0~100% */
  opacity: number;
  /** px */
  blur: number;
}

export interface GameDir {
  path: string;
  name: string;
}

export const settings = $state<Settings>(
  load("amc.settings", {
    memoryMode: "auto",
    memory: 4096,
    java: "auto",
    windowMode: "default",
    theme: "system",
    source: "bmcl",
    threads: 64,
    gameDir: "",
    gameDirs: [],
    selectedVersion: "",
    jvmArgs: "",
    msClientId: "",
    extraJavas: [],
    autoDeps: true,
    accent: "#ef323d",
    material: "solid",
    background: "none",
    bgBlur: 0,
    cards: {
      solid: { kind: "custom", opacity: 100, blur: 0 },
      mica: { kind: "custom", opacity: 55, blur: 0 },
    },
  }),
);

export const WINDOW_LABELS: Record<Settings["windowMode"], string> = {
  default: "默认",
  "720p": "1280×720",
  fullscreen: "全屏",
};

export function fmtMem(mb: number) {
  return mb >= 1024 ? `${(mb / 1024).toFixed(1).replace(/\.0$/, "")} GB` : `${mb} MB`;
}

/** 离线账户启动时带上的皮肤（游戏里也显示）；还没有皮肤图（没装过游戏）就不带 */
function offlineSkinOf(acc: Account): { png: string; slim: boolean } | null {
  if (acc.type !== "offline") return null;
  const png = skins.of(acc);
  if (!png) return null;
  const slim = acc.skin === "alex" || (acc.skin === "custom" && !!acc.skinSlim);
  return { png, slim };
}

/* ---------- 外观 ---------- */
class Appearance {
  /** 背景图（data URL），从应用数据目录读出来 */
  bgImage = $state<string | null>(null);
  /** 系统是不是 Win11（云母只有 Win11 有） */
  win11 = $state(true);
  /** 当前是不是暗色（跟随系统时看系统） */
  dark = $state(false);

  /** 实际显示的背景：云母不显示背景图（会挡住系统效果） */
  get backgroundKind(): "none" | "image" {
    if (settings.material !== "solid") return "none";
    if (settings.background === "image" && !this.bgImage) return "none";
    return settings.background;
  }

  /** 当前背景材质对应的卡片材质 */
  get card(): CardMaterial {
    // 旧设置里可能还是“亚克力”，迁移之前先当实色
    return settings.cards[settings.material] ?? settings.cards.solid;
  }

  async load() {
    // 液态玻璃、亚克力、渐变背景都去掉了，之前选过的换成相近的
    const m = settings.material as string;
    if (m === "liquid") settings.material = "solid";
    if (m === "acrylic") settings.material = "mica";
    if ((settings.background as string) === "gradient") settings.background = "none";
    if (!isTauri()) return;
    this.win11 = await api.isWin11().catch(() => true);
    if (!this.win11 && settings.material === "mica") settings.material = "solid";
    if (settings.background === "image") this.bgImage = await api.backgroundImage().catch(() => null);
  }

  /** 选一张图当背景 */
  async pickImage() {
    if (!isTauri()) {
      toast.show("浏览器预览里不能选图片");
      return;
    }
    const p = await api.pickImage();
    if (!p) return;
    try {
      this.bgImage = await api.setBackgroundImage(p);
      settings.background = "image";
    } catch (e) {
      toast.show(String(e));
    }
  }
}
export const appearance = new Appearance();

/* ---------- 内存分配 ---------- */
class Memory {
  /** key：游戏目录 + 版本名（空 = 原版） */
  plans = $state<Record<string, MemoryPlan>>({});

  /** 取一个版本的内存方案；第一次取时去后端读，之后用缓存 */
  plan(version: string | null): MemoryPlan | null {
    const key = `${settings.gameDir}|${version ?? ""}`;
    const p = this.plans[key];
    if (!p && !this.#loading.has(key)) {
      this.#loading.add(key);
      api
        .memoryPlan(settings.gameDir, version)
        .then((r) => (this.plans[key] = r))
        .catch(() => {})
        .finally(() => this.#loading.delete(key));
    }
    return p ?? null;
  }
  #loading = new Set<string>();
}
export const memory = new Memory();

/* ---------- 账户 ---------- */
export type AccountType = "offline" | "microsoft" | "authlib";

export const ACCOUNT_LABELS: Record<AccountType, string> = {
  offline: "离线账户",
  microsoft: "微软账户",
  authlib: "外置登录",
};

export interface Account {
  id: string;
  name: string;
  type: AccountType;
  /** 正版账户的 UUID */
  uuid?: string;
  /** 离线账户选的皮肤，默认 Steve */
  skin?: SkinChoice;
  /** 自定义皮肤，或者正版账户的皮肤（data URL） */
  skinData?: string;
  /** 自定义皮肤是不是细手臂（Alex 模型） */
  skinSlim?: boolean;
}

/* ---------- 皮肤 ---------- */
export type SkinChoice = "steve" | "alex" | "custom";

class Skins {
  /** 游戏自带的皮肤（从装好的客户端里取，没装过游戏时为 null） */
  steve = $state<string | null>(null);
  alex = $state<string | null>(null);
  #loading = false;

  async loadDefaults() {
    if (this.steve || this.#loading) return;
    this.#loading = true;
    try {
      const r = await api.defaultSkins([settings.gameDir, ...settings.gameDirs.map((d) => d.path)].filter(Boolean));
      if (r) Object.assign(this, r);
    } catch {
      /* 取不到就先用像素头像 */
    } finally {
      this.#loading = false;
    }
  }

  /** 账户头像用的皮肤；返回 null 时用像素头像代替 */
  of(a: Account): string | null {
    if (a.type === "microsoft") return a.skinData ?? this.steve;
    if (a.skin === "custom" && a.skinData) return a.skinData;
    return a.skin === "alex" ? this.alex : this.steve;
  }

  /** 重新读取正版账户的皮肤（在官网换了皮肤也能跟上） */
  async refreshMs(a: Account) {
    const url = await api.msSkin(a.id).catch(() => null);
    const cur = accounts.list.find((x) => x.id === a.id);
    if (url && cur) cur.skinData = url;
  }
}
export const skins = new Skins();

/** 读取用户选的皮肤文件，检查尺寸，返回 data URL */
/** 看皮肤是不是细手臂（Alex 模型）：细手臂的皮肤，右臂第 4 列像素是透明的 */
export function detectSlim(dataUrl: string): Promise<boolean> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onerror = () => resolve(false);
    img.onload = () => {
      if (img.height !== img.width) return resolve(false); // 64×32 的老皮肤没有细手臂
      const u = img.width / 64;
      const c = document.createElement("canvas");
      c.width = img.width;
      c.height = img.height;
      const g = c.getContext("2d", { willReadFrequently: true })!;
      g.drawImage(img, 0, 0);
      const a = g.getImageData(54 * u, 20 * u, 2 * u, 12 * u).data;
      let clear = true;
      for (let i = 3; i < a.length; i += 4) if (a[i] > 0) clear = false;
      resolve(clear);
    };
    img.src = dataUrl;
  });
}

export function readSkinFile(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    if (file.size > 512 * 1024) return reject(new Error("皮肤文件太大了，正常的皮肤只有几 KB"));
    const fr = new FileReader();
    fr.onerror = () => reject(new Error("读取文件失败"));
    fr.onload = () => {
      const url = String(fr.result);
      if (!url.startsWith("data:image/png")) return reject(new Error("皮肤要是 PNG 图片"));
      const img = new Image();
      img.onerror = () => reject(new Error("这张图片打不开"));
      img.onload = () => {
        const { width: w, height: h } = img;
        if (w < 64 || w % 64 || (h !== w && h * 2 !== w))
          return reject(new Error(`尺寸不对（${w}×${h}），皮肤应该是 64×64 或 64×32`));
        resolve(url);
      };
      img.src = url;
    };
    fr.readAsDataURL(file);
  });
}

class Accounts {
  #saved = load<{ list: Account[]; currentId: string }>("amc.accounts", {
    list: [{ id: "a-steve", name: "Steve", type: "offline" }],
    currentId: "a-steve",
  });
  list = $state<Account[]>(this.#saved.list.length ? this.#saved.list : [{ id: "a-steve", name: "Steve", type: "offline" }]);
  currentId = $state(this.#saved.currentId);
  current = $derived(this.list.find((a) => a.id === this.currentId) ?? this.list[0]);

  /** 返回错误提示；没问题时返回空字符串 */
  validate(name: string) {
    const n = name.trim();
    if (!n) return "用户名不能为空";
    if (!/^[A-Za-z0-9_]{3,16}$/.test(n)) return "只能用 3～16 位英文字母、数字或下划线";
    if (this.list.some((a) => a.type === "offline" && a.name.toLowerCase() === n.toLowerCase()))
      return "已经有同名的离线账户了";
    return "";
  }

  setSkin(id: string, skin: SkinChoice, data?: string) {
    const a = this.list.find((x) => x.id === id);
    if (!a || a.type === "microsoft") return;
    a.skin = skin;
    if (data) {
      a.skinData = data;
      // 换了新图就重新判断手臂粗细
      detectSlim(data).then((slim) => {
        const cur = this.list.find((x) => x.id === id);
        if (cur) cur.skinSlim = slim;
      });
    }
  }

  setSlim(id: string, slim: boolean) {
    const a = this.list.find((x) => x.id === id);
    if (a) a.skinSlim = slim;
  }

  addOffline(name: string) {
    const acc: Account = { id: `a-${Date.now()}`, name: name.trim(), type: "offline" };
    this.list.push(acc);
    this.currentId = acc.id;
  }

  remove(id: string) {
    if (this.list.length <= 1) return;
    const acc = this.list.find((a) => a.id === id);
    // 正版账户的登录令牌存在后端，一起删掉
    if (acc?.type === "microsoft") api.msRemove(id).catch(() => {});
    this.list = this.list.filter((a) => a.id !== id);
    if (this.currentId === id) this.currentId = this.list[0].id;
  }

  /** 微软登录成功后加入（同一个角色重复登录只更新名字） */
  addMicrosoft(info: MsAccountInfo) {
    const old = this.list.find((a) => a.id === info.id);
    if (old) old.name = info.name;
    else this.list.push({ id: info.id, name: info.name, type: "microsoft", uuid: info.uuid });
    this.currentId = info.id;
    skins.refreshMs({ id: info.id, name: info.name, type: "microsoft" });
  }

  /** 启动时和后端保存的正版账户对一下：登录信息丢了的就去掉 */
  async syncMicrosoft() {
    if (!isTauri()) return;
    const saved = await api.msAccounts().catch(() => null);
    if (!saved) return;
    for (const s of saved) if (!this.list.some((a) => a.id === s.id)) this.list.push({ id: s.id, name: s.name, type: "microsoft", uuid: s.uuid });
    const keep = this.list.filter((a) => a.type !== "microsoft" || saved.some((s) => s.id === a.id));
    if (keep.length && keep.length !== this.list.length) {
      this.list = keep;
      if (!keep.some((a) => a.id === this.currentId)) this.currentId = keep[0].id;
    }
    for (const a of this.list) if (a.type === "microsoft") skins.refreshMs(a);
  }

  persist() {
    save("amc.accounts", { list: this.list, currentId: this.currentId });
  }
}
export const accounts = new Accounts();

export function saveSettings() {
  save("amc.settings", settings);
}

/* ---------- 已安装的版本 ----------
 * Tauri 里：扫描游戏目录的 versions 文件夹。浏览器预览：用演示数据。
 */
class Library {
  packs = $state<Pack[]>(isTauri() ? [] : structuredClone(mockPacks));
  currentId = $state(isTauri() ? settings.selectedVersion : "survival");
  /** 第一次扫描完成前为 false，避免一打开就闪“还没有版本” */
  loaded = $state(!isTauri());
  error = $state("");
  current = $derived<Pack | undefined>(this.packs.find((p) => p.id === this.currentId) ?? this.packs[0]);

  /** 上次扫描的游戏目录；换了目录就不沿用旧版本的数据 */
  #dir = "";

  async refresh() {
    if (!isTauri() || !settings.gameDir) return;
    const dir = settings.gameDir;
    if (dir !== this.#dir) {
      this.#dir = dir;
      this.packs = [];
      this.loaded = false;
    }
    try {
      const list = await api.listInstalled(dir);
      if (dir !== settings.gameDir) return; // 扫描期间又换了目录
      const old = new Map(this.packs.map((p) => [p.id, p]));
      this.packs = list.map((v) => {
        const prev = old.get(v.name);
        return {
          id: v.name,
          name: v.name,
          mc: v.mc,
          loader: (v.loader ?? "原版") as Loader,
          loaderVersion: v.loaderVersion,
          optifine: v.optifine,
          ...packLook(v.loader, v.optifine),
          lastPlayed: "",
          mods: prev?.mods ?? [],
          shaders: prev?.shaders ?? [],
          resourcepacks: prev?.resourcepacks ?? [],
          filesLoaded: prev?.filesLoaded ?? false,
          real: true,
          dir: v.dir,
          modified: v.modified,
          isolated: v.isolated,
          vs: prev?.vs,
        };
      });
      this.error = "";
      if (!this.packs.some((p) => p.id === this.currentId)) this.currentId = this.packs[0]?.id ?? "";
      // 刚装好第一个版本时，默认皮肤就能取到了
      if (this.packs.length) skins.loadDefaults();
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loaded = true;
    }
  }

  /** 删除版本：整个版本文件夹移到回收站，再重新扫描；浏览器预览里只从列表去掉 */
  async deleteVersion(pack: Pack) {
    if (pack.real) {
      await api.deleteVersion(settings.gameDir, pack.name);
      await this.refresh();
    } else {
      this.packs = this.packs.filter((p) => p.id !== pack.id);
      if (this.currentId === pack.id) this.currentId = this.packs[0]?.id ?? "";
    }
  }

  /** 读取某个版本文件夹里的 Mod、光影、材质包 */
  async loadFiles(pack: Pack, force = false) {
    if (!pack.real || (pack.filesLoaded && !force)) return;
    const f = await api.versionFiles(settings.gameDir, pack.name);
    const kb = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
    // 重读时同一个文件沿用已经查到的版本号和图标，不会先变空再填回来（新加的文件之后再查）
    const prev = new Map(pack.mods.map((m) => [m.id.replace(/\.disabled$/i, ""), m]));
    pack.mods = f.mods.map((m) => {
      const old = prev.get(m.file.replace(/\.disabled$/i, ""));
      return {
        id: m.file,
        name: m.file.replace(/\.disabled$/i, "").replace(/\.jar$/i, ""),
        version: old?.version ?? "",
        icon: old?.icon,
        size: kb(m.size),
        enabled: m.enabled,
      };
    });
    pack.shaders = f.shaderpacks.map((x) => x.file);
    pack.resourcepacks = f.resourcepacks.map((x) => x.file);
    pack.filesLoaded = true;
    this.identify(pack);
  }

  /** 在 Modrinth 上查出每个文件是哪个 Mod，补上版本号和图标（名字一直用文件名） */
  async identify(pack: Pack) {
    if (!pack.real) return;
    try {
      const info = await api.identify(settings.gameDir, pack.name);
      pack.projects = info;
      for (const m of pack.mods) {
        const i = info[`mods/${m.id}`];
        if (i) {
          // 名字一直显示文件名，不替换；这里只补上版本号和图标
          m.version = i.versionNumber;
          m.icon = i.icon;
        }
      }
    } catch {
      /* 没网时就显示文件名 */
    }
  }

  /** 读取版本独立设置 */
  async loadSettings(pack: Pack) {
    if (pack.vs) return pack.vs;
    pack.vs = pack.real ? await api.versionSettings(settings.gameDir, pack.name) : {};
    return pack.vs;
  }

  /** 保存版本独立设置；改了版本隔离要重新读文件夹 */
  async saveSettings(pack: Pack, patch: Partial<VersionSettings>) {
    const prev = pack.vs ?? {};
    const next: VersionSettings = { ...prev, ...patch };
    for (const k of Object.keys(next) as (keyof VersionSettings)[]) if (next[k] == null || next[k] === "") delete next[k];
    pack.vs = next;
    if (!pack.real) {
      if ("isolation" in patch) pack.isolated = patch.isolation ?? true;
      return;
    }
    try {
      await api.setVersionSettings(settings.gameDir, pack.name, next);
      if ("isolation" in patch) {
        await this.refresh();
        const p = this.packs.find((x) => x.id === pack.id);
        if (p) await this.loadFiles(p, true);
      }
    } catch (e) {
      pack.vs = prev;
      toast.show(`保存失败：${e}`);
    }
  }
}
export const library = new Library();

/* ---------- 游戏目录 ---------- */
function dirLabel(path: string) {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  const last = parts[parts.length - 1] ?? path;
  // ...\HMCL\.minecraft → HMCL
  return last.toLowerCase() === ".minecraft" && parts.length > 1 ? parts[parts.length - 2] : last;
}

const samePath = (a: string, b: string) => a.replace(/[\\/]+$/, "").toLowerCase() === b.replace(/[\\/]+$/, "").toLowerCase();

class GameDirs {
  #default = $state("");
  get list() {
    return settings.gameDirs;
  }
  isDefault(path: string) {
    return !!this.#default && samePath(path, this.#default);
  }

  /** 启动时：保证默认目录和当前目录都在列表里 */
  async init() {
    this.#default = await api.defaultGameDir();
    if (!settings.gameDir) settings.gameDir = this.#default;
    const list = settings.gameDirs.filter((d) => !samePath(d.path, this.#default));
    list.unshift({ path: this.#default, name: "默认目录" });
    if (!list.some((d) => samePath(d.path, settings.gameDir))) list.push({ path: settings.gameDir, name: dirLabel(settings.gameDir) });
    settings.gameDirs = list;
  }

  switchTo(path: string) {
    settings.gameDir = path;
    library.refresh();
  }

  /** 弹出选择文件夹，加进列表并切换过去 */
  async add() {
    if (!isTauri()) return toast.show("浏览器预览里不能选文件夹");
    const picked = await api.pickFolder("选择游戏文件夹（.minecraft 或它的上一级）");
    if (!picked) return;
    try {
      const info = await api.inspectGameDir(picked);
      const exist = settings.gameDirs.find((d) => samePath(d.path, info.path));
      if (!exist) settings.gameDirs.push({ path: info.path, name: dirLabel(info.path) });
      this.switchTo(exist?.path ?? info.path);
      toast.show(info.versions ? `找到 ${info.versions} 个已安装的版本` : "这个文件夹里还没有版本，下载的游戏会装到这里");
    } catch (e) {
      toast.show(String(e));
    }
  }

  /** 从列表移除（不删除文件）；默认目录不能移除 */
  remove(path: string) {
    if (this.isDefault(path)) return;
    settings.gameDirs = settings.gameDirs.filter((d) => !samePath(d.path, path));
    if (samePath(path, settings.gameDir)) this.switchTo(this.#default);
  }
}
export const gameDirs = new GameDirs();

/* ---------- Java ---------- */
class Javas {
  found = $state<JavaInfo[] | null>(null);
  /** 自动找到的 + 手动添加的 */
  all = $derived([
    ...(this.found ?? []),
    ...settings.extraJavas.filter((x) => !(this.found ?? []).some((f) => f.path.toLowerCase() === x.path.toLowerCase())),
  ]);
  #loading: Promise<void> | undefined;

  load() {
    this.#loading ??= api.findJava().then((j) => {
      this.found = j;
    });
    return this.#loading;
  }

  /** 重新查找（下载了新的 Java 之后） */
  refresh() {
    this.#loading = undefined;
    return Promise.all([this.load(), this.loadOffers()]);
  }

  /* ---------- 下载 Java ---------- */
  offers = $state<JavaOffer[] | null>(null);

  async loadOffers() {
    this.offers = await api.javaOffers().catch(() => this.offers ?? []);
  }

  /** 正在下载这个组件的任务 */
  taskOf(component: string) {
    return Object.values(installs.tasks).find((t) => t.java === component);
  }

  download(o: JavaOffer) {
    if (this.taskOf(o.component)) return;
    installs.startJava(o);
  }

  label(path: string) {
    if (path === "auto") return "自动选择";
    const j = this.all.find((x) => x.path === path);
    return j ? `Java ${j.major}` : "找不到这个 Java";
  }

  /** 选一个 java.exe，校验后加入列表；返回它的路径 */
  async browse(): Promise<string | null> {
    if (!isTauri()) {
      toast.show("浏览器预览里不能选文件");
      return null;
    }
    const p = await api.pickJava();
    if (!p) return null;
    try {
      const info = await api.probeJava(p);
      if (!this.all.some((x) => x.path.toLowerCase() === info.path.toLowerCase())) settings.extraJavas.push(info);
      toast.show(`已添加 Java ${info.major}（${info.version}）`);
      return info.path;
    } catch (e) {
      toast.show(String(e));
      return null;
    }
  }
}
export const javas = new Javas();

/** Java 下拉框的选项：自动选择 + 每个 Java */
export function javaOptions() {
  return [
    { value: "auto", label: "自动选择" },
    ...javas.all.map((j) => ({ value: j.path, label: `Java ${j.major}`, hint: j.version, group: "已找到的 Java" })),
  ];
}

/* ---------- 下载安装任务 ---------- */
export interface InstallTask {
  id: string;
  mc: string;
  name: string;
  stage: string;
  stageIndex: number;
  stageCount: number;
  done: number;
  total: number;
  speed: number;
  status: "running" | "error" | "cancelling";
  error?: string;
  /** 整合包安装：Modrinth 项目号 */
  modpack?: string;
  /** Java 下载：组件名 */
  java?: string;
  /** 更换加载器：版本名 */
  loaderFor?: string;
}

export interface InstallOptions {
  mc: string;
  name: string;
  loader: { kind: LoaderKind; id: string } | null;
  optifine: { type: string; patch: string } | null;
}

class Installs {
  tasks = $state<Record<string, InstallTask>>({});
  #listening = false;

  /** 某个游戏版本正在进行（或失败待处理）的任务 */
  forMc(mc: string) {
    return Object.values(this.tasks).find((t) => t.mc === mc);
  }

  /** 正在安装的游戏版本数（不算 Java 下载和换加载器） */
  get running() {
    return Object.values(this.tasks).filter((t) => t.status !== "error" && !t.java && !t.loaderFor).length;
  }

  /** 给已有版本换加载器（Mod、存档、配置都留着） */
  startChangeLoader(pack: Pack, loader: { kind: LoaderKind; id: string } | null) {
    const id = `c-${Date.now()}`;
    const label = loader ? loader.kind : "原版";
    this.tasks[id] = {
      id,
      mc: "",
      name: pack.name,
      loaderFor: pack.name,
      stage: "准备中",
      stageIndex: 0,
      stageCount: loader ? 5 : 4,
      done: 0,
      total: 0,
      speed: 0,
      status: "running",
    };
    const done = async () => {
      delete this.tasks[id];
      await library.refresh();
      const p = library.packs.find((x) => x.name === pack.name);
      if (p) library.loadFiles(p, true).catch(() => {});
      toast.show(`${pack.name} 已经换成 ${label}${loader ? ` ${loader.id.replace(`${pack.mc}-`, "")}` : ""}`);
    };
    if (!isTauri()) {
      let p = 0;
      const timer = setInterval(() => {
        const t = this.tasks[id];
        if (!t || t.status === "cancelling") {
          clearInterval(timer);
          delete this.tasks[id];
          return;
        }
        p += 5;
        Object.assign(t, { stage: p < 60 ? "下载游戏本体和依赖库" : `安装 ${label}`, stageIndex: Math.min(t.stageCount, 1 + Math.floor(p / 25)), done: p % 25, total: 25 });
        if (p >= 100) {
          clearInterval(timer);
          const pk = library.packs.find((x) => x.name === pack.name);
          if (pk) Object.assign(pk, { loader: (loader?.kind ?? "原版") as Loader, loaderVersion: loader?.id ?? null, optifine: null, ...packLook(loader?.kind ?? null) });
          delete this.tasks[id];
          toast.show(`${pack.name} 已经换成 ${label}（演示）`);
        }
      }, 100);
      return;
    }
    this.#listen();
    api
      .changeLoader({ taskId: id, gameDir: settings.gameDir, name: pack.name, loader, source: settings.source, threads: settings.threads })
      .then(done)
      .catch((e) => {
        const msg = String(e);
        if (msg.includes("已取消")) {
          delete this.tasks[id];
          toast.show(`已取消更换 ${pack.name} 的加载器`);
        } else {
          const t = this.tasks[id];
          if (t) Object.assign(t, { status: "error", error: msg });
        }
      });
  }

  /** 下载 Java：先测速，用官方源和镜像里快的那个 */
  startJava(o: JavaOffer) {
    const id = `j-${Date.now()}`;
    this.tasks[id] = {
      id,
      mc: "",
      name: `Java ${o.major}`,
      java: o.component,
      stage: "准备中",
      stageIndex: 0,
      stageCount: 2,
      done: 0,
      total: 0,
      speed: 0,
      status: "running",
    };
    if (!isTauri()) {
      // 浏览器预览：假进度
      let p = 0;
      const timer = setInterval(() => {
        const t = this.tasks[id];
        if (!t || t.status === "cancelling") {
          clearInterval(timer);
          delete this.tasks[id];
          return;
        }
        p += 6;
        Object.assign(t, p < 20 ? { stage: "测试下载源速度", stageIndex: 1, done: p, total: 20 } : { stage: `下载 Java ${o.major}（BMCLAPI 镜像更快）`, stageIndex: 2, done: p - 20, total: 80 });
        if (p >= 100) {
          clearInterval(timer);
          delete this.tasks[id];
          const off = javas.offers?.find((x) => x.component === o.component);
          if (off) off.installed = { path: `java-${o.major}`, version: `${o.major}.0.0`, major: o.major };
          toast.show(`Java ${o.major} 下载好了（演示）`);
        }
      }, 120);
      return;
    }
    this.#listen();
    api
      .downloadJava(id, o.component, settings.threads)
      .then(async (info) => {
        const src = this.tasks[id]?.stage.match(/（(.+?)更快）/)?.[1];
        delete this.tasks[id];
        await javas.refresh();
        toast.show(`Java ${info.major} 下载好了${src ? `（用的${src}）` : ""}，启动游戏时会自动用它`);
      })
      .catch((e) => {
        const msg = String(e);
        if (msg.includes("已取消")) {
          delete this.tasks[id];
          toast.show(`已取消下载 Java ${o.major}`);
        } else {
          const t = this.tasks[id];
          if (t) Object.assign(t, { status: "error", error: msg });
        }
      });
  }

  #listen() {
    if (this.#listening || !isTauri()) return;
    this.#listening = true;
    listen<ProgressEvent>("install-progress", (e) => {
      const p = e.payload;
      const t = this.tasks[p.taskId];
      if (!t || t.status === "error") return;
      Object.assign(t, {
        stage: p.stage,
        stageIndex: p.stageIndex,
        stageCount: p.stageCount,
        done: p.done,
        total: p.total,
        speed: p.speed,
      });
    });
  }

  start(opt: InstallOptions) {
    const id = `t-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
    this.tasks[id] = {
      id,
      mc: opt.mc,
      name: opt.name,
      stage: "准备中",
      stageIndex: 0,
      stageCount: 4 + (opt.loader ? 1 : 0) + (opt.optifine ? 1 : 0),
      done: 0,
      total: 0,
      speed: 0,
      status: "running",
    };
    if (!isTauri()) return this.#simulate(id, opt);

    this.#listen();
    const req: InstallRequest = {
      taskId: id,
      gameDir: settings.gameDir,
      name: opt.name,
      mc: opt.mc,
      loader: opt.loader,
      optifine: opt.optifine,
      source: settings.source,
      threads: settings.threads,
    };
    api
      .install(req)
      .then(async () => {
        delete this.tasks[id];
        await library.refresh();
        if (!library.current) library.currentId = opt.name;
        toast.show(`${opt.name} 安装完成，可以在版本管理里找到`);
      })
      .catch((e) => {
        const msg = String(e);
        if (msg.includes("已取消")) {
          delete this.tasks[id];
          toast.show(`已取消安装 ${opt.name}`);
        } else {
          const t = this.tasks[id];
          if (t) Object.assign(t, { status: "error", error: msg });
        }
      });
  }

  cancel(id: string) {
    const t = this.tasks[id];
    if (!t) return;
    if (t.status === "error") {
      delete this.tasks[id];
      return;
    }
    t.status = "cancelling";
    if (isTauri()) api.cancel(id);
  }

  /** 某个整合包正在进行（或失败待处理）的安装 */
  forModpack(projectId: string) {
    return Object.values(this.tasks).find((t) => t.modpack === projectId);
  }

  /** 安装整合包：新建一个游戏版本，装好游戏、加载器和整合包里的文件 */
  startModpack(opt: { name: string; mc: string; loader: string | null; projectId: string; versionId: string; title: string; file?: string }) {
    const id = `t-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
    this.tasks[id] = {
      id,
      mc: opt.mc,
      name: opt.name,
      modpack: opt.projectId,
      stage: "准备中",
      stageIndex: 0,
      stageCount: opt.loader ? 8 : 7,
      done: 0,
      total: 0,
      speed: 0,
      status: "running",
    };
    if (!isTauri()) {
      const kind = opt.loader ? (opt.loader[0].toUpperCase() + opt.loader.slice(1)).replace("Neoforge", "NeoForge") : null;
      return this.#simulate(id, { mc: opt.mc, name: opt.name, loader: kind ? ({ kind } as InstallOptions["loader"]) : null, optifine: null } as InstallOptions, [
        "下载整合包",
        "获取版本信息",
        "下载游戏本体和依赖库",
        "下载资源文件",
        ...(kind ? [`安装 ${kind}`] : []),
        "写入版本配置",
        "下载整合包里的 Mod",
        "解压整合包配置",
      ]);
    }
    this.#listen();
    api
      .installModpack({
        taskId: id,
        gameDir: settings.gameDir,
        name: opt.name,
        projectId: opt.projectId,
        versionId: opt.versionId,
        file: opt.file ?? null,
        source: settings.source,
        threads: settings.threads,
      })
      .then(async () => {
        delete this.tasks[id];
        await library.refresh();
        library.currentId = opt.name;
        toast.show(`${opt.title} 装好了，在版本管理里叫「${opt.name}」`);
      })
      .catch((e) => {
        const msg = String(e);
        if (msg.includes("已取消")) {
          delete this.tasks[id];
          toast.show(`已取消安装 ${opt.title}`);
        } else {
          const t = this.tasks[id];
          if (t) Object.assign(t, { status: "error", error: msg });
        }
      });
  }

  /** 浏览器预览：假进度 */
  #simulate(id: string, opt: InstallOptions, custom?: string[]) {
    const stages = custom ?? ["获取版本信息", "下载游戏本体和依赖库", "下载资源文件", ...(opt.loader ? [`安装 ${opt.loader.kind}`] : []), ...(opt.optifine ? ["安装 OptiFine"] : []), "写入版本配置"];
    let i = 0;
    let done = 0;
    const timer = setInterval(() => {
      const t = this.tasks[id];
      if (!t || t.status === "cancelling") {
        clearInterval(timer);
        delete this.tasks[id];
        return;
      }
      done += 7;
      if (done >= 100) {
        done = 0;
        i++;
      }
      if (i >= stages.length) {
        clearInterval(timer);
        delete this.tasks[id];
        library.packs.unshift({
          id: opt.name,
          name: opt.name,
          mc: opt.mc,
          loader: (opt.loader?.kind ?? "原版") as Loader,
          ...packLook(opt.loader?.kind, opt.optifine ? "x" : null),
          lastPlayed: "从未",
          mods: [],
          shaders: [],
          resourcepacks: [],
        });
        toast.show(`${opt.name} 安装完成（演示）`);
        return;
      }
      Object.assign(t, { stage: stages[i], stageIndex: i + 1, stageCount: stages.length, done, total: 100, speed: 6_500_000 });
    }, 80);
  }
}
export const installs = new Installs();

/** 启动时：取默认游戏目录，然后扫描已安装的版本 */
export async function initBackend() {
  appearance.load();
  javas.load();
  accounts.syncMicrosoft();
  await gameDirs.init();
  skins.loadDefaults();
  await library.refresh();
}

/* ---------- 启动游戏 ----------
 * Tauri 里：调用后端补全文件、启动 Java，之后靠 game-ready / game-exit 事件更新状态。
 * 浏览器预览：假进度。
 */
export interface GameProblem {
  title: string;
  detail: string;
  /** 崩溃前最后几行日志 */
  tail?: string[];
  /** 日志文件路径 */
  log?: string;
}

const LAUNCH_STAGES = 5;

class Game {
  status = $state<"idle" | "preparing" | "running">("idle");
  /** 游戏窗口已经出现 */
  ready = $state(false);
  /** 正在启动 / 运行的版本 */
  packId = $state("");
  stage = $state("准备中");
  stageIndex = $state(0);
  /** 一共几步（自动下载 Java 时会多出两步） */
  stageCount = $state(LAUNCH_STAGES);
  #gotJava = false;
  done = $state(0);
  total = $state(0);
  elapsed = $state(0);
  /** 启动失败或崩溃时弹出的说明 */
  problem = $state<GameProblem | null>(null);
  /** 停止要点两次，防止误关丢存档 */
  confirmStop = $state(false);

  progress = $derived(
    Math.min(100, ((Math.max(this.stageIndex, 1) - 1 + (this.total ? this.done / this.total : 0)) / this.stageCount) * 100),
  );
  step = $derived(this.stage);

  #taskId = "";
  #timer: ReturnType<typeof setInterval> | undefined;
  #readyTimer: ReturnType<typeof setTimeout> | undefined;
  #confirmTimer: ReturnType<typeof setTimeout> | undefined;
  #listening = false;

  #listen() {
    if (this.#listening || !isTauri()) return;
    this.#listening = true;
    listen<ProgressEvent>("launch-progress", (e) => {
      const p = e.payload;
      if (p.taskId !== this.#taskId || this.status !== "preparing") return;
      this.stage = p.stage;
      this.stageIndex = p.stageIndex;
      this.stageCount = Math.max(LAUNCH_STAGES, p.stageCount);
      this.done = p.done;
      this.total = p.total;
      if (p.stage.startsWith("下载 Java")) this.#gotJava = true;
    });
    listen<string>("game-ready", () => {
      if (this.status === "running") this.ready = true;
    });
    listen<GameExit>("game-exit", (e) => {
      const x = e.payload;
      const name = this.#packName();
      this.#reset();
      if (x.crashed) {
        this.problem = {
          title: x.code === null ? "游戏异常退出" : `游戏异常退出（退出码 ${x.code}）`,
          detail: `${x.name} 意外关闭了。下面是最后的日志，常见原因：Mod 不兼容、缺少前置 Mod、内存不够或 Java 版本不对。`,
          tail: x.tail,
          log: x.log,
        };
      } else {
        toast.show(x.killed ? `已关闭 ${name}` : `${name} 已退出`);
      }
    });
  }

  #packName() {
    return library.packs.find((p) => p.id === this.packId)?.name ?? this.packId;
  }

  #reset() {
    clearInterval(this.#timer);
    clearTimeout(this.#readyTimer);
    clearTimeout(this.#confirmTimer);
    this.status = "idle";
    this.ready = false;
    this.confirmStop = false;
    this.elapsed = 0;
    this.stageIndex = 0;
    this.stageCount = LAUNCH_STAGES;
    this.done = 0;
    this.total = 0;
  }

  #startClock() {
    this.status = "running";
    this.elapsed = 0;
    this.#timer = setInterval(() => this.elapsed++, 1000);
    // 有些版本日志里没有就绪标记，过一会儿就当作窗口已经出来了
    this.#readyTimer = setTimeout(() => (this.ready = true), 25_000);
  }

  async start(pack?: Pack) {
    if (this.status !== "idle") return;
    const p = pack ?? library.current;
    if (!p) return;
    library.currentId = p.id;
    this.problem = null;
    this.packId = p.id;
    this.stage = "准备中";
    this.stageIndex = 0;
    this.stageCount = LAUNCH_STAGES;

    const vs = await library.loadSettings(p).catch(() => ({}) as VersionSettings);
    const acc = accounts.list.find((a) => a.id === vs.account) ?? accounts.current;
    if (acc.type === "authlib") {
      toast.show("外置登录还没接入，请先用离线或微软账户");
      return;
    }

    this.status = "preparing";
    if (!isTauri()) return this.#simulate();
    this.#listen();
    const taskId = `l-${Date.now()}`;
    this.#taskId = taskId;
    try {
      await api.launch({
        taskId,
        gameDir: settings.gameDir,
        name: p.name,
        java: vs.java ?? settings.java,
        // 0 = 自动分配，后端按 Mod 数量算
        memory: vs.memory ?? (settings.memoryMode === "auto" ? 0 : settings.memory),
        windowMode: vs.windowMode ?? settings.windowMode,
        jvmArgs: vs.jvmArgs ?? settings.jvmArgs,
        player: acc.name,
        msAccount: acc.type === "microsoft" ? acc.id : null,
        offlineSkin: offlineSkinOf(acc),
        source: settings.source,
        threads: settings.threads,
      });
      if (this.#taskId !== taskId) return;
      // 启动时自动下载了 Java：刷新一下列表
      if (this.#gotJava) {
        this.#gotJava = false;
        javas.refresh();
      }
      // 准备期间点了取消：进程刚起来就关掉
      if (this.status !== "preparing") {
        api.stopGame();
        return;
      }
      this.#startClock();
    } catch (e) {
      if (this.#taskId !== taskId) return;
      this.#reset();
      const msg = String(e);
      if (!msg.includes("已取消")) this.problem = { title: "启动失败", detail: msg };
    }
  }

  stop() {
    if (this.status === "preparing") {
      if (isTauri()) api.cancel(this.#taskId);
      this.#reset();
      toast.show("已取消启动");
      return;
    }
    if (this.status !== "running") return;
    // 第一次点只提示，3 秒内再点才真的关掉（强制关闭可能丢失最后的存档进度）
    if (!this.confirmStop) {
      this.confirmStop = true;
      clearTimeout(this.#confirmTimer);
      this.#confirmTimer = setTimeout(() => (this.confirmStop = false), 3000);
      return;
    }
    this.confirmStop = false;
    if (isTauri()) api.stopGame();
    else {
      const name = this.#packName();
      this.#reset();
      toast.show(`已关闭 ${name}`);
    }
  }

  toggle() {
    if (this.status === "idle") this.start();
    else this.stop();
  }

  /** 浏览器预览：假进度 */
  #simulate() {
    const stages = ["检查版本信息", "补全游戏文件", "补全资源文件", "解压 natives", "启动 Java"];
    this.stageIndex = 1;
    this.stage = stages[0];
    this.total = 100;
    this.done = 0;
    this.#timer = setInterval(() => {
      if (this.status !== "preparing") return clearInterval(this.#timer);
      this.done += 9;
      if (this.done >= 100) {
        this.done = 0;
        this.stageIndex++;
        if (this.stageIndex > stages.length) {
          clearInterval(this.#timer);
          this.#startClock();
          setTimeout(() => (this.ready = true), 1500);
          return;
        }
        this.stage = stages[this.stageIndex - 1];
      }
    }, 40);
  }
}
export const game = new Game();

/* ---------- 系统状态 ----------
 * 在 Tauri 里：监听 Rust 每秒推送的 `system-stats` 事件，是真实读数。
 * 在普通浏览器里预览：没有后端，退回模拟数据。
 */
interface SystemStats {
  cpu: number;
  memUsedGb: number;
  memTotalGb: number;
  gpu: number | null;
}

function walk(v: number, target: number, noise: number, min: number, max: number) {
  const n = v + (target - v) * 0.35 + (Math.random() * 2 - 1) * noise;
  return Math.min(max, Math.max(min, n));
}

class SysMon {
  /** true = 真实读数；false = 浏览器预览的模拟数据 */
  readonly live = isTauri();
  /** 还没收到第一次读数 */
  ready = $state(!isTauri());
  cpu = $state(0);
  gpu = $state<number | null>(null);
  mem = $state(0);
  memTotal = $state(16);
  #timer: ReturnType<typeof setInterval> | undefined;
  #unlisten: (() => void) | undefined;
  #users = 0;

  start() {
    if (this.#users++ > 0) return;
    if (this.live) {
      invoke("sysmon_watch", { on: true }).catch(() => {});
      listen<SystemStats>("system-stats", (e) => {
        const s = e.payload;
        this.cpu = s.cpu;
        this.gpu = s.gpu;
        this.mem = s.memUsedGb;
        this.memTotal = s.memTotalGb;
        this.ready = true;
      }).then((un) => {
        // 监听建立前就已经 stop 了
        if (this.#users === 0) un();
        else this.#unlisten = un;
      });
    } else {
      this.cpu = 16;
      this.gpu = 8;
      this.mem = 7.6;
      this.#timer = setInterval(() => this.#tick(), 1000);
    }
  }

  stop() {
    if (--this.#users > 0) return;
    if (this.live) invoke("sysmon_watch", { on: false }).catch(() => {});
    this.#unlisten?.();
    this.#unlisten = undefined;
    clearInterval(this.#timer);
  }

  /** 仅用于浏览器预览的模拟数据 */
  #tick() {
    const running = game.status === "running";
    const preparing = game.status === "preparing";
    this.cpu = walk(this.cpu, running ? 52 : preparing ? 68 : 15, 6, 1, 100);
    this.gpu = walk(this.gpu ?? 0, running ? 66 : 7, 5, 0, 100);
    const base = 7.4 + (running ? Math.min(settings.memory / 1024, 8) * 0.85 : 0);
    this.mem = walk(this.mem, base, 0.15, 2, this.memTotal);
  }
}
export const sysmon = new SysMon();

/* ---------- 轻提示 ---------- */
class Toast {
  msg = $state("");
  visible = $state(false);
  #t: ReturnType<typeof setTimeout> | undefined;

  show(msg: string) {
    this.msg = msg;
    this.visible = true;
    clearTimeout(this.#t);
    this.#t = setTimeout(() => (this.visible = false), 1800);
  }
}
export const toast = new Toast();

/* ---------- 本次运行内的小缓存：切页面回来不用重新请求 ---------- */
const memo = new Map<string, { at: number; value: unknown }>();

/** 取缓存；超过 maxAgeMs 就当没有 */
export function cached<T>(key: string, maxAgeMs: number): T | undefined {
  const hit = memo.get(key);
  if (!hit || Date.now() - hit.at > maxAgeMs) return undefined;
  return hit.value as T;
}

export function remember(key: string, value: unknown) {
  memo.set(key, { at: Date.now(), value });
  // 别无限长：最旧的先丢
  if (memo.size > 60) memo.delete(memo.keys().next().value!);
}

/* ---------- 工具函数 ---------- */
export function fmtTime(sec: number) {
  const h = String(Math.floor(sec / 3600)).padStart(2, "0");
  const m = String(Math.floor((sec % 3600) / 60)).padStart(2, "0");
  const s = String(sec % 60).padStart(2, "0");
  return `${h}:${m}:${s}`;
}

export function fmtCount(n: number) {
  if (n >= 1e8) return `${(n / 1e8).toFixed(1)} 亿`;
  if (n >= 1e4) return `${Math.round(n / 1e4)} 万`;
  return String(n);
}
