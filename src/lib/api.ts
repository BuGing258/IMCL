// 后端接口封装。在 Tauri 窗口里调用 Rust；在普通浏览器里预览时返回假数据，保证界面能看。
import { invoke } from "@tauri-apps/api/core";
import { mockMarket, mockVersions } from "./mock";

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type Source = "bmcl" | "official";
export type LoaderKind = "Fabric" | "Quilt" | "Forge" | "NeoForge";

export interface ManifestEntry {
  id: string;
  type: string;
  releaseTime: string;
}

export interface LoaderVersion {
  /** 安装用的完整版本号 */
  id: string;
  /** 显示用 */
  version: string;
  stable: boolean;
}

export interface OptiFineVersion {
  type: string;
  patch: string;
  filename: string;
  /** "Forge 47.2.18"；"Forge N/A" 表示不兼容 Forge */
  forge: string | null;
  preview: boolean;
}

export interface JavaInfo {
  path: string;
  version: string;
  major: number;
}

export interface InstalledVersion {
  name: string;
  mc: string;
  loader: string | null;
  loaderVersion: string | null;
  optifine: string | null;
  modified: number;
  dir: string;
  isolated: boolean;
}

/** 版本独立设置；字段为空 = 跟随全局 */
/** 内存分配：总内存、手动上限、自动会给多少（MB） */
export interface MemoryPlan {
  totalMb: number;
  maxMb: number;
  autoMb: number;
}

/** 内置联机 Mod 在某个版本里的情况（见 online_mod.rs） */
export interface OnlineModStatus {
  /** 内置的 Mod 版本 */
  version: string;
  /** 这个游戏版本 + 加载器能不能装 */
  supported: boolean;
  /** 已经装着的文件名 */
  installed: string | null;
  /** 已经装着的服务器部署工具文件名 */
  addon: string | null;
  /** 装着的文件和内置的内容一模一样（版本号不变只修 bug 时靠这个判断要不要更新） */
  current: boolean;
}

/** 联机 Mod 支持的游戏版本和加载器（和 online_mod.rs 里的 BUILDS 一致） */
/** 内置联机 Mod 的版本（和 online_mod.rs 的 VERSION 一致） */
export const ONLINE_MOD_VERSION = "1.3.6";
export const ONLINE_MOD_BUILDS: { mc: string; loader: string; note: string }[] = [
  { mc: "1.20.1", loader: "Fabric", note: "Fabric Loader 0.14.21+ · Java 17+" },
  { mc: "1.20.1", loader: "Forge", note: "Forge 47+ · Java 17+" },
  { mc: "1.21.1", loader: "Fabric", note: "Fabric Loader 0.15.0+ · Java 21+" },
  { mc: "1.21.1", loader: "NeoForge", note: "NeoForge 21.1+ · Java 21+" },
  { mc: "1.21.10", loader: "Fabric", note: "Fabric Loader 0.17.0+ · Java 21+" },
  { mc: "1.21.10", loader: "NeoForge", note: "NeoForge 21.10+ · Java 21+" },
  { mc: "1.21.11", loader: "Fabric", note: "Fabric Loader 0.17.0+ · Java 21+" },
  { mc: "1.21.11", loader: "NeoForge", note: "NeoForge 21.11+ · Java 21+" },
  { mc: "26.1.2", loader: "Fabric", note: "Fabric Loader 0.18.4+ · Java 25+" },
  { mc: "26.1.2", loader: "NeoForge", note: "NeoForge 26.1.2+ · Java 25+" },
  { mc: "26.3", loader: "Fabric", note: "Fabric Loader 0.18.4+ · Java 25+" },
  { mc: "26.3", loader: "NeoForge", note: "NeoForge 26.3+ · Java 25+" },
];
export const onlineModSupports = (mc: string, loader: string) =>
  ONLINE_MOD_BUILDS.some((b) => b.mc === mc && b.loader.toLowerCase() === loader.toLowerCase());

export interface VersionSettings {
  java?: string | null;
  /** 0 表示自动分配 */
  memory?: number | null;
  account?: string | null;
  windowMode?: string | null;
  jvmArgs?: string | null;
  isolation?: boolean | null;
}

export interface LaunchRequest {
  taskId: string;
  gameDir: string;
  name: string;
  java: string;
  memory: number;
  windowMode: string;
  jvmArgs: string;
  player: string;
  source: Source;
  threads: number;
  /** 微软账户 id；离线账户不填 */
  msAccount?: string | null;
  /** 离线账户的皮肤（PNG data URL），游戏里也显示 */
  offlineSkin?: { png: string; slim: boolean } | null;
}

/** 游戏进程结束 */
export interface GameExit {
  name: string;
  code: number | null;
  /** 是被启动器关掉的 */
  killed: boolean;
  crashed: boolean;
  /** 最后几十行日志 */
  tail: string[];
  /** 完整日志文件 */
  log: string;
}

/* ---------- Modrinth ---------- */
export type ResKindApi = "mod" | "shader" | "resourcepack" | "modpack";

export interface SearchReq {
  kind: ResKindApi;
  query: string;
  mc?: string | null;
  loader?: string | null;
  category?: string | null;
  sort: "relevance" | "downloads" | "updated";
  offset: number;
}

export interface Hit {
  id: string;
  slug: string;
  title: string;
  author: string;
  description: string;
  downloads: number;
  icon: string | null;
  categories: string[];
  color: number | null;
}

export interface SearchResult {
  hits: Hit[];
  total: number;
}

export interface ResInstallReq {
  gameDir: string;
  version: string;
  kind: ResKindApi;
  projectId: string;
  mc: string;
  loader: string | null;
  /** 指定版本；不填就装最新的正式版 */
  versionId?: string | null;
  /** 已经装了也重新装（旧文件移到回收站） */
  reinstall?: boolean;
  /** 自动装必需的前置，默认开 */
  withDeps?: boolean;
}

export interface ResDepsReq {
  gameDir: string;
  version: string;
  kind: ResKindApi;
  versionId: string;
  mc: string;
  loader: string | null;
}

/** 选中的资源版本需要的一个前置 */
export interface ResDep {
  projectId: string;
  title: string;
  icon: string | null;
  /** 这个游戏版本里已经装了 */
  installed: boolean;
  /** 有适合这个游戏版本的文件 */
  available: boolean;
}

export interface ResInstallResult {
  title: string;
  file: string;
  versionNumber: string;
  /** 被替换掉的旧文件 */
  replaced: string[];
  deps: string[];
  missing: string[];
}

export interface ResVersionsReq {
  projectId: string;
  kind: ResKindApi;
  mc: string;
  loader: string | null;
}

/** 一个可以选的资源版本 */
export interface ModVersion {
  id: string;
  name: string;
  versionNumber: string;
  versionType: "release" | "beta" | "alpha";
  published: string;
  downloads: number;
  file: string;
  size: number;
  /** 支持的游戏版本和加载器（整合包显示用） */
  gameVersions: string[];
  loaders: string[];
}

/** 可以下载的 Java */
export interface JavaOffer {
  component: string;
  major: number;
  version: string;
  /** 适合哪些游戏版本 */
  forGames: string;
  /** 已经下载好的话是它的信息 */
  installed: JavaInfo | null;
}

/** 安装整合包：新建一个游戏版本 */
export interface ModpackRequest {
  taskId: string;
  gameDir: string;
  name: string;
  /** 从 Modrinth 下载时填 */
  projectId?: string;
  versionId?: string;
  /** 从本地 .mrpack 导入时填 */
  file?: string | null;
  source: Source;
  threads: number;
}

/** 给已有版本换加载器 */
export interface ChangeLoaderRequest {
  taskId: string;
  gameDir: string;
  name: string;
  /** null = 改回原版 */
  loader: { kind: LoaderKind; id: string } | null;
  source: Source;
  threads: number;
}

/** 一个有新版本的 Mod / 光影 / 材质包 */
export interface ModUpdate {
  /** "mods/文件名" */
  file: string;
  projectId: string;
  title: string;
  icon: string | null;
  current: string;
  versionId: string;
  versionNumber: string;
}

/** 本地整合包里写的信息 */
export interface PackInfo {
  name: string;
  version: string;
  summary: string | null;
  mc: string;
  /** 比如 "Fabric 0.16.10" */
  loader: string | null;
  files: number;
}

/** 导出时可以选的一项（版本文件夹第一层） */
export interface ExportEntry {
  name: string;
  isDir: boolean;
  size: number;
  files: number;
  default: boolean;
}

export interface ExportRequest {
  gameDir: string;
  version: string;
  out: string;
  name: string;
  packVersion: string;
  summary?: string | null;
  include: string[];
}

export interface ExportResult {
  path: string;
  /** 写成 Modrinth 下载地址的文件数 */
  linked: number;
  /** 直接打包进去的文件数 */
  bundled: number;
  size: number;
  warnings: string[];
}

/** 已安装文件对应的 Modrinth 项目；键是 "mods/文件名" */
export interface FileInfo {
  projectId: string;
  title: string;
  icon: string | null;
  versionNumber: string;
}

/* ---------- 微软账户 ---------- */
export interface MsAccountInfo {
  id: string;
  name: string;
  uuid: string;
}

export interface DeviceCode {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
}

export interface GameDirInfo {
  path: string;
  versions: number;
}

export interface FileEntry {
  file: string;
  size: number;
  enabled: boolean;
}

export interface VersionFiles {
  mods: FileEntry[];
  shaderpacks: FileEntry[];
  resourcepacks: FileEntry[];
}

export interface InstallRequest {
  taskId: string;
  gameDir: string;
  name: string;
  mc: string;
  loader: { kind: LoaderKind; id: string } | null;
  optifine: { type: string; patch: string } | null;
  source: Source;
  threads: number;
}

export interface ProgressEvent {
  taskId: string;
  stage: string;
  stageIndex: number;
  stageCount: number;
  done: number;
  total: number;
  speed: number;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const api = {
  defaultGameDir: (): Promise<string> =>
    isTauri() ? invoke("default_game_dir") : Promise.resolve("C:\\Users\\你\\AppData\\Roaming\\.minecraft"),

  manifest: async (source: Source): Promise<ManifestEntry[]> => {
    if (isTauri()) return invoke("version_manifest", { source });
    await sleep(300);
    return mockVersions.map((v) => ({ id: v.id, type: v.type, releaseTime: v.date }));
  },

  loaderVersions: async (source: Source, loader: LoaderKind, mc: string): Promise<LoaderVersion[]> => {
    if (isTauri()) return invoke("loader_versions", { source, loader, mc });
    await sleep(400);
    const v = { Fabric: "0.16.9", Quilt: "0.27.1", Forge: "47.3.0", NeoForge: "21.1.80" }[loader];
    return [{ id: v, version: v, stable: true }];
  },

  optifineVersions: async (mc: string): Promise<OptiFineVersion[]> => {
    if (isTauri()) return invoke("optifine_versions", { mc });
    await sleep(400);
    return [{ type: "HD_U", patch: "I6", filename: `OptiFine_${mc}_HD_U_I6.jar`, forge: "Forge 47.2.18", preview: false }];
  },

  findJava: (): Promise<JavaInfo[]> =>
    isTauri() ? invoke("find_java") : Promise.resolve([{ path: "java", version: "21", major: 21 }]),

  javaOffers: async (): Promise<JavaOffer[]> => {
    if (isTauri()) return invoke("java_offers");
    return [
      { component: "java-runtime-epsilon", major: 25, version: "", forGames: "26.1 及以后", installed: null },
      { component: "java-runtime-delta", major: 21, version: "", forGames: "1.20.5 ～ 1.21.x", installed: { path: "java", version: "21.0.7", major: 21 } },
      { component: "java-runtime-gamma", major: 17, version: "", forGames: "1.18 ～ 1.20.4", installed: null },
      { component: "jre-legacy", major: 8, version: "", forGames: "1.16.5 及以前", installed: null },
    ];
  },
  /** 下载 Java（自动测速选官方源或镜像），进度走 install-progress */
  downloadJava: (taskId: string, component: string, threads: number): Promise<JavaInfo> =>
    invoke("download_java", { taskId, component, threads }),

  install: (req: InstallRequest): Promise<void> => invoke("install_version", { req }),
  installModpack: (req: ModpackRequest): Promise<void> => invoke("install_modpack", { req }),
  changeLoader: (req: ChangeLoaderRequest): Promise<void> => invoke("change_loader", { req }),
  /** 检查某类资源的更新（禁用的 Mod 不检查） */
  resourceUpdates: async (gameDir: string, name: string, kind: ResKindApi, mc: string, loader: string | null): Promise<ModUpdate[]> => {
    if (isTauri()) return invoke("modrinth_updates", { gameDir, name, kind, mc, loader });
    await sleep(600);
    return [
      { file: "mods/sodium", projectId: "AANobbMI", title: "Sodium", icon: null, current: "0.5.8", versionId: "s-new", versionNumber: "0.5.13" },
      { file: "mods/lithium", projectId: "gvQqBUqZ", title: "Lithium", icon: null, current: "0.11.2", versionId: "l-new", versionNumber: "0.11.3" },
      { file: "mods/jade", projectId: "nvQzSEkH", title: "Jade", icon: null, current: "11.8.0", versionId: "j-new", versionNumber: "11.13.1" },
    ];
  },
  readModpack: async (path: string): Promise<PackInfo> => {
    if (isTauri()) return invoke("read_modpack", { path });
    await sleep(200);
    return { name: "好友生存包", version: "1.0.0", summary: "我们服务器用的 Mod", mc: "1.20.1", loader: "Fabric 0.15.7", files: 12 };
  },
  exportEntries: async (gameDir: string, name: string): Promise<ExportEntry[]> => {
    if (isTauri()) return invoke("export_entries", { gameDir, name });
    await sleep(200);
    return [
      { name: "mods", isDir: true, size: 48_000_000, files: 12, default: true },
      { name: "config", isDir: true, size: 320_000, files: 41, default: true },
      { name: "resourcepacks", isDir: true, size: 9_800_000, files: 1, default: true },
      { name: "options.txt", isDir: false, size: 5_200, files: 1, default: true },
      { name: "servers.dat", isDir: false, size: 300, files: 1, default: true },
      { name: "saves", isDir: true, size: 260_000_000, files: 820, default: false },
      { name: "screenshots", isDir: true, size: 12_000_000, files: 9, default: false },
    ];
  },
  exportModpack: async (req: ExportRequest): Promise<ExportResult> => {
    if (isTauri()) return invoke("export_modpack", { req });
    await sleep(900);
    return { path: req.out, linked: 10, bundled: 46, size: 180_000, warnings: [] };
  },
  cancel: (taskId: string): Promise<void> => invoke("cancel_install", { taskId }),

  listInstalled: (gameDir: string): Promise<InstalledVersion[]> => invoke("list_installed", { gameDir }),
  /** 把整个版本文件夹移到回收站 */
  deleteVersion: (gameDir: string, name: string): Promise<void> => invoke("delete_version", { gameDir, name }),
  versionFiles: (gameDir: string, name: string): Promise<VersionFiles> => invoke("version_files", { gameDir, name }),
  setModEnabled: (gameDir: string, name: string, file: string, enabled: boolean): Promise<string> =>
    invoke("set_mod_enabled", { gameDir, name, file, enabled }),
  trashFile: (gameDir: string, name: string, kind: string, file: string): Promise<void> =>
    invoke("trash_version_file", { gameDir, name, kind, file }),

  /** 补全文件并启动，Java 进程起来后返回进程号 */
  launch: (req: LaunchRequest): Promise<number> => invoke("launch_game", { req }),
  stopGame: (): Promise<boolean> => invoke("stop_game"),

  searchResources: async (req: SearchReq): Promise<SearchResult> => {
    if (isTauri()) return invoke("modrinth_search", { req });
    await sleep(250);
    const q = req.query.toLowerCase();
    const hits = mockMarket
      .filter((m) => m.kind === req.kind && (!q || m.name.toLowerCase().includes(q) || m.id.includes(q)))
      .map((m) => ({
        id: m.id,
        slug: m.id,
        title: m.name,
        author: m.author,
        description: m.desc,
        downloads: m.downloads,
        icon: null,
        categories: [],
        color: parseInt(m.color.slice(1), 16),
      }));
    return { hits: req.offset ? [] : hits, total: hits.length };
  },
  /** 这个资源适合目标游戏版本的所有版本（新的在前） */
  resourceVersions: async (req: ResVersionsReq): Promise<ModVersion[]> => {
    if (isTauri()) return invoke("modrinth_versions", { req });
    await sleep(400);
    const types: ModVersion["versionType"][] = ["release", "release", "beta", "release", "alpha", "release"];
    const mcs = ["1.21.1", "1.20.1", "1.20.1", "1.19.2", "1.19.2", "1.18.2"];
    return types.map((t, i) => ({
      gameVersions: [req.mc || mcs[i]],
      loaders: req.kind === "modpack" ? [i % 3 === 2 ? "forge" : "fabric"] : [],
      id: `${req.projectId}-v${i}`,
      name: `${req.projectId} ${3 - i * 0.1}`,
      versionNumber: `${(3 - i * 0.1).toFixed(1)}.0+${req.mc || mcs[i]}`,
      versionType: t,
      published: new Date(Date.now() - i * 26 * 86400_000).toISOString(),
      downloads: Math.round(900_000 / (i + 1)),
      file: `${req.projectId}-${i}.jar`,
      size: 1_200_000 - i * 40_000,
    }));
  },
  /** 选中版本的必需前置 */
  resourceDeps: async (req: ResDepsReq): Promise<ResDep[]> => {
    if (isTauri()) return invoke("modrinth_deps", { req });
    await sleep(300);
    if (req.kind !== "mod" || req.versionId.endsWith("v5")) return [];
    return [
      { projectId: "P7dR8mSH", title: "Fabric API", icon: null, installed: true, available: true },
      { projectId: "9s6osm5g", title: "Cloth Config API", icon: null, installed: false, available: true },
      { projectId: "mOgUt4GM", title: "Mod Menu", icon: null, installed: false, available: !req.versionId.endsWith("v1") },
    ];
  },
  installResource: async (req: ResInstallReq): Promise<ResInstallResult> => {
    if (isTauri()) return invoke("modrinth_install", { req });
    await sleep(700);
    return {
      title: req.projectId,
      file: `${req.projectId}.jar`,
      versionNumber: req.versionId ?? "最新",
      replaced: req.reinstall ? [`${req.projectId}-old.jar`] : [],
      deps: req.withDeps === false ? [] : ["Cloth Config API"],
      missing: [],
    };
  },
  identify: (gameDir: string, name: string): Promise<Record<string, FileInfo>> =>
    isTauri() ? invoke("modrinth_identify", { gameDir, name }) : Promise.resolve({}),

  msLogin: (clientId: string): Promise<MsAccountInfo> => invoke("ms_login", { clientId }),
  msLoginCancel: (): Promise<void> => invoke("ms_login_cancel"),
  msAccounts: (): Promise<MsAccountInfo[]> => (isTauri() ? invoke("ms_accounts") : Promise.resolve([])),
  msRemove: (id: string): Promise<void> => (isTauri() ? invoke("ms_remove", { id }) : Promise.resolve()),
  /** 游戏自带的 Steve / Alex 皮肤（data URL），从已安装的客户端里取；一个都没装时为 null */
  defaultSkins: (dirs: string[]): Promise<{ steve: string; alex: string } | null> =>
    isTauri() ? invoke("default_skins", { dirs }) : Promise.resolve(null),
  /** 正版账户当前的皮肤（data URL） */
  msSkin: (id: string): Promise<string | null> => (isTauri() ? invoke("ms_skin", { id }) : Promise.resolve(null)),

  probeJava: async (path: string): Promise<JavaInfo> => {
    if (isTauri()) return invoke("probe_java", { path });
    return { path, version: "17.0.2", major: 17 };
  },
  inspectGameDir: async (path: string): Promise<GameDirInfo> => {
    if (isTauri()) return invoke("inspect_game_dir", { path });
    return { path, versions: 0 };
  },
  versionSettings: (gameDir: string, name: string): Promise<VersionSettings> => invoke("version_settings", { gameDir, name }),
  setVersionSettings: (gameDir: string, name: string, settings: VersionSettings): Promise<void> =>
    invoke("set_version_settings", { gameDir, name, settings }),

  /** 系统的“选择文件夹”对话框；取消时返回 null */
  pickFolder: async (title: string): Promise<string | null> => {
    if (!isTauri()) return null;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ directory: true, title });
    return typeof r === "string" ? r : null;
  },
  pickMrpack: async (): Promise<string | null> => {
    if (!isTauri()) return "C:/Users/demo/Downloads/好友生存包.mrpack";
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ title: "选择整合包", filters: [{ name: "Modrinth 整合包", extensions: ["mrpack", "zip"] }] });
    return typeof r === "string" ? r : null;
  },
  pickSaveMrpack: async (defaultName: string): Promise<string | null> => {
    if (!isTauri()) return `C:/Users/demo/Desktop/${defaultName}.mrpack`;
    const { save } = await import("@tauri-apps/plugin-dialog");
    const r = await save({ title: "导出整合包", defaultPath: `${defaultName}.mrpack`, filters: [{ name: "Modrinth 整合包", extensions: ["mrpack"] }] });
    return r ?? null;
  },
  pickImage: async (): Promise<string | null> => {
    if (!isTauri()) return null;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ title: "选择背景图片", filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }] });
    return typeof r === "string" ? r : null;
  },
  /** 把图片复制到应用数据目录当背景，返回 data URL */
  setBackgroundImage: (path: string): Promise<string> => invoke("set_background_image", { path }),
  backgroundImage: (): Promise<string | null> => invoke("background_image"),
  isWin11: (): Promise<boolean> => invoke("is_win11"),
  onlineModStatus: (gameDir: string, name: string, mc: string, loader: string): Promise<OnlineModStatus> =>
    isTauri()
      ? invoke("online_mod_status", { gameDir, name, mc, loader })
      : Promise.resolve({ version: ONLINE_MOD_VERSION, supported: onlineModSupports(mc, loader), installed: null, addon: null, current: false }),
  installOnlineMod: (gameDir: string, name: string, mc: string, loader: string, addon: boolean): Promise<OnlineModStatus> =>
    isTauri()
      ? invoke("install_online_mod", { gameDir, name, mc, loader, addon })
      : new Promise((r) =>
          setTimeout(() => r({ version: ONLINE_MOD_VERSION, supported: true, installed: `mconline-${loader.toLowerCase()}-${ONLINE_MOD_VERSION}.jar`, addon: addon ? `mconline-server-addon-${ONLINE_MOD_VERSION}.jar` : null, current: true }), 600),
        ),
  memoryPlan: (gameDir: string, version: string | null): Promise<MemoryPlan> =>
    isTauri()
      ? invoke("memory_plan", { gameDir, version })
      : Promise.resolve({ totalMb: 16384, maxMb: 14336, autoMb: version ? 4096 : 2048 }),
  pickJava: async (): Promise<string | null> => {
    if (!isTauri()) return null;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ title: "选择 java.exe 或 javaw.exe", filters: [{ name: "Java", extensions: ["exe"] }] });
    return typeof r === "string" ? r : null;
  },
};

export function fmtBytes(n: number) {
  if (n >= 1024 * 1024 * 1024) return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (n >= 1024) return `${Math.round(n / 1024)} KB`;
  return `${n} B`;
}

export function fmtRelative(ms: number) {
  const d = Date.now() - ms;
  const min = 60_000;
  if (d < min) return "刚刚";
  if (d < 60 * min) return `${Math.floor(d / min)} 分钟前`;
  if (d < 24 * 60 * min) return `${Math.floor(d / 60 / min)} 小时前`;
  const days = Math.floor(d / 24 / 60 / min);
  if (days < 30) return `${days} 天前`;
  return new Date(ms).toLocaleDateString("zh-CN");
}

/** 和后端 validate_name 保持一致 */
export function checkVersionName(name: string, existing: string[]): string {
  if (!name.trim()) return "版本名称不能为空";
  if (name.trim() !== name) return "版本名称前后不能有空格";
  if ([...name].length > 64) return "版本名称太长了（最多 64 个字）";
  if (/[\\/:*?"<>|\x00-\x1f]/.test(name)) return '不能包含 \\ / : * ? " < > | 这些字符';
  if (name === "." || name === ".." || name.endsWith(".")) return "版本名称不能以 . 结尾";
  if (existing.some((e) => e.toLowerCase() === name.toLowerCase())) return `已经有名为「${name}」的版本了`;
  return "";
}
