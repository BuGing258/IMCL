//! 不开界面直接测试启动流程：补全文件、拼参数、启动游戏，看到窗口出现后自动关掉。
//! cargo run --example launch -- <游戏目录> <版本名> [玩家名] [--keep]
//! 加 --keep 则不自动关闭，游戏退出后打印退出码。
//! 加 --skin <png> [--slim] 测试离线皮肤（authlib-injector + 本机皮肤服务器）。

use imcl_lib::game::launch::{self, LaunchRequest};
use imcl_lib::net::Source;
use imcl_lib::task::TaskCtx;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let keep = args.iter().any(|a| a == "--keep");
    let skin_path = args.iter().position(|a| a == "--skin").and_then(|i| args.get(i + 1)).cloned();
    let mut req = LaunchRequest {
        task_id: "cli".into(),
        game_dir: args[0].clone(),
        name: args[1].clone(),
        java: "auto".into(),
        memory: 4096,
        window_mode: "720p".into(),
        jvm_args: String::new(),
        player: args.get(2).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "IMCLTest".into()),
        source: Source::Bmcl,
        threads: 64,
        ms_account: None,
        offline_skin: None,
        agent_args: Vec::new(),
        uuid: None,
        access_token: None,
    };
    if let Some(path) = skin_path {
        use base64::Engine;
        let png = base64::engine::general_purpose::STANDARD.encode(std::fs::read(&path)?);
        let skin = imcl_lib::ygg::OfflineSkin { png, slim: args.iter().any(|a| a == "--slim") };
        let dir = std::env::temp_dir().join("imcl-skin-test");
        let uuid = launch::offline_uuid(&req.player);
        req.agent_args = imcl_lib::ygg::prepare(&dir, &req.player, &uuid, &skin).await?;
        println!("皮肤参数：{:?}", req.agent_args);
    }
    let ctx = TaskCtx::new("cli".into(), Arc::new(AtomicBool::new(false)), launch::STAGES);
    let t = Instant::now();
    let p = launch::prepare(&ctx, &req).await?;
    println!("准备完成 {:.1}s，Java {}（{}）", t.elapsed().as_secs_f32(), p.java.major, p.java.path);
    let main_idx = p.args.iter().position(|a| a.starts_with("net.") || a.contains(".client.") || a.contains("Main") || a.contains("Launch")).unwrap_or(0);
    println!("主类：{}", p.args.get(main_idx).cloned().unwrap_or_default());
    let masked: Vec<String> = p.args.iter().map(|a| if a.len() > 160 { format!("{}…({} 字)", &a[..60], a.len()) } else { a.clone() }).collect();
    println!("参数：{}", masked.join(" "));

    let mut child = Command::new(&p.java.path)
        .args(&p.args)
        .current_dir(&p.run_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let out = BufReader::new(child.stdout.take().unwrap());
    let started = Instant::now();
    for line in out.lines() {
        let line = line.unwrap_or_default();
        println!("  | {line}");
        let ready = ["Backend library", "LWJGL Version", "OpenAL initialized", "Sound engine started"].iter().any(|m| line.contains(m));
        if ready && !keep {
            println!("看到窗口就绪标记，用时 {:.1}s，关闭游戏", started.elapsed().as_secs_f32());
            // 等几秒，让游戏把自己的皮肤读完
            std::thread::sleep(Duration::from_secs(if req.agent_args.is_empty() { 3 } else { 12 }));
            child.kill()?;
            break;
        }
        if started.elapsed() > Duration::from_secs(180) {
            println!("超时，关闭游戏");
            child.kill()?;
            break;
        }
    }
    let status = child.wait()?;
    println!("退出：{status}");
    Ok(())
}
