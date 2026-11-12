//! 微软正版登录（设备码流程）：微软账户 → Xbox Live → XSTS → Minecraft。
//! 令牌只保存在本机的应用配置目录里，不交给前端。

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

use crate::net;

const SCOPE: &str = "XboxLive.signin offline_access";
const MS_DEVICE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MS_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MsAccount {
    pub id: String,
    pub name: String,
    pub uuid: String,
    pub client_id: String,
    pub refresh_token: String,
    pub access_token: String,
    /// Minecraft 令牌过期时间（unix 秒）
    pub expires_at: u64,
    /// 当前使用的皮肤地址
    #[serde(default)]
    pub skin_url: Option<String>,
}

/// 给前端看的部分（不含令牌）
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
    pub uuid: String,
}

impl From<&MsAccount> for AccountInfo {
    fn from(a: &MsAccount) -> Self {
        Self { id: a.id.clone(), name: a.name.clone(), uuid: a.uuid.clone() }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

/// 正在进行的登录（一次一个），用于取消
#[derive(Default)]
pub struct Logins(pub Mutex<Option<Arc<AtomicBool>>>);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/* ---------------- 本地保存 ---------------- */

fn store_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_config_dir().context("找不到应用配置目录")?.join("ms-accounts.json"))
}

fn load_all(app: &AppHandle) -> Vec<MsAccount> {
    store_path(app)
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_all(app: &AppHandle, list: &[MsAccount]) -> Result<()> {
    let p = store_path(app)?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&p, serde_json::to_vec_pretty(list)?).context("无法保存账户")
}

fn upsert(app: &AppHandle, acc: &MsAccount) -> Result<()> {
    let mut list = load_all(app);
    list.retain(|a| a.id != acc.id);
    list.push(acc.clone());
    save_all(app, &list)
}

pub fn remove(app: &AppHandle, id: &str) -> Result<()> {
    let mut list = load_all(app);
    list.retain(|a| a.id != id);
    save_all(app, &list)
}

pub fn list(app: &AppHandle) -> Vec<AccountInfo> {
    load_all(app).iter().map(AccountInfo::from).collect()
}

/* ---------------- 网络请求 ---------------- */

async fn form(url: &str, params: &[(&str, &str)]) -> Result<Value> {
    let r = net::client().post(url).form(params).timeout(Duration::from_secs(20)).send().await.context("连不上微软登录服务器")?;
    Ok(r.json().await?)
}

/// 发 JSON，返回 (状态码, JSON)
async fn post_json(url: &str, body: &Value) -> Result<(u16, Value)> {
    let r = net::client()
        .post(url)
        .header("Accept", "application/json")
        .json(body)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .with_context(|| format!("连接失败：{url}"))?;
    let status = r.status().as_u16();
    let text = r.text().await.unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn ms_error(v: &Value) -> Option<String> {
    let e = v["error"].as_str()?;
    let desc = v["error_description"].as_str().unwrap_or("").lines().next().unwrap_or("");
    let desc = desc.split(" Trace ID").next().unwrap_or(desc).trim();
    Some(match e {
        "invalid_client" | "unauthorized_client" => format!("Client ID 不可用，请检查设置里填的 Client ID（{desc}）"),
        "invalid_grant" => "登录已过期，请在账户管理里重新登录".into(),
        _ => format!("微软登录失败：{e} {desc}"),
    })
}

/// 用微软令牌换 Minecraft 令牌，并取得角色名和 UUID
async fn finish(client_id: &str, refresh_token: &str, ms_token: &str) -> Result<MsAccount> {
    // Xbox Live
    let (s, xbl) = post_json(
        XBL,
        &json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_token}") },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| anyhow!("Xbox Live 登录失败（{s}）"))?;
    let uhs = xbl.pointer("/DisplayClaims/xui/0/uhs").and_then(Value::as_str).ok_or_else(|| anyhow!("Xbox Live 返回的数据不完整"))?;

    // XSTS
    let (s, xsts) = post_json(
        XSTS,
        &json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let Some(xsts_token) = xsts["Token"].as_str() else {
        let msg = match xsts["XErr"].as_u64() {
            Some(2148916233) => "这个微软账户还没有 Xbox 档案，先去 xbox.com 登录一次创建档案".into(),
            Some(2148916235) => "你所在的地区不支持 Xbox Live".into(),
            Some(2148916236) | Some(2148916237) => "这个账户需要先在 xbox.com 完成成人验证".into(),
            Some(2148916238) => "这是儿童账户，需要家长把它加入家庭组后才能登录".into(),
            other => format!("Xbox 验证失败（{s}，{other:?}）"),
        };
        bail!(msg);
    };

    // Minecraft
    let (s, mc) = post_json(MC_LOGIN, &json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") })).await?;
    let Some(mc_token) = mc["access_token"].as_str() else {
        if s == 403 {
            bail!("这个 Client ID 还没有获得 Minecraft 登录权限，需要先向 Mojang 申请");
        }
        bail!("Minecraft 登录失败（{s}）");
    };
    let expires_in = mc["expires_in"].as_u64().unwrap_or(86400);

    // 角色
    let r = net::client().get(MC_PROFILE).bearer_auth(mc_token).timeout(Duration::from_secs(20)).send().await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("这个微软账户还没有购买 Minecraft Java 版");
    }
    let profile: Value = r.error_for_status().context("读取 Minecraft 角色失败")?.json().await?;
    let uuid = profile["id"].as_str().ok_or_else(|| anyhow!("读取 Minecraft 角色失败"))?.to_string();
    let name = profile["name"].as_str().unwrap_or("Player").to_string();
    let skin_url = active_skin(&profile);

    Ok(MsAccount {
        id: format!("ms-{uuid}"),
        name,
        uuid,
        client_id: client_id.to_string(),
        refresh_token: refresh_token.to_string(),
        access_token: mc_token.to_string(),
        expires_at: now() + expires_in,
        skin_url,
    })
}

/// 角色信息里正在使用的皮肤
fn active_skin(profile: &Value) -> Option<String> {
    let skins = profile["skins"].as_array()?;
    skins
        .iter()
        .find(|s| s["state"].as_str() == Some("ACTIVE"))
        .or_else(|| skins.first())
        .and_then(|s| s["url"].as_str())
        .map(str::to_string)
}

/// 登录还有效时重新读一次角色，更新名字和皮肤；返回皮肤地址（读不到就用上次的）
pub async fn refresh_skin(app: &AppHandle, id: &str) -> Result<Option<String>> {
    let Some(mut acc) = load_all(app).into_iter().find(|a| a.id == id) else {
        bail!("找不到这个微软账户的登录信息");
    };
    if acc.expires_at > now() + 60 {
        let r = net::client().get(MC_PROFILE).bearer_auth(&acc.access_token).timeout(Duration::from_secs(15)).send().await;
        if let Ok(profile) = async { r?.error_for_status()?.json::<Value>().await }.await {
            if let Some(n) = profile["name"].as_str() {
                acc.name = n.to_string();
            }
            acc.skin_url = active_skin(&profile);
            upsert(app, &acc)?;
        }
    }
    Ok(acc.skin_url)
}

/* ---------------- 对外 ---------------- */

/// 设备码登录：先推送 `ms-device-code` 事件让用户去浏览器输入代码，再等用户完成
pub async fn login(app: &AppHandle, client_id: &str, cancel: Arc<AtomicBool>) -> Result<AccountInfo> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        bail!("还没填写微软登录用的 Client ID，请先到设置里填写");
    }
    let dc = form(MS_DEVICE, &[("client_id", client_id), ("scope", SCOPE)]).await?;
    if let Some(e) = ms_error(&dc) {
        bail!(e);
    }
    let device_code = dc["device_code"].as_str().ok_or_else(|| anyhow!("微软没有返回登录代码"))?.to_string();
    let expires_in = dc["expires_in"].as_u64().unwrap_or(900);
    let _ = app.emit(
        "ms-device-code",
        DeviceCode {
            user_code: dc["user_code"].as_str().unwrap_or_default().to_string(),
            verification_uri: dc["verification_uri"].as_str().unwrap_or("https://www.microsoft.com/link").to_string(),
            expires_in,
        },
    );

    let deadline = now() + expires_in;
    let mut interval = dc["interval"].as_u64().unwrap_or(5).max(1);
    let tokens = loop {
        for _ in 0..interval * 4 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if cancel.load(Ordering::Relaxed) {
                bail!("已取消");
            }
        }
        if now() > deadline {
            bail!("登录代码过期了，请重新登录");
        }
        let r = form(
            MS_TOKEN,
            &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", client_id), ("device_code", &device_code)],
        )
        .await?;
        match r["error"].as_str() {
            Some("authorization_pending") => continue,
            Some("slow_down") => interval += 5,
            Some("authorization_declined") => bail!("你在网页上拒绝了授权"),
            Some("expired_token") => bail!("登录代码过期了，请重新登录"),
            Some(_) => bail!(ms_error(&r).unwrap_or_default()),
            None => break r,
        }
    };
    let ms_token = tokens["access_token"].as_str().ok_or_else(|| anyhow!("微软没有返回令牌"))?;
    let refresh = tokens["refresh_token"].as_str().unwrap_or_default();
    let acc = finish(client_id, refresh, ms_token).await?;
    upsert(app, &acc)?;
    Ok(AccountInfo::from(&acc))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 联网测试：无效的 Client ID 应该得到能看懂的中文错误
    #[tokio::test]
    #[ignore]
    async fn bad_client_id_live() {
        let r = form(MS_DEVICE, &[("client_id", "00000000-0000-0000-0000-000000000000"), ("scope", SCOPE)]).await.unwrap();
        let msg = ms_error(&r).expect("应该返回错误");
        println!("{msg}");
        assert!(msg.contains("Client ID") || msg.contains("微软登录失败"));
    }
}

/// 启动前调用：令牌快过期时自动刷新
pub async fn ensure(app: &AppHandle, id: &str) -> Result<MsAccount> {
    let acc = load_all(app)
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| anyhow!("找不到这个微软账户的登录信息，请在账户管理里重新登录"))?;
    if acc.expires_at > now() + 600 {
        return Ok(acc);
    }
    let r = form(
        MS_TOKEN,
        &[("grant_type", "refresh_token"), ("client_id", &acc.client_id), ("refresh_token", &acc.refresh_token), ("scope", SCOPE)],
    )
    .await?;
    if let Some(e) = ms_error(&r) {
        bail!(e);
    }
    let ms_token = r["access_token"].as_str().ok_or_else(|| anyhow!("刷新登录失败"))?;
    let refresh = r["refresh_token"].as_str().unwrap_or(&acc.refresh_token);
    let fresh = finish(&acc.client_id, refresh, ms_token).await?;
    // 同一个角色才替换；万一换了角色，删掉旧的
    if fresh.id != acc.id {
        remove(app, &acc.id)?;
    }
    upsert(app, &fresh)?;
    Ok(fresh)
}
