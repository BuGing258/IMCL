//! 下载 Java：用 Mojang 官方启动器同款的 Java 运行时（按游戏版本 JSON 里的 javaVersion.component），
//! 下载前同时测一下官方源和 BMCLAPI 镜像的速度，用快的那个，另一个当备用。

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::java::{self, JavaInfo};
use crate::net::{self, DlItem};
use crate::task::TaskCtx;

const ALL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[cfg(all(windows, target_arch = "x86_64"))]
const PLATFORM: &str = "windows-x64";
#[cfg(all(windows, target_arch = "x86"))]
const PLATFORM: &str = "windows-x86";
#[cfg(all(windows, target_arch = "aarch64"))]
const PLATFORM: &str = "windows-arm64";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const PLATFORM: &str = "mac-os-arm64";
#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
const PLATFORM: &str = "mac-os";
#[cfg(target_os = "linux")]
const PLATFORM: &str = "linux";

/// 启动器自己下载的 Java 放在哪（应用数据目录/java/<组件>）
static ROOT: OnceLock<PathBuf> = OnceLock::new();

pub fn set_root(dir: PathBuf) {
    let _ = ROOT.set(dir);
}

pub fn root() -> Option<&'static Path> {
    ROOT.get().map(PathBuf::as_path)
}

/// 主版本号对应的组件
pub fn component_for(major: u32) -> &'static str {
    match major {
        0..=8 => "jre-legacy",
        9..=16 => "java-runtime-alpha",
        17..=20 => "java-runtime-gamma",
        21..=24 => "java-runtime-delta",
        _ => "java-runtime-epsilon",
    }
}

/// 可以下载的 Java（给设置页列出来）
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub component: String,
    pub major: u32,
    pub version: String,
    /// 适合哪些游戏版本
    pub for_games: String,
}

/// 设置页列出的几个（同一个主版本只列一个）
const OFFERS: &[(&str, u32, &str)] = &[
    ("java-runtime-epsilon", 25, "26.1 及以后"),
    ("java-runtime-delta", 21, "1.20.5 ～ 1.21.x"),
    ("java-runtime-gamma", 17, "1.18 ～ 1.20.4"),
    ("jre-legacy", 8, "1.16.5 及以前"),
];

/* ---------------- 测速 ---------------- */

/// 测速结果：毫秒，连不上为 None
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    pub official: Option<u32>,
    pub mirror: Option<u32>,
    pub use_mirror: bool,
}

async fn latency(url: &str) -> Option<u32> {
    let mut best: Option<u32> = None;
    // 测两次取快的，第一次可能包含建立连接的时间
    for _ in 0..2 {
        let t = Instant::now();
        let ok = net::client()
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            let ms = t.elapsed().as_millis().min(u32::MAX as u128) as u32;
            best = Some(best.map_or(ms, |b| b.min(ms)));
        }
    }
    best
}

/// 同时测官方源和镜像，挑快的
pub async fn probe() -> Speed {
    let mirror_url = net::mirror(ALL).unwrap_or_else(|| ALL.to_string());
    let (official, mirror) = futures_util::join!(latency(ALL), latency(&mirror_url));
    let use_mirror = match (official, mirror) {
        (Some(o), Some(m)) => m < o,
        (None, Some(_)) => true,
        _ => false,
    };
    Speed { official, mirror, use_mirror }
}

/// 按测速结果排好候选地址：快的在前，另一个备用
fn ordered(url: &str, use_mirror: bool) -> Vec<String> {
    let official = url.replacen("http://", "https://", 1);
    match net::mirror(url) {
        Some(m) if use_mirror => vec![m, official],
        Some(m) => vec![official, m],
        None => vec![official],
    }
}

async fn get_json(url: &str, use_mirror: bool) -> Result<Value> {
    let mut last = anyhow!("没有可用的地址");
    for u in ordered(url, use_mirror) {
        match net::client().get(&u).timeout(Duration::from_secs(30)).send().await {
            Ok(r) if r.status().is_success() => match r.json::<Value>().await {
                Ok(v) => return Ok(v),
                Err(e) => last = e.into(),
            },
            Ok(r) => last = anyhow!("{u} 返回 {}", r.status()),
            Err(e) => last = e.into(),
        }
    }
    Err(last)
}

/* ---------------- 已下载的 ---------------- */

fn dir_of(component: &str) -> Result<PathBuf> {
    Ok(root().ok_or_else(|| anyhow!("还没初始化 Java 目录"))?.join(component))
}

/// 下载完成的标记：里面写着版本号
const MARK: &str = ".amc-complete";

/// 已经完整下载过的组件
pub fn installed(component: &str) -> Option<JavaInfo> {
    let dir = dir_of(component).ok()?;
    if !dir.join(MARK).is_file() {
        return None;
    }
    java::probe(&dir)
}

/// 设置页用：可下载的版本 + 是否已经下载
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferState {
    #[serde(flatten)]
    pub offer: Offer,
    pub installed: Option<JavaInfo>,
}

pub fn offers() -> Vec<OfferState> {
    OFFERS
        .iter()
        .map(|(c, major, games)| OfferState {
            offer: Offer { component: c.to_string(), major: *major, version: String::new(), for_games: games.to_string() },
            installed: installed(c),
        })
        .collect()
}

/* ---------------- 下载 ---------------- */

/// 下载一个 Java 组件（已经下好就直接返回）。会占用 2 个阶段：测速、下载
pub async fn install(ctx: &TaskCtx, component: &str, threads: usize) -> Result<JavaInfo> {
    if let Some(j) = installed(component) {
        return Ok(j);
    }
    let dir = dir_of(component)?;
    let major_hint = OFFERS.iter().find(|o| o.0 == component).map(|o| o.1);
    let label = major_hint.map_or_else(|| "Java".to_string(), |m| format!("Java {m}"));

    ctx.stage("测试下载源速度");
    let speed = probe().await;
    if speed.official.is_none() && speed.mirror.is_none() {
        bail!("官方源和镜像都连不上，检查一下网络");
    }
    let src_name = if speed.use_mirror { "BMCLAPI 镜像" } else { "官方源" };
    ctx.stage(&format!("下载 {label}（{src_name}更快）"));

    let all = get_json(ALL, speed.use_mirror).await.context("获取 Java 列表失败")?;
    let entry = all
        .pointer(&format!("/{PLATFORM}/{component}/0"))
        .ok_or_else(|| anyhow!("官方没有提供这个系统能用的 {label}"))?;
    let version = entry.pointer("/version/name").and_then(Value::as_str).unwrap_or_default().to_string();
    let manifest_url = entry.pointer("/manifest/url").and_then(Value::as_str).ok_or_else(|| anyhow!("Java 清单地址缺失"))?;
    let manifest = get_json(manifest_url, speed.use_mirror).await.context("获取 Java 文件清单失败")?;
    let files = manifest["files"].as_object().ok_or_else(|| anyhow!("Java 文件清单格式不对"))?;

    std::fs::create_dir_all(&dir).context("无法创建 Java 文件夹")?;
    let _ = std::fs::remove_file(dir.join(MARK));
    let mut items = Vec::new();
    for (rel, f) in files {
        let path = crate::game::zipx::safe_join(&dir, rel)?;
        match f["type"].as_str() {
            Some("directory") => std::fs::create_dir_all(&path)?,
            Some("file") => {
                let raw = &f["downloads"]["raw"];
                let url = raw["url"].as_str().ok_or_else(|| anyhow!("{rel} 没有下载地址"))?;
                items.push(DlItem {
                    urls: ordered(url, speed.use_mirror),
                    path,
                    sha1: raw["sha1"].as_str().map(str::to_lowercase),
                    size: raw["size"].as_u64(),
                    verify_existing: true,
                });
            }
            _ => {} // 链接文件只在 Linux / macOS 上有
        }
    }
    net::download_all(items, threads, ctx).await?;

    #[cfg(unix)]
    for (rel, f) in files {
        if f["executable"].as_bool() == Some(true) {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir.join(rel), std::fs::Permissions::from_mode(0o755));
        }
    }

    let info = java::probe(&dir).ok_or_else(|| anyhow!("{label} 下载好了但运行不了"))?;
    std::fs::write(dir.join(MARK), &version)?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components() {
        assert_eq!(component_for(8), "jre-legacy");
        assert_eq!(component_for(16), "java-runtime-alpha");
        assert_eq!(component_for(17), "java-runtime-gamma");
        assert_eq!(component_for(21), "java-runtime-delta");
        assert_eq!(component_for(25), "java-runtime-epsilon");
        let u = "https://piston-data.mojang.com/v1/objects/abc";
        assert!(ordered(u, true)[0].contains("bmclapi"));
        assert!(ordered(u, false)[0].contains("mojang"));
        assert_eq!(ordered(u, false).len(), 2);
    }

    /// 真的下载 Java 17（约 90 MB，手动跑）
    #[tokio::test]
    #[ignore]
    async fn download_live() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        let dir = std::env::temp_dir().join(format!("amc-jre-{}", std::process::id()));
        set_root(dir.clone());
        let s = probe().await;
        println!("测速：{s:?}");
        let ctx = TaskCtx::new("jre".into(), Arc::new(AtomicBool::new(false)), 2);
        let t = Instant::now();
        let j = install(&ctx, "java-runtime-gamma", 64).await.unwrap();
        println!("{j:?} 用时 {:?} 阶段 {:?}", t.elapsed(), ctx.current_stage());
        assert_eq!(j.major, 17);
        assert!(installed("java-runtime-gamma").is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
