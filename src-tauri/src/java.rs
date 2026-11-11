//! 查找电脑上已安装的 Java。

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    /// java(.exe) 的完整路径
    pub path: String,
    pub version: String,
    pub major: u32,
}

#[cfg(windows)]
const JAVA_EXE: &str = "java.exe";
#[cfg(not(windows))]
const JAVA_EXE: &str = "java";

/// 把 "1.8.0_392" / "17.0.2" / "21" 转成主版本号
pub fn parse_major(version: &str) -> Option<u32> {
    let mut it = version.split(|c: char| c == '.' || c == '_' || c == '-' || c == '+');
    let first: u32 = it.next()?.parse().ok()?;
    if first == 1 {
        it.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// 从 Java 安装目录（含 bin）读取版本
fn probe(home: &Path) -> Option<JavaInfo> {
    let exe = home.join("bin").join(JAVA_EXE);
    if !exe.is_file() {
        return None;
    }
    // 优先读 release 文件，不用启动进程
    let version = std::fs::read_to_string(home.join("release")).ok().and_then(|s| {
        s.lines()
            .find_map(|l| l.strip_prefix("JAVA_VERSION="))
            .map(|v| v.trim().trim_matches('"').to_string())
    })?;
    let major = parse_major(&version)?;
    Some(JavaInfo { path: exe.to_string_lossy().to_string(), version, major })
}

fn push_children(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                out.push(e.path());
            }
        }
    }
}

fn candidate_homes() -> Vec<PathBuf> {
    let mut homes = Vec::new();
    if let Ok(h) = std::env::var("JAVA_HOME") {
        homes.push(PathBuf::from(h));
    }
    if let Ok(path) = std::env::var("PATH") {
        for p in std::env::split_paths(&path) {
            if p.join(JAVA_EXE).is_file() {
                if let Some(home) = p.parent() {
                    homes.push(home.to_path_buf());
                }
            }
        }
    }
    #[cfg(windows)]
    {
        for pf in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Ok(base) = std::env::var(pf) {
                for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "BellSoft", "Amazon Corretto", "Semeru", "Eclipse Foundation"] {
                    push_children(&Path::new(&base).join(vendor), &mut homes);
                }
            }
        }
        // 官方启动器自带的 Java 运行时：.minecraft/runtime/<组件>/windows-x64/<组件>/
        if let Ok(appdata) = std::env::var("APPDATA") {
            let runtime = Path::new(&appdata).join(".minecraft").join("runtime");
            let mut comps = Vec::new();
            push_children(&runtime, &mut comps);
            for comp in comps {
                let mut plats = Vec::new();
                push_children(&comp, &mut plats);
                for plat in plats {
                    push_children(&plat, &mut homes);
                }
            }
        }
    }
    homes
}

/// 检查用户手动选的 java(w).exe
pub fn probe_exe(exe: &Path) -> Option<JavaInfo> {
    if !exe.is_file() {
        return None;
    }
    let bin = exe.parent()?;
    if let Some(mut info) = bin.parent().and_then(probe) {
        info.path = exe.to_string_lossy().to_string();
        return Some(info);
    }
    // 没有 release 文件：运行一次 java -version
    let java = bin.join(JAVA_EXE);
    let runner = if java.is_file() { java } else { exe.to_path_buf() };
    let mut cmd = std::process::Command::new(runner);
    cmd.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let version = text.split('"').nth(1)?.to_string();
    let major = parse_major(&version)?;
    Some(JavaInfo { path: exe.to_string_lossy().to_string(), version, major })
}

pub fn find_all() -> Vec<JavaInfo> {
    let mut seen = HashSet::new();
    let mut out: Vec<JavaInfo> = candidate_homes()
        .into_iter()
        .filter_map(|h| probe(&h))
        .filter(|j| seen.insert(j.path.to_lowercase()))
        .collect();
    out.sort_by(|a, b| b.major.cmp(&a.major));
    out
}

/// 为需要 `required` 版本的游戏挑一个 Java：优先完全匹配，其次最小的更高版本，最后用最高的
pub fn pick(javas: &[JavaInfo], required: Option<u32>) -> Option<&JavaInfo> {
    let Some(req) = required else { return javas.iter().max_by_key(|j| j.major) };
    javas
        .iter()
        .find(|j| j.major == req)
        .or_else(|| javas.iter().filter(|j| j.major > req).min_by_key(|j| j.major))
        .or_else(|| javas.iter().max_by_key(|j| j.major))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn majors() {
        assert_eq!(parse_major("1.8.0_392"), Some(8));
        assert_eq!(parse_major("17.0.2"), Some(17));
        assert_eq!(parse_major("21"), Some(21));
        assert_eq!(parse_major("25.0.3+9"), Some(25));
    }

    /// 本机有 Java 时，手动选它的 java.exe / javaw.exe 都能识别出同样的版本
    #[test]
    fn probe_selected_exe() {
        let Some(j) = find_all().into_iter().next() else { return };
        let exe = PathBuf::from(&j.path);
        assert_eq!(probe_exe(&exe).map(|x| x.major), Some(j.major));
        let javaw = exe.with_file_name(if cfg!(windows) { "javaw.exe" } else { "java" });
        if javaw.is_file() {
            assert_eq!(probe_exe(&javaw).map(|x| x.major), Some(j.major));
        }
        assert!(probe_exe(Path::new("Z:/no/such/java.exe")).is_none());
    }

    #[test]
    fn picks() {
        let j = |m: u32| JavaInfo { path: format!("j{m}"), version: m.to_string(), major: m };
        let list = vec![j(25), j(21), j(17), j(8)];
        assert_eq!(pick(&list, Some(17)).unwrap().major, 17);
        assert_eq!(pick(&list, Some(16)).unwrap().major, 17);
        assert_eq!(pick(&list, Some(30)).unwrap().major, 25);
        assert_eq!(pick(&list, None).unwrap().major, 25);
    }
}
