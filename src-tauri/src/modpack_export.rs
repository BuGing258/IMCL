//! 把一个版本导出成 Modrinth 整合包（.mrpack），发给朋友导入就能玩一样的 Mod。
//! Modrinth 上认得出来的 Mod / 光影 / 材质包只写下载地址（包很小），其余文件直接打包进 overrides。

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::game::installed;
use crate::modrinth;

/// 永远不导出（启动器 / 游戏运行时生成的东西）
const NEVER: &[&str] = &[
    "logs",
    "crash-reports",
    "natives",
    "versions",
    "libraries",
    "assets",
    "runtime",
    ".amc-tmp",
    ".fabric",
    ".mixin.out",
    ".cache",
    "debug",
    "downloads",
    "webcache",
    "launcher_profiles.json",
    "launcher_accounts.json",
    "launcher_settings.json",
    "usercache.json",
    "usernamecache.json",
    "amc.json",
    "amc-settings.json",
    "amc-latest.log",
    "command_history.txt",
];

/// 默认勾上的（朋友玩同一个整合包需要的）
const DEFAULT_ON: &[&str] = &[
    "mods",
    "config",
    "defaultconfigs",
    "kubejs",
    "scripts",
    "resourcepacks",
    "shaderpacks",
    "global_packs",
    "datapacks",
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
];

/// 可以选的一项（版本运行目录下的第一层）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub files: usize,
    /// 默认勾选
    pub default: bool,
}

fn dir_stats(p: &Path) -> (u64, usize) {
    let mut size = 0;
    let mut files = 0;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                size += e.metadata().map(|m| m.len()).unwrap_or(0);
                files += 1;
            }
        }
    }
    (size, files)
}

fn hidden(name: &str, version: &str) -> bool {
    let low = name.to_lowercase();
    NEVER.contains(&low.as_str())
        || low.ends_with(".log")
        || low.starts_with("hs_err_pid")
        || low.ends_with(".amc-part")
        || name == format!("{version}.jar")
        || name == format!("{version}.json")
}

/// 列出可以导出的东西
pub fn entries(game_dir: &Path, version: &str) -> Result<Vec<Entry>> {
    let run_dir = installed::run_dir(game_dir, version)?;
    let mut out = Vec::new();
    for e in std::fs::read_dir(&run_dir).context("读取版本文件夹失败")?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if hidden(&name, version) {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        let (size, files) = if ft.is_dir() { dir_stats(&e.path()) } else { (e.metadata().map(|m| m.len()).unwrap_or(0), 1) };
        if files == 0 {
            continue; // 空文件夹
        }
        out.push(Entry { default: DEFAULT_ON.contains(&name.to_lowercase().as_str()), name, is_dir: ft.is_dir(), size, files });
    }
    // 默认勾的在前，其余按名字
    out.sort_by(|a, b| b.default.cmp(&a.default).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReq {
    pub game_dir: String,
    pub version: String,
    /// 保存到哪（.mrpack）
    pub out: String,
    pub name: String,
    pub pack_version: String,
    #[serde(default)]
    pub summary: Option<String>,
    /// 勾选的第一层文件 / 文件夹名
    pub include: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    /// 写成下载地址的文件数（Modrinth 上有的）
    pub linked: usize,
    /// 直接打包进去的文件数
    pub bundled: usize,
    pub size: u64,
    pub warnings: Vec<String>,
}

/// 能写成下载地址的文件：mods / resourcepacks / shaderpacks 第一层的 jar / zip
fn linkable(rel: &str) -> bool {
    let mut parts = rel.split('/');
    let (Some(dir), Some(file), None) = (parts.next(), parts.next(), parts.next()) else { return false };
    let low = file.to_lowercase();
    matches!(dir, "mods" | "resourcepacks" | "shaderpacks") && (low.ends_with(".jar") || low.ends_with(".zip") || low.ends_with(".jar.disabled"))
}

/// mrpack 里加载器的写法；Forge 存的是 1.20.1-47.2.0，整合包里只要 47.2.0
fn loader_dep(loader: &str, version: &str, mc: &str) -> Option<(&'static str, String)> {
    let v = version.strip_prefix(&format!("{mc}-")).unwrap_or(version);
    let v = v.strip_suffix(&format!("-{mc}")).unwrap_or(v).to_string();
    let key = match loader {
        "Fabric" => "fabric-loader",
        "Quilt" => "quilt-loader",
        "Forge" => "forge",
        "NeoForge" => "neoforge",
        _ => return None,
    };
    Some((key, v))
}

/// 用 sha1 去 Modrinth 查：sha1 → (下载地址, sha512, 大小)
async fn lookup(hashes: &[String]) -> Result<HashMap<String, (String, String, u64)>> {
    let mut out = HashMap::new();
    for chunk in hashes.chunks(500) {
        let versions: HashMap<String, Value> = modrinth::post("/version_files", &json!({ "hashes": chunk, "algorithm": "sha1" })).await?;
        for (h, v) in versions {
            let file = v["files"].as_array().and_then(|fs| fs.iter().find(|f| f.pointer("/hashes/sha1").and_then(Value::as_str) == Some(h.as_str())));
            let Some(f) = file else { continue };
            let (Some(url), Some(sha512)) = (f["url"].as_str(), f.pointer("/hashes/sha512").and_then(Value::as_str)) else { continue };
            // 整合包里只能写官方允许的下载域名
            if !url.starts_with("https://cdn.modrinth.com/") {
                continue;
            }
            out.insert(h, (url.to_string(), sha512.to_string(), f["size"].as_u64().unwrap_or(0)));
        }
    }
    Ok(out)
}

pub async fn export(req: &ExportReq) -> Result<ExportResult> {
    let game_dir = PathBuf::from(&req.game_dir);
    let info = installed::list(&game_dir)?
        .into_iter()
        .find(|v| v.name == req.version)
        .ok_or_else(|| anyhow!("找不到版本 {}", req.version))?;
    let run_dir = installed::run_dir(&game_dir, &req.version)?;
    if req.name.trim().is_empty() {
        bail!("整合包名称不能为空");
    }
    let mut warnings = Vec::new();

    // 只允许导出列表里有的（防止传进来 ../ 之类的路径）
    let allowed: HashSet<String> = entries(&game_dir, &req.version)?.into_iter().map(|e| e.name).collect();
    let picked: Vec<&String> = req.include.iter().filter(|n| allowed.contains(*n)).collect();

    // 收集文件：(整合包里的相对路径, 本地路径)
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for name in picked {
        let p = run_dir.join(name);
        if p.is_file() {
            files.push((name.clone(), p));
            continue;
        }
        let mut stack = vec![p];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let path = e.path();
                match e.file_type() {
                    Ok(t) if t.is_dir() => stack.push(path),
                    Ok(t) if t.is_file() => {
                        let rel = path.strip_prefix(&run_dir).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                        files.push((rel, path));
                    }
                    _ => {}
                }
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    // Modrinth 上有的写成下载地址
    let candidates: Vec<(String, PathBuf)> = files.iter().filter(|(r, _)| linkable(r)).cloned().collect();
    let hashed: Vec<(String, String)> = tauri::async_runtime::spawn_blocking(move || {
        candidates.into_iter().filter_map(|(r, p)| crate::net::sha1_file(&p).ok().map(|h| (r, h))).collect()
    })
    .await?;
    let found = match lookup(&hashed.iter().map(|(_, h)| h.clone()).collect::<Vec<_>>()).await {
        Ok(m) => m,
        Err(_) => {
            warnings.push("连不上 Modrinth，所有 Mod 都直接打包进去了，文件会大一些".into());
            HashMap::new()
        }
    };
    let link_of: HashMap<&str, &(String, String, u64)> = hashed.iter().filter_map(|(r, h)| found.get(h).map(|x| (r.as_str(), x))).collect();

    let mut index_files = Vec::new();
    let mut bundle: Vec<(String, PathBuf)> = Vec::new();
    for (rel, path) in &files {
        match (link_of.get(rel.as_str()), hashed.iter().find(|(r, _)| r == rel)) {
            (Some((url, sha512, size)), Some((_, sha1))) => index_files.push(json!({
                "path": rel,
                "hashes": { "sha1": sha1, "sha512": sha512 },
                "env": { "client": "required", "server": "required" },
                "downloads": [url],
                "fileSize": size,
            })),
            _ => bundle.push((rel.clone(), path.clone())),
        }
    }

    // 依赖：游戏版本 + 加载器
    let mut deps = Map::new();
    deps.insert("minecraft".into(), json!(info.mc));
    if let (Some(l), Some(v)) = (info.loader.as_deref(), info.loader_version.as_deref()) {
        if let Some((k, v)) = loader_dep(l, v, &info.mc) {
            deps.insert(k.into(), json!(v));
        }
    } else if info.loader.is_some() {
        warnings.push("读不到加载器的版本号，朋友导入时可能装不上加载器".into());
    }
    if info.optifine.is_some() {
        warnings.push("OptiFine 不能放进整合包（Modrinth 格式不支持），朋友那边需要自己装".into());
    }

    let mut index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": req.pack_version.trim(),
        "name": req.name.trim(),
        "files": index_files,
        "dependencies": deps,
    });
    if let Some(s) = req.summary.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        index["summary"] = json!(s);
    }

    let out = PathBuf::from(&req.out);
    let linked = index["files"].as_array().map_or(0, Vec::len);
    let bundled = bundle.len();
    let out2 = out.clone();
    tauri::async_runtime::spawn_blocking(move || write_zip(&out2, &index, &bundle)).await??;
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    Ok(ExportResult { path: out.to_string_lossy().to_string(), linked, bundled, size, warnings })
}

fn write_zip(out: &Path, index: &Value, bundle: &[(String, PathBuf)]) -> Result<()> {
    if let Some(p) = out.parent() {
        std::fs::create_dir_all(p)?;
    }
    // 先写到临时文件，成功了再改名，失败不会留下半个整合包
    let tmp = out.with_extension("mrpack.part");
    let r = (|| -> Result<()> {
        let mut w = zip::ZipWriter::new(std::fs::File::create(&tmp).context("无法创建整合包文件")?);
        let opt = zip::write::SimpleFileOptions::default().large_file(true);
        w.start_file("modrinth.index.json", opt)?;
        w.write_all(&serde_json::to_vec_pretty(index)?)?;
        for (rel, path) in bundle {
            w.start_file(format!("overrides/{rel}"), opt)?;
            let mut f = std::fs::File::open(path).with_context(|| format!("读取 {rel} 失败"))?;
            std::io::copy(&mut f, &mut w)?;
        }
        w.finish()?;
        Ok(())
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, out).or_else(|_| {
        // 目标已存在时先删掉
        std::fs::remove_file(out)?;
        std::fs::rename(&tmp, out)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linkable_paths() {
        assert!(linkable("mods/sodium.jar"));
        assert!(linkable("mods/old.jar.disabled"));
        assert!(linkable("shaderpacks/bsl.zip"));
        assert!(!linkable("mods/sub/x.jar"));
        assert!(!linkable("config/x.jar"));
        assert!(!linkable("options.txt"));
    }

    #[test]
    fn loader_versions() {
        assert_eq!(loader_dep("Forge", "1.20.1-47.2.0", "1.20.1"), Some(("forge", "47.2.0".into())));
        assert_eq!(loader_dep("Forge", "1.7.10-10.13.4.1614-1.7.10", "1.7.10"), Some(("forge", "10.13.4.1614".into())));
        assert_eq!(loader_dep("Fabric", "0.16.10", "1.21.1"), Some(("fabric-loader", "0.16.10".into())));
        assert_eq!(loader_dep("NeoForge", "21.1.77", "1.21.1"), Some(("neoforge", "21.1.77".into())));
        assert_eq!(loader_dep("原版", "x", "1"), None);
    }

    #[test]
    fn hides_launcher_files() {
        assert!(hidden("logs", "v"));
        assert!(hidden("amc.json", "v"));
        assert!(hidden("v.jar", "v"));
        assert!(hidden("latest.log", "v"));
        assert!(!hidden("mods", "v"));
        assert!(!hidden("saves", "v"));
    }

    /// 造一个装了 Iris + Sodium 的版本，导出后检查整合包内容（要联网，手动跑）
    #[tokio::test]
    #[ignore]
    async fn export_with_mods_live() {
        let root = std::env::temp_dir().join(format!("amc-exp-{}", std::process::id()));
        let vdir = root.join("versions").join("T");
        std::fs::create_dir_all(vdir.join("config")).unwrap();
        std::fs::write(vdir.join("T.json"), r#"{"id":"T"}"#).unwrap();
        std::fs::write(vdir.join("amc.json"), r#"{"mc":"1.20.1","loader":"Fabric","loaderVersion":"0.15.7"}"#).unwrap();
        std::fs::write(vdir.join("config").join("iris.properties"), "shaderPack=\n").unwrap();
        std::fs::write(vdir.join("options.txt"), "fov:0.5\n").unwrap();
        std::fs::create_dir_all(vdir.join("logs")).unwrap();
        std::fs::write(vdir.join("logs").join("latest.log"), "x").unwrap();
        std::fs::create_dir_all(vdir.join("mods")).unwrap();
        std::fs::write(vdir.join("mods").join("my-own-mod.jar"), b"not on modrinth").unwrap();
        modrinth::install(&modrinth::InstallReq {
            game_dir: root.to_string_lossy().into(),
            version: "T".into(),
            kind: "mod".into(),
            project_id: "YL57xq9U".into(),
            mc: "1.20.1".into(),
            loader: Some("Fabric".into()),
            version_id: None,
            reinstall: false,
            with_deps: true,
        })
        .await
        .unwrap();

        let list = entries(&root, "T").unwrap();
        let names: Vec<_> = list.iter().map(|e| (e.name.as_str(), e.default)).collect();
        println!("{names:?}");
        assert!(!names.iter().any(|(n, _)| *n == "logs" || *n == "amc.json" || *n == "T.json"));
        let out = root.join("pack.mrpack");
        let r = export(&ExportReq {
            game_dir: root.to_string_lossy().into(),
            version: "T".into(),
            out: out.to_string_lossy().into(),
            name: "好友包".into(),
            pack_version: "1.0.0".into(),
            summary: None,
            include: list.iter().filter(|e| e.default).map(|e| e.name.clone()).collect(),
        })
        .await
        .unwrap();
        println!("{r:?}");
        assert_eq!(r.linked, 2, "Iris 和 Sodium 应该写成下载地址");
        assert_eq!(r.bundled, 3, "自己的 Mod、配置、options.txt 打包进去");

        let mut zip = crate::game::zipx::open(&out).unwrap();
        let idx = crate::game::zipx::read_json(&mut zip, "modrinth.index.json").unwrap();
        println!("{}", serde_json::to_string_pretty(&idx).unwrap());
        assert_eq!(idx["dependencies"]["fabric-loader"], "0.15.7");
        for f in idx["files"].as_array().unwrap() {
            assert!(f["hashes"]["sha512"].as_str().unwrap().len() == 128);
            assert!(f["downloads"][0].as_str().unwrap().starts_with("https://cdn.modrinth.com/"));
        }
        assert!(crate::game::zipx::has(&zip, "overrides/mods/my-own-mod.jar"));
        assert!(crate::game::zipx::has(&zip, "overrides/config/iris.properties"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// 导出再导入：朋友那边拿到的版本应该有同样的 Mod 和配置（要联网、下载整个游戏，手动跑）
    #[tokio::test]
    #[ignore]
    async fn roundtrip_live() {
        use crate::task::TaskCtx;
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("amc-rt-{}", std::process::id()));
        let vdir = root.join("a").join("versions").join("T");
        std::fs::create_dir_all(vdir.join("config")).unwrap();
        std::fs::write(vdir.join("T.json"), r#"{"id":"T"}"#).unwrap();
        std::fs::write(vdir.join("amc.json"), r#"{"mc":"1.20.1","loader":"Fabric","loaderVersion":"0.15.7"}"#).unwrap();
        std::fs::write(vdir.join("config").join("iris.properties"), "shaderPack=\n").unwrap();
        modrinth::install(&modrinth::InstallReq {
            game_dir: root.join("a").to_string_lossy().into(),
            version: "T".into(),
            kind: "mod".into(),
            project_id: "AANobbMI".into(), // Sodium
            mc: "1.20.1".into(),
            loader: Some("Fabric".into()),
            version_id: None,
            reinstall: false,
            with_deps: true,
        })
        .await
        .unwrap();
        let out = root.join("pack.mrpack");
        export(&ExportReq {
            game_dir: root.join("a").to_string_lossy().into(),
            version: "T".into(),
            out: out.to_string_lossy().into(),
            name: "好友包".into(),
            pack_version: "1.0.0".into(),
            summary: None,
            include: vec!["mods".into(), "config".into()],
        })
        .await
        .unwrap();

        // 朋友那边：全新的游戏目录
        let friend = root.join("b");
        std::fs::create_dir_all(&friend).unwrap();
        let ctx = TaskCtx::new("rt".into(), Arc::new(AtomicBool::new(false)), crate::modpack::STAGES);
        crate::modpack::install(
            &ctx,
            &crate::modpack::ModpackReq {
                task_id: "rt".into(),
                game_dir: friend.to_string_lossy().into(),
                name: "好友包".into(),
                project_id: String::new(),
                version_id: String::new(),
                file: Some(out.to_string_lossy().into()),
                source: crate::net::Source::Bmcl,
                threads: 32,
            },
        )
        .await
        .unwrap();
        let v = friend.join("versions").join("好友包");
        let mods: Vec<_> = std::fs::read_dir(v.join("mods")).unwrap().flatten().map(|e| e.file_name()).collect();
        println!("朋友那边的 Mod：{mods:?}，阶段 {:?}", ctx.current_stage());
        assert!(mods.iter().any(|m| m.to_string_lossy().contains("sodium")));
        assert!(v.join("config").join("iris.properties").is_file());
        let meta: Value = serde_json::from_slice(&std::fs::read(v.join("amc.json")).unwrap()).unwrap();
        assert_eq!(meta["loader"], "Fabric");
        assert_eq!(meta["modpack"]["name"], "好友包");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// 用本机真实的版本导出一次（手动跑：AMC_EXPORT_VERSION=版本名）
    #[tokio::test]
    #[ignore]
    async fn export_live() {
        let game_dir = PathBuf::from(std::env::var("APPDATA").unwrap()).join(".minecraft");
        let version = std::env::var("AMC_EXPORT_VERSION").unwrap_or_else(|_| "26.2-Fabric".into());
        let list = entries(&game_dir, &version).unwrap();
        println!("{list:#?}");
        let out = std::env::temp_dir().join("amc-export-test.mrpack");
        let r = export(&ExportReq {
            game_dir: game_dir.to_string_lossy().into(),
            version: version.clone(),
            out: out.to_string_lossy().into(),
            name: "测试整合包".into(),
            pack_version: "1.0.0".into(),
            summary: Some("导出测试".into()),
            include: list.iter().filter(|e| e.default).map(|e| e.name.clone()).collect(),
        })
        .await
        .unwrap();
        println!("{r:?}");
        let info = crate::modpack::read_pack(&out).unwrap();
        println!("{info:?}");
        assert_eq!(info.name, "测试整合包");
        std::fs::remove_file(&out).unwrap();
    }
}
