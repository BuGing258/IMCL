//! Modrinth 整合包（.mrpack）安装：先装好对应的游戏版本和加载器，再下载整合包里的文件，最后解压覆盖文件。

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::game::{self, installed, meta, zipx, InstallRequest, LoaderReq};
use crate::modrinth;
use crate::net::{self, DlItem, Source};
use crate::task::TaskCtx;
use meta::LoaderKind;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackReq {
    pub task_id: String,
    pub game_dir: String,
    /// 新版本的名字
    pub name: String,
    pub project_id: String,
    pub version_id: String,
    pub source: Source,
    pub threads: usize,
}

/// 下载整合包 + 游戏 4 步 + 加载器 1 步 + 整合包文件 + 覆盖文件
pub const STAGES: u32 = 8;

/// modrinth.index.json 里需要的部分
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    dependencies: std::collections::HashMap<String, String>,
    #[serde(default)]
    files: Vec<IndexFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    #[serde(default)]
    hashes: std::collections::HashMap<String, String>,
    #[serde(default)]
    env: Option<Env>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Env {
    #[serde(default)]
    client: Option<String>,
}

/// 整合包用的游戏版本和加载器
async fn loader_of(idx: &Index) -> Result<(String, Option<LoaderReq>)> {
    let mc = idx.dependencies.get("minecraft").cloned().ok_or_else(|| anyhow!("整合包没写游戏版本"))?;
    let d = |k: &str| idx.dependencies.get(k).cloned();
    let loader = if let Some(v) = d("fabric-loader") {
        Some(LoaderReq { kind: LoaderKind::Fabric, id: v })
    } else if let Some(v) = d("quilt-loader") {
        Some(LoaderReq { kind: LoaderKind::Quilt, id: v })
    } else if let Some(v) = d("neoforge") {
        Some(LoaderReq { kind: LoaderKind::NeoForge, id: v })
    } else if let Some(v) = d("forge") {
        // 整合包里写的是 47.2.0，安装器要的是 1.20.1-47.2.0（老版本后面还带着游戏版本）
        let full = meta::loader_versions(LoaderKind::Forge, &mc, Source::Official)
            .await
            .ok()
            .and_then(|list| list.into_iter().find(|x| x.version == v || x.id == v))
            .map(|x| x.id)
            .unwrap_or_else(|| format!("{mc}-{v}"));
        Some(LoaderReq { kind: LoaderKind::Forge, id: full })
    } else {
        None
    };
    Ok((mc, loader))
}

/// 把 cdn.modrinth.com 的地址加上国内镜像备用
fn with_mirror(urls: &[String]) -> Vec<String> {
    let mut out: Vec<String> = urls.to_vec();
    for u in urls {
        if let Some(m) = modrinth::mirror_cdn(u) {
            out.push(m);
        }
    }
    out
}

/// 解压 overrides / client-overrides 到版本的运行目录
fn extract_overrides(pack: &Path, run_dir: &Path) -> Result<usize> {
    let mut zip = zipx::open(pack)?;
    let mut n = 0;
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry.name()?.replace('\\', "/");
            let Some(rel) = name.strip_prefix(prefix) else { continue };
            if rel.is_empty() || entry.is_dir() {
                continue;
            }
            let out = zipx::safe_join(run_dir, rel)?;
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut f = std::fs::File::create(&out).with_context(|| format!("写入 {rel} 失败"))?;
            std::io::copy(&mut entry, &mut f)?;
            n += 1;
        }
    }
    Ok(n)
}

pub async fn install(ctx: &TaskCtx, req: &ModpackReq) -> Result<()> {
    let game_dir = PathBuf::from(&req.game_dir);
    game::validate_name(&game_dir, &req.name)?;
    let tmp = game_dir.join(".amc-tmp").join(&req.task_id);
    std::fs::create_dir_all(&tmp)?;
    let r = run(ctx, req, &game_dir, &tmp).await;
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

async fn run(ctx: &TaskCtx, req: &ModpackReq, game_dir: &Path, tmp: &Path) -> Result<()> {
    // 1. 整合包文件
    ctx.stage("下载整合包");
    let v = modrinth::version(&req.version_id).await?;
    if v["project_id"].as_str() != Some(req.project_id.as_str()) {
        bail!("选的版本不属于这个整合包");
    }
    let f = modrinth::primary_file(&v).ok_or_else(|| anyhow!("这个版本没有文件"))?;
    let url = f["url"].as_str().ok_or_else(|| anyhow!("没有下载地址"))?.to_string();
    let pack = tmp.join("pack.mrpack");
    let item = DlItem {
        urls: with_mirror(&[url]),
        path: pack.clone(),
        sha1: f.pointer("/hashes/sha1").and_then(Value::as_str).map(str::to_lowercase),
        size: f["size"].as_u64(),
        verify_existing: true,
    };
    net::download_one(&item, ctx).await.context("下载整合包失败")?;
    let idx: Index = {
        let mut zip = zipx::open(&pack)?;
        serde_json::from_value(zipx::read_json(&mut zip, "modrinth.index.json")?).context("整合包格式不对（modrinth.index.json）")?
    };
    let (mc, loader) = loader_of(&idx).await?;
    if loader.is_none() {
        ctx.set_stage_count(STAGES - 1);
    }

    // 2. 游戏和加载器（失败时它会自己删掉版本文件夹）
    let game_req = InstallRequest {
        task_id: format!("{}-game", req.task_id),
        game_dir: req.game_dir.clone(),
        name: req.name.clone(),
        mc: mc.clone(),
        loader,
        optifine: None,
        source: req.source,
        threads: req.threads,
    };
    game::install(ctx, &game_req).await?;

    // 3、4 失败就把装了一半的版本删掉
    let version_dir = game_dir.join("versions").join(&req.name);
    let r = files_and_overrides(ctx, req, &idx, &pack, game_dir, &version_dir, &v).await;
    if r.is_err() {
        let _ = std::fs::remove_dir_all(&version_dir);
    }
    r
}

async fn files_and_overrides(
    ctx: &TaskCtx,
    req: &ModpackReq,
    idx: &Index,
    pack: &Path,
    game_dir: &Path,
    version_dir: &Path,
    v: &Value,
) -> Result<()> {
    let run_dir = installed::run_dir(game_dir, &req.name)?;

    // 3. 整合包里列出的 Mod、配置等
    ctx.stage("下载整合包里的 Mod");
    let mut items = Vec::new();
    for f in &idx.files {
        if f.env.as_ref().and_then(|e| e.client.as_deref()) == Some("unsupported") {
            continue; // 只给服务器用的
        }
        if f.downloads.is_empty() {
            continue;
        }
        let path = zipx::safe_join(&run_dir, &f.path)?;
        items.push(DlItem {
            urls: with_mirror(&f.downloads),
            path,
            sha1: f.hashes.get("sha1").map(|s| s.to_lowercase()),
            size: f.file_size,
            verify_existing: true,
        });
    }
    net::download_all(items, req.threads, ctx).await?;

    // 4. 覆盖文件（配置、资源包等）
    ctx.stage("解压整合包配置");
    ctx.check_cancel()?;
    let pack = pack.to_path_buf();
    let rd = run_dir.clone();
    tauri::async_runtime::spawn_blocking(move || extract_overrides(&pack, &rd)).await??;

    // 记下来源，以后可以检查整合包更新
    let meta_path = version_dir.join("amc.json");
    let mut meta: Value = std::fs::read(&meta_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    meta["modpack"] = json!({
        "projectId": req.project_id,
        "versionId": req.version_id,
        "versionNumber": v["version_number"],
    });
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loader_detection() {
        let idx = |deps: &[(&str, &str)]| Index {
            dependencies: deps.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            files: vec![],
        };
        let (mc, l) = loader_of(&idx(&[("minecraft", "1.20.1"), ("fabric-loader", "0.15.7")])).await.unwrap();
        assert_eq!(mc, "1.20.1");
        let l = l.unwrap();
        assert_eq!((l.kind, l.id.as_str()), (LoaderKind::Fabric, "0.15.7"));
        let (_, l) = loader_of(&idx(&[("minecraft", "1.21.1"), ("neoforge", "21.1.77")])).await.unwrap();
        assert_eq!(l.unwrap().kind, LoaderKind::NeoForge);
        let (_, l) = loader_of(&idx(&[("minecraft", "1.20.1")])).await.unwrap();
        assert!(l.is_none());
        assert!(loader_of(&idx(&[("fabric-loader", "1")])).await.is_err());
    }

    #[test]
    fn overrides_stay_inside() {
        let dir = std::env::temp_dir().join(format!("amc-ov-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pack = dir.join("p.mrpack");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&pack).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("overrides/config/a.txt", o).unwrap();
            std::io::Write::write_all(&mut w, b"a").unwrap();
            w.start_file("client-overrides/options.txt", o).unwrap();
            std::io::Write::write_all(&mut w, b"b").unwrap();
            w.start_file("modrinth.index.json", o).unwrap();
            w.finish().unwrap();
        }
        let out = dir.join("run");
        assert_eq!(extract_overrides(&pack, &out).unwrap(), 2);
        assert!(out.join("config/a.txt").is_file() && out.join("options.txt").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 真的装一个小整合包（要联网，几百 MB，手动跑）
    #[tokio::test]
    #[ignore]
    async fn modpack_live() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("amc-mp-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        // Fabulously Optimized：Fabric 的小整合包
        let versions = modrinth::versions(&modrinth::VersionsReq {
            project_id: "1KVo5zza".into(),
            kind: "modpack".into(),
            mc: "1.20.1".into(),
            loader: None,
        })
        .await
        .unwrap();
        let v = versions.first().expect("没有 1.20.1 的版本");
        println!("装 {} ({:?} {:?})", v.version_number, v.game_versions, v.loaders);
        let ctx = TaskCtx::new("mp".into(), Arc::new(AtomicBool::new(false)), STAGES);
        install(
            &ctx,
            &ModpackReq {
                task_id: "mp".into(),
                game_dir: root.to_string_lossy().into(),
                name: "FO".into(),
                project_id: "1KVo5zza".into(),
                version_id: v.id.clone(),
                source: Source::Bmcl,
                threads: 32,
            },
        )
        .await
        .unwrap();
        let mods = std::fs::read_dir(root.join("versions/FO/mods")).unwrap().count();
        println!("mods: {mods}, stage {:?}", ctx.current_stage());
        assert!(mods > 5);
        assert!(root.join("versions/FO/config").is_dir());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
