//! Modrinth：搜索并安装 Mod / 光影 / 材质包（自动装必需的前置），以及识别已安装的文件是哪个项目。
//! 官方接口优先，失败时用 MCIM 国内镜像。

use anyhow::{anyhow, bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::game::installed;
use crate::net::{self, DlItem};
use crate::task::TaskCtx;

const API: &str = "https://api.modrinth.com/v2";
const MIRROR_API: &str = "https://mod.mcimirror.top/modrinth/v2";
const CDN: &str = "https://cdn.modrinth.com";
const MIRROR_CDN: &str = "https://mod.mcimirror.top";

/// 资源类型 → 版本里的文件夹
pub fn kind_folder(kind: &str) -> Result<&'static str> {
    Ok(match kind {
        "mod" => "mods",
        "shader" => "shaderpacks",
        "resourcepack" => "resourcepacks",
        _ => bail!("未知的资源类型：{kind}"),
    })
}

async fn send_json<T: DeserializeOwned>(req: reqwest::RequestBuilder) -> Result<T> {
    let resp = req.timeout(Duration::from_secs(20)).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("Modrinth 上找不到这个内容");
    }
    let resp = resp.error_for_status()?;
    Ok(resp.json().await?)
}

/// GET，官方失败时换镜像
async fn get<T: DeserializeOwned>(path: &str, query: &[(&str, String)]) -> Result<T> {
    let mut last = anyhow!("无法连接 Modrinth");
    for base in [API, MIRROR_API] {
        match send_json(net::client().get(format!("{base}{path}")).query(query)).await {
            Ok(v) => return Ok(v),
            Err(e) if e.to_string().contains("找不到") => return Err(e),
            Err(e) => last = e,
        }
    }
    Err(last.context("连接 Modrinth 失败，检查一下网络"))
}

async fn post<T: DeserializeOwned>(path: &str, body: &Value) -> Result<T> {
    let mut last = anyhow!("无法连接 Modrinth");
    for base in [API, MIRROR_API] {
        match send_json(net::client().post(format!("{base}{path}")).json(body)).await {
            Ok(v) => return Ok(v),
            Err(e) => last = e,
        }
    }
    Err(last.context("连接 Modrinth 失败，检查一下网络"))
}

/// 加载器在 Modrinth 上的名字；Quilt 也能用 Fabric 的 Mod
fn mod_loaders(loader: &str) -> Vec<String> {
    match loader.to_lowercase().as_str() {
        "quilt" => vec!["quilt".into(), "fabric".into()],
        l => vec![l.to_string()],
    }
}

/* ---------------- 搜索 ---------------- */

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchReq {
    /// mod / shader / resourcepack
    pub kind: String,
    pub query: String,
    /// 游戏版本，为空不过滤
    pub mc: Option<String>,
    /// Mod：fabric / forge / neoforge / quilt；光影：iris / optifine
    pub loader: Option<String>,
    /// Modrinth 的分类名
    pub category: Option<String>,
    /// relevance / downloads / updated
    pub sort: String,
    pub offset: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    /// 图标主色（没有图标时用来画占位）
    pub color: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total: u64,
}

pub const PAGE_SIZE: u32 = 24;

pub fn facets(req: &SearchReq) -> Value {
    let mut f: Vec<Value> = vec![json!([format!("project_type:{}", req.kind)])];
    if let Some(mc) = req.mc.as_deref().filter(|s| !s.is_empty()) {
        f.push(json!([format!("versions:{mc}")]));
    }
    if let Some(l) = req.loader.as_deref().filter(|s| !s.is_empty()) {
        let ls: Vec<String> = mod_loaders(l).into_iter().map(|x| format!("categories:{x}")).collect();
        f.push(json!(ls));
    }
    if let Some(c) = req.category.as_deref().filter(|s| !s.is_empty()) {
        f.push(json!([format!("categories:{c}")]));
    }
    Value::Array(f)
}

pub async fn search(req: &SearchReq) -> Result<SearchResult> {
    let index = match req.sort.as_str() {
        "downloads" | "updated" | "newest" | "follows" => req.sort.clone(),
        _ => "relevance".into(),
    };
    let v: Value = get(
        "/search",
        &[
            ("query", req.query.trim().to_string()),
            ("facets", facets(req).to_string()),
            ("index", index),
            ("limit", PAGE_SIZE.to_string()),
            ("offset", req.offset.to_string()),
        ],
    )
    .await?;
    let s = |h: &Value, k: &str| h[k].as_str().unwrap_or_default().to_string();
    let hits = v["hits"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|h| Hit {
            id: s(h, "project_id"),
            slug: s(h, "slug"),
            title: s(h, "title"),
            author: s(h, "author"),
            description: s(h, "description"),
            downloads: h["downloads"].as_u64().unwrap_or(0),
            icon: h["icon_url"].as_str().filter(|x| !x.is_empty()).map(str::to_string),
            categories: h["display_categories"]
                .as_array()
                .or(h["categories"].as_array())
                .map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            color: h["color"].as_u64().map(|c| c as u32),
        })
        .collect();
    Ok(SearchResult { hits, total: v["total_hits"].as_u64().unwrap_or(0) })
}

/* ---------------- 识别已安装的文件 ---------------- */

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub project_id: String,
    pub title: String,
    pub icon: Option<String>,
    pub version_number: String,
}

/// 文件 SHA1 缓存：(路径, 大小, 修改时间) → sha1，避免每次都重新算
fn hash_cached(p: &Path) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, u64, u128), String>>> = OnceLock::new();
    let md = std::fs::metadata(p).ok()?;
    let mtime = md.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let key = (p.to_path_buf(), md.len(), mtime);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(h) = cache.lock().unwrap().get(&key) {
        return Some(h.clone());
    }
    let h = net::sha1_file(p).ok()?;
    cache.lock().unwrap().insert(key, h.clone());
    Some(h)
}

fn version_files(game_dir: &Path, version: &str) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    for kind in ["mods", "shaderpacks", "resourcepacks"] {
        let dir = installed::kind_dir(game_dir, version, kind)?;
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let low = name.to_lowercase();
            let ok = if kind == "mods" { low.ends_with(".jar") || low.ends_with(".jar.disabled") } else { low.ends_with(".zip") };
            if ok && e.path().is_file() {
                out.push((format!("{kind}/{name}"), e.path()));
            }
        }
    }
    Ok(out)
}

/// 返回 "mods/文件名" → 对应的 Modrinth 项目。不在 Modrinth 上的文件不会出现。
pub async fn identify(game_dir: &Path, version: &str) -> Result<HashMap<String, FileInfo>> {
    let files = version_files(game_dir, version)?;
    if files.is_empty() {
        return Ok(HashMap::new());
    }
    let hashed: Vec<(String, String)> = tauri::async_runtime::spawn_blocking(move || {
        files.into_iter().filter_map(|(k, p)| hash_cached(&p).map(|h| (k, h))).collect()
    })
    .await?;
    let hashes: Vec<&String> = hashed.iter().map(|(_, h)| h).collect();
    let versions: HashMap<String, Value> = post("/version_files", &json!({ "hashes": hashes, "algorithm": "sha1" })).await?;

    let ids: HashSet<String> = versions.values().filter_map(|v| v["project_id"].as_str().map(str::to_string)).collect();
    let projects: Vec<Value> = if ids.is_empty() {
        vec![]
    } else {
        get("/projects", &[("ids", json!(ids.iter().collect::<Vec<_>>()).to_string())]).await?
    };
    let by_id: HashMap<&str, &Value> = projects.iter().filter_map(|p| Some((p["id"].as_str()?, p))).collect();

    let mut out = HashMap::new();
    for (key, hash) in &hashed {
        let Some(v) = versions.get(hash) else { continue };
        let Some(pid) = v["project_id"].as_str() else { continue };
        let p = by_id.get(pid);
        out.insert(
            key.clone(),
            FileInfo {
                project_id: pid.to_string(),
                title: p.and_then(|p| p["title"].as_str()).unwrap_or(pid).to_string(),
                icon: p.and_then(|p| p["icon_url"].as_str()).filter(|x| !x.is_empty()).map(str::to_string),
                version_number: v["version_number"].as_str().unwrap_or_default().to_string(),
            },
        );
    }
    Ok(out)
}

/* ---------------- 安装 ---------------- */

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReq {
    pub game_dir: String,
    /// 版本名（装到这个版本里）
    pub version: String,
    pub kind: String,
    pub project_id: String,
    pub mc: String,
    /// Mod：版本的加载器；光影：iris / optifine；材质包：空
    pub loader: Option<String>,
    /// 指定装哪个版本；不填就挑最新的正式版
    #[serde(default)]
    pub version_id: Option<String>,
    /// 已经装了也重新装（替换旧文件）
    #[serde(default)]
    pub reinstall: bool,
    /// 自动装必需的前置（默认开）
    #[serde(default = "yes")]
    pub with_deps: bool,
}

fn yes() -> bool {
    true
}

/// 依赖条目对应的项目号（有的只给了版本号没给项目号）
async fn dep_project_id(d: &Value) -> Option<String> {
    match (d["project_id"].as_str(), d["version_id"].as_str()) {
        (Some(p), _) => Some(p.to_string()),
        (None, Some(vid)) => get::<Value>(&format!("/version/{vid}"), &[]).await.ok()?["project_id"].as_str().map(str::to_string),
        _ => None,
    }
    .filter(|p| !p.is_empty())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepsReq {
    pub game_dir: String,
    pub version: String,
    pub kind: String,
    /// 选中的资源版本
    pub version_id: String,
    pub mc: String,
    pub loader: Option<String>,
}

/// 选中的版本需要的一个前置
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dep {
    pub project_id: String,
    pub title: String,
    pub icon: Option<String>,
    /// 这个游戏版本里已经装了
    pub installed: bool,
    /// 有适合这个游戏版本的文件
    pub available: bool,
}

/// 列出选中版本的必需前置（只列直接依赖；自动安装时会继续往下找）
pub async fn deps(req: &DepsReq) -> Result<Vec<Dep>> {
    let loaders = loaders_for(&req.kind, req.loader.as_deref())?;
    let v: Value = get(&format!("/version/{}", req.version_id), &[]).await?;
    let self_id = v["project_id"].as_str().unwrap_or_default().to_string();
    let mut ids: Vec<String> = Vec::new();
    for d in v["dependencies"].as_array().into_iter().flatten() {
        if d["dependency_type"] != "required" {
            continue;
        }
        if let Some(p) = dep_project_id(d).await {
            if p != self_id && !ids.contains(&p) {
                ids.push(p);
            }
        }
    }
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let projects: Vec<Value> = get("/projects", &[("ids", json!(ids).to_string())]).await?;
    let installed: HashSet<String> = identify(&PathBuf::from(&req.game_dir), &req.version)
        .await
        .map(|m| m.into_values().map(|f| f.project_id).collect())
        .unwrap_or_default();
    let avail = futures_util::future::join_all(ids.iter().map(|p| pick_version(p, &req.mc, &loaders))).await;
    Ok(ids
        .iter()
        .zip(avail)
        .map(|(id, a)| {
            let p = projects.iter().find(|p| p["id"].as_str() == Some(id.as_str()));
            Dep {
                project_id: id.clone(),
                title: p.and_then(|p| p["title"].as_str()).unwrap_or(id).to_string(),
                icon: p.and_then(|p| p["icon_url"].as_str()).filter(|x| !x.is_empty()).map(str::to_string),
                installed: installed.contains(id),
                available: matches!(a, Ok(Some(_))),
            }
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub title: String,
    pub file: String,
    pub version_number: String,
    /// 被替换掉（移到回收站）的旧文件
    pub replaced: Vec<String>,
    /// 一起装上的前置
    pub deps: Vec<String>,
    /// 找不到合适版本的前置
    pub missing: Vec<String>,
}

/// 这个项目适合这个游戏版本和加载器的全部版本（新的在前）
async fn list_versions(project: &str, mc: &str, loaders: &[String]) -> Result<Vec<Value>> {
    // 整合包不按游戏版本过滤（整合包自己决定用哪个版本）
    let mut q = vec![];
    if !mc.is_empty() {
        q.push(("game_versions", json!([mc]).to_string()));
    }
    if !loaders.is_empty() {
        q.push(("loaders", json!(loaders).to_string()));
    }
    get(&format!("/project/{project}/version"), &q).await
}

/// 按资源类型决定用哪些加载器过滤
fn loaders_for(kind: &str, loader: Option<&str>) -> Result<Vec<String>> {
    Ok(match (kind, loader) {
        ("mod", Some(l)) => mod_loaders(l),
        ("mod", None) => bail!("原版不能装 Mod"),
        ("shader", Some(l)) if !l.is_empty() => vec![l.to_lowercase()],
        ("resourcepack", _) => vec!["minecraft".into()],
        _ => vec![],
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionsReq {
    pub project_id: String,
    pub kind: String,
    pub mc: String,
    pub loader: Option<String>,
}

/// 给用户挑的一个版本
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    /// release / beta / alpha
    pub version_type: String,
    pub published: String,
    pub downloads: u64,
    pub file: String,
    pub size: u64,
    /// 支持的游戏版本和加载器（整合包用来显示）
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

pub async fn versions(req: &VersionsReq) -> Result<Vec<ModVersion>> {
    let loaders = loaders_for(&req.kind, req.loader.as_deref())?;
    let list = list_versions(&req.project_id, &req.mc, &loaders).await?;
    let strs = |v: &Value| -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default() };
    Ok(list
        .iter()
        .filter_map(|v| {
            let f = primary_file(v)?;
            Some(ModVersion {
                id: v["id"].as_str()?.to_string(),
                name: v["name"].as_str().unwrap_or_default().to_string(),
                version_number: v["version_number"].as_str().unwrap_or_default().to_string(),
                version_type: v["version_type"].as_str().unwrap_or("release").to_string(),
                published: v["date_published"].as_str().unwrap_or_default().to_string(),
                downloads: v["downloads"].as_u64().unwrap_or(0),
                file: f["filename"].as_str().unwrap_or_default().to_string(),
                size: f["size"].as_u64().unwrap_or(0),
                game_versions: strs(&v["game_versions"]),
                loaders: strs(&v["loaders"]),
            })
        })
        .collect())
}

/// 挑一个适合这个游戏版本和加载器的版本：优先正式版
async fn pick_version(project: &str, mc: &str, loaders: &[String]) -> Result<Option<Value>> {
    let list = list_versions(project, mc, loaders).await?;
    Ok(list
        .iter()
        .find(|v| v["version_type"] == "release")
        .or(list.first())
        .cloned())
}

/// cdn.modrinth.com 的地址换成国内镜像
pub fn mirror_cdn(url: &str) -> Option<String> {
    url.strip_prefix(CDN).map(|rest| format!("{MIRROR_CDN}{rest}"))
}

/// 读取一个版本的完整信息
pub async fn version(id: &str) -> Result<Value> {
    get(&format!("/version/{id}"), &[]).await
}

pub fn primary_file(v: &Value) -> Option<&Value> {
    let files = v["files"].as_array()?;
    files.iter().find(|f| f["primary"].as_bool() == Some(true)).or(files.first())
}

async fn download_file(f: &Value, dir: &Path) -> Result<String> {
    let url = f["url"].as_str().ok_or_else(|| anyhow!("没有下载地址"))?;
    let name = f["filename"].as_str().ok_or_else(|| anyhow!("没有文件名"))?;
    installed::check_plain_name(name)?;
    let mut urls = vec![url.to_string()];
    if let Some(rest) = url.strip_prefix(CDN) {
        urls.push(format!("{MIRROR_CDN}{rest}"));
    }
    let item = DlItem { urls, path: dir.join(name), sha1: f.pointer("/hashes/sha1").and_then(Value::as_str).map(str::to_lowercase), size: f["size"].as_u64(), verify_existing: true };
    let ctx = TaskCtx::new("res".into(), Arc::new(AtomicBool::new(false)), 1);
    net::download_one(&item, &ctx).await?;
    Ok(name.to_string())
}

pub async fn install(req: &InstallReq) -> Result<InstallResult> {
    let game_dir = PathBuf::from(&req.game_dir);
    let folder = kind_folder(&req.kind)?;
    let dir = installed::kind_dir(&game_dir, &req.version, folder)?;
    let loaders = loaders_for(&req.kind, req.loader.as_deref())?;

    // 已经装了哪些项目（用来跳过已有的前置，以及重新安装时找旧文件）
    let found = identify(&game_dir, &req.version).await.unwrap_or_default();
    let prefix = format!("{folder}/");
    let old_files: Vec<String> = found
        .iter()
        .filter(|(k, f)| f.project_id == req.project_id && k.starts_with(&prefix))
        .map(|(k, _)| k[prefix.len()..].to_string())
        .collect();
    if !old_files.is_empty() && !req.reinstall {
        bail!("这个版本里已经装了");
    }
    let installed_ids: HashSet<String> = found.into_values().map(|f| f.project_id).collect();

    let root = match &req.version_id {
        Some(id) => {
            let v: Value = get(&format!("/version/{id}"), &[]).await?;
            if v["project_id"].as_str() != Some(req.project_id.as_str()) {
                bail!("选的版本不属于这个项目");
            }
            v
        }
        None => pick_version(&req.project_id, &req.mc, &loaders)
            .await?
            .ok_or_else(|| anyhow!("没有适合 {} {} 的版本", req.mc, req.loader.clone().unwrap_or_default()))?,
    };
    let title: String = get::<Value>(&format!("/project/{}", req.project_id), &[]).await?["title"].as_str().unwrap_or("").to_string();
    let version_number = root["version_number"].as_str().unwrap_or_default().to_string();
    let f = primary_file(&root).ok_or_else(|| anyhow!("这个版本没有文件"))?;
    let file = download_file(f, &dir).await.context("下载失败")?;

    // 新文件下好了再处理旧的：移到回收站（同名的就是刚下好的，不动）
    let mut replaced = Vec::new();
    for old in old_files.into_iter().filter(|o| *o != file) {
        let p = dir.join(&old);
        if trash::delete(&p).is_ok() || std::fs::remove_file(&p).is_ok() {
            replaced.push(old);
        }
    }

    // 必需的前置，逐层往下找
    let mut deps = Vec::new();
    let mut missing = Vec::new();
    let mut seen: HashSet<String> = installed_ids;
    seen.insert(req.project_id.clone());
    let mut queue: VecDeque<Value> = if req.with_deps { VecDeque::from([root]) } else { VecDeque::new() };
    while let Some(v) = queue.pop_front() {
        for d in v["dependencies"].as_array().into_iter().flatten() {
            if d["dependency_type"] != "required" {
                continue;
            }
            let Some(pid) = dep_project_id(d).await else { continue };
            if !seen.insert(pid.clone()) || deps.len() + missing.len() > 30 {
                continue;
            }
            let dep_title = get::<Value>(&format!("/project/{pid}"), &[])
                .await
                .ok()
                .and_then(|p| p["title"].as_str().map(str::to_string))
                .unwrap_or_else(|| pid.clone());
            let Some(dv) = pick_version(&pid, &req.mc, &loaders).await.ok().flatten() else {
                missing.push(dep_title);
                continue;
            };
            match primary_file(&dv) {
                Some(df) if download_file(df, &dir).await.is_ok() => {
                    deps.push(dep_title);
                    queue.push_back(dv);
                }
                _ => missing.push(dep_title),
            }
        }
    }
    Ok(InstallResult { title, file, version_number, replaced, deps, missing })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facet_building() {
        let req = SearchReq {
            kind: "mod".into(),
            query: "".into(),
            mc: Some("1.20.1".into()),
            loader: Some("Quilt".into()),
            category: Some("optimization".into()),
            sort: "downloads".into(),
            offset: 0,
        };
        assert_eq!(
            facets(&req),
            json!([["project_type:mod"], ["versions:1.20.1"], ["categories:quilt", "categories:fabric"], ["categories:optimization"]])
        );
    }

    /// 联网测试：cargo test -- --ignored modrinth_live
    #[tokio::test]
    #[ignore]
    async fn modrinth_live() {
        let r = search(&SearchReq {
            kind: "mod".into(),
            query: "sodium".into(),
            mc: Some("1.20.1".into()),
            loader: Some("fabric".into()),
            category: None,
            sort: "relevance".into(),
            offset: 0,
        })
        .await
        .unwrap();
        assert!(r.hits.iter().any(|h| h.slug == "sodium"));
        let v = pick_version("AANobbMI", "1.20.1", &["fabric".into()]).await.unwrap().unwrap();
        assert!(primary_file(&v).is_some());
    }

    /// 联网测试：装 Iris（会自动带上它需要的前置），再识别出来
    #[tokio::test]
    #[ignore]
    async fn modrinth_install_live() {
        let root = std::env::temp_dir().join(format!("amc-mr-{}", std::process::id()));
        let vdir = root.join("versions").join("t");
        std::fs::create_dir_all(&vdir).unwrap();
        std::fs::write(vdir.join("amc.json"), "{}").unwrap();
        let req = |version_id: Option<String>, reinstall: bool| InstallReq {
            game_dir: root.to_string_lossy().into(),
            version: "t".into(),
            kind: "mod".into(),
            project_id: "YL57xq9U".into(), // Iris
            mc: "1.20.1".into(),
            loader: Some("Fabric".into()),
            version_id,
            reinstall,
            with_deps: true,
        };
        // 先看前置：Iris 1.20.1 需要 Sodium
        let first = pick_version("YL57xq9U", "1.20.1", &mod_loaders("Fabric")).await.unwrap().unwrap();
        let ds = deps(&DepsReq {
            game_dir: root.to_string_lossy().into(),
            version: "t".into(),
            kind: "mod".into(),
            version_id: first["id"].as_str().unwrap().into(),
            mc: "1.20.1".into(),
            loader: Some("Fabric".into()),
        })
        .await
        .unwrap();
        println!("前置：{ds:?}");
        assert!(ds.iter().any(|d| d.title == "Sodium" && !d.installed && d.available));

        // 关掉自动前置：只装 Iris
        let mut solo = req(None, false);
        solo.with_deps = false;
        let r0 = install(&solo).await.unwrap();
        assert!(r0.deps.is_empty());
        std::fs::remove_file(installed::kind_dir(&root, "t", "mods").unwrap().join(&r0.file)).unwrap();

        let r = install(&req(None, false)).await.unwrap();
        println!("{r:?}");
        let ids = identify(&root, "t").await.unwrap();
        assert!(ids.values().any(|f| f.project_id == "YL57xq9U"));
        // 再装一次应该提示已安装
        assert!(install(&req(None, false)).await.is_err());

        // 列出可选版本，换成一个旧版本重新安装
        let list = versions(&VersionsReq { project_id: "YL57xq9U".into(), kind: "mod".into(), mc: "1.20.1".into(), loader: Some("Fabric".into()) })
            .await
            .unwrap();
        println!("{} 个版本，前几个：{:?}", list.len(), list.iter().take(3).map(|v| (&v.version_number, &v.version_type)).collect::<Vec<_>>());
        let old = list.iter().rev().find(|v| v.version_number != r.version_number).expect("至少要有两个版本");
        let r2 = install(&req(Some(old.id.clone()), true)).await.unwrap();
        println!("{r2:?}");
        assert_eq!(r2.version_number, old.version_number);
        assert_eq!(r2.replaced, vec![r.file.clone()]);
        let ids = identify(&root, "t").await.unwrap();
        let iris: Vec<_> = ids.values().filter(|f| f.project_id == "YL57xq9U").collect();
        assert_eq!(iris.len(), 1, "旧文件应该被替换掉");
        assert_eq!(iris[0].version_number, old.version_number);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
