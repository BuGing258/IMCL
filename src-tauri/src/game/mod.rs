//! 游戏版本安装：原版 + 可选的加载器 + 可选的 OptiFine，最后合并成一个独立的版本。

pub mod installed;
pub mod json;
pub mod meta;

mod forge;
mod optifine;
mod zipx;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::java::{self, JavaInfo};
use crate::net::{self, DlItem, Source};
use crate::task::TaskCtx;
use meta::LoaderKind;
pub use optifine::OptiFineReq;

#[derive(Clone, Debug, Deserialize)]
pub struct LoaderReq {
    pub kind: LoaderKind,
    /// 完整版本号（来自 loader_versions 的 id）
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub task_id: String,
    pub game_dir: String,
    pub name: String,
    pub mc: String,
    pub loader: Option<LoaderReq>,
    pub optifine: Option<OptiFineReq>,
    pub source: Source,
    pub threads: usize,
}

/// 安装过程中各步骤共用的环境
pub struct InstallEnv<'a> {
    pub mc: &'a str,
    pub game_dir: &'a Path,
    pub libdir: &'a Path,
    pub version_dir: &'a Path,
    pub client_jar: &'a Path,
    pub tmp: &'a Path,
    pub src: Source,
    pub threads: usize,
    pub java: Option<&'a JavaInfo>,
}

/// 版本名称检查：能当文件夹名、不和已有版本重复
pub fn validate_name(game_dir: &Path, name: &str) -> Result<()> {
    let n = name.trim();
    if n.is_empty() {
        bail!("版本名称不能为空");
    }
    if n != name {
        bail!("版本名称前后不能有空格");
    }
    if n.chars().count() > 64 {
        bail!("版本名称太长了（最多 64 个字）");
    }
    if n.chars().any(|c| "\\/:*?\"<>|".contains(c) || c.is_control()) {
        bail!("版本名称不能包含 \\ / : * ? \" < > | 这些字符");
    }
    if n == "." || n == ".." || n.ends_with('.') {
        bail!("版本名称不能以 . 结尾");
    }
    if game_dir.join("versions").join(n).exists() {
        bail!("已经有名为「{n}」的版本了");
    }
    Ok(())
}

pub fn needs_java(req: &InstallRequest) -> bool {
    req.optifine.is_some() || matches!(req.loader.as_ref().map(|l| l.kind), Some(LoaderKind::Forge | LoaderKind::NeoForge))
}

pub async fn install(ctx: &TaskCtx, req: &InstallRequest) -> Result<()> {
    let game_dir = PathBuf::from(&req.game_dir);
    validate_name(&game_dir, &req.name)?;
    if let (Some(l), Some(_)) = (&req.loader, &req.optifine) {
        if l.kind != LoaderKind::Forge {
            bail!("OptiFine 只能和原版或 Forge 一起安装，{} 请改用 Sodium + Iris", l.kind.label());
        }
    }
    let javas = if needs_java(req) { java::find_all() } else { Vec::new() };
    if needs_java(req) && javas.is_empty() {
        bail!("安装 Forge、NeoForge 或 OptiFine 需要 Java，但电脑上没找到。请先安装 Java 17 或 21。");
    }

    let version_dir = game_dir.join("versions").join(&req.name);
    let tmp = game_dir.join(".amc-tmp").join(&req.task_id);
    std::fs::create_dir_all(&tmp)?;
    std::fs::create_dir_all(&version_dir).context("无法创建版本文件夹")?;

    let result = run(ctx, req, &game_dir, &version_dir, &tmp, &javas).await;

    let _ = std::fs::remove_dir_all(&tmp);
    if result.is_err() {
        // 失败或取消：删掉这次新建的版本文件夹（共享的 libraries / assets 保留，下次可复用）
        let _ = std::fs::remove_dir_all(&version_dir);
    }
    result
}

async fn run(
    ctx: &TaskCtx,
    req: &InstallRequest,
    game_dir: &Path,
    version_dir: &Path,
    tmp: &Path,
    javas: &[JavaInfo],
) -> Result<()> {
    let src = req.source;
    let libdir = game_dir.join("libraries");
    let assets = game_dir.join("assets");
    let client_jar = version_dir.join(format!("{}.jar", req.name));

    // 1. 原版版本信息
    ctx.stage("获取版本信息");
    let entry = meta::find_version(&req.mc, src).await?;
    let vanilla: Value = net::get_json(&entry.url, src).await.context("获取版本 JSON 失败")?;
    let required_java = vanilla.pointer("/javaVersion/majorVersion").and_then(Value::as_u64).map(|v| v as u32);
    let java = java::pick(javas, required_java);

    let env = InstallEnv {
        mc: &req.mc,
        game_dir,
        libdir: &libdir,
        version_dir,
        client_jar: &client_jar,
        tmp,
        src,
        threads: req.threads,
        java,
    };

    // 2. 游戏本体 + 依赖库 + 资源索引
    ctx.stage("下载游戏本体和依赖库");
    let mut items = Vec::new();
    let client = vanilla.pointer("/downloads/client").ok_or_else(|| anyhow!("版本 JSON 里没有客户端下载地址"))?;
    items.push(DlItem::new(
        client["url"].as_str().unwrap_or_default(),
        src,
        client_jar.clone(),
        client["sha1"].as_str(),
        client["size"].as_u64(),
    ));
    items.extend(json::libraries_items(&vanilla, &libdir, src));
    if let Some(f) = vanilla.pointer("/logging/client/file") {
        if let (Some(id), Some(url)) = (f["id"].as_str(), f["url"].as_str()) {
            items.push(DlItem::new(url, src, assets.join("log_configs").join(id), f["sha1"].as_str(), f["size"].as_u64()));
        }
    }
    let ai = vanilla.get("assetIndex").ok_or_else(|| anyhow!("版本 JSON 里没有资源索引"))?;
    let index_id = ai["id"].as_str().unwrap_or("legacy");
    let index_path = assets.join("indexes").join(format!("{index_id}.json"));
    items.push(DlItem::new(ai["url"].as_str().unwrap_or_default(), src, index_path.clone(), ai["sha1"].as_str(), ai["size"].as_u64()));
    net::download_all(items, req.threads, ctx).await?;

    // 3. 资源文件（声音、语言等，数量最多）
    ctx.stage("下载资源文件");
    let index: Value = serde_json::from_slice(&std::fs::read(&index_path)?).context("资源索引损坏")?;
    let mut seen = HashSet::new();
    let objects: Vec<DlItem> = index["objects"]
        .as_object()
        .map(|objs| {
            objs.values()
                .filter_map(|o| {
                    let hash = o["hash"].as_str()?;
                    if hash.len() < 2 || !seen.insert(hash.to_string()) {
                        return None;
                    }
                    let rel = format!("{}/{hash}", &hash[..2]);
                    let mut item = DlItem::new(
                        &format!("https://resources.download.minecraft.net/{rel}"),
                        src,
                        assets.join("objects").join(&rel),
                        Some(hash),
                        o["size"].as_u64(),
                    );
                    item.verify_existing = false;
                    Some(item)
                })
                .collect()
        })
        .unwrap_or_default();
    net::download_all(objects, req.threads, ctx).await?;

    // 4. 加载器
    let mut final_json = match &req.loader {
        None => {
            let mut v = vanilla.clone();
            v["id"] = json!(req.name);
            v
        }
        Some(l) => {
            ctx.stage(&format!("安装 {}", l.kind.label()));
            let child = match l.kind {
                LoaderKind::Fabric => {
                    fabric_like(ctx, &env, &format!("https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json", req.mc, l.id)).await?
                }
                LoaderKind::Quilt => {
                    fabric_like(ctx, &env, &format!("https://meta.quiltmc.org/v3/versions/loader/{}/{}/profile/json", req.mc, l.id)).await?
                }
                LoaderKind::Forge | LoaderKind::NeoForge => forge::install(ctx, &env, l.kind, &l.id).await?,
            };
            json::merge(&vanilla, &child, &req.name)
        }
    };

    // 5. OptiFine
    if let Some(of) = &req.optifine {
        ctx.stage("安装 OptiFine");
        let with_forge = req.loader.as_ref().is_some_and(|l| l.kind == LoaderKind::Forge);
        optifine::install(ctx, &env, of, with_forge, &mut final_json).await?;
    }

    // 6. 写入版本文件
    ctx.stage("写入版本配置");
    ctx.check_cancel()?;
    final_json["clientVersion"] = json!(req.mc);
    std::fs::write(
        version_dir.join(format!("{}.json", req.name)),
        serde_json::to_vec_pretty(&final_json)?,
    )?;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let meta = json!({
        "mc": req.mc,
        "loader": req.loader.as_ref().map(|l| l.kind.label()),
        "loaderVersion": req.loader.as_ref().map(|l| l.id.clone()),
        "optifine": req.optifine.as_ref().map(OptiFineReq::label),
        "createdAt": created,
    });
    std::fs::write(version_dir.join("amc.json"), serde_json::to_vec_pretty(&meta)?)?;
    Ok(())
}

async fn fabric_like(ctx: &TaskCtx, env: &InstallEnv<'_>, profile_url: &str) -> Result<Value> {
    let profile: Value = net::get_json(profile_url, env.src).await.context("获取加载器配置失败")?;
    net::download_all(json::libraries_items(&profile, env.libdir, env.src), env.threads, ctx).await?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let dir = std::env::temp_dir().join("amc-name-test-nonexistent");
        assert!(validate_name(&dir, "1.20.1-Fabric").is_ok());
        assert!(validate_name(&dir, "好友生存服").is_ok());
        assert!(validate_name(&dir, "").is_err());
        assert!(validate_name(&dir, " a").is_err());
        assert!(validate_name(&dir, "a/b").is_err());
        assert!(validate_name(&dir, "a:b").is_err());
        assert!(validate_name(&dir, "..").is_err());
    }
}
