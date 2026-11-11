// 全局界面状态（Svelte 5 runes）。
// 已接后端：系统状态、下载安装、已安装版本。其余（Mod 市场、启动游戏、账户登录）仍是前端模拟。
import { listen } from "@tauri-apps/api/event";
import { api, isTauri, type InstallRequest, type JavaInfo, type LoaderKind, type ProgressEvent, type VersionSettings } from "./api";
import { mockPacks, packLook, type Loader, type Pack, type ResKind } from "./mock";

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

class Nav {
  stack = $state<PageId[]>(["home"]);
  /** 1 = 前进（新页面从右往左滑入），-1 = 返回 */
  dir = $state<1 | -1>(1);
  current = $derived(this.stack[this.stack.length - 1]);

  go(page: PageId) {
    if (page === this.current) return;
    this.dir = 1;
    this.stack = [...this.stack, page];
  }

  back() {
    if (this.stack.length <= 1) return;
    this.dir = -1;
    this.stack = this.stack.slice(0, -1);
  }

  home() {
    if (this.stack.length <= 1) return;
    this.dir = -1;
    this.stack = ["home"];
  }
}
export const nav = new Nav();

/** 跨页面共享的小状态，比如从版本管理跳到 Mod 页时预先选好“光影”标签 */
export const ui = $state<{ resKind: ResKind; resQuery: string }>({ resKind: "mod", resQuery: "" });

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
}

export interface GameDir {
  path: string;
  name: string;
}

export const settings = $state<Settings>(
  load("amc.settings", {
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
    extraJavas: [],
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

  addOffline(name: string) {
    const acc: Account = { id: `a-${Date.now()}`, name: name.trim(), type: "offline" };
    this.list.push(acc);
    this.currentId = acc.id;
  }

  remove(id: string) {
    if (this.list.length <= 1) return;
    this.list = this.list.filter((a) => a.id !== id);
    if (this.currentId === id) this.currentId = this.list[0].id;
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
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loaded = true;
    }
  }

  /** 读取某个版本文件夹里的 Mod、光影、材质包 */
  async loadFiles(pack: Pack, force = false) {
    if (!pack.real || (pack.filesLoaded && !force)) return;
    const f = await api.versionFiles(settings.gameDir, pack.name);
    const kb = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
    pack.mods = f.mods.map((m) => ({
      id: m.file,
      name: m.file.replace(/\.disabled$/i, "").replace(/\.jar$/i, ""),
      version: "",
      size: kb(m.size),
      enabled: m.enabled,
    }));
    pack.shaders = f.shaderpacks.map((x) => x.file);
    pack.resourcepacks = f.resourcepacks.map((x) => x.file);
    pack.filesLoaded = true;
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
    { value: "auto", label: "自动选择", hint: "按游戏版本挑" },
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

  get running() {
    return Object.values(this.tasks).filter((t) => t.status !== "error").length;
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

  /** 浏览器预览：假进度 */
  #simulate(id: string, opt: InstallOptions) {
    const stages = ["获取版本信息", "下载游戏本体和依赖库", "下载资源文件", ...(opt.loader ? [`安装 ${opt.loader.kind}`] : []), ...(opt.optifine ? ["安装 OptiFine"] : []), "写入版本配置"];
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
  javas.load();
  await gameDirs.init();
  await library.refresh();
}

/* ---------- 游戏启动（模拟） ---------- */
class Game {
  status = $state<"idle" | "preparing" | "running">("idle");
  progress = $state(0);
  elapsed = $state(0);
  step = $derived(
    this.progress < 25 ? "检查游戏文件" : this.progress < 75 ? "补全资源文件" : "启动 Java 进程",
  );
  #timer: ReturnType<typeof setInterval> | undefined;

  start() {
    if (this.status !== "idle") return;
    this.status = "preparing";
    this.progress = 0;
    this.#timer = setInterval(() => {
      this.progress = Math.min(100, this.progress + 1.6);
      if (this.progress >= 100) {
        clearInterval(this.#timer);
        this.status = "running";
        this.elapsed = 0;
        this.#timer = setInterval(() => this.elapsed++, 1000);
      }
    }, 40);
  }

  stop() {
    clearInterval(this.#timer);
    this.status = "idle";
    this.progress = 0;
    this.elapsed = 0;
  }

  toggle() {
    if (this.status === "idle") this.start();
    else this.stop();
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
