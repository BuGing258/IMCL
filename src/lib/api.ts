// 后端接口封装。在 Tauri 窗口里调用 Rust；在普通浏览器里预览时返回假数据，保证界面能看。
import { invoke } from "@tauri-apps/api/core";
import { mockVersions } from "./mock";

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
export interface VersionSettings {
  java?: string | null;
  memory?: number | null;
  account?: string | null;
  windowMode?: string | null;
  jvmArgs?: string | null;
  isolation?: boolean | null;
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

  install: (req: InstallRequest): Promise<void> => invoke("install_version", { req }),
  cancel: (taskId: string): Promise<void> => invoke("cancel_install", { taskId }),

  listInstalled: (gameDir: string): Promise<InstalledVersion[]> => invoke("list_installed", { gameDir }),
  versionFiles: (gameDir: string, name: string): Promise<VersionFiles> => invoke("version_files", { gameDir, name }),
  setModEnabled: (gameDir: string, name: string, file: string, enabled: boolean): Promise<string> =>
    invoke("set_mod_enabled", { gameDir, name, file, enabled }),
  trashFile: (gameDir: string, name: string, kind: string, file: string): Promise<void> =>
    invoke("trash_version_file", { gameDir, name, kind, file }),

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
