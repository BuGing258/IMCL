//! 版本清单、加载器版本列表、OptiFine 版本列表。

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::net::{self, Source};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const BMCL: &str = "https://bmclapi2.bangbang93.com";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub release_time: String,
    #[serde(skip_serializing)]
    pub url: String,
    #[serde(skip_serializing, default)]
    pub sha1: String,
}

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

static MANIFEST: Mutex<Option<(Instant, Vec<ManifestEntry>)>> = Mutex::new(None);

/// 版本清单（缓存 10 分钟）
pub async fn manifest(src: Source) -> Result<Vec<ManifestEntry>> {
    if let Some((t, v)) = MANIFEST.lock().unwrap().as_ref() {
        if t.elapsed() < Duration::from_secs(600) {
            return Ok(v.clone());
        }
    }
    let m: Manifest = net::get_json(MANIFEST_URL, src).await.context("获取版本清单失败")?;
    *MANIFEST.lock().unwrap() = Some((Instant::now(), m.versions.clone()));
    Ok(m.versions)
}

pub async fn find_version(mc: &str, src: Source) -> Result<ManifestEntry> {
    manifest(src)
        .await?
        .into_iter()
        .find(|v| v.id == mc)
        .ok_or_else(|| anyhow!("版本清单里没有 {mc}"))
}

/* ---------------- 加载器 ---------------- */

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl LoaderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    /// 安装时用的完整版本号（Forge 是 "1.20.1-47.2.0" 这种）
    pub id: String,
    /// 界面上显示的版本号
    pub version: String,
    /// 稳定版 / 推荐版
    pub stable: bool,
}

pub async fn loader_versions(kind: LoaderKind, mc: &str, src: Source) -> Result<Vec<LoaderVersion>> {
    match kind {
        LoaderKind::Fabric => fabric_like(&format!("https://meta.fabricmc.net/v2/versions/loader/{mc}"), src).await,
        LoaderKind::Quilt => fabric_like(&format!("https://meta.quiltmc.org/v3/versions/loader/{mc}"), src).await,
        LoaderKind::Forge => forge_versions(mc, src).await,
        LoaderKind::NeoForge => neoforge_versions(mc).await,
    }
}

async fn fabric_like(url: &str, src: Source) -> Result<Vec<LoaderVersion>> {
    let list: Vec<Value> = net::get_json(url, src).await?;
    Ok(list
        .iter()
        .filter_map(|e| {
            let l = e.get("loader")?;
            let v = l.get("version")?.as_str()?.to_string();
            let stable = l
                .get("stable")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| !v.contains("beta") && !v.contains("pre") && !v.contains("rc"));
            Some(LoaderVersion { id: v.clone(), version: v, stable })
        })
        .collect())
}

async fn forge_promos() -> HashMap<String, String> {
    #[derive(Deserialize)]
    struct Promos {
        promos: HashMap<String, String>,
    }
    let fut = net::get_json::<Promos>(
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
        Source::Official,
    );
    match tokio::time::timeout(Duration::from_secs(6), fut).await {
        Ok(Ok(p)) => p.promos,
        _ => HashMap::new(),
    }
}

async fn forge_versions(mc: &str, src: Source) -> Result<Vec<LoaderVersion>> {
    let promos = forge_promos();
    let list = async {
        if src == Source::Bmcl {
            if let Ok(list) = net::get_json::<Vec<Value>>(&format!("{BMCL}/forge/minecraft/{mc}"), Source::Official).await {
                let mut items: Vec<(i64, String, String)> = list
                    .iter()
                    .filter_map(|e| {
                        let v = e.get("version")?.as_str()?.to_string();
                        let build = e.get("build").and_then(Value::as_i64).unwrap_or(0);
                        let full = match e.get("branch").and_then(Value::as_str) {
                            Some(b) if !b.is_empty() => format!("{mc}-{v}-{b}"),
                            _ => format!("{mc}-{v}"),
                        };
                        Some((build, full, v))
                    })
                    .collect();
                items.sort_by(|a, b| b.0.cmp(&a.0));
                return Ok(items.into_iter().map(|(_, id, v)| (id, v)).collect::<Vec<_>>());
            }
        }
        let meta: HashMap<String, Vec<String>> = net::get_json(
            "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json",
            Source::Official,
        )
        .await
        .context("获取 Forge 版本列表失败")?;
        let ids = meta.get(mc).cloned().unwrap_or_default();
        Ok::<_, anyhow::Error>(
            ids.into_iter()
                .rev()
                .map(|id| {
                    let short = id.strip_prefix(&format!("{mc}-")).unwrap_or(&id);
                    let short = short.strip_suffix(&format!("-{mc}")).unwrap_or(short).to_string();
                    (id, short)
                })
                .collect(),
        )
    };
    let (promos, list) = futures_util::join!(promos, list);
    let recommended = promos.get(&format!("{mc}-recommended"));
    Ok(list?
        .into_iter()
        .map(|(id, version)| LoaderVersion { stable: Some(&version) == recommended, id, version })
        .collect())
}

/// MC 版本对应的 NeoForge 版本前缀：1.21.1 → "21.1."，1.21 → "21.0."，26.3 → "26.3.0."
pub fn neoforge_prefix(mc: &str) -> Option<String> {
    let parts: Vec<&str> = mc.split('.').collect();
    if parts.iter().any(|p| p.parse::<u32>().is_err()) {
        return None; // 快照等
    }
    match parts.as_slice() {
        ["1", minor] => Some(format!("{minor}.0.")),
        ["1", minor, patch] => Some(format!("{minor}.{patch}.")),
        [year, drop] => Some(format!("{year}.{drop}.0.")),
        [year, drop, hotfix] => Some(format!("{year}.{drop}.{hotfix}.")),
        _ => None,
    }
}

async fn neoforge_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    let Some(prefix) = neoforge_prefix(mc) else { return Ok(vec![]) };
    #[derive(Deserialize)]
    struct Versions {
        versions: Vec<String>,
    }
    let v: Versions = net::get_json(
        "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
        Source::Official,
    )
    .await
    .context("获取 NeoForge 版本列表失败")?;
    Ok(v.versions
        .into_iter()
        .rev()
        .filter(|s| s.starts_with(&prefix))
        .map(|s| LoaderVersion { stable: !s.contains("beta") && !s.contains("alpha"), id: s.clone(), version: s })
        .collect())
}

/* ---------------- OptiFine ---------------- */

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptiFineVersion {
    #[serde(rename = "type")]
    pub kind: String,
    pub patch: String,
    pub filename: String,
    /// 需要搭配的 Forge 版本，比如 "Forge 47.2.18"；"Forge N/A" 表示不兼容 Forge
    pub forge: Option<String>,
    pub preview: bool,
}

pub async fn optifine_versions(mc: &str) -> Result<Vec<OptiFineVersion>> {
    let list: Vec<Value> = net::get_json(&format!("{BMCL}/optifine/{mc}"), Source::Official)
        .await
        .context("获取 OptiFine 列表失败（OptiFine 只能通过 BMCLAPI 获取）")?;
    let mut out: Vec<OptiFineVersion> = list
        .iter()
        .filter_map(|e| {
            let patch = e.get("patch")?.as_str()?.to_string();
            let filename = e.get("filename").and_then(Value::as_str).unwrap_or_default().to_string();
            Some(OptiFineVersion {
                kind: e.get("type")?.as_str()?.to_string(),
                preview: patch.to_lowercase().starts_with("pre") || filename.to_lowercase().contains("preview"),
                patch,
                filename,
                forge: e.get("forge").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect();
    out.reverse();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neo_prefix() {
        assert_eq!(neoforge_prefix("1.21.1").as_deref(), Some("21.1."));
        assert_eq!(neoforge_prefix("1.21").as_deref(), Some("21.0."));
        assert_eq!(neoforge_prefix("1.20.4").as_deref(), Some("20.4."));
        assert_eq!(neoforge_prefix("26.3").as_deref(), Some("26.3.0."));
        assert_eq!(neoforge_prefix("26.3.1").as_deref(), Some("26.3.1."));
        assert_eq!(neoforge_prefix("26.4-snapshot-8"), None);
    }
}
