//! 不开界面直接测试安装流程：
//! cargo run --example install -- <游戏目录> <版本名> <MC版本> [fabric|quilt|forge|neoforge|-] [optifine]
//! 加载器版本自动选最新的稳定版。

use imcl_lib::game::{self, meta, LoaderReq, OptiFineReq};
use imcl_lib::net::Source;
use imcl_lib::task::TaskCtx;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (game_dir, name, mc) = (args[0].clone(), args[1].clone(), args[2].clone());
    let src = Source::Bmcl;

    let loader = match args.get(3).map(String::as_str) {
        Some("fabric") => Some(meta::LoaderKind::Fabric),
        Some("quilt") => Some(meta::LoaderKind::Quilt),
        Some("forge") => Some(meta::LoaderKind::Forge),
        Some("neoforge") => Some(meta::LoaderKind::NeoForge),
        _ => None,
    };
    let loader = match loader {
        Some(kind) => {
            let list = meta::loader_versions(kind, &mc, src).await?;
            let pick = list.iter().find(|v| v.stable).or(list.first()).expect("没有可用的加载器版本").clone();
            println!("加载器 {} {}（共 {} 个版本）", kind.label(), pick.id, list.len());
            Some(LoaderReq { kind, id: pick.id })
        }
        None => None,
    };
    let optifine = if args.get(4).map(String::as_str) == Some("optifine") {
        let list = meta::optifine_versions(&mc).await?;
        let pick = list.iter().find(|v| !v.preview).or(list.first()).expect("没有 OptiFine").clone();
        println!("OptiFine {} {}（要求 {:?}）", pick.kind, pick.patch, pick.forge);
        Some(OptiFineReq { kind: pick.kind, patch: pick.patch })
    } else {
        None
    };

    let req = game::InstallRequest {
        task_id: format!("cli-{}", std::process::id()),
        game_dir,
        name,
        mc,
        loader,
        optifine,
        source: src,
        threads: 64,
    };
    let ctx = TaskCtx::new(req.task_id.clone(), Arc::new(AtomicBool::new(false)), 6);
    let t0 = Instant::now();

    let progress = {
        let ctx = Arc::clone(&ctx);
        tokio::spawn(async move {
            let mut last = String::new();
            loop {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let (stage, i, n) = ctx.current_stage();
                let line = format!(
                    "[{i}/{n}] {stage} {}/{}  已下载 {:.1} MB",
                    ctx.done.load(Ordering::Relaxed),
                    ctx.total.load(Ordering::Relaxed),
                    ctx.bytes.load(Ordering::Relaxed) as f64 / 1048576.0
                );
                if line != last {
                    println!("{line}");
                    last = line;
                }
            }
        })
    };

    let res = game::install(&ctx, &req).await;
    progress.abort();
    match res {
        Ok(()) => println!("完成，用时 {:.1}s，共下载 {:.1} MB", t0.elapsed().as_secs_f64(), ctx.bytes.load(Ordering::Relaxed) as f64 / 1048576.0),
        Err(e) => println!("失败：{e:#}"),
    }
    Ok(())
}
