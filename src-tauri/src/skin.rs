//! 账户头像用的皮肤：游戏自带的 Steve / Alex（从已安装的客户端 jar 里取）和正版账户的皮肤。
//! 都缓存在应用配置目录的 skins 文件夹里，返回给前端的是 data URL。

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::auth;
use crate::game::zipx;
use crate::net;

/// 不同年代的客户端里，默认皮肤放的位置不一样（新的在前）
const STEVE: &[&str] = &[
    "assets/minecraft/textures/entity/player/wide/steve.png",
    "assets/minecraft/textures/entity/steve.png",
];
const ALEX: &[&str] = &[
    "assets/minecraft/textures/entity/player/slim/alex.png",
    "assets/minecraft/textures/entity/alex.png",
];

#[derive(Clone, Debug, Serialize)]
pub struct DefaultSkins {
    pub steve: String,
    pub alex: String,
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf> {
    let d = app.path().app_config_dir().context("找不到应用配置目录")?.join("skins");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// PNG 的宽高；不是 PNG 返回 None
pub fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 24 || &b[..8] != b"\x89PNG\r\n\x1a\n" || &b[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
    Some((w, h))
}

/// 皮肤尺寸：宽是 64 的倍数，高等于宽（新格式）或一半（1.8 以前的旧格式）
pub fn is_skin(b: &[u8]) -> bool {
    matches!(png_size(b), Some((w, h)) if w >= 64 && w % 64 == 0 && w <= 1024 && (h == w || h * 2 == w))
}

pub fn data_url(b: &[u8]) -> String {
    data_url_of(b, "image/png")
}

/// 任意类型的 data URL
pub fn data_url_of(b: &[u8], mime: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(32 + b.len() * 4 / 3 + 4);
    s.push_str("data:");
    s.push_str(mime);
    s.push_str(";base64,");
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

fn read_first(zip: &mut zip::ZipArchive<std::fs::File>, names: &[&str]) -> Option<Vec<u8>> {
    names.iter().find_map(|n| zipx::read_bytes(zip, n).ok()).filter(|b: &Vec<u8>| is_skin(b))
}

/// 在游戏目录里找一个装着默认皮肤的客户端 jar
pub fn find_in_dirs(dirs: &[PathBuf]) -> Option<(Vec<u8>, Vec<u8>)> {
    for d in dirs {
        let d = if d.join("versions").is_dir() { d.clone() } else { d.join(".minecraft") };
        let Ok(rd) = std::fs::read_dir(d.join("versions")) else { continue };
        for v in rd.flatten() {
            let jar = v.path().join(format!("{}.jar", v.file_name().to_string_lossy()));
            let Ok(mut zip) = zipx::open(&jar) else { continue };
            if let (Some(s), Some(a)) = (read_first(&mut zip, STEVE), read_first(&mut zip, ALEX)) {
                return Some((s, a));
            }
        }
    }
    None
}

/// 游戏自带的 Steve 和 Alex；一个客户端都没装时返回 None
pub fn defaults(app: &AppHandle, dirs: &[PathBuf]) -> Result<Option<DefaultSkins>> {
    let cache = cache_dir(app)?;
    let (sp, ap) = (cache.join("steve.png"), cache.join("alex.png"));
    let cached = std::fs::read(&sp).ok().zip(std::fs::read(&ap).ok()).filter(|(s, a)| is_skin(s) && is_skin(a));
    let (s, a) = match cached {
        Some(x) => x,
        None => match find_in_dirs(dirs) {
            Some((s, a)) => {
                std::fs::write(&sp, &s)?;
                std::fs::write(&ap, &a)?;
                (s, a)
            }
            None => return Ok(None),
        },
    };
    Ok(Some(DefaultSkins { steve: data_url(&s), alex: data_url(&a) }))
}

fn allowed(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or("");
    rest.starts_with("textures.minecraft.net/")
}

/// 下载皮肤（按地址缓存，皮肤地址变了就是换了皮肤）
pub async fn fetch(cache: &Path, url: &str) -> Result<Vec<u8>> {
    if !allowed(url) {
        bail!("不是 Minecraft 的皮肤地址：{url}");
    }
    let name = format!("tex-{}.png", sha1_smol::Sha1::from(url).digest().to_string());
    let path = cache.join(&name);
    if let Ok(b) = std::fs::read(&path) {
        if is_skin(&b) {
            return Ok(b);
        }
    }
    // 皮肤服务器用 http 也能访问；统一走 https
    let url = url.replacen("http://", "https://", 1);
    let b = net::client().get(&url).timeout(Duration::from_secs(20)).send().await?.error_for_status()?.bytes().await?.to_vec();
    if !is_skin(&b) {
        bail!("下载到的不是皮肤图片");
    }
    std::fs::write(&path, &b)?;
    Ok(b)
}

/// 正版账户当前的皮肤；登录还有效时会顺便刷新一下（在官网换了皮肤这里也会跟着变）
pub async fn ms_skin(app: &AppHandle, id: &str) -> Result<Option<String>> {
    let url = auth::refresh_skin(app, id).await?;
    let Some(url) = url else { return Ok(None) };
    let b = fetch(&cache_dir(app)?, &url).await?;
    Ok(Some(data_url(&b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_reference() {
        assert_eq!(data_url(b""), "data:image/png;base64,");
        assert_eq!(data_url(b"f"), "data:image/png;base64,Zg==");
        assert_eq!(data_url(b"fo"), "data:image/png;base64,Zm8=");
        assert_eq!(data_url(b"foo"), "data:image/png;base64,Zm9v");
        assert_eq!(data_url(b"foobar"), "data:image/png;base64,Zm9vYmFy");
    }

    #[test]
    fn skin_sizes() {
        let png = |w: u32, h: u32| {
            let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            b.extend(w.to_be_bytes());
            b.extend(h.to_be_bytes());
            b
        };
        assert!(is_skin(&png(64, 64)));
        assert!(is_skin(&png(64, 32)));
        assert!(is_skin(&png(128, 128)));
        assert!(!is_skin(&png(64, 48)));
        assert!(!is_skin(&png(32, 32)));
        assert!(!is_skin(b"not a png"));
        assert!(allowed("http://textures.minecraft.net/texture/abc"));
        assert!(!allowed("https://evil.example/textures.minecraft.net/"));
    }

    /// 从本机装好的客户端里取默认皮肤（没装游戏时跳过）
    #[test]
    #[ignore]
    fn defaults_from_installed() {
        let dir = PathBuf::from(std::env::var("APPDATA").unwrap()).join(".minecraft");
        let (s, a) = find_in_dirs(&[dir]).expect("找不到客户端 jar");
        println!("steve {:?} alex {:?}", png_size(&s), png_size(&a));
        assert!(is_skin(&s) && is_skin(&a));
    }

    /// 用公开接口查 Notch 的皮肤地址，测试下载
    #[tokio::test]
    #[ignore]
    async fn fetch_live() {
        let p: serde_json::Value = net::client()
            .get("https://sessionserver.mojang.com/session/minecraft/profile/069a79f444e94726a5befca90e38aaf5")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let v = p["properties"][0]["value"].as_str().unwrap();
        // 只是测试：手动解 base64 太麻烦，直接在 JSON 文本里找地址
        let raw = String::from_utf8(decode_b64(v)).unwrap();
        let start = raw.find("http://textures.minecraft.net/texture/").unwrap();
        let url: String = raw[start..].chars().take_while(|c| *c != '"').collect();
        let dir = std::env::temp_dir().join("amc-skin-test");
        std::fs::create_dir_all(&dir).unwrap();
        let b = fetch(&dir, &url).await.unwrap();
        println!("{url} {:?}", png_size(&b));
        assert!(is_skin(&b));
    }

    fn decode_b64(s: &str) -> Vec<u8> {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let (mut acc, mut bits) = (0u32, 0);
        for c in s.bytes() {
            let Some(v) = T.iter().position(|&t| t == c) else { continue };
            acc = (acc << 6) | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        out
    }
}
