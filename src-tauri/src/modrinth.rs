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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub title: String,
    pub file: String,
    /// 一起装上的前置
    pub deps: Vec<String>,
    /// 找不到合适版本的前置
    pub missing: Vec<String>,
}

/// 挑一个适合这个游戏版本和加载器的版本：优先正式版
async fn pick_version(project: &str, mc: &str, loaders: &[String]) -> Result<Option<Value>> {
    let mut q = vec![("game_versions", json!([mc]).to_string())];
    if !loaders.is_empty() {
        q.push(("loaders", json!(loaders).to_string()));
    }
    let list: Vec<Value> = get(&format!("/project/{project}/version"), &q).await?;
    Ok(list
        .iter()
        .find(|v| v["version_type"] == "release")
        .or(list.first())
        .cloned())
}

fn primary_file(v: &Value) -> Option<&Value> {
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
    let loaders = match (req.kind.as_str(), req.loader.as_deref()) {
        ("mod", Some(l)) => mod_loaders(l),
        ("mod", None) => bail!("原版不能装 Mod"),
        ("shader", Some(l)) if !l.is_empty() => vec![l.to_lowercase()],
        ("resourcepack", _) => vec!["minecraft".into()],
        _ => vec![],
    };

    // 已经装了哪些项目（用来跳过已有的前置）
    let installed_ids: HashSet<String> = identify(&game_dir, &req.version)
        .await
        .map(|m| m.into_values().map(|f| f.project_id).collect())
        .unwrap_or_default();
    if installed_ids.contains(&req.project_id) {
        bail!("这个版本里已经装了");
    }

    let root = pick_version(&req.project_id, &req.mc, &loaders)
        .await?
        .ok_or_else(|| anyhow!("没有适合 {} {} 的版本", req.mc, req.loader.clone().unwrap_or_default()))?;
    let title: String = get::<Value>(&format!("/project/{}", req.project_id), &[]).await?["title"].as_str().unwrap_or("").to_string();
    let f = primary_file(&root).ok_or_else(|| anyhow!("这个版本没有文件"))?;
    let file = download_file(f, &dir).await.context("下载失败")?;

    // 必需的前置，逐层往下找
    let mut deps = Vec::new();
    let mut missing = Vec::new();
    let mut seen: HashSet<String> = installed_ids;
    seen.insert(req.project_id.clone());
    let mut queue: VecDeque<Value> = VecDeque::from([root]);
    while let Some(v) = queue.pop_front() {
        for d in v["dependencies"].as_array().into_iter().flatten() {
            if d["dependency_type"] != "required" {
                continue;
            }
            // 有的只给了版本号没给项目号
            let pid = match (d["project_id"].as_str(), d["version_id"].as_str()) {
                (Some(p), _) => p.to_string(),
                (None, Some(vid)) => match get::<Value>(&format!("/version/{vid}"), &[]).await {
                    Ok(dv) => dv["project_id"].as_str().unwrap_or_default().to_string(),
                    Err(_) => continue,
                },
                _ => continue,
            };
            if pid.is_empty() || !seen.insert(pid.clone()) || deps.len() + missing.len() > 30 {
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
    Ok(InstallResult { title, file, deps, missing })
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
        let r = install(&InstallReq {
            game_dir: root.to_string_lossy().into(),
            version: "t".into(),
            kind: "mod".into(),
            project_id: "YL57xq9U".into(), // Iris
            mc: "1.20.1".into(),
            loader: Some("Fabric".into()),
        })
        .await
        .unwrap();
        println!("{r:?}");
        let ids = identify(&root, "t").await.unwrap();
        println!("{ids:#?}");
        assert!(ids.values().any(|f| f.project_id == "YL57xq9U"));
        // 再装一次应该提示已安装
        let again = install(&InstallReq {
            game_dir: root.to_string_lossy().into(),
            version: "t".into(),
            kind: "mod".into(),
            project_id: "YL57xq9U".into(),
            mc: "1.20.1".into(),
            loader: Some("Fabric".into()),
        })
        .await;
        assert!(again.is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
