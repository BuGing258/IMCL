//! 读取安装器 / OptiFine jar 里内容的小工具。

use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use zip::ZipArchive;

pub fn open(path: &Path) -> Result<ZipArchive<File>> {
    let f = File::open(path).with_context(|| format!("打不开 {}", path.display()))?;
    ZipArchive::new(f).with_context(|| format!("{} 不是有效的 jar/zip 文件", path.display()))
}

pub fn has(zip: &ZipArchive<File>, name: &str) -> bool {
    zip.index_for_name(name).is_some()
}

pub fn read_bytes(zip: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>> {
    let mut entry = zip.by_name(name).map_err(|_| anyhow!("压缩包里没有 {name}"))?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn read_string(zip: &mut ZipArchive<File>, name: &str) -> Result<String> {
    Ok(String::from_utf8_lossy(&read_bytes(zip, name)?).into_owned())
}

pub fn read_json(zip: &mut ZipArchive<File>, name: &str) -> Result<serde_json::Value> {
    serde_json::from_slice(&read_bytes(zip, name)?).with_context(|| format!("解析 {name} 失败"))
}

/// 拒绝 `..`、绝对路径等会跳出目标目录的条目名
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let rel = Path::new(rel);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(anyhow!("压缩包里有不安全的路径：{}", rel.display()));
    }
    Ok(base.join(rel))
}

pub fn extract_to(zip: &mut ZipArchive<File>, name: &str, dest: &Path) -> Result<()> {
    let bytes = read_bytes(zip, name)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(dest, bytes)?;
    Ok(())
}

/// 把所有以 `prefix` 开头的文件解压到 `dest_root`（去掉前缀）
pub fn extract_prefix(zip: &mut ZipArchive<File>, prefix: &str, dest_root: &Path) -> Result<usize> {
    let mut n = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name()?.to_string();
        let Some(rel) = name.strip_prefix(prefix).map(str::to_string) else { continue };
        if rel.is_empty() {
            continue;
        }
        let dest = safe_join(dest_root, &rel)?;
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut out = File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
        n += 1;
    }
    Ok(n)
}

/// 读取 jar 的 Main-Class
pub fn main_class(jar: &Path) -> Result<String> {
    let mut zip = open(jar)?;
    let mf = read_string(&mut zip, "META-INF/MANIFEST.MF")?;
    // MANIFEST 每行最多 72 字节，超长的行以空格开头续接
    let mut joined = String::new();
    for line in mf.lines() {
        if let Some(cont) = line.strip_prefix(' ') {
            joined.push_str(cont);
        } else {
            joined.push('\n');
            joined.push_str(line);
        }
    }
    joined
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class:"))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| anyhow!("{} 没有 Main-Class", jar.display()))
}
