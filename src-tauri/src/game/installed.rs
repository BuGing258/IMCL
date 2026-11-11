//! 扫描已安装的版本，以及每个版本里的 Mod、光影、材质包。

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledVersion {
    pub name: String,
    pub mc: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub optifine: Option<String>,
    /// 最后修改时间（毫秒）
    pub modified: u64,
    pub dir: String,
    /// 版本隔离：Mod、存档、配置放在版本文件夹里，而不是游戏目录根下
    pub isolated: bool,
}

fn mtime_ms(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 没有 amc.json 的版本（其他启动器装的）：从版本 JSON 推断加载器
fn detect(json: &Value) -> (Option<String>, Option<String>, Option<String>) {
    let mut loader = None;
    let mut loader_version = None;
    let mut optifine = None;
    let libs = json["libraries"].as_array().cloned().unwrap_or_default();
    for l in &libs {
        let Some(name) = l["name"].as_str() else { continue };
        let parts: Vec<&str> = name.split(':').collect();
        let (g, a, v) = match parts.as_slice() {
            [g, a, v, ..] => (*g, *a, *v),
            _ => continue,
        };
        let found = match (g, a) {
            ("net.fabricmc", "fabric-loader") => Some("Fabric"),
            ("org.quiltmc", "quilt-loader") => Some("Quilt"),
            ("net.neoforged", "neoforge") | ("net.neoforged.fancymodloader", _) => Some("NeoForge"),
            ("net.minecraftforge", "forge") | ("net.minecraftforge", "fmlloader") => Some("Forge"),
            ("optifine", "OptiFine") => {
                // "1.20.1_HD_U_I6" → "HD_U I6"（和 amc.json 里的写法一致）
                let rest = v.split_once('_').map_or(v, |(_, r)| r);
                optifine = Some(rest.rsplit_once('_').map_or(rest.to_string(), |(t, p)| format!("{t} {p}")));
                None
            }
            _ => None,
        };
        if let (Some(f), None) = (found, &loader) {
            loader = Some(f.to_string());
            loader_version = Some(v.to_string());
        }
    }
    (loader, loader_version, optifine)
}

pub fn list(game_dir: &Path) -> Result<Vec<InstalledVersion>> {
    let versions = game_dir.join("versions");
    let Ok(rd) = std::fs::read_dir(&versions) else { return Ok(vec![]) };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let dir = e.path();
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
        let json_path = dir.join(format!("{name}.json"));
        let Ok(bytes) = std::fs::read(&json_path) else { continue };
        let Ok(json) = serde_json::from_slice::<Value>(&bytes) else { continue };

        let meta: Option<Value> = std::fs::read(dir.join("amc.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let s = |v: &Value, k: &str| v[k].as_str().map(str::to_string);
        let (loader, loader_version, optifine) = match &meta {
            Some(m) => (s(m, "loader"), s(m, "loaderVersion"), s(m, "optifine")),
            None => detect(&json),
        };
        let mc = meta
            .as_ref()
            .and_then(|m| s(m, "mc"))
            .or_else(|| s(&json, "clientVersion"))
            .or_else(|| s(&json, "inheritsFrom"))
            .unwrap_or_else(|| s(&json, "id").unwrap_or_else(|| name.clone()));

        out.push(InstalledVersion {
            isolated: isolated(&dir, meta.is_some()),
            modified: mtime_ms(&json_path).max(mtime_ms(&dir)),
            dir: dir.to_string_lossy().to_string(),
            name,
            mc,
            loader,
            loader_version,
            optifine,
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

/* ---------------- 版本独立设置 ---------------- */

/// 每个版本自己的设置，存在 versions/<name>/amc-settings.json。字段为空表示跟随全局。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionSettings {
    /// java(w).exe 路径，或 "auto"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java: Option<String>,
    /// 最大内存（MB）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<u32>,
    /// 启动用的账户 id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_mode: Option<String>,
    /// 额外 JVM 参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jvm_args: Option<String>,
    /// 版本隔离；为空时自动判断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolation: Option<bool>,
}

const SETTINGS_FILE: &str = "amc-settings.json";

fn version_dir(game_dir: &Path, version: &str) -> Result<PathBuf> {
    check_plain_name(version)?;
    Ok(game_dir.join("versions").join(version))
}

pub fn read_settings(game_dir: &Path, version: &str) -> Result<VersionSettings> {
    let p = version_dir(game_dir, version)?.join(SETTINGS_FILE);
    match std::fs::read(&p) {
        Ok(b) => Ok(serde_json::from_slice(&b).unwrap_or_default()),
        Err(_) => Ok(VersionSettings::default()),
    }
}

pub fn write_settings(game_dir: &Path, version: &str, s: &VersionSettings) -> Result<()> {
    let dir = version_dir(game_dir, version)?;
    if !dir.is_dir() {
        bail!("版本不存在：{version}");
    }
    let p = dir.join(SETTINGS_FILE);
    if *s == VersionSettings::default() {
        // 全部跟随全局时不留文件
        let _ = std::fs::remove_file(&p);
        return Ok(());
    }
    std::fs::write(&p, serde_json::to_vec_pretty(s)?).context("无法保存版本设置")
}

/// 没手动设置时自动判断：AMC 装的版本默认隔离；其他启动器装的，看版本文件夹里有没有 mods / saves 等
fn isolated(dir: &Path, by_amc: bool) -> bool {
    let explicit = std::fs::read(dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<VersionSettings>(&b).ok())
        .and_then(|s| s.isolation);
    explicit.unwrap_or_else(|| by_amc || ["mods", "saves", "config", "options.txt"].iter().any(|f| dir.join(f).exists()))
}

/// 游戏运行目录：隔离时是版本文件夹，否则是游戏目录
pub fn run_dir(game_dir: &Path, version: &str) -> Result<PathBuf> {
    let dir = version_dir(game_dir, version)?;
    Ok(if isolated(&dir, dir.join("amc.json").is_file()) { dir } else { game_dir.to_path_buf() })
}

/* ---------------- 游戏目录 ---------------- */

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirInfo {
    /// 实际使用的目录（选了 .minecraft 的上一级时会自动进到 .minecraft）
    pub path: String,
    pub versions: usize,
}

pub fn inspect_dir(path: &Path) -> Result<GameDirInfo> {
    if path.is_file() {
        bail!("这是一个文件，请选择文件夹");
    }
    let dir = if path.join("versions").is_dir() {
        path.to_path_buf()
    } else if path.join(".minecraft").join("versions").is_dir() {
        path.join(".minecraft")
    } else {
        path.to_path_buf()
    };
    let versions = list(&dir)?.len();
    Ok(GameDirInfo { path: dir.to_string_lossy().to_string(), versions })
}

/* ---------------- 版本里的文件 ---------------- */

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub file: String,
    pub size: u64,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFiles {
    pub mods: Vec<FileEntry>,
    pub shaderpacks: Vec<FileEntry>,
    pub resourcepacks: Vec<FileEntry>,
}

/// 只允许单纯的文件名 / 版本名，防止拼出版本目录以外的路径
pub fn check_plain_name(s: &str) -> Result<()> {
    if s.is_empty() || s == "." || s == ".." || s.contains(['/', '\\', ':']) {
        bail!("无效的名称：{s}");
    }
    Ok(())
}

pub fn kind_dir(game_dir: &Path, version: &str, kind: &str) -> Result<PathBuf> {
    if !matches!(kind, "mods" | "shaderpacks" | "resourcepacks") {
        bail!("未知的文件类型：{kind}");
    }
    Ok(run_dir(game_dir, version)?.join(kind))
}

fn scan(dir: &Path, accept: impl Fn(&str, bool) -> Option<bool>) -> Vec<FileEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<FileEntry> = rd
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().to_string();
            let md = e.metadata().ok()?;
            let enabled = accept(&file.to_lowercase(), md.is_dir())?;
            Some(FileEntry { size: if md.is_dir() { 0 } else { md.len() }, file, enabled })
        })
        .collect();
    out.sort_by_key(|f| f.file.to_lowercase());
    out
}

pub fn files(game_dir: &Path, version: &str) -> Result<VersionFiles> {
    let mods = scan(&kind_dir(game_dir, version, "mods")?, |f, is_dir| {
        if is_dir {
            None
        } else if f.ends_with(".jar") {
            Some(true)
        } else if f.ends_with(".jar.disabled") {
            Some(false)
        } else {
            None
        }
    });
    let packs = |f: &str, is_dir: bool| (is_dir || f.ends_with(".zip")).then_some(true);
    Ok(VersionFiles {
        mods,
        shaderpacks: scan(&kind_dir(game_dir, version, "shaderpacks")?, packs),
        resourcepacks: scan(&kind_dir(game_dir, version, "resourcepacks")?, packs),
    })
}

/// 通过改名启用 / 禁用 Mod，返回新文件名
pub fn set_mod_enabled(game_dir: &Path, version: &str, file: &str, enabled: bool) -> Result<String> {
    check_plain_name(file)?;
    let dir = kind_dir(game_dir, version, "mods")?;
    let from = dir.join(file);
    let new_name = if enabled {
        file.strip_suffix(".disabled").unwrap_or(file).to_string()
    } else if file.ends_with(".disabled") {
        file.to_string()
    } else {
        format!("{file}.disabled")
    };
    if new_name != file {
        std::fs::rename(&from, dir.join(&new_name)).with_context(|| format!("无法重命名 {file}"))?;
    }
    Ok(new_name)
}

/// 移到回收站（可以从回收站恢复）
pub fn trash_file(game_dir: &Path, version: &str, kind: &str, file: &str) -> Result<()> {
    check_plain_name(file)?;
    let path = kind_dir(game_dir, version, kind)?.join(file);
    if !path.exists() {
        bail!("文件不存在：{file}");
    }
    trash::delete(&path).with_context(|| format!("无法移到回收站：{file}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_loaders() {
        let j = json!({ "libraries": [
            { "name": "org.ow2.asm:asm:9.6" },
            { "name": "net.fabricmc:fabric-loader:0.16.9" },
        ]});
        assert_eq!(detect(&j), (Some("Fabric".into()), Some("0.16.9".into()), None));
        let j = json!({ "libraries": [{ "name": "optifine:OptiFine:1.20.1_HD_U_I6" }] });
        assert_eq!(detect(&j).2.as_deref(), Some("HD_U I6"));
    }

    #[test]
    fn settings_roundtrip_and_isolation() {
        let root = std::env::temp_dir().join(format!("amc-test-{}", std::process::id()));
        let v = root.join("versions").join("a");
        std::fs::create_dir_all(&v).unwrap();
        std::fs::write(v.join("a.json"), r#"{"id":"a","inheritsFrom":"1.20.1"}"#).unwrap();

        // 其他启动器装的、没有 mods 文件夹：不隔离
        assert_eq!(run_dir(&root, "a").unwrap(), root);
        assert_eq!(list(&root).unwrap()[0].mc, "1.20.1");
        // 有 mods 文件夹：隔离
        std::fs::create_dir(v.join("mods")).unwrap();
        assert_eq!(run_dir(&root, "a").unwrap(), v);
        // 手动关闭隔离
        let s = VersionSettings { isolation: Some(false), memory: Some(6144), ..Default::default() };
        write_settings(&root, "a", &s).unwrap();
        assert_eq!(read_settings(&root, "a").unwrap(), s);
        assert_eq!(run_dir(&root, "a").unwrap(), root);
        assert!(!list(&root).unwrap()[0].isolated);
        // 全部恢复默认会删掉设置文件
        write_settings(&root, "a", &VersionSettings::default()).unwrap();
        assert!(!v.join(SETTINGS_FILE).exists());

        // 选了 .minecraft 的上一级
        let parent = root.join("p");
        std::fs::create_dir_all(parent.join(".minecraft").join("versions")).unwrap();
        assert!(inspect_dir(&parent).unwrap().path.ends_with(".minecraft"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plain_names() {
        assert!(check_plain_name("sodium.jar").is_ok());
        assert!(check_plain_name("../x").is_err());
        assert!(check_plain_name("a\\b").is_err());
        assert!(check_plain_name("").is_err());
    }
}
