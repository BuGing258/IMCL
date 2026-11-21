//! 离线账户在游戏里显示皮肤：启动器在本机开一个只给自己用的小“皮肤服务器”（兼容 authlib-injector 的 Yggdrasil 接口），
//! 启动游戏时用 authlib-injector 让游戏改为信任这个服务器。皮肤数据用启动器自己生成的密钥签名。
//!
//! 只实现游戏会用到的几个接口：元数据、查角色（带签名的皮肤）、按名字查 UUID、皮肤图片，以及新版本会问的几个服务接口。
//! authlib-injector（AGPL-3.0，带例外）在运行时单独下载，不打包进启动器。

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::signature::{SignatureEncoding, Signer};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::net;

/// 启动请求里带的离线皮肤
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineSkin {
    /// PNG 的 base64（可以带 data:image/png;base64, 前缀）
    pub png: String,
    /// 细手臂（Alex 模型）
    #[serde(default)]
    pub slim: bool,
}

#[derive(Clone)]
struct Profile {
    uuid: String,
    name: String,
    skin_hash: String,
    slim: bool,
}

struct Server {
    port: u16,
    key: RsaPrivateKey,
    public_pem: String,
    public_der_b64: String,
    profiles: Mutex<HashMap<String, Profile>>,
    textures: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

static SERVER: OnceLock<Arc<Server>> = OnceLock::new();
static STARTING: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

/* ---------------- 密钥 ---------------- */

/// 读取或生成签名用的密钥（第一次生成要一两秒，所以启动器打开时就在后台准备好）
pub fn load_key(dir: &Path) -> Result<RsaPrivateKey> {
    let path = dir.join("skin-key.pem");
    if let Ok(pem) = std::fs::read_to_string(&path) {
        if let Ok(k) = RsaPrivateKey::from_pkcs8_pem(&pem) {
            return Ok(k);
        }
    }
    let key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048).context("生成皮肤签名密钥失败")?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, key.to_pkcs8_pem(LineEnding::LF)?.as_bytes())?;
    Ok(key)
}

fn sign(key: &RsaPrivateKey, data: &[u8]) -> String {
    let signer = SigningKey::<sha1::Sha1>::new(key.clone());
    B64.encode(signer.sign(data).to_bytes())
}

/* ---------------- 接口 ---------------- */

impl Server {
    fn api(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn metadata(&self) -> Value {
        json!({
            "meta": {
                "serverName": "IMCL 离线皮肤",
                "implementationName": "IMCL",
                "implementationVersion": env!("CARGO_PKG_VERSION"),
                "feature.no_mojang_namespace": true,
            },
            "skinDomains": ["127.0.0.1", "localhost"],
            "signaturePublickey": self.public_pem,
        })
    }

    /// 角色信息，皮肤属性带签名
    fn profile_json(&self, p: &Profile, signed: bool) -> Value {
        let mut skin = json!({ "url": format!("{}/textures/{}", self.api(), p.skin_hash) });
        if p.slim {
            skin["metadata"] = json!({ "model": "slim" });
        }
        let textures = json!({
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
            "profileId": p.uuid,
            "profileName": p.name,
            "textures": { "SKIN": skin },
        });
        let value = B64.encode(textures.to_string());
        let mut prop = json!({ "name": "textures", "value": value });
        if signed {
            prop["signature"] = json!(sign(&self.key, value.as_bytes()));
        }
        json!({ "id": p.uuid, "name": p.name, "properties": [prop] })
    }

    fn by_name(&self, name: &str) -> Option<Profile> {
        self.profiles.lock().unwrap().values().find(|p| p.name.eq_ignore_ascii_case(name)).cloned()
    }

    /// 路由：返回 (状态码, 内容类型, 内容)
    fn handle(&self, method: &str, target: &str, body: &[u8]) -> (u16, &'static str, Vec<u8>) {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let q = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(&format!("{k}="))).map(|v| v.replace("%20", " "));
        let ok = |v: Value| (200, "application/json; charset=utf-8", v.to_string().into_bytes());
        let none = (204, "text/plain", Vec::new());
        let path = path.trim_end_matches('/');
        match (method, path) {
            ("GET", "") => ok(self.metadata()),
            ("GET", p) if p.starts_with("/sessionserver/session/minecraft/profile/") => {
                let uuid = p.rsplit('/').next().unwrap_or_default().replace('-', "").to_lowercase();
                let signed = q("unsigned").as_deref() != Some("true");
                match self.profiles.lock().unwrap().get(&uuid).cloned() {
                    Some(prof) => ok(self.profile_json(&prof, signed)),
                    None => none,
                }
            }
            ("GET", "/sessionserver/session/minecraft/hasJoined") => match q("username").and_then(|n| self.by_name(&n)) {
                Some(prof) => ok(self.profile_json(&prof, true)),
                None => none,
            },
            ("POST", "/sessionserver/session/minecraft/join") => none,
            ("POST", "/api/profiles/minecraft") | ("POST", "/minecraftservices/minecraft/profile/lookup/bulk/byname") => {
                let names: Vec<String> = serde_json::from_slice(body).unwrap_or_default();
                let list: Vec<Value> = names.iter().filter_map(|n| self.by_name(n)).map(|p| json!({ "id": p.uuid, "name": p.name })).collect();
                ok(json!(list))
            }
            ("GET", p) if p.starts_with("/textures/") => {
                let hash = p.rsplit('/').next().unwrap_or_default();
                match self.textures.lock().unwrap().get(hash) {
                    Some(png) => (200, "image/png", png.as_ref().clone()),
                    None => (404, "text/plain", b"not found".to_vec()),
                }
            }
            // 1.16.4 以后游戏会问这些，给个“都允许”的回答
            ("GET", "/minecraftservices/player/attributes") => ok(json!({
                "privileges": {
                    "onlineChat": { "enabled": true },
                    "multiplayerServer": { "enabled": true },
                    "multiplayerRealms": { "enabled": false },
                    "telemetry": { "enabled": false },
                },
                "profanityFilterPreferences": { "profanityFilterOn": false },
            })),
            ("GET", "/minecraftservices/privacy/blocklist") => ok(json!({ "blockedProfiles": [] })),
            ("GET", "/minecraftservices/publickeys") => ok(json!({
                "profilePropertyKeys": [{ "publicKey": self.public_der_b64 }],
                "playerCertificateKeys": [{ "publicKey": self.public_der_b64 }],
            })),
            ("POST", "/authserver/validate") | ("POST", "/authserver/invalidate") => none,
            _ => (404, "application/json", br#"{"error":"Not Found"}"#.to_vec()),
        }
    }
}

/* ---------------- HTTP（只给本机的游戏用，够用就行） ---------------- */

async fn serve_conn(server: Arc<Server>, mut s: TcpStream) -> Result<()> {
    let mut buf = Vec::with_capacity(2048);
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = tokio::time::timeout(Duration::from_secs(10), s.read(&mut tmp)).await??;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 64 * 1024 {
            bail!("请求头太大");
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let (method, target) = (first.next().unwrap_or("GET").to_string(), first.next().unwrap_or("/").to_string());
    let len: usize = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0)
        .min(1024 * 1024);
    let mut body = buf[header_end..].to_vec();
    while body.len() < len {
        let n = tokio::time::timeout(Duration::from_secs(10), s.read(&mut tmp)).await??;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    let (code, ctype, content) = server.handle(&method, &target, &body[..len.min(body.len())]);
    if cfg!(debug_assertions) {
        eprintln!("[皮肤服务器] {method} {target} -> {code}");
    }
    let reason = match code {
        200 => "OK",
        204 => "No Content",
        _ => "Not Found",
    };
    let header = format!("HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", content.len());
    s.write_all(header.as_bytes()).await?;
    if method != "HEAD" {
        s.write_all(&content).await?;
    }
    s.shutdown().await.ok();
    Ok(())
}

/// 启动（只启动一次）本机皮肤服务器，返回它的地址
async fn ensure_server(data_dir: &Path) -> Result<Arc<Server>> {
    if let Some(s) = SERVER.get() {
        return Ok(Arc::clone(s));
    }
    let _guard = STARTING.get_or_init(Default::default).lock().await;
    if let Some(s) = SERVER.get() {
        return Ok(Arc::clone(s));
    }
    let dir = data_dir.to_path_buf();
    let key = tauri::async_runtime::spawn_blocking(move || load_key(&dir)).await??;
    let public = RsaPublicKey::from(&key);
    let listener = TcpListener::bind("127.0.0.1:0").await.context("开不了本机皮肤服务器")?;
    let server = Arc::new(Server {
        port: listener.local_addr()?.port(),
        public_pem: public.to_public_key_pem(LineEnding::LF)?,
        public_der_b64: B64.encode(public.to_public_key_der()?.as_bytes()),
        key,
        profiles: Mutex::new(HashMap::new()),
        textures: Mutex::new(HashMap::new()),
    });
    let srv = Arc::clone(&server);
    tauri::async_runtime::spawn(async move {
        loop {
            let Ok((conn, _)) = listener.accept().await else { continue };
            let srv = Arc::clone(&srv);
            tauri::async_runtime::spawn(async move {
                let _ = serve_conn(srv, conn).await;
            });
        }
    });
    let _ = SERVER.set(Arc::clone(&server));
    Ok(server)
}

/* ---------------- authlib-injector ---------------- */

const INJECTOR_META: [&str; 2] = [
    "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json",
    "https://authlib-injector.yushi.moe/artifact/latest.json",
];

fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// 下载 authlib-injector（已经有就直接用；一周检查一次新版本）
async fn ensure_injector(data_dir: &Path) -> Result<PathBuf> {
    let jar = data_dir.join("authlib-injector.jar");
    let fresh = std::fs::metadata(&jar)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < Duration::from_secs(7 * 86400));
    if fresh {
        return Ok(jar);
    }
    let mut last = anyhow!("下载不到 authlib-injector");
    for meta_url in INJECTOR_META {
        let r: Result<PathBuf> = async {
            let meta: Value = net::client().get(meta_url).timeout(Duration::from_secs(15)).send().await?.error_for_status()?.json().await?;
            let url = meta["download_url"].as_str().ok_or_else(|| anyhow!("没有下载地址"))?;
            let want = meta.pointer("/checksums/sha256").and_then(Value::as_str).unwrap_or_default().to_lowercase();
            // 已经是最新版就只更新一下时间
            if let Ok(old) = std::fs::read(&jar) {
                if !want.is_empty() && sha256_hex(&old) == want {
                    let f = std::fs::File::options().append(true).open(&jar)?;
                    f.set_modified(std::time::SystemTime::now())?;
                    return Ok(jar.clone());
                }
            }
            let bytes = net::client().get(url).timeout(Duration::from_secs(60)).send().await?.error_for_status()?.bytes().await?;
            if !want.is_empty() && sha256_hex(&bytes) != want {
                bail!("authlib-injector 校验失败");
            }
            std::fs::create_dir_all(data_dir)?;
            std::fs::write(&jar, &bytes)?;
            Ok(jar.clone())
        }
        .await;
        match r {
            Ok(p) => return Ok(p),
            Err(e) => last = e,
        }
    }
    // 下载失败但以前下过：先用旧的
    if jar.is_file() {
        return Ok(jar);
    }
    Err(last)
}

/* ---------------- 对外 ---------------- */

/// 启动前调用：登记这个离线角色的皮肤，返回要加给 Java 的参数
pub async fn prepare(data_dir: &Path, name: &str, uuid: &str, skin: &OfflineSkin) -> Result<Vec<String>> {
    let b64 = skin.png.split_once("base64,").map(|(_, b)| b).unwrap_or(&skin.png);
    let png = B64.decode(b64.trim()).context("皮肤数据不对")?;
    if !crate::skin::is_skin(&png) {
        bail!("皮肤图片尺寸不对");
    }
    let (jar, server) = tokio::try_join!(ensure_injector(data_dir), ensure_server(data_dir))?;
    let hash = sha256_hex(&png);
    server.textures.lock().unwrap().insert(hash.clone(), Arc::new(png));
    server.profiles.lock().unwrap().insert(
        uuid.to_string(),
        Profile { uuid: uuid.to_string(), name: name.to_string(), skin_hash: hash, slim: skin.slim },
    );
    Ok(vec![
        format!("-javaagent:{}={}", jar.display(), server.api()),
        "-Dauthlibinjector.side=client".into(),
        "-Dauthlibinjector.noShowServerName".into(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::signature::Verifier;

    #[test]
    fn signature_verifies() {
        let dir = std::env::temp_dir().join(format!("amc-ygg-{}", std::process::id()));
        let key = load_key(&dir).unwrap();
        // 第二次读出来是同一把钥匙
        let again = load_key(&dir).unwrap();
        assert_eq!(RsaPublicKey::from(&key), RsaPublicKey::from(&again));
        let data = b"eyJ0ZXh0dXJlcyI6e319";
        let sig = B64.decode(sign(&key, data)).unwrap();
        let vk = VerifyingKey::<sha1::Sha1>::new(RsaPublicKey::from(&key));
        vk.verify(data, &Signature::try_from(sig.as_slice()).unwrap()).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn server_answers() {
        let dir = std::env::temp_dir().join(format!("amc-ygg-srv-{}", std::process::id()));
        let server = ensure_server(&dir).await.unwrap();
        let png = {
            let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            b.extend(64u32.to_be_bytes());
            b.extend(64u32.to_be_bytes());
            b
        };
        let hash = sha256_hex(&png);
        server.textures.lock().unwrap().insert(hash.clone(), Arc::new(png.clone()));
        server.profiles.lock().unwrap().insert("abc".into(), Profile { uuid: "abc".into(), name: "Steve".into(), skin_hash: hash.clone(), slim: true });

        let api = server.api();
        let c = reqwest::Client::new();
        let meta: Value = c.get(&api).send().await.unwrap().json().await.unwrap();
        assert!(meta["signaturePublickey"].as_str().unwrap().contains("BEGIN PUBLIC KEY"));
        let p: Value = c.get(format!("{api}/sessionserver/session/minecraft/profile/abc?unsigned=false")).send().await.unwrap().json().await.unwrap();
        let value = p["properties"][0]["value"].as_str().unwrap();
        let tex: Value = serde_json::from_slice(&B64.decode(value).unwrap()).unwrap();
        assert_eq!(tex["textures"]["SKIN"]["metadata"]["model"], "slim");
        assert!(p["properties"][0]["signature"].is_string());
        let got = c.get(tex["textures"]["SKIN"]["url"].as_str().unwrap()).send().await.unwrap().bytes().await.unwrap();
        assert_eq!(got.as_ref(), png.as_slice());
        let ids: Value = c.post(format!("{api}/api/profiles/minecraft")).json(&json!(["steve"])).send().await.unwrap().json().await.unwrap();
        assert_eq!(ids[0]["id"], "abc");
        assert_eq!(c.get(format!("{api}/sessionserver/session/minecraft/profile/zzz")).send().await.unwrap().status(), 204);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    #[ignore]
    async fn injector_download_live() {
        let dir = std::env::temp_dir().join(format!("amc-ygg-dl-{}", std::process::id()));
        let jar = ensure_injector(&dir).await.unwrap();
        println!("{} {} 字节", jar.display(), std::fs::metadata(&jar).unwrap().len());
        assert!(std::fs::metadata(&jar).unwrap().len() > 100_000);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
