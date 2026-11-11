// 全局界面状态（Svelte 5 runes）。除系统状态外，目前都是前端模拟。
import { listen } from "@tauri-apps/api/event";
import { mockPacks, type Pack, type ResKind } from "./mock";

/** 是否运行在 Tauri 窗口里（普通浏览器预览时为 false） */
export function isTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

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
}

export const settings = $state<Settings>(
  load("amc.settings", {
    memory: 4096,
    java: "auto",
    windowMode: "default",
    theme: "system",
    source: "bmcl",
    threads: 64,
  }),
);

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

/* ---------- 已安装的版本 ---------- */
class Library {
  packs = $state<Pack[]>(structuredClone(mockPacks));
  currentId = $state("survival");
  current = $derived(this.packs.find((p) => p.id === this.currentId) ?? this.packs[0]);
}
export const library = new Library();

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
