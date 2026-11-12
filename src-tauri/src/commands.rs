//! 暴露给前端的 Tauri 命令。错误统一转成中文字符串，前端直接显示。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

use crate::auth;
use crate::game::{self, installed, launch, meta};
use crate::modpack;
use crate::modrinth;
use crate::skin;
use crate::java::{self, JavaInfo};
use crate::jre;
use crate::net::Source;
use crate::task::TaskCtx;

type CmdResult<T> = Result<T, String>;

fn err(e: anyhow::Error) -> String {
    // 只给用户看最外层 + 根本原因，避免一长串
    let root = e.root_cause().to_string();
    let top = e.to_string();
    if top == root {
        top
    } else {
        format!("{top}：{root}")
    }
}

/// 正在进行的安装任务：任务 id → 取消标记
#[derive(Default)]
pub struct Installs(Mutex<HashMap<String, Arc<AtomicBool>>>);

#[tauri::command]
pub fn default_game_dir() -> String {
    #[cfg(windows)]
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_default().join(".minecraft");
    #[cfg(target_os = "macos")]
    let base = std::env::var("HOME").map(PathBuf::from).unwrap_or_default().join("Library/Application Support/minecraft");
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let base = std::env::var("HOME").map(PathBuf::from).unwrap_or_default().join(".minecraft");
    base.to_string_lossy().to_string()
}

#[tauri::command]
pub async fn version_manifest(source: Source) -> CmdResult<Vec<meta::ManifestEntry>> {
    meta::manifest(source).await.map_err(err)
}

#[tauri::command]
pub async fn loader_versions(source: Source, loader: meta::LoaderKind, mc: String) -> CmdResult<Vec<meta::LoaderVersion>> {
    meta::loader_versions(loader, &mc, source).await.map_err(err)
}

#[tauri::command]
pub async fn optifine_versions(mc: String) -> CmdResult<Vec<meta::OptiFineVersion>> {
    meta::optifine_versions(&mc).await.map_err(err)
}

#[tauri::command]
pub async fn find_java() -> Vec<JavaInfo> {
    tauri::async_runtime::spawn_blocking(java::find_all).await.unwrap_or_default()
}

/// 安装 Modrinth 整合包，进度和普通安装一样走 `install-progress`
#[tauri::command]
pub async fn install_modpack(app: AppHandle, installs: State<'_, Installs>, req: modpack::ModpackReq) -> CmdResult<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    installs.0.lock().unwrap().insert(req.task_id.clone(), Arc::clone(&cancel));
    let ctx = TaskCtx::new(req.task_id.clone(), cancel, modpack::STAGES);
    ctx.spawn_reporter(app, "install-progress");
    let result = modpack::install(&ctx, &req).await;
    ctx.finish();
    installs.0.lock().unwrap().remove(&req.task_id);
    result.map_err(err)
}

/// 可以下载的 Java（设置页）
#[tauri::command]
pub async fn java_offers() -> Vec<jre::OfferState> {
    tauri::async_runtime::spawn_blocking(jre::offers).await.unwrap_or_default()
}

/// 下载一个 Java（自动测速选官方源或镜像），进度走 `install-progress`
#[tauri::command]
pub async fn download_java(
    app: AppHandle,
    installs: State<'_, Installs>,
    task_id: String,
    component: String,
    threads: usize,
) -> CmdResult<JavaInfo> {
    let cancel = Arc::new(AtomicBool::new(false));
    installs.0.lock().unwrap().insert(task_id.clone(), Arc::clone(&cancel));
    let ctx = TaskCtx::new(task_id.clone(), cancel, 2);
    ctx.spawn_reporter(app, "install-progress");
    let result = jre::install(&ctx, &component, threads).await;
    ctx.finish();
    installs.0.lock().unwrap().remove(&task_id);
    result.map_err(err)
}

#[tauri::command]
pub async fn install_version(app: AppHandle, installs: State<'_, Installs>, req: game::InstallRequest) -> CmdResult<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    installs.0.lock().unwrap().insert(req.task_id.clone(), Arc::clone(&cancel));

    let stages = 4 + u32::from(req.loader.is_some()) + u32::from(req.optifine.is_some());
    let ctx = TaskCtx::new(req.task_id.clone(), cancel, stages);
    ctx.spawn_reporter(app, "install-progress");

    let result = game::install(&ctx, &req).await;
    ctx.finish();
    installs.0.lock().unwrap().remove(&req.task_id);
    result.map_err(err)
}

/// 补全文件并启动游戏；Java 进程起来后返回进程号。之后的状态通过 `game-ready` / `game-exit` 事件推送
#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    installs: State<'_, Installs>,
    games: State<'_, launch::Games>,
    mut req: launch::LaunchRequest,
) -> CmdResult<u32> {
    if let Some(r) = games.0.lock().unwrap().as_ref() {
        return Err(format!("{} 正在运行，先关掉它再启动", r.name));
    }
    // 正版账户：先确认登录还有效（快过期会自动刷新）
    if let Some(id) = req.ms_account.clone() {
        let acc = auth::ensure(&app, &id).await.map_err(err)?;
        req.player = acc.name;
        req.uuid = Some(acc.uuid);
        req.access_token = Some(acc.access_token);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    installs.0.lock().unwrap().insert(req.task_id.clone(), Arc::clone(&cancel));
    let ctx = TaskCtx::new(req.task_id.clone(), cancel, launch::STAGES);
    ctx.spawn_reporter(app.clone(), "launch-progress");

    let prepared = launch::prepare(&ctx, &req).await;
    ctx.finish();
    installs.0.lock().unwrap().remove(&req.task_id);
    let prepared = prepared.map_err(err)?;
    if ctx.is_cancelled() {
        return Err("已取消".into());
    }
    launch::spawn(app, req.name.clone(), prepared).map_err(err)
}

/* ---------- Modrinth ---------- */

#[tauri::command]
pub async fn modrinth_search(req: modrinth::SearchReq) -> CmdResult<modrinth::SearchResult> {
    modrinth::search(&req).await.map_err(err)
}

#[tauri::command]
pub async fn modrinth_versions(req: modrinth::VersionsReq) -> CmdResult<Vec<modrinth::ModVersion>> {
    modrinth::versions(&req).await.map_err(err)
}

#[tauri::command]
pub async fn modrinth_deps(req: modrinth::DepsReq) -> CmdResult<Vec<modrinth::Dep>> {
    modrinth::deps(&req).await.map_err(err)
}

#[tauri::command]
pub async fn modrinth_install(req: modrinth::InstallReq) -> CmdResult<modrinth::InstallResult> {
    modrinth::install(&req).await.map_err(err)
}

#[tauri::command]
pub async fn modrinth_identify(game_dir: String, name: String) -> CmdResult<HashMap<String, modrinth::FileInfo>> {
    modrinth::identify(&PathBuf::from(game_dir), &name).await.map_err(err)
}

/* ---------- 皮肤 ---------- */

/// 游戏自带的 Steve / Alex 皮肤，从这些游戏目录里装好的客户端取（取到一次就缓存）
#[tauri::command]
pub async fn default_skins(app: AppHandle, dirs: Vec<String>) -> CmdResult<Option<skin::DefaultSkins>> {
    let dirs: Vec<PathBuf> = dirs.into_iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || skin::defaults(&app, &dirs)).await.map_err(|e| e.to_string())?.map_err(err)
}

/// 正版账户当前的皮肤（data URL）
#[tauri::command]
pub async fn ms_skin(app: AppHandle, id: String) -> CmdResult<Option<String>> {
    skin::ms_skin(&app, &id).await.map_err(err)
}

/* ---------- 微软账户 ---------- */

/// 设备码登录，完成后返回账户。等待期间会推送 `ms-device-code` 事件
#[tauri::command]
pub async fn ms_login(app: AppHandle, logins: State<'_, auth::Logins>, client_id: String) -> CmdResult<auth::AccountInfo> {
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(old) = logins.0.lock().unwrap().replace(Arc::clone(&cancel)) {
        old.store(true, Ordering::Relaxed);
    }
    let r = auth::login(&app, &client_id, cancel).await;
    logins.0.lock().unwrap().take();
    r.map_err(err)
}

#[tauri::command]
pub fn ms_login_cancel(logins: State<'_, auth::Logins>) {
    if let Some(c) = logins.0.lock().unwrap().take() {
        c.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn ms_accounts(app: AppHandle) -> Vec<auth::AccountInfo> {
    auth::list(&app)
}

#[tauri::command]
pub fn ms_remove(app: AppHandle, id: String) -> CmdResult<()> {
    auth::remove(&app, &id).map_err(err)
}

#[tauri::command]
pub fn stop_game(games: State<'_, launch::Games>) -> bool {
    games.stop()
}

#[tauri::command]
pub fn cancel_install(installs: State<'_, Installs>, task_id: String) {
    if let Some(flag) = installs.0.lock().unwrap().get(&task_id) {
        flag.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn list_installed(game_dir: String) -> CmdResult<Vec<installed::InstalledVersion>> {
    installed::list(&PathBuf::from(game_dir)).map_err(err)
}

#[tauri::command]
pub fn version_files(game_dir: String, name: String) -> CmdResult<installed::VersionFiles> {
    installed::files(&PathBuf::from(game_dir), &name).map_err(err)
}

#[tauri::command]
pub fn set_mod_enabled(game_dir: String, name: String, file: String, enabled: bool) -> CmdResult<String> {
    installed::set_mod_enabled(&PathBuf::from(game_dir), &name, &file, enabled).map_err(err)
}

#[tauri::command]
pub async fn probe_java(path: String) -> CmdResult<JavaInfo> {
    let p = path.clone();
    tauri::async_runtime::spawn_blocking(move || java::probe_exe(&PathBuf::from(p)))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| format!("这不是可用的 Java：{path}"))
}

#[tauri::command]
pub fn inspect_game_dir(path: String) -> CmdResult<installed::GameDirInfo> {
    installed::inspect_dir(&PathBuf::from(path)).map_err(err)
}

#[tauri::command]
pub fn version_settings(game_dir: String, name: String) -> CmdResult<installed::VersionSettings> {
    installed::read_settings(&PathBuf::from(game_dir), &name).map_err(err)
}

#[tauri::command]
pub fn set_version_settings(game_dir: String, name: String, settings: installed::VersionSettings) -> CmdResult<()> {
    installed::write_settings(&PathBuf::from(game_dir), &name, &settings).map_err(err)
}

#[tauri::command]
pub fn trash_version_file(game_dir: String, name: String, kind: String, file: String) -> CmdResult<()> {
    installed::trash_file(&PathBuf::from(game_dir), &name, &kind, &file).map_err(err)
}
