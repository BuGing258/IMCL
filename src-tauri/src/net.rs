//! 网络层：HTTP 客户端、BMCLAPI 镜像改写、带校验的并发下载。

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::{de::DeserializeOwned, Deserialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use crate::task::TaskCtx;

const BMCL: &str = "https://bmclapi2.bangbang93.com";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// BMCLAPI 镜像优先，失败再回退官方源
    Bmcl,
    /// 只用官方源
    Official,
}

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("AMCLauncher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .build()
            .expect("无法创建 HTTP 客户端")
    })
}

/// 把官方地址改写成 BMCLAPI 镜像地址；没有对应镜像时返回 None
pub fn mirror(url: &str) -> Option<String> {
    let url = url.strip_prefix("http://").map(|r| format!("https://{r}")).unwrap_or_else(|| url.to_string());
    const MAP: &[(&str, &str)] = &[
        ("https://piston-meta.mojang.com", ""),
        ("https://launchermeta.mojang.com", ""),
        ("https://piston-data.mojang.com", ""),
        ("https://launcher.mojang.com", ""),
        ("https://resources.download.minecraft.net", "/assets"),
        ("https://libraries.minecraft.net", "/maven"),
        ("https://maven.minecraftforge.net", "/maven"),
        ("https://files.minecraftforge.net/maven", "/maven"),
        ("https://maven.fabricmc.net", "/maven"),
        ("https://meta.fabricmc.net", "/fabric-meta"),
        ("https://maven.neoforged.net/releases", "/maven"),
    ];
    MAP.iter()
        .find_map(|(from, to)| url.strip_prefix(from).map(|rest| format!("{BMCL}{to}{rest}")))
}

/// 按优先级排好的候选地址
pub fn candidates(url: &str, src: Source) -> Vec<String> {
    let official = url.strip_prefix("http://").map(|r| format!("https://{r}")).unwrap_or_else(|| url.to_string());
    match (src, mirror(url)) {
        (Source::Bmcl, Some(m)) => vec![m, official],
        _ => vec![official],
    }
}

/// 依次尝试候选地址获取 JSON
pub async fn get_json<T: DeserializeOwned>(url: &str, src: Source) -> Result<T> {
    let bytes = get_bytes(url, src).await?;
    serde_json::from_slice(&bytes).with_context(|| format!("解析 JSON 失败：{url}"))
}

pub async fn get_bytes(url: &str, src: Source) -> Result<Vec<u8>> {
    let mut last = anyhow!("没有可用的下载地址");
    for u in candidates(url, src) {
        match fetch_bytes(&u).await {
            Ok(b) => return Ok(b),
            Err(e) => last = e,
        }
    }
    Err(last)
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = client()
        .get(url)
        .timeout(Duration::from_secs(40))
        .send()
        .await
        .with_context(|| format!("连接失败：{url}"))?
        .error_for_status()
        .with_context(|| format!("请求失败：{url}"))?;
    Ok(resp.bytes().await.with_context(|| format!("读取失败：{url}"))?.to_vec())
}

/* ---------------- 批量下载 ---------------- */

#[derive(Clone, Debug)]
pub struct DlItem {
    pub urls: Vec<String>,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// 文件已存在时是否重新计算 SHA1 校验（资源文件以哈希命名，只比对大小即可）
    pub verify_existing: bool,
}

impl DlItem {
    pub fn new(url: &str, src: Source, path: PathBuf, sha1: Option<&str>, size: Option<u64>) -> Self {
        Self {
            urls: candidates(url, src),
            path,
            sha1: sha1.filter(|s| !s.is_empty()).map(|s| s.to_lowercase()),
            size,
            verify_existing: true,
        }
    }
}

pub fn sha1_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = sha1_smol::Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.digest().to_string())
}

/// 文件已经存在且完好时返回 true
fn already_ok(item: &DlItem) -> bool {
    let Ok(meta) = std::fs::metadata(&item.path) else { return false };
    if !meta.is_file() {
        return false;
    }
    if let Some(size) = item.size {
        if meta.len() != size {
            return false;
        }
    }
    match (&item.sha1, item.verify_existing) {
        (Some(expect), true) => sha1_file(&item.path).map(|h| &h == expect).unwrap_or(false),
        _ => meta.len() > 0 || item.size == Some(0),
    }
}

/// 并发下载一批文件。已存在且校验通过的文件会跳过。
pub async fn download_all(items: Vec<DlItem>, threads: usize, ctx: &TaskCtx) -> Result<()> {
    ctx.total.store(items.len() as u64, Ordering::Relaxed);
    ctx.done.store(0, Ordering::Relaxed);
    let threads = threads.clamp(1, 256);

    stream::iter(items.into_iter().map(Ok::<_, anyhow::Error>))
        .try_for_each_concurrent(threads, |item| async move {
            ctx.check_cancel()?;
            if !already_ok(&item) {
                download_one(&item, ctx).await?;
            }
            ctx.done.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .await
}

pub async fn download_one(item: &DlItem, ctx: &TaskCtx) -> Result<()> {
    let mut last = anyhow!("没有可用的下载地址");
    for attempt in 0..3 {
        for url in &item.urls {
            ctx.check_cancel()?;
            match fetch_to_file(url, item, ctx).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if ctx.is_cancelled() {
                        bail!("已取消");
                    }
                    last = e;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(400 * (attempt + 1))).await;
    }
    let name = item.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Err(last.context(format!("下载 {name} 失败")))
}

async fn fetch_to_file(url: &str, item: &DlItem, ctx: &TaskCtx) -> Result<()> {
    if let Some(dir) = item.path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = PathBuf::from(format!("{}.amc-part", item.path.display()));

    let resp = client().get(url).send().await?.error_for_status()?;
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut written: u64 = 0;
    let mut body = resp.bytes_stream();

    let result: Result<()> = async {
        while let Some(chunk) = body.next().await {
            ctx.check_cancel()?;
            let chunk = chunk?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            written += chunk.len() as u64;
            ctx.bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }
        file.flush().await?;
        Ok(())
    }
    .await;
    drop(file);

    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    if let Some(size) = item.size {
        if written != size {
            let _ = tokio::fs::remove_file(&tmp).await;
            bail!("大小不对（应为 {size}，实际 {written}）");
        }
    }
    if let Some(expect) = &item.sha1 {
        let got = hasher.digest().to_string();
        if &got != expect {
            let _ = tokio::fs::remove_file(&tmp).await;
            bail!("SHA1 校验失败");
        }
    }
    tokio::fs::rename(&tmp, &item.path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_rewrites_known_hosts() {
        assert_eq!(
            mirror("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json").as_deref(),
            Some("https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json")
        );
        assert_eq!(
            mirror("https://resources.download.minecraft.net/ab/abcdef").as_deref(),
            Some("https://bmclapi2.bangbang93.com/assets/ab/abcdef")
        );
        assert_eq!(
            mirror("http://files.minecraftforge.net/maven/net/x.jar").as_deref(),
            Some("https://bmclapi2.bangbang93.com/maven/net/x.jar")
        );
        assert_eq!(mirror("https://maven.quiltmc.org/repository/release/x.jar"), None);
    }

    #[test]
    fn candidates_order() {
        let c = candidates("https://libraries.minecraft.net/a/b.jar", Source::Bmcl);
        assert_eq!(c[0], "https://bmclapi2.bangbang93.com/maven/a/b.jar");
        assert_eq!(c[1], "https://libraries.minecraft.net/a/b.jar");
        assert_eq!(candidates("https://libraries.minecraft.net/a/b.jar", Source::Official).len(), 1);
    }
}
