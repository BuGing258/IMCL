// 演示用假数据：界面预览阶段使用，接入后端后替换为真实数据源。
import type { VersionSettings } from "./api";

export type PackIcon = "sword" | "cube" | "leaf";
export type Loader = "Fabric" | "Forge" | "NeoForge" | "Quilt" | "原版";

export interface ModEntry {
  id: string;
  name: string;
  version: string;
  size: string;
  enabled: boolean;
}

/** 一个已安装的游戏版本（实例），里面装着自己的 Mod、光影和材质包 */
export interface Pack {
  id: string;
  name: string;
  mc: string;
  loader: Loader;
  loaderVersion?: string | null;
  optifine?: string | null;
  color: string;
  icon: PackIcon;
  lastPlayed: string;
  /** Mod 列表；真实版本里 id 是文件名 */
  mods: ModEntry[];
  /** 真实版本里是文件名，演示数据里是市场 id */
  shaders: string[];
  resourcepacks: string[];
  /** 以下只有真实安装的版本才有 */
  real?: boolean;
  dir?: string;
  modified?: number;
  /** 是否已经读取过版本文件夹里的 Mod/光影/材质包 */
  filesLoaded?: boolean;
  /** 版本隔离（Mod 放在版本文件夹里） */
  isolated?: boolean;
  /** 版本独立设置；undefined = 还没读取 */
  vs?: VersionSettings;
}

/** 按加载器给版本挑颜色和图标 */
export function packLook(loader: string | null | undefined, optifine?: string | null): Pick<Pack, "color" | "icon"> {
  switch (loader) {
    case "Fabric":
      return { color: "#C98A2B", icon: "cube" };
    case "Quilt":
      return { color: "#7F77DD", icon: "cube" };
    case "Forge":
      return { color: "#378ADD", icon: "sword" };
    case "NeoForge":
      return { color: "#D85A30", icon: "sword" };
    default:
      return optifine ? { color: "#E24B4A", icon: "leaf" } : { color: "#639922", icon: "leaf" };
  }
}

export const mockPacks: Pack[] = [
  {
    id: "survival",
    name: "好友生存服",
    mc: "1.20.1",
    loader: "Fabric",
    color: "#1D9E75",
    icon: "sword",
    lastPlayed: "昨天",
    mods: [
      { id: "fabric-api", name: "Fabric API", version: "0.92.2", size: "2.0 MB", enabled: true },
      { id: "sodium", name: "Sodium", version: "0.5.8", size: "1.1 MB", enabled: true },
      { id: "lithium", name: "Lithium", version: "0.11.2", size: "0.7 MB", enabled: true },
      { id: "iris", name: "Iris Shaders", version: "1.7.0", size: "2.4 MB", enabled: true },
      { id: "xaero-minimap", name: "Xaero 小地图", version: "24.2.0", size: "1.4 MB", enabled: true },
      { id: "jei", name: "JEI 物品管理器", version: "15.3.0", size: "1.2 MB", enabled: true },
      { id: "jade", name: "Jade", version: "11.8.0", size: "0.6 MB", enabled: true },
      { id: "appleskin", name: "AppleSkin", version: "2.5.1", size: "0.1 MB", enabled: true },
      { id: "modmenu", name: "Mod Menu", version: "7.2.2", size: "0.5 MB", enabled: true },
      { id: "farmers-delight", name: "农夫乐事", version: "1.2.4", size: "3.1 MB", enabled: false },
      { id: "create", name: "机械动力", version: "0.5.1", size: "16.8 MB", enabled: true },
      { id: "ferritecore", name: "FerriteCore", version: "6.0.1", size: "0.1 MB", enabled: true },
    ],
    shaders: ["complementary-reimagined"],
    resourcepacks: ["fresh-animations"],
  },
  {
    id: "creative",
    name: "创造测试服",
    mc: "1.21.4",
    loader: "Fabric",
    color: "#7F77DD",
    icon: "cube",
    lastPlayed: "3 天前",
    mods: [
      { id: "fabric-api", name: "Fabric API", version: "0.114.0", size: "2.2 MB", enabled: true },
      { id: "worldedit", name: "WorldEdit", version: "7.3.10", size: "5.2 MB", enabled: true },
      { id: "litematica", name: "Litematica", version: "0.21.0", size: "2.8 MB", enabled: true },
      { id: "sodium", name: "Sodium", version: "0.6.5", size: "1.2 MB", enabled: true },
      { id: "modmenu", name: "Mod Menu", version: "13.0.0", size: "0.5 MB", enabled: true },
    ],
    shaders: [],
    resourcepacks: [],
  },
  {
    id: "vanilla",
    name: "原版 1.21.4",
    mc: "1.21.4",
    loader: "原版",
    color: "#C98A2B",
    icon: "leaf",
    lastPlayed: "上周",
    mods: [],
    shaders: [],
    resourcepacks: ["faithful-32x"],
  },
];

export interface GameVersion {
  id: string;
  type: "release" | "snapshot";
  date: string;
}

export const mockVersions: GameVersion[] = [
  { id: "1.21.4", type: "release", date: "2024-12-03" },
  { id: "1.21.3", type: "release", date: "2024-10-23" },
  { id: "1.21.1", type: "release", date: "2024-08-08" },
  { id: "1.20.6", type: "release", date: "2024-04-29" },
  { id: "24w14a", type: "snapshot", date: "2024-04-03" },
  { id: "1.20.4", type: "release", date: "2023-12-07" },
  { id: "23w45a", type: "snapshot", date: "2023-11-08" },
  { id: "1.20.1", type: "release", date: "2023-06-12" },
  { id: "1.19.4", type: "release", date: "2023-03-14" },
  { id: "22w42a", type: "snapshot", date: "2022-10-19" },
  { id: "1.18.2", type: "release", date: "2022-02-28" },
  { id: "1.16.5", type: "release", date: "2021-01-15" },
  { id: "1.12.2", type: "release", date: "2017-09-18" },
  { id: "1.8.9", type: "release", date: "2015-12-09" },
];

/* ---------- 资源市场：Mod / 光影 / 材质包 ---------- */
export type ResKind = "mod" | "shader" | "resourcepack";

export const RES_LABELS: Record<ResKind, string> = {
  mod: "Mod",
  shader: "光影",
  resourcepack: "材质包",
};

export const RES_CATEGORIES: Record<ResKind, string[]> = {
  mod: ["优化", "实用", "科技", "装饰", "前置"],
  shader: ["写实", "风格化", "性能"],
  resourcepack: ["高清", "动画", "原版风", "界面"],
};

export interface MarketItem {
  id: string;
  kind: ResKind;
  name: string;
  author: string;
  desc: string;
  downloads: number;
  category: string;
  color: string;
}

export const mockMarket: MarketItem[] = [
  // Mod
  { id: "sodium", kind: "mod", name: "Sodium", author: "CaffeineMC", desc: "现代化渲染引擎，大幅提升帧率", downloads: 72_000_000, category: "优化", color: "#1D9E75" },
  { id: "lithium", kind: "mod", name: "Lithium", author: "CaffeineMC", desc: "优化游戏逻辑，服务器更流畅", downloads: 35_000_000, category: "优化", color: "#378ADD" },
  { id: "iris", kind: "mod", name: "Iris Shaders", author: "coderbot", desc: "光影加载器，装了它才能用光影", downloads: 51_000_000, category: "优化", color: "#7F77DD" },
  { id: "ferritecore", kind: "mod", name: "FerriteCore", author: "malte0811", desc: "显著降低内存占用", downloads: 28_000_000, category: "优化", color: "#888780" },
  { id: "fabric-api", kind: "mod", name: "Fabric API", author: "FabricMC", desc: "大部分 Fabric Mod 的前置库", downloads: 120_000_000, category: "前置", color: "#C98A2B" },
  { id: "jei", kind: "mod", name: "JEI 物品管理器", author: "mezz", desc: "查看物品与合成配方", downloads: 64_000_000, category: "实用", color: "#D85A30" },
  { id: "xaero-minimap", kind: "mod", name: "Xaero 小地图", author: "xaero96", desc: "小地图与路径点", downloads: 41_000_000, category: "实用", color: "#185FA5" },
  { id: "jade", kind: "mod", name: "Jade", author: "Snownee", desc: "准星指向时显示方块与生物信息", downloads: 38_000_000, category: "实用", color: "#0F6E56" },
  { id: "appleskin", kind: "mod", name: "AppleSkin", author: "squeek502", desc: "显示饱食度与饱和度", downloads: 47_000_000, category: "实用", color: "#E24B4A" },
  { id: "create", kind: "mod", name: "机械动力", author: "simibubi", desc: "齿轮、传送带与各种机械装置", downloads: 56_000_000, category: "科技", color: "#BA7517" },
  { id: "farmers-delight", kind: "mod", name: "农夫乐事", author: "vectorwing", desc: "扩展农业与烹饪玩法", downloads: 33_000_000, category: "装饰", color: "#639922" },
  { id: "supplementaries", kind: "mod", name: "Supplementaries", author: "MehVahdJukaar", desc: "大量原版风格的装饰方块", downloads: 30_000_000, category: "装饰", color: "#D4537E" },
  // 光影
  { id: "complementary-reimagined", kind: "shader", name: "Complementary Reimagined", author: "EminGT", desc: "保留原版味道的增强光影，好看又均衡", downloads: 9_800_000, category: "风格化", color: "#378ADD" },
  { id: "complementary-unbound", kind: "shader", name: "Complementary Unbound", author: "EminGT", desc: "更写实的天空、云和光照", downloads: 6_200_000, category: "写实", color: "#185FA5" },
  { id: "bsl", kind: "shader", name: "BSL Shaders", author: "capttatsu", desc: "经典光影，色彩鲜明", downloads: 11_000_000, category: "风格化", color: "#D85A30" },
  { id: "sildurs-vibrant", kind: "shader", name: "Sildur's Vibrant", author: "Sildur", desc: "多档画质可选的老牌光影", downloads: 7_400_000, category: "写实", color: "#BA7517" },
  { id: "makeup-ultra-fast", kind: "shader", name: "MakeUp Ultra Fast", author: "XorDev", desc: "低配电脑也能流畅运行", downloads: 4_100_000, category: "性能", color: "#1D9E75" },
  { id: "photon", kind: "shader", name: "Photon", author: "SixthSurge", desc: "柔和自然的光照效果", downloads: 3_300_000, category: "写实", color: "#7F77DD" },
  // 材质包
  { id: "faithful-32x", kind: "resourcepack", name: "Faithful 32x", author: "Faithful Team", desc: "原版风格的 32 倍高清材质", downloads: 15_000_000, category: "高清", color: "#639922" },
  { id: "fresh-animations", kind: "resourcepack", name: "Fresh Animations", author: "FreshLX", desc: "给生物加上更生动的动作", downloads: 12_000_000, category: "动画", color: "#D4537E" },
  { id: "stay-true", kind: "resourcepack", name: "Stay True", author: "haha_yes", desc: "保持原版质感的细节增强", downloads: 3_800_000, category: "原版风", color: "#888780" },
  { id: "better-leaves", kind: "resourcepack", name: "Better Leaves", author: "Motschen", desc: "让树叶看起来更茂密", downloads: 5_600_000, category: "原版风", color: "#3B6D11" },
  { id: "default-dark-mode", kind: "resourcepack", name: "Default Dark Mode", author: "nebulr", desc: "把游戏界面换成深色", downloads: 4_900_000, category: "界面", color: "#444441" },
  { id: "xali-books", kind: "resourcepack", name: "Xali 附魔书", author: "Xali", desc: "不同附魔的书显示不同颜色", downloads: 2_700_000, category: "界面", color: "#993556" },
];

export function marketName(id: string) {
  return mockMarket.find((m) => m.id === id)?.name ?? id;
}
