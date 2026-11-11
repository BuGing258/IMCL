//! Forge / NeoForge 安装：下载安装器，自己解析 install_profile.json 并运行处理器，
//! 不调用安装器的图形界面，也不会改动官方启动器的 launcher_profiles.json。

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::meta::LoaderKind;
use super::{json, zipx, InstallEnv};
use crate::net::{self, DlItem};
use crate::{maven, task::TaskCtx};

pub fn installer_url(kind: LoaderKind, id: &str) -> String {
    match kind {
        LoaderKind::NeoForge => {
            format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{id}/neoforge-{id}-installer.jar")
        }
        _ => format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{id}/forge-{id}-installer.jar"),
    }
}

/// 返回加载器的版本 JSON（之后与原版合并）
pub async fn install(ctx: &TaskCtx, env: &InstallEnv<'_>, kind: LoaderKind, id: &str) -> Result<Value> {
    let installer = env.tmp.join("installer.jar");
    net::download_one(&DlItem::new(&installer_url(kind, id), env.src, installer.clone(), None, None), ctx)
        .await
        .with_context(|| format!("下载 {} 安装器失败", kind.label()))?;

    let profile = zipx::read_json(&mut zipx::open(&installer)?, "install_profile.json")?;
    if profile.get("versionInfo").is_some() {
        legacy(ctx, env, &installer, &profile).await
    } else {
        modern(ctx, env, &installer, &profile).await
    }
}

/// 1.12.2 及更早的 Forge：install_profile 里直接带着版本 JSON 和 universal jar
async fn legacy(ctx: &TaskCtx, env: &InstallEnv<'_>, installer: &Path, profile: &Value) -> Result<Value> {
    let child = profile["versionInfo"].clone();
    let coord = profile.pointer("/install/path").and_then(Value::as_str).ok_or_else(|| anyhow!("安装器缺少 install.path"))?;
    let file_path = profile.pointer("/install/filePath").and_then(Value::as_str).ok_or_else(|| anyhow!("安装器缺少 install.filePath"))?;
    let dest = env.libdir.join(maven::path(coord).ok_or_else(|| anyhow!("无效坐标 {coord}"))?);
    zipx::extract_to(&mut zipx::open(installer)?, file_path, &dest)?;

    let forge_key = maven::dedupe_key(coord);
    let items: Vec<DlItem> = child["libraries"]
        .as_array()
        .map(|libs| {
            libs.iter()
                .filter(|l| l["name"].as_str().map(maven::dedupe_key).as_deref() != Some(forge_key.as_str()))
                .flat_map(|l| json::library_items(l, env.libdir, env.src))
                .collect()
        })
        .unwrap_or_default();
    net::download_all(items, env.threads, ctx).await?;
    Ok(child)
}

/// 1.13+ 的 Forge 和所有 NeoForge：需要运行处理器生成补丁后的游戏文件
async fn modern(ctx: &TaskCtx, env: &InstallEnv<'_>, installer: &Path, profile: &Value) -> Result<Value> {
    let child = {
        let mut zip = zipx::open(installer)?;
        let json_name = profile["json"].as_str().unwrap_or("/version.json").trim_start_matches('/');
        let child = zipx::read_json(&mut zip, json_name)?;
        zipx::extract_prefix(&mut zip, "maven/", env.libdir)?;
        child
    };

    // 安装器和游戏运行都需要的依赖库
    let mut seen = HashSet::new();
    let items: Vec<DlItem> = json::libraries_items(profile, env.libdir, env.src)
        .into_iter()
        .chain(json::libraries_items(&child, env.libdir, env.src))
        .filter(|i| seen.insert(i.path.clone()))
        .collect();
    net::download_all(items, env.threads, ctx).await?;

    run_processors(ctx, env, installer, profile).await?;
    Ok(child)
}

fn lib_path(libdir: &Path, coord: &str) -> Result<PathBuf> {
    Ok(libdir.join(maven::path(coord).ok_or_else(|| anyhow!("无效坐标 {coord}"))?))
}

async fn run_processors(ctx: &TaskCtx, env: &InstallEnv<'_>, installer: &Path, profile: &Value) -> Result<()> {
    let java = env.java.ok_or_else(|| anyhow!("运行 Forge 安装处理器需要 Java"))?;
    let data_dir = env.tmp.join("data");

    // 1. 准备变量表
    let mut data: HashMap<String, String> = HashMap::new();
    {
        let mut zip = zipx::open(installer)?;
        if let Some(obj) = profile["data"].as_object() {
            for (k, v) in obj {
                let Some(raw) = v["client"].as_str() else { continue };
                let val = if let Some(inner) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                    lib_path(env.libdir, inner)?.to_string_lossy().to_string()
                } else if let Some(inner) = raw.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
                    inner.to_string()
                } else if let Some(rel) = raw.strip_prefix('/') {
                    let dest = data_dir.join(rel);
                    zipx::extract_to(&mut zip, rel, &dest)?;
                    dest.to_string_lossy().to_string()
                } else {
                    raw.to_string()
                };
                data.insert(k.clone(), val);
            }
        }
    }
    let s = |p: &Path| p.to_string_lossy().to_string();
    data.insert("SIDE".into(), "client".into());
    data.insert("MINECRAFT_JAR".into(), s(env.client_jar));
    data.insert("MINECRAFT_VERSION".into(), env.mc.to_string());
    data.insert("ROOT".into(), s(env.game_dir));
    data.insert("INSTALLER".into(), s(installer));
    data.insert("LIBRARY_DIR".into(), s(env.libdir));

    let resolve = |arg: &str| -> Result<String> {
        if let Some(inner) = arg.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            return Ok(lib_path(env.libdir, inner)?.to_string_lossy().to_string());
        }
        if let Some(key) = arg.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
            return data.get(key).cloned().ok_or_else(|| anyhow!("安装器变量 {key} 未定义"));
        }
        let mut out = arg.to_string();
        for (k, v) in &data {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        Ok(out)
    };

    // 2. 依次运行处理器
    let procs: Vec<&Value> = profile["processors"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|p| {
                    p["sides"].as_array().map_or(true, |s| s.iter().any(|x| x.as_str() == Some("client")))
                })
                .collect()
        })
        .unwrap_or_default();
    ctx.done.store(0, Ordering::Relaxed);
    ctx.total.store(procs.len() as u64, Ordering::Relaxed);

    for p in procs {
        ctx.check_cancel()?;
        let jar_coord = p["jar"].as_str().ok_or_else(|| anyhow!("处理器缺少 jar"))?;
        let jar = lib_path(env.libdir, jar_coord)?;
        let main = zipx::main_class(&jar)?;
        let mut cp = vec![jar.clone()];
        if let Some(c) = p["classpath"].as_array() {
            for coord in c.iter().filter_map(Value::as_str) {
                cp.push(lib_path(env.libdir, coord)?);
            }
        }
        let args: Vec<String> = p["args"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).map(resolve).collect::<Result<_>>())
            .transpose()?
            .unwrap_or_default();

        // 输出文件都已存在且校验通过就跳过（重装或中断后继续时省时间）
        let mut outputs: Vec<(PathBuf, String)> = Vec::new();
        if let Some(o) = p["outputs"].as_object() {
            for (k, v) in o {
                let sha = resolve(v.as_str().unwrap_or_default())?;
                outputs.push((PathBuf::from(resolve(k)?), sha.trim_matches('\'').to_lowercase()));
            }
        }
        let outputs_ok = |outs: &[(PathBuf, String)]| {
            outs.iter().all(|(path, sha)| path.is_file() && net::sha1_file(path).map(|h| &h == sha).unwrap_or(false))
        };
        if !outputs.is_empty() && outputs_ok(&outputs) {
            ctx.done.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        let classpath = std::env::join_paths(&cp)?;
        let mut cmd = tokio::process::Command::new(&java.path);
        cmd.arg("-cp").arg(classpath).arg(&main).args(&args).current_dir(env.tmp).kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW：不弹黑色命令行窗口
        let out = cmd.output().await.with_context(|| format!("无法运行处理器 {main}"))?;
        if !out.status.success() {
            let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).rev().take(12).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            bail!("处理器 {main} 运行失败：\n{}", tail.join("\n"));
        }
        if !outputs.is_empty() && !outputs_ok(&outputs) {
            bail!("处理器 {main} 生成的文件校验失败");
        }
        ctx.done.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}
