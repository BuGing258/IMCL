//! 启动游戏：补全文件 → 解压 natives → 拼启动参数 → 启动 Java 进程并收集日志。

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::Notify;

use super::json::{self, OS_NAME};
use super::{installed, zipx};
use crate::java::{self, JavaInfo};
use crate::maven;
use crate::net::{self, DlItem, Source};
use crate::task::TaskCtx;

#[cfg(windows)]
const CP_SEP: &str = ";";
#[cfg(not(windows))]
const CP_SEP: &str = ":";

pub const STAGES: u32 = 5;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub task_id: String,
    pub game_dir: String,
    pub name: String,
    /// java(w).exe 路径，或 "auto"
    pub java: String,
    /// 最大内存（MB）
    pub memory: u32,
    /// "default" / "720p" / "fullscreen"
    pub window_mode: String,
    pub jvm_args: String,
    /// 角色名
    pub player: String,
    pub source: Source,
    pub threads: usize,
    /// 微软账户 id；为空表示离线账户
    #[serde(default)]
    pub ms_account: Option<String>,
    /// 以下由后端在正版登录后填写
    #[serde(skip)]
    pub uuid: Option<String>,
    #[serde(skip)]
    pub access_token: Option<String>,
}

/// 准备好的启动命令
#[derive(Debug)]
pub struct Prepared {
    pub java: JavaInfo,
    pub args: Vec<String>,
    pub run_dir: PathBuf,
    pub log_path: PathBuf,
    /// 正版令牌：写日志时要遮住
    pub secret: Option<String>,
}

/* ---------------- 版本 JSON ---------------- */

/// 读取版本 JSON；其他启动器装的版本可能用 inheritsFrom 依赖另一个版本，这里合并成完整的一份。
/// 返回 (完整 JSON, 客户端 jar 路径)
pub fn resolve_version(game_dir: &Path, name: &str) -> Result<(Value, PathBuf)> {
    let versions = game_dir.join("versions");
    let read = |id: &str| -> Result<Value> {
        let p = versions.join(id).join(format!("{id}.json"));
        let b = std::fs::read(&p).with_context(|| format!("找不到版本文件 {}", p.display()))?;
        serde_json::from_slice(&b).with_context(|| format!("版本文件 {id}.json 已损坏"))
    };
    let mut json = read(name)?;
    let mut chain = vec![name.to_string()];
    let mut jar_hint = json["jar"].as_str().map(str::to_string);
    let mut next = json["inheritsFrom"].as_str().map(str::to_string);
    while let Some(pid) = next {
        if chain.contains(&pid) || chain.len() > 8 {
            bail!("版本之间的依赖关系有问题：{pid}");
        }
        let parent = read(&pid).with_context(|| format!("这个版本依赖的 {pid} 没有安装"))?;
        next = parent["inheritsFrom"].as_str().map(str::to_string);
        if jar_hint.is_none() {
            jar_hint = parent["jar"].as_str().map(str::to_string);
        }
        json = json::merge(&parent, &json, name);
        chain.push(pid);
    }

    // 客户端 jar：自己的 → jar 字段指定的 → 父版本的
    let mut cands = vec![name.to_string()];
    cands.extend(jar_hint);
    cands.extend(chain.iter().skip(1).cloned());
    let jar_of = |id: &String| versions.join(id).join(format!("{id}.jar"));
    let jar = cands
        .iter()
        .map(jar_of)
        .find(|p| p.is_file())
        .unwrap_or_else(|| jar_of(chain.last().unwrap()));
    Ok((json, jar))
}

fn required_java(json: &Value) -> u32 {
    json.pointer("/javaVersion/majorVersion").and_then(Value::as_u64).map_or(8, |v| v as u32)
}

pub fn choose_java(selected: &str, required: u32) -> Result<JavaInfo> {
    if !selected.is_empty() && selected != "auto" {
        let info = java::probe_exe(Path::new(selected)).ok_or_else(|| anyhow!("选择的 Java 不存在或不能用：{selected}"))?;
        if info.major < required {
            bail!("这个版本需要 Java {required} 或更高，当前选的是 Java {}，请在版本设置里换一个", info.major);
        }
        return Ok(info);
    }
    let all = java::find_all();
    let j = java::pick(&all, Some(required))
        .cloned()
        .ok_or_else(|| anyhow!("电脑上没找到 Java，这个版本需要 Java {required}"))?;
    if j.major < required {
        bail!("这个版本需要 Java {required}，但电脑上最高只有 Java {}，请先安装 Java {required}", j.major);
    }
    Ok(j)
}

/* ---------------- 依赖库 / natives ---------------- */

fn natives_classifier(lib: &Value) -> Option<String> {
    let c = lib.pointer(&format!("/natives/{OS_NAME}"))?.as_str()?;
    Some(c.replace("${arch}", if cfg!(target_pointer_width = "32") { "32" } else { "64" }))
}

/// 类路径上的依赖库（按顺序、去重）
fn classpath_libs(json: &Value, libdir: &Path) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for lib in json["libraries"].as_array().into_iter().flatten() {
        if !json::rules_allow(lib.get("rules")) || lib.get("clientreq").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let art = lib.pointer("/downloads/artifact");
        // 旧版只有 natives 的库（比如 lwjgl-platform）不放进类路径
        if lib.get("natives").is_some() && art.is_none() {
            continue;
        }
        let name = lib["name"].as_str().unwrap_or_default();
        let rel = art.and_then(|a| a["path"].as_str()).map(str::to_string).or_else(|| maven::path(name));
        if let Some(rel) = rel {
            let p = libdir.join(rel);
            if seen.insert(p.to_string_lossy().to_lowercase()) {
                out.push(p);
            }
        }
    }
    out
}

/// 旧版本（1.19 以前）的 natives 要解压出来；新版本 LWJGL 会自己从 jar 里取
fn extract_natives(json: &Value, libdir: &Path, dest: &Path) -> Result<usize> {
    let mut n = 0;
    for lib in json["libraries"].as_array().into_iter().flatten() {
        if !json::rules_allow(lib.get("rules")) {
            continue;
        }
        let Some(classifier) = natives_classifier(lib) else { continue };
        let name = lib["name"].as_str().unwrap_or_default();
        let rel = lib
            .pointer(&format!("/downloads/classifiers/{classifier}/path"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| maven::path(&format!("{name}:{classifier}")));
        let Some(rel) = rel else { continue };
        let jar = libdir.join(rel);
        let excludes: Vec<String> = lib
            .pointer("/extract/exclude")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let mut zip = zipx::open(&jar)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let entry_name = entry.name()?.to_string();
            if entry.is_dir() || entry_name.starts_with("META-INF/") || excludes.iter().any(|e| entry_name.starts_with(e.as_str())) {
                continue;
            }
            let out = zipx::safe_join(dest, &entry_name)?;
            if std::fs::metadata(&out).map(|m| m.len() == entry.size()).unwrap_or(false) {
                continue;
            }
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut f = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut f)?;
            n += 1;
        }
    }
    Ok(n)
}

/* ---------------- 资源文件 ---------------- */

/// 补全资源文件；很老的版本还要把资源复制成按名字存放的形式。返回 ${game_assets} 的路径
async fn complete_assets(ctx: &TaskCtx, json: &Value, assets: &Path, run_dir: &Path, src: Source, threads: usize) -> Result<PathBuf> {
    let index_id = json.pointer("/assetIndex/id").or_else(|| json.get("assets")).and_then(Value::as_str).unwrap_or("legacy");
    let index_path = assets.join("indexes").join(format!("{index_id}.json"));
    if !index_path.is_file() {
        let ai = json.get("assetIndex").ok_or_else(|| anyhow!("资源索引 {index_id} 不存在，版本 JSON 里也没有下载地址"))?;
        let mut item = DlItem::new(ai["url"].as_str().unwrap_or_default(), src, index_path.clone(), ai["sha1"].as_str(), ai["size"].as_u64());
        item.verify_existing = false;
        net::download_one(&item, ctx).await.context("下载资源索引失败")?;
    }
    let index: Value = serde_json::from_slice(&std::fs::read(&index_path)?).context("资源索引损坏")?;
    let objects = index["objects"].as_object().cloned().unwrap_or_default();

    let mut seen = HashSet::new();
    let items: Vec<DlItem> = objects
        .values()
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
        .collect();
    net::download_all(items, threads, ctx).await.context("补全资源文件失败")?;

    // 1.7.2 以前：资源要按原文件名放
    let virtual_ = index["virtual"].as_bool() == Some(true);
    let to_resources = index["map_to_resources"].as_bool() == Some(true);
    let target = if to_resources {
        run_dir.join("resources")
    } else if virtual_ {
        assets.join("virtual").join(index_id)
    } else {
        return Ok(assets.to_path_buf());
    };
    for (key, o) in &objects {
        let Some(hash) = o["hash"].as_str().filter(|h| h.len() >= 2) else { continue };
        let Ok(dest) = zipx::safe_join(&target, key) else { continue };
        if dest.is_file() {
            continue;
        }
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let _ = std::fs::copy(assets.join("objects").join(&hash[..2]).join(hash), &dest);
    }
    Ok(target)
}

/* ---------------- 启动参数 ---------------- */

/// 离线账户的 UUID：和官方服务端一致，md5("OfflinePlayer:名字") 转成 v3 UUID
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest, Md5};
    let mut h: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// 把 arguments.game / arguments.jvm 里的字符串和带规则的条目展开
fn collect_args(list: Option<&Value>, feature: &dyn Fn(&str) -> bool, out: &mut Vec<String>) {
    for a in list.and_then(Value::as_array).into_iter().flatten() {
        match a {
            Value::String(s) => out.push(s.clone()),
            Value::Object(_) => {
                if !json::rules_allow_with(a.get("rules"), feature) {
                    continue;
                }
                match &a["value"] {
                    Value::String(s) => out.push(s.clone()),
                    Value::Array(v) => out.extend(v.iter().filter_map(|x| x.as_str().map(str::to_string))),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    if !arg.contains("${") {
        return arg.to_string();
    }
    let mut s = arg.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("${{{k}}}"), v);
    }
    s
}

/// 按空格拆分用户填的 JVM 参数，支持用双引号包住带空格的值
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut has = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

/* ---------------- 准备 ---------------- */

pub async fn prepare(ctx: &TaskCtx, req: &LaunchRequest) -> Result<Prepared> {
    let game_dir = PathBuf::from(&req.game_dir);
    let libdir = game_dir.join("libraries");
    let assets = game_dir.join("assets");
    let src = req.source;
    installed::check_plain_name(&req.name)?;
    let version_dir = game_dir.join("versions").join(&req.name);
    let run_dir = installed::run_dir(&game_dir, &req.name)?;

    // 1. 版本信息和 Java
    ctx.stage("检查版本信息");
    let (json, client_jar) = resolve_version(&game_dir, &req.name)?;
    let main_class = json["mainClass"].as_str().ok_or_else(|| anyhow!("版本 JSON 里没有 mainClass"))?.to_string();
    let java = {
        let sel = req.java.clone();
        let need = required_java(&json);
        tauri::async_runtime::spawn_blocking(move || choose_java(&sel, need)).await??
    };

    // 2. 游戏本体和依赖库（只看文件在不在、大小对不对，启动要快）
    ctx.stage("补全游戏文件");
    ctx.check_cancel()?;
    let mut items: Vec<DlItem> = json::libraries_items(&json, &libdir, src);
    if let Some(c) = json.pointer("/downloads/client") {
        items.push(DlItem::new(c["url"].as_str().unwrap_or_default(), src, client_jar.clone(), c["sha1"].as_str(), c["size"].as_u64()));
    }
    let log_config = json.pointer("/logging/client").and_then(|lc| {
        let f = lc.get("file")?;
        let id = f["id"].as_str()?;
        let path = assets.join("log_configs").join(id);
        if let Some(url) = f["url"].as_str() {
            items.push(DlItem::new(url, src, path.clone(), f["sha1"].as_str(), f["size"].as_u64()));
        }
        Some((lc["argument"].as_str()?.to_string(), path))
    });
    for it in &mut items {
        it.verify_existing = false;
    }
    net::download_all(items, req.threads, ctx).await.context("补全游戏文件失败")?;
    if !client_jar.is_file() {
        bail!("找不到游戏本体 {}，可能需要重新安装这个版本", client_jar.display());
    }
    let mut cp = classpath_libs(&json, &libdir);
    if let Some(missing) = cp.iter().find(|p| !p.is_file()) {
        bail!("缺少依赖库 {}，可能需要重新安装这个版本", missing.display());
    }
    if !cp.iter().any(|p| p == &client_jar) {
        cp.push(client_jar.clone());
    }

    // 3. 资源文件
    ctx.stage("补全资源文件");
    ctx.check_cancel()?;
    std::fs::create_dir_all(&run_dir)?;
    let game_assets = complete_assets(ctx, &json, &assets, &run_dir, src, req.threads).await?;

    // 4. natives
    ctx.stage("解压 natives");
    ctx.check_cancel()?;
    let natives = version_dir.join("natives");
    std::fs::create_dir_all(&natives)?;
    {
        let (j, l, n) = (json.clone(), libdir.clone(), natives.clone());
        tauri::async_runtime::spawn_blocking(move || extract_natives(&j, &l, &n)).await?.context("解压 natives 失败")?;
    }

    // 5. 启动参数
    ctx.stage("启动 Java");
    ctx.check_cancel()?;
    let custom_res = req.window_mode == "720p";
    let feature = |f: &str| f == "has_custom_resolution" && custom_res;
    let index_name = json.pointer("/assetIndex/id").or_else(|| json.get("assets")).and_then(Value::as_str).unwrap_or("legacy");
    let online = req.access_token.is_some();
    let uuid = req.uuid.clone().unwrap_or_else(|| offline_uuid(&req.player));
    let token = req.access_token.clone().unwrap_or_else(|| uuid.clone());
    let s = |p: &Path| p.to_string_lossy().to_string();
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", req.player.clone()),
        ("version_name", req.name.clone()),
        ("game_directory", s(&run_dir)),
        ("assets_root", s(&assets)),
        ("game_assets", s(&game_assets)),
        ("assets_index_name", index_name.to_string()),
        ("auth_uuid", uuid),
        ("auth_access_token", token.clone()),
        ("auth_session", token),
        ("auth_xuid", "0".into()),
        ("clientid", "0".into()),
        ("user_type", if online { "msa" } else { "legacy" }.into()),
        ("user_properties", "{}".into()),
        ("version_type", "AMC".into()),
        ("launcher_name", "AMCLauncher".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("natives_directory", s(&natives)),
        ("library_directory", s(&libdir)),
        ("classpath_separator", CP_SEP.into()),
        ("classpath", cp.iter().map(|p| s(p)).collect::<Vec<_>>().join(CP_SEP)),
        ("primary_jar", s(&client_jar)),
        ("resolution_width", "1280".into()),
        ("resolution_height", "720".into()),
    ]);

    let mut jvm: Vec<String> = vec![
        format!("-Xmx{}m", req.memory.max(512)),
        "-XX:+UseG1GC".into(),
        "-XX:-UseAdaptiveSizePolicy".into(),
        "-XX:-OmitStackTraceInFastThrow".into(),
        "-Dfml.ignoreInvalidMinecraftCertificates=true".into(),
        "-Dfml.ignorePatchDiscrepancies=true".into(),
        "-Dlog4j2.formatMsgNoLookups=true".into(),
        "-Dstdout.encoding=UTF-8".into(),
        "-Dstderr.encoding=UTF-8".into(),
    ];
    // 官方的日志配置（老版本靠它修 Log4Shell 漏洞）
    if let Some((arg, path)) = &log_config {
        if path.is_file() {
            jvm.push(arg.replace("${path}", &s(path)));
        }
    }
    let mut game: Vec<String> = Vec::new();
    if json.pointer("/arguments/jvm").is_some() || json.pointer("/arguments/game").is_some() {
        collect_args(json.pointer("/arguments/jvm"), &feature, &mut jvm);
        collect_args(json.pointer("/arguments/game"), &feature, &mut game);
    } else {
        // 1.12 及以前的旧格式
        jvm.extend(["-Djava.library.path=${natives_directory}".into(), "-Dminecraft.launcher.brand=${launcher_name}".into(), "-Dminecraft.launcher.version=${launcher_version}".into()]);
        game.extend(json["minecraftArguments"].as_str().unwrap_or_default().split_whitespace().map(str::to_string));
        if custom_res {
            game.extend(["--width".into(), "1280".into(), "--height".into(), "720".into()]);
        }
    }
    if !jvm.iter().any(|a| a == "${classpath}") {
        jvm.extend(["-cp".into(), "${classpath}".into()]);
    }
    if req.window_mode == "fullscreen" {
        game.push("--fullscreen".into());
    }
    // 用户的 JVM 参数放在最后，可以覆盖前面的设置
    jvm.extend(split_args(&req.jvm_args));

    let mut args: Vec<String> = jvm.iter().map(|a| substitute(a, &vars)).collect();
    args.push(main_class);
    args.extend(game.iter().map(|a| substitute(a, &vars)));

    Ok(Prepared { java, args, run_dir, log_path: version_dir.join("amc-latest.log"), secret: req.access_token.clone() })
}

/* ---------------- 运行中的游戏 ---------------- */

pub struct Running {
    pub name: String,
    pub pid: u32,
    kill: Arc<Notify>,
}

/// 当前正在运行的游戏（一次只运行一个）
#[derive(Default)]
pub struct Games(pub Mutex<Option<Running>>);

impl Games {
    pub fn stop(&self) -> bool {
        match self.0.lock().unwrap().as_ref() {
            Some(r) => {
                r.kill.notify_one();
                true
            }
            None => false,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExitEvent {
    name: String,
    code: Option<i32>,
    killed: bool,
    crashed: bool,
    tail: Vec<String>,
    log: String,
}

struct LogSink {
    file: Option<std::fs::File>,
    tail: VecDeque<String>,
    ready: bool,
}

/// 游戏窗口出现前后会打印的几行，看到其中之一就认为启动成功
const READY_MARKERS: [&str; 5] = ["Backend library", "LWJGL Version", "LWJGL version", "OpenAL initialized", "Sound engine started"];

/// 老版本用 XML 格式输出日志，只保留里面的文字
fn clean_line(line: &str) -> Option<String> {
    let t = line.trim_end();
    if let Some(start) = t.find("<![CDATA[") {
        let rest = &t[start + 9..];
        return Some(rest.split("]]>").next().unwrap_or(rest).to_string());
    }
    let tt = t.trim_start();
    if tt.starts_with("<log4j:") || tt.starts_with("</log4j:") || tt.is_empty() {
        return None;
    }
    // 多行消息的最后一行带着 XML 结尾
    Some(t.split("]]>").next().unwrap_or(t).to_string())
}

async fn pump<R: AsyncRead + Unpin>(r: R, sink: Arc<Mutex<LogSink>>, app: AppHandle, name: String) {
    let mut reader = BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let raw = String::from_utf8_lossy(&buf);
        let mut s = sink.lock().unwrap();
        if let Some(f) = s.file.as_mut() {
            let _ = f.write_all(raw.as_bytes());
        }
        let Some(line) = clean_line(&raw) else { continue };
        if !s.ready && READY_MARKERS.iter().any(|m| line.contains(m)) {
            s.ready = true;
            let _ = app.emit("game-ready", &name);
        }
        s.tail.push_back(line);
        if s.tail.len() > 200 {
            s.tail.pop_front();
        }
    }
}

/// 启动 Java 进程；返回进程号。进程结束时推送 `game-exit` 事件
pub fn spawn(app: AppHandle, name: String, p: Prepared) -> Result<u32> {
    let mut cmd = tokio::process::Command::new(&p.java.path);
    cmd.args(&p.args)
        .current_dir(&p.run_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // 不弹黑色控制台窗口

    let mut file = std::fs::File::create(&p.log_path).ok();
    if let Some(f) = file.as_mut() {
        let mut cmdline = p.args.join(" ");
        if let Some(t) = p.secret.as_deref().filter(|t| !t.is_empty()) {
            cmdline = cmdline.replace(t, "******");
        }
        let _ = writeln!(f, "[AMC] Java {} ({})\n[AMC] {} {}\n", p.java.version, p.java.path, p.java.path, cmdline);
    }
    let mut child = cmd.spawn().with_context(|| format!("无法启动 Java：{}", p.java.path))?;
    let pid = child.id().unwrap_or(0);
    let kill = Arc::new(Notify::new());
    *app.state::<Games>().0.lock().unwrap() = Some(Running { name: name.clone(), pid, kill: Arc::clone(&kill) });

    let sink = Arc::new(Mutex::new(LogSink { file, tail: VecDeque::new(), ready: false }));
    let out = child.stdout.take().map(|o| tauri::async_runtime::spawn(pump(o, Arc::clone(&sink), app.clone(), name.clone())));
    let err = child.stderr.take().map(|e| tauri::async_runtime::spawn(pump(e, Arc::clone(&sink), app.clone(), name.clone())));
    let log = p.log_path.to_string_lossy().to_string();

    tauri::async_runtime::spawn(async move {
        let mut killed = false;
        let status = tokio::select! {
            s = child.wait() => s,
            _ = kill.notified() => {
                killed = true;
                let _ = child.kill().await;
                child.wait().await
            }
        };
        for h in [out, err].into_iter().flatten() {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), h).await;
        }
        let code = status.ok().and_then(|s| s.code());
        app.state::<Games>().0.lock().unwrap().take();
        let tail: Vec<String> = sink.lock().unwrap().tail.iter().rev().take(60).rev().cloned().collect();
        let _ = app.emit(
            "game-exit",
            ExitEvent { crashed: !killed && code != Some(0), name, code, killed, tail, log },
        );
    });
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        // 和服务端 UUID.nameUUIDFromBytes("OfflinePlayer:Notch") 的结果一致
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }

    #[test]
    fn split_user_args() {
        assert_eq!(split_args(r#"-Xss2m  -Dfoo="a b" "-Dbar=c d""#), ["-Xss2m", "-Dfoo=a b", "-Dbar=c d"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn xml_log_lines() {
        assert_eq!(clean_line("<log4j:Message><![CDATA[Setting user: Steve]]></log4j:Message>\n").as_deref(), Some("Setting user: Steve"));
        assert_eq!(clean_line("<log4j:Event logger=\"x\">"), None);
        assert_eq!(clean_line("\t- minecraft 26.2]]></log4j:Message>").as_deref(), Some("\t- minecraft 26.2"));
        assert_eq!(clean_line("[12:00:00] [main/INFO]: hi\r\n").as_deref(), Some("[12:00:00] [main/INFO]: hi"));
    }

    #[test]
    fn args_with_rules() {
        let list = serde_json::json!([
            "--username", "${auth_player_name}",
            { "rules": [{ "action": "allow", "features": { "is_demo_user": true } }], "value": "--demo" },
            { "rules": [{ "action": "allow", "features": { "has_custom_resolution": true } }], "value": ["--width", "${resolution_width}"] }
        ]);
        let mut out = Vec::new();
        collect_args(Some(&list), &|f| f == "has_custom_resolution", &mut out);
        assert_eq!(out, ["--username", "${auth_player_name}", "--width", "${resolution_width}"]);
        let vars = HashMap::from([("auth_player_name", "Steve".to_string())]);
        assert_eq!(substitute("${auth_player_name}", &vars), "Steve");
    }

    /// 真实启动一次本机已安装的版本（需要手动运行：cargo test -- --ignored launch_real）
    #[test]
    #[ignore]
    fn launch_real_dry() {
        let game_dir = PathBuf::from(std::env::var("APPDATA").unwrap()).join(".minecraft");
        let name = installed::list(&game_dir).unwrap().first().expect("没有已安装的版本").name.clone();
        let (json, jar) = resolve_version(&game_dir, &name).unwrap();
        assert!(jar.is_file());
        assert!(classpath_libs(&json, &game_dir.join("libraries")).iter().all(|p| p.is_file()));
    }
}
