//! 版本 JSON 的通用处理：规则判断、依赖库下载项、继承合并。

use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::Path;

use crate::maven;
use crate::net::{DlItem, Source};

#[cfg(windows)]
pub const OS_NAME: &str = "windows";
#[cfg(target_os = "macos")]
pub const OS_NAME: &str = "osx";
#[cfg(all(not(windows), not(target_os = "macos")))]
pub const OS_NAME: &str = "linux";

/// 规则里的 "x86" 指 32 位
const IS_32BIT: bool = cfg!(target_pointer_width = "32");

/// 判断一组 rules 在当前系统上是否允许
pub fn rules_allow(rules: Option<&Value>) -> bool {
    let Some(arr) = rules.and_then(Value::as_array) else { return true };
    let mut allow = false;
    for r in arr {
        // 带 features 的规则（演示模式、自定义分辨率等）这里一律当作不满足
        if r.get("features").is_some() {
            continue;
        }
        if let Some(os) = r.get("os") {
            let name_ok = os.get("name").and_then(Value::as_str).map_or(true, |n| n == OS_NAME);
            let arch_ok = os.get("arch").and_then(Value::as_str).map_or(true, |a| (a == "x86") == IS_32BIT);
            if !(name_ok && arch_ok) {
                continue;
            }
        }
        allow = r.get("action").and_then(Value::as_str) == Some("allow");
    }
    allow
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// 一个依赖库需要下载的文件（主文件 + 旧版本的 natives）
pub fn library_items(lib: &Value, libdir: &Path, src: Source) -> Vec<DlItem> {
    let mut out = Vec::new();
    if !rules_allow(lib.get("rules")) {
        return out;
    }
    // 旧版 Forge：clientreq=false 的只给服务端用
    if lib.get("clientreq").and_then(Value::as_bool) == Some(false) {
        return out;
    }
    let name = str_of(lib, "name").unwrap_or_default();

    if let Some(art) = lib.pointer("/downloads/artifact") {
        let url = str_of(art, "url").unwrap_or_default();
        // url 为空的是安装器处理器生成的文件，不需要下载
        if !url.is_empty() {
            if let Some(rel) = str_of(art, "path").map(str::to_string).or_else(|| maven::path(name)) {
                out.push(DlItem::new(
                    url,
                    src,
                    libdir.join(&rel),
                    str_of(art, "sha1"),
                    art.get("size").and_then(Value::as_u64),
                ));
            }
        }
    } else if lib.get("downloads").is_none() && lib.get("natives").is_none() {
        // 没有 downloads 字段：按 maven 坐标拼地址（Fabric、Quilt、旧版 Forge）
        if let Some(rel) = maven::path(name) {
            let mut base = str_of(lib, "url").unwrap_or("https://libraries.minecraft.net/").to_string();
            if !base.ends_with('/') {
                base.push('/');
            }
            out.push(DlItem::new(
                &format!("{base}{rel}"),
                src,
                libdir.join(&rel),
                str_of(lib, "sha1"),
                lib.get("size").and_then(Value::as_u64),
            ));
        }
    }

    // 旧版 natives：natives.windows = "natives-windows-${arch}"
    if let Some(classifier) = lib.pointer(&format!("/natives/{OS_NAME}")).and_then(Value::as_str) {
        let classifier = classifier.replace("${arch}", if IS_32BIT { "32" } else { "64" });
        if let Some(art) = lib.pointer(&format!("/downloads/classifiers/{classifier}")) {
            if let (Some(url), Some(rel)) = (str_of(art, "url"), str_of(art, "path")) {
                out.push(DlItem::new(url, src, libdir.join(rel), str_of(art, "sha1"), art.get("size").and_then(Value::as_u64)));
            }
        }
    }
    out
}

pub fn libraries_items(json: &Value, libdir: &Path, src: Source) -> Vec<DlItem> {
    json.get("libraries")
        .and_then(Value::as_array)
        .map(|libs| libs.iter().flat_map(|l| library_items(l, libdir, src)).collect())
        .unwrap_or_default()
}

/// 把子版本（加载器）合并进父版本（原版），得到一个不依赖 inheritsFrom 的完整版本 JSON。
/// 子版本的依赖库排在前面，同名库以子版本为准。
pub fn merge(parent: &Value, child: &Value, id: &str) -> Value {
    let mut out: Map<String, Value> = parent.as_object().cloned().unwrap_or_default();

    if let Some(c) = child.as_object() {
        for (k, v) in c {
            match k.as_str() {
                "id" | "inheritsFrom" | "jar" | "libraries" | "arguments" => {}
                _ => {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
    }

    // 依赖库：子在前，按 group:artifact(:classifier) 去重
    let mut seen = HashSet::new();
    let mut libs = Vec::new();
    for src in [child, parent] {
        if let Some(arr) = src.get("libraries").and_then(Value::as_array) {
            for l in arr {
                let key = str_of(l, "name").map(maven::dedupe_key).unwrap_or_default();
                if key.is_empty() || seen.insert(key) {
                    libs.push(l.clone());
                }
            }
        }
    }
    out.insert("libraries".into(), Value::Array(libs));

    // 新版启动参数：父 + 子
    if parent.get("arguments").is_some() || child.get("arguments").is_some() {
        let mut args = Map::new();
        for kind in ["game", "jvm"] {
            let mut v = Vec::new();
            for src in [parent, child] {
                if let Some(a) = src.pointer(&format!("/arguments/{kind}")).and_then(Value::as_array) {
                    v.extend(a.iter().cloned());
                }
            }
            args.insert(kind.into(), Value::Array(v));
        }
        out.insert("arguments".into(), Value::Object(args));
    }

    out.insert("id".into(), json!(id));
    out.remove("inheritsFrom");
    out.remove("jar");
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        assert!(rules_allow(None));
        let only_osx = json!([{ "action": "allow", "os": { "name": "osx" } }]);
        assert_eq!(rules_allow(Some(&only_osx)), OS_NAME == "osx");
        let not_osx = json!([{ "action": "allow" }, { "action": "disallow", "os": { "name": "osx" } }]);
        assert_eq!(rules_allow(Some(&not_osx)), OS_NAME != "osx");
        let feature = json!([{ "action": "allow", "features": { "is_demo_user": true } }]);
        assert!(!rules_allow(Some(&feature)));
    }

    #[test]
    fn merge_child_first_and_dedupe() {
        let parent = json!({
            "id": "1.20.1", "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{ "name": "org.ow2.asm:asm:9.1" }, { "name": "com.mojang:brigadier:1.1.8" }],
            "arguments": { "game": ["--username"], "jvm": ["-Xss1M"] }
        });
        let child = json!({
            "id": "fabric", "inheritsFrom": "1.20.1", "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{ "name": "org.ow2.asm:asm:9.6" }, { "name": "net.fabricmc:fabric-loader:0.16.0" }],
            "arguments": { "game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "] }
        });
        let m = merge(&parent, &child, "1.20.1-Fabric");
        assert_eq!(m["id"], "1.20.1-Fabric");
        assert_eq!(m["mainClass"], "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert!(m.get("inheritsFrom").is_none());
        let names: Vec<_> = m["libraries"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["org.ow2.asm:asm:9.6", "net.fabricmc:fabric-loader:0.16.0", "com.mojang:brigadier:1.1.8"]);
        assert_eq!(m["arguments"]["jvm"].as_array().unwrap().len(), 2);
    }
}
