//! 暴露给前端的 Tauri 命令。错误统一转成中文字符串，前端直接显示。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};

use crate::auth;
use crate::game::{self, installed, launch, meta};
use crate::modpack;
use crate::modpack_export;
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

/// 读写文件这类会花点时间的事放到后台线程做。
/// 不带 async 的命令跑在窗口的主线程上：启动时扫版本、读背景图的那一下，整个窗口都会卡住
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f).await.expect("后台任务出错")
}

/// 看看本地整合包里是什么（导入前）
#[tauri::command]
pub async fn read_modpack(path: String) -> CmdResult<modpack::PackInfo> {
    blocking(move || {
        modpack::read_pack(&PathBuf::from(path)).map_err(err)
    })
    .await
}

/// 导出整合包时可以选的文件和文件夹
#[tauri::command]
pub async fn export_entries(game_dir: String, name: String) -> CmdResult<Vec<modpack_export::Entry>> {
    tauri::async_runtime::spawn_blocking(move || modpack_export::entries(&PathBuf::from(game_dir), &name))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// 把版本导出成 .mrpack
#[tauri::command]
pub async fn export_modpack(req: modpack_export::ExportReq) -> CmdResult<modpack_export::ExportResult> {
    modpack_export::export(&req).await.map_err(err)
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

/// 给已有版本换加载器，进度走 `install-progress`
#[tauri::command]
pub async fn change_loader(app: AppHandle, installs: State<'_, Installs>, req: game::ChangeLoaderRequest) -> CmdResult<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    installs.0.lock().unwrap().insert(req.task_id.clone(), Arc::clone(&cancel));
    let ctx = TaskCtx::new(req.task_id.clone(), cancel, 4 + u32::from(req.loader.is_some()));
    ctx.spawn_reporter(app, "install-progress");
    let result = game::change_loader(&ctx, &req).await;
    ctx.finish();
    installs.0.lock().unwrap().remove(&req.task_id);
    result.map_err(err)
}

/// 检查版本里 Mod / 光影 / 材质包的更新
#[tauri::command]
pub async fn modrinth_updates(game_dir: String, name: String, kind: String, mc: String, loader: Option<String>) -> CmdResult<Vec<modrinth::Update>> {
    modrinth::updates(&PathBuf::from(game_dir), &name, &kind, &mc, loader.as_deref()).await.map_err(err)
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
    } else if let Some(skin) = req.offline_skin.clone() {
        // 离线账户：开本机皮肤服务器 + authlib-injector；失败了就不带皮肤照常启动
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let uuid = launch::offline_uuid(&req.player);
        match crate::ygg::prepare(&dir, &req.player, &uuid, &skin).await {
            Ok(args) => req.agent_args = args,
            Err(e) => eprintln!("[skin] 离线皮肤没能加载，按默认皮肤启动：{e:#}"),
        }
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

/// 首页显示系统状态时打开采样，离开时关掉
#[tauri::command]
pub fn sysmon_watch(on: bool) {
    crate::sysmon::watch(on);
}

/* ---------- 外观 ---------- */

fn image_mime(ext: &str) -> Option<&'static str> {
    Some(match ext.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => return None,
    })
}

fn background_files(app: &AppHandle) -> Vec<PathBuf> {
    let Ok(dir) = app.path().app_data_dir() else { return vec![] };
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_stem().is_some_and(|s| s == "background")).collect())
        .unwrap_or_default()
}

/// 把选的图片复制到应用数据目录当背景（原图删了也不影响），返回 data URL
#[tauri::command]
pub async fn set_background_image(app: AppHandle, path: String) -> CmdResult<String> {
    blocking(move || {
        let src = PathBuf::from(&path);
        let ext = src.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        let mime = image_mime(&ext).ok_or("只支持 PNG、JPG、WebP、BMP、GIF 图片")?;
        let bytes = std::fs::read(&src).map_err(|e| format!("读不了这张图片：{e}"))?;
        if bytes.len() > 30 * 1024 * 1024 {
            return Err("图片太大了（超过 30 MB）".into());
        }
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for old in background_files(&app) {
            let _ = std::fs::remove_file(old);
        }
        std::fs::write(dir.join(format!("background.{}", ext.to_lowercase())), &bytes).map_err(|e| e.to_string())?;
        Ok(skin::data_url_of(&bytes, mime))
    })
    .await
}

/// 之前设的背景图
#[tauri::command]
pub async fn background_image(app: AppHandle) -> Option<String> {
    blocking(move || {
        background_files(&app).into_iter().find_map(|p| {
            let mime = image_mime(&p.extension()?.to_string_lossy())?;
            Some(skin::data_url_of(&std::fs::read(&p).ok()?, mime))
        })
    })
    .await
}

/// 内存分配：总内存、手动上限、自动会给多少（version 为空按原版算）
#[tauri::command]
pub async fn memory_plan(game_dir: String, version: Option<String>) -> crate::memory::MemoryPlan {
    blocking(move || {
        crate::memory::plan(std::path::Path::new(&game_dir), version.as_deref())
    })
    .await
}

/// 打开 / 关闭云母（直接调 DWM），返回系统读回的背景类型：2 = 云母，1 = 无
#[tauri::command]
pub fn set_backdrop(window: tauri::Window, mica: bool) -> Result<Option<i32>, String> {
    crate::backdrop::set(&window, mica)
}

/// 云母效果只有 Win11（内部版本 22000 起）有
#[tauri::command]
pub fn is_win11() -> bool {
    cfg!(windows)
        && sysinfo::System::kernel_version()
            .ok()
            .and_then(|v| v.split('.').next_back().and_then(|b| b.trim().parse::<u32>().ok()))
            .is_some_and(|b| b >= 22000)
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
pub async fn ms_accounts(app: AppHandle) -> Vec<auth::AccountInfo> {
    blocking(move || {
        auth::list(&app)
    })
    .await
}

#[tauri::command]
pub async fn ms_remove(app: AppHandle, id: String) -> CmdResult<()> {
    blocking(move || {
        auth::remove(&app, &id).map_err(err)
    })
    .await
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
pub async fn list_installed(game_dir: String) -> CmdResult<Vec<installed::InstalledVersion>> {
    blocking(move || {
        installed::list(&PathBuf::from(game_dir)).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn version_files(game_dir: String, name: String) -> CmdResult<installed::VersionFiles> {
    blocking(move || {
        installed::files(&PathBuf::from(game_dir), &name).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn set_mod_enabled(game_dir: String, name: String, file: String, enabled: bool) -> CmdResult<String> {
    blocking(move || {
        installed::set_mod_enabled(&PathBuf::from(game_dir), &name, &file, enabled).map_err(err)
    })
    .await
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
pub async fn inspect_game_dir(path: String) -> CmdResult<installed::GameDirInfo> {
    blocking(move || {
        installed::inspect_dir(&PathBuf::from(path)).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn version_settings(game_dir: String, name: String) -> CmdResult<installed::VersionSettings> {
    blocking(move || {
        installed::read_settings(&PathBuf::from(game_dir), &name).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn set_version_settings(game_dir: String, name: String, settings: installed::VersionSettings) -> CmdResult<()> {
    blocking(move || {
        installed::write_settings(&PathBuf::from(game_dir), &name, &settings).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn delete_version(game_dir: String, name: String) -> CmdResult<()> {
    blocking(move || installed::delete_version(&PathBuf::from(game_dir), &name).map_err(err)).await
}

#[tauri::command]
pub async fn trash_version_file(game_dir: String, name: String, kind: String, file: String) -> CmdResult<()> {
    blocking(move || {
        installed::trash_file(&PathBuf::from(game_dir), &name, &kind, &file).map_err(err)
    })
    .await
}

/* ---------------- 内置联机 Mod ---------------- */

#[tauri::command]
pub async fn online_mod_status(game_dir: String, name: String, mc: String, loader: String) -> CmdResult<crate::online_mod::Status> {
    blocking(move || crate::online_mod::status(&PathBuf::from(game_dir), &name, &mc, &loader).map_err(err)).await
}

#[tauri::command]
pub async fn install_online_mod(game_dir: String, name: String, mc: String, loader: String, addon: bool) -> CmdResult<crate::online_mod::Status> {
    blocking(move || crate::online_mod::install(&PathBuf::from(game_dir), &name, &mc, &loader, addon).map_err(err)).await
}
