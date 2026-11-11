//! OptiFine 安装。
//! - 搭配 Forge：把 OptiFine 放进该版本的 mods 文件夹，作为 Forge 模组加载。
//! - 搭配原版：用 OptiFine 自带的 Patcher 生成补丁 jar，再通过 LaunchWrapper 加载（和 HMCL 的做法一致）。

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use super::meta::BMCL;
use super::{json as vjson, zipx, InstallEnv};
use crate::net::{self, DlItem, Source};
use crate::{maven, task::TaskCtx};

#[derive(Clone, Debug, Deserialize)]
pub struct OptiFineReq {
    #[serde(rename = "type")]
    pub kind: String,
    pub patch: String,
}

impl OptiFineReq {
    pub fn label(&self) -> String {
        format!("{} {}", self.kind, self.patch)
    }
}

pub async fn install(
    ctx: &TaskCtx,
    env: &InstallEnv<'_>,
    of: &OptiFineReq,
    with_forge: bool,
    final_json: &mut Value,
) -> Result<()> {
    let mc = env.mc;
    let ver = format!("{mc}_{}_{}", of.kind, of.patch);
    let url = format!("{BMCL}/optifine/{mc}/{}/{}", of.kind, of.patch);
    let file = env.tmp.join(format!("OptiFine_{ver}.jar"));
    net::download_one(&DlItem::new(&url, Source::Official, file.clone(), None, None), ctx)
        .await
        .context("下载 OptiFine 失败")?;
    let mut zip = zipx::open(&file).context("下载的 OptiFine 文件不完整")?;

    if with_forge {
        let mods = env.version_dir.join("mods");
        std::fs::create_dir_all(&mods)?;
        std::fs::copy(&file, mods.join(format!("OptiFine_{ver}.jar")))?;
        return Ok(());
    }

    // 1. LaunchWrapper：新版 OptiFine 自带改过的 launchwrapper-of
    let launchwrapper = if zipx::has(&zip, "launchwrapper-of.txt") {
        let v = zipx::read_string(&mut zip, "launchwrapper-of.txt")?.trim().to_string();
        let coord = format!("optifine:launchwrapper-of:{v}");
        zipx::extract_to(&mut zip, &format!("launchwrapper-of-{v}.jar"), &env.libdir.join(maven::path(&coord).unwrap()))?;
        json!({ "name": coord })
    } else if zipx::has(&zip, "launchwrapper-2.0.jar") {
        let coord = "optifine:launchwrapper:2.0";
        zipx::extract_to(&mut zip, "launchwrapper-2.0.jar", &env.libdir.join(maven::path(coord).unwrap()))?;
        json!({ "name": coord })
    } else {
        let lib = json!({ "name": "net.minecraft:launchwrapper:1.12", "url": "https://libraries.minecraft.net/" });
        net::download_all(vjson::library_items(&lib, env.libdir, env.src), env.threads, ctx).await?;
        lib
    };

    // 2. OptiFine 本体：有 Patcher 就生成补丁 jar，否则直接使用
    let of_coord = format!("optifine:OptiFine:{ver}");
    let of_path = env.libdir.join(maven::path(&of_coord).unwrap());
    if let Some(dir) = of_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if zipx::has(&zip, "optifine/Patcher.class") {
        let java = env.java.ok_or_else(|| anyhow!("安装 OptiFine 需要 Java"))?;
        let mut cmd = tokio::process::Command::new(&java.path);
        cmd.arg("-cp")
            .arg(&file)
            .arg("optifine.Patcher")
            .arg(env.client_jar)
            .arg(&file)
            .arg(&of_path)
            .current_dir(env.tmp)
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let out = cmd.output().await.context("无法运行 OptiFine Patcher")?;
        if !out.status.success() || !of_path.is_file() {
            let err = String::from_utf8_lossy(&out.stderr);
            bail!("OptiFine Patcher 运行失败：{}", err.lines().last().unwrap_or("未知错误"));
        }
    } else {
        std::fs::copy(&file, &of_path)?;
    }

    // 3. 改写版本 JSON：用 LaunchWrapper 启动并加载 OptiFine 的 Tweaker
    let mut child = json!({
        "mainClass": "net.minecraft.launchwrapper.Launch",
        "libraries": [{ "name": of_coord }, launchwrapper],
    });
    if final_json.get("arguments").is_some() {
        child["arguments"] = json!({ "game": ["--tweakClass", "optifine.OptiFineTweaker"], "jvm": [] });
    } else {
        let old = final_json["minecraftArguments"].as_str().unwrap_or_default();
        child["minecraftArguments"] = json!(format!("{old} --tweakClass optifine.OptiFineTweaker").trim());
    }
    let id = final_json["id"].as_str().unwrap_or_default().to_string();
    *final_json = vjson::merge(final_json, &child, &id);
    Ok(())
}
