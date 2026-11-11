//! 暴露给前端的 Tauri 命令。错误统一转成中文字符串，前端直接显示。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

use crate::game::{self, installed, launch, meta};
use crate::java::{self, JavaInfo};
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
    req: launch::LaunchRequest,
) -> CmdResult<u32> {
    if let Some(r) = games.0.lock().unwrap().as_ref() {
        return Err(format!("{} 正在运行，先关掉它再启动", r.name));
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
