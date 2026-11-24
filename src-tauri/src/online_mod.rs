//! 内置的联机 Mod：Remote Play（mconline，https://github.com/BuGing258/MinecraftOnline，GPL-3.0）。
//! jar 直接编进启动器（src-tauri/bundled/mconline），安装时按版本的游戏版本和加载器挑一个写进 mods 文件夹，不用联网。
//! 更新 Mod 时：换掉 bundled 里的 jar，改下面的 VERSION 和 BUILDS 里的文件名（前端 api.ts 的 ONLINE_MOD_BUILDS 也要对上）。

use crate::game::installed;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::Path;

pub const VERSION: &str = "1.4.0";

struct Build {
    mc: &'static str,
    loader: &'static str,
    file: &'static str,
    bytes: &'static [u8],
}

/// 附属「服务器部署工具」：所有版本共用一个文件
const ADDON_FILE: &str = "mconline-server-addon-1.4.0.jar";
static ADDON: &[u8] = include_bytes!("../bundled/mconline/mconline-server-addon-1.4.0.jar");

/// 一行一个构建：游戏版本、加载器、bundled/mconline 里的文件名
macro_rules! build {
    ($mc:literal, $loader:literal, $file:literal) => {
        Build { mc: $mc, loader: $loader, file: $file, bytes: include_bytes!(concat!("../bundled/mconline/", $file)) }
    };
}

// static：每个 jar 在程序里只放一份（const 会在每个用到的地方展开）
static BUILDS: &[Build] = &[
    build!("1.20.1", "Fabric", "mconline-fabric-1.4.0.jar"),
    build!("1.20.1", "Forge", "mconline-forge-1.4.0.jar"),
    build!("1.21.1", "Fabric", "mconline-fabric-1.21.1-1.4.0.jar"),
    build!("1.21.1", "NeoForge", "mconline-neoforge-1.21.1-1.4.0.jar"),
    build!("1.21.10", "Fabric", "mconline-fabric-1.21.10-1.4.0.jar"),
    build!("1.21.10", "NeoForge", "mconline-neoforge-1.21.10-1.4.0.jar"),
    build!("1.21.11", "Fabric", "mconline-fabric-1.21.11-1.4.0.jar"),
    build!("1.21.11", "NeoForge", "mconline-neoforge-1.21.11-1.4.0.jar"),
    build!("26.1.2", "Fabric", "mconline-fabric-26.1.2-1.4.0.jar"),
    build!("26.1.2", "NeoForge", "mconline-neoforge-26.1.2-1.4.0.jar"),
    build!("26.3", "Fabric", "mconline-fabric-26.3-1.4.0.jar"),
    build!("26.3", "NeoForge", "mconline-neoforge-26.3-1.4.0.jar"),
];

fn build_for(mc: &str, loader: &str) -> Option<&'static Build> {
    BUILDS.iter().find(|b| b.mc == mc && b.loader.eq_ignore_ascii_case(loader))
}

/// 文件名去掉 .disabled 后是不是联机 Mod 的 jar（任何版本的都算，重新安装时一起换掉）
fn is_ours(name: &str, addon: bool) -> bool {
    let n = name.to_ascii_lowercase();
    let n = n.strip_suffix(".disabled").unwrap_or(&n);
    n.starts_with("mconline-") && n.ends_with(".jar") && n.starts_with("mconline-server-addon") == addon
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// 内置的 Mod 版本
    pub version: &'static str,
    /// 这个游戏版本 + 加载器有没有对应的构建
    pub supported: bool,
    /// 已经装着的联机 Mod 文件名（没装是 None）
    pub installed: Option<String>,
    /// 已经装着的服务器部署工具文件名
    pub addon: Option<String>,
    /// 装着的文件和内置的一模一样（按内容比，版本号不变只修 bug 时也能看出要更新）
    pub current: bool,
}

/// 文件内容和内置的是不是同一个（先比大小，省得每次都读整个 jar）
fn same_bytes(path: &Path, bytes: &[u8]) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.len() == bytes.len() as u64) && std::fs::read(path).is_ok_and(|b| b == bytes)
}

fn find(dir: &Path, addon: bool) -> Option<String> {
    let rd = std::fs::read_dir(dir).ok()?;
    rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).find(|n| is_ours(n, addon))
}

pub fn status(game_dir: &Path, version: &str, mc: &str, loader: &str) -> Result<Status> {
    let dir = installed::kind_dir(game_dir, version, "mods")?;
    let build = build_for(mc, loader);
    let installed = find(&dir, false);
    let addon = find(&dir, true);
    let current = match (build, &installed) {
        (Some(b), Some(name)) => {
            same_bytes(&dir.join(name), b.bytes) && addon.as_ref().is_none_or(|a| same_bytes(&dir.join(a), ADDON))
        }
        _ => false,
    };
    Ok(Status { version: VERSION, supported: build.is_some(), installed, addon, current })
}

/// 安装（或重新安装）：先删掉 mods 里所有旧的联机 Mod 文件（包括禁用的），再写入对应版本的 jar。
/// addon = false 时也会删掉已有的部署工具，装完的结果就是界面上勾选的样子
pub fn install(game_dir: &Path, version: &str, mc: &str, loader: &str, addon: bool) -> Result<Status> {
    let Some(build) = build_for(mc, loader) else {
        bail!("联机 Mod 不支持 {mc} {loader}，支持 1.20.1 Fabric / Forge，1.21.1、1.21.10、1.21.11、26.1.2、26.3 的 Fabric / NeoForge");
    };
    let dir = installed::kind_dir(game_dir, version, "mods")?;
    std::fs::create_dir_all(&dir).context("无法创建 mods 文件夹")?;
    for e in std::fs::read_dir(&dir).context("无法读取 mods 文件夹")?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if is_ours(&name, false) || is_ours(&name, true) {
            std::fs::remove_file(e.path()).with_context(|| format!("无法替换旧文件 {name}（游戏是不是还开着？）"))?;
        }
    }
    std::fs::write(dir.join(build.file), build.bytes).with_context(|| format!("无法写入 {}", build.file))?;
    if addon {
        std::fs::write(dir.join(ADDON_FILE), ADDON).with_context(|| format!("无法写入 {ADDON_FILE}"))?;
    }
    status(game_dir, version, mc, loader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_build() {
        assert_eq!(build_for("1.20.1", "fabric").map(|b| b.file), Some("mconline-fabric-1.4.0.jar"));
        assert_eq!(build_for("26.1.2", "NeoForge").map(|b| b.file), Some("mconline-neoforge-26.1.2-1.4.0.jar"));
        assert_eq!(build_for("26.3", "fabric").map(|b| b.file), Some("mconline-fabric-26.3-1.4.0.jar"));
        assert!(build_for("1.21.1", "Forge").is_none());
        // 1.21.10 和 1.21.1 不能混：游戏版本要完全一样
        assert_eq!(build_for("1.21.10", "Fabric").map(|b| b.file), Some("mconline-fabric-1.21.10-1.4.0.jar"));
        assert!(build_for("1.20.4", "Fabric").is_none());
        assert!(build_for("1.20.1", "原版").is_none());
    }

    #[test]
    fn recognizes_files() {
        assert!(is_ours("mconline-fabric-1.1.0.jar", false));
        assert!(is_ours("mconline-forge-1.2.0.jar.disabled", false));
        assert!(is_ours("mconline-neoforge-26.3-1.3.0.jar", false));
        assert!(!is_ours("mconline-server-addon-1.4.0.jar", false));
        assert!(is_ours("mconline-server-addon-1.2.0.jar", true));
        assert!(!is_ours("sodium-fabric-0.5.jar", false));
    }

    /// 在临时的游戏目录里装一遍、重装一遍：IMCL 装的版本（有 amc.json）是隔离的，mods 在版本文件夹里
    #[test]
    fn install_and_reinstall() {
        let game = std::env::temp_dir().join(format!("imcl-online-test-{}", std::process::id()));
        let ver = game.join("versions").join("测试 1.20.1");
        let mods = ver.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(ver.join("amc.json"), "{}").unwrap();
        // 旧版本的联机 Mod（禁用的也算）和别的 Mod
        std::fs::write(mods.join("mconline-fabric-1.1.0.jar.disabled"), b"old").unwrap();
        std::fs::write(mods.join("sodium.jar"), b"other").unwrap();

        let s = status(&game, "测试 1.20.1", "1.20.1", "Fabric").unwrap();
        assert!(s.supported);
        assert_eq!(s.installed.as_deref(), Some("mconline-fabric-1.1.0.jar.disabled"));

        let s = install(&game, "测试 1.20.1", "1.20.1", "Fabric", true).unwrap();
        assert_eq!(s.installed.as_deref(), Some("mconline-fabric-1.4.0.jar"));
        assert_eq!(s.addon.as_deref(), Some(ADDON_FILE));
        assert!(!mods.join("mconline-fabric-1.1.0.jar.disabled").exists());
        assert!(mods.join("sodium.jar").exists());
        assert_eq!(std::fs::read(mods.join("mconline-fabric-1.4.0.jar")).unwrap().len(), BUILDS[0].bytes.len());
        assert!(s.current);

        // 同名但内容是旧构建（版本号没变、只修了 bug）：要显示成可以更新
        std::fs::write(mods.join("mconline-fabric-1.4.0.jar"), b"PK old build").unwrap();
        let s = status(&game, "测试 1.20.1", "1.20.1", "Fabric").unwrap();
        assert_eq!(s.installed.as_deref(), Some("mconline-fabric-1.4.0.jar"));
        assert!(!s.current);
        // 部署工具旧了也算
        let s = install(&game, "测试 1.20.1", "1.20.1", "Fabric", true).unwrap();
        assert!(s.current);
        std::fs::write(mods.join(ADDON_FILE), b"PK old addon").unwrap();
        assert!(!status(&game, "测试 1.20.1", "1.20.1", "Fabric").unwrap().current);

        // 重新安装、不要部署工具
        let s = install(&game, "测试 1.20.1", "1.20.1", "Fabric", false).unwrap();
        assert_eq!(s.installed.as_deref(), Some("mconline-fabric-1.4.0.jar"));
        assert_eq!(s.addon, None);
        let names: Vec<_> = std::fs::read_dir(&mods).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(names.len(), 2, "{names:?}");

        // 不支持的版本
        assert!(install(&game, "测试 1.20.1", "1.21.4", "Fabric", true).is_err());
        std::fs::remove_dir_all(&game).unwrap();
    }

    #[test]
    fn bundled_jars_are_zips() {
        for b in BUILDS {
            assert_eq!(&b.bytes[..2], b"PK", "{}", b.file);
        }
        assert_eq!(&ADDON[..2], b"PK");
    }
}
