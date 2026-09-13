use iced::futures::SinkExt;
use iced::{Subscription, stream};
use reqwest::{Client, StatusCode, Url};
use serde_json::{Value, json};
use std::hash::Hash;
use std::sync::LazyLock;
/// Shared HTTP client (one connection pool for all auth requests).
static HTTP: LazyLock<Client> = LazyLock::new(Client::new);
const AZURE_CLIENT_ID: &str = "7f8e9d75-ca8f-4603-b2ab-ae7fc0f871d9";
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MinecraftAccount {
    pub username: String,
    pub token: String,
    pub uuid: String,
}
#[derive(Clone, Debug, Default)]
pub struct AuthCode {
    pub code: String,
    pub link: String,
    pub device_code: String,
}
#[derive(Debug, Clone, Default)]
pub struct AuthToken {
    pub access_token: String,
    pub refresh_token: String,
}
#[derive(Clone, Debug, Default)]
pub struct XboxLiveData {
    user_hash: String,
    xsts_token: String,
}
pub async fn request_code() -> Result<AuthCode, String> {
    let mut url =
        Url::parse("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
            .map_err(|e| format!("Auth URL failed: {e}"))?;
    url.query_pairs_mut()
        .append_pair("client_id", AZURE_CLIENT_ID)
        .append_pair("scope", "XboxLive.signin offline_access");
    let response = HTTP
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Code request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Code read failed: {e}"))?;
    let response_json: Value =
        serde_json::from_str(&response).map_err(|e| format!("Code parse failed: {e}"))?;
    let code = response_json["user_code"]
        .as_str()
        .ok_or_else(|| "Auth response has no code.".to_string())?
        .to_owned();
    let link = response_json["verification_uri"]
        .as_str()
        .ok_or_else(|| "Auth response has no link.".to_string())?
        .to_owned();
    let device_code = response_json["device_code"]
        .as_str()
        .ok_or_else(|| "Auth response has no device code.".to_string())?
        .to_owned();
    Ok(AuthCode {
        code,
        link,
        device_code,
    })
}
#[derive(Debug, Clone)]
pub enum WaitProgress {
    GotAuthToken(AuthToken),
    Waiting,
    Finished,
}
pub fn start_wait_for_login<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    device_code: String,
) -> Subscription<(I, WaitProgress)> {
    Subscription::run_with((id, device_code), |data| {
        let (id, device_code) = data.clone();
        stream::channel(100, async move |mut output| {
            let client = HTTP.clone();
            loop {
                let response = match client
                    .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(format!(
                        "client_id={}&scope={}&grant_type={}&device_code={}",
                        AZURE_CLIENT_ID,
                        "XboxLive.signin offline_access",
                        "urn:ietf:params:oauth:grant-type:device_code",
                        device_code
                    ))
                    .send()
                    .await
                {
                    // Lost network while polling: report waiting and retry
                    // instead of killing the subscription with a panic.
                    Ok(ok) => ok,
                    Err(e) => {
                        eprintln!("Auth poll failed, retrying: {e}");
                        let _ = output.send((id, WaitProgress::Waiting)).await;
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                };
                match response.status() {
                    StatusCode::OK => {
                        let text = match response.text().await {
                            Ok(t) => t,
                            Err(e) => {
                                eprintln!("Auth poll read failed, retrying: {e}");
                                let _ = output.send((id, WaitProgress::Waiting)).await;
                                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                continue;
                            }
                        };
                        let response_json: Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("Auth poll parse failed, retrying: {e}");
                                let _ = output.send((id, WaitProgress::Waiting)).await;
                                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                continue;
                            }
                        };
                        let (Some(access_token), Some(refresh_token)) = (
                            response_json["access_token"].as_str(),
                            response_json["refresh_token"].as_str(),
                        ) else {
                            eprintln!("Auth poll got a token response without tokens, retrying.");
                            let _ = output.send((id, WaitProgress::Waiting)).await;
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            continue;
                        };
                        let token = AuthToken {
                            access_token: access_token.to_string(),
                            refresh_token: refresh_token.to_string(),
                        };
                        let _ = output.send((id, WaitProgress::GotAuthToken(token))).await;
                        let _ = output.send((id, WaitProgress::Finished)).await;
                        break;
                    }
                    _ => {
                        let _ = output.send((id, WaitProgress::Waiting)).await;
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        })
    })
}
pub async fn login_to_xbox(access_token: String) -> Result<XboxLiveData, String> {
    let xbox_live_response_request_data = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": &format!("d={}", access_token)
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let xbox_live_response = HTTP
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&xbox_live_response_request_data)
        .send()
        .await
        .map_err(|e| format!("Xbox login failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Xbox login read failed: {e}"))?;
    let xbox_live_response_json: Value = serde_json::from_str(&xbox_live_response)
        .map_err(|e| format!("Xbox login parse failed: {e}"))?;
    let xbox_live_token = xbox_live_response_json["Token"]
        .as_str()
        .ok_or_else(|| "Xbox response has no token.".to_string())?
        .to_owned();
    let user_hash = xbox_live_response_json["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| "Xbox response has no user hash.".to_string())?
        .to_owned();
    let xbox_xsts_response_request_data = json!(
        {
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [
                    xbox_live_token
                ]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
         }
    );
    let xbox_xsts_response = HTTP
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&xbox_xsts_response_request_data)
        .send()
        .await
        .map_err(|e| format!("Xbox XSTS failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Xbox XSTS read failed: {e}"))?;
    let xbox_xsts_response_json: Value = serde_json::from_str(&xbox_xsts_response)
        .map_err(|e| format!("Xbox XSTS parse failed: {e}"))?;
    let xsts_token = xbox_xsts_response_json["Token"]
        .as_str()
        .ok_or_else(|| "Xbox XSTS response has no token.".to_string())?
        .to_owned();
    Ok(XboxLiveData {
        xsts_token,
        user_hash,
    })
}
pub async fn login_to_minecraft(xbox_data: XboxLiveData) -> Result<MinecraftAccount, String> {
    let minecraft_data_response_request_data = json!(
        {
        "identityToken": format!("XBL3.0 x={};{}", xbox_data.user_hash, xbox_data.xsts_token)
        }
    );
    let minecraft_data_response = HTTP
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&minecraft_data_response_request_data)
        .send()
        .await
        .map_err(|e| format!("Minecraft login failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Minecraft login read failed: {e}"))?;
    let minecraft_data_json: Value = serde_json::from_str(&minecraft_data_response)
        .map_err(|e| format!("Minecraft login parse failed: {e}"))?;
    let token = minecraft_data_json["access_token"]
        .as_str()
        .ok_or_else(|| "Minecraft response has no token.".to_string())?
        .to_owned();
    let mc_profile_response = HTTP
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(token.clone())
        .send()
        .await
        .map_err(|e| format!("Minecraft profile failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Minecraft profile read failed: {e}"))?;
    let mc_profile_json: Value = serde_json::from_str(&mc_profile_response)
        .map_err(|e| format!("Minecraft profile parse failed: {e}"))?;
    let uuid = mc_profile_json["id"]
        .as_str()
        .ok_or_else(|| "Minecraft profile has no id.".to_string())?
        .to_owned();
    let username = mc_profile_json["name"]
        .as_str()
        .ok_or_else(|| "Minecraft profile has no name.".to_string())?
        .to_owned();
    Ok(MinecraftAccount {
        username,
        token,
        uuid,
    })
}
pub async fn login_with_refresh_token(refresh_token: String) -> Option<MinecraftAccount> {
    // Any failure here means "offline mode" for the caller, never a crash.
    let response = HTTP
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&scope={}&grant_type={}&refresh_token={}",
            AZURE_CLIENT_ID, "XboxLive.signin offline_access", "refresh_token", refresh_token
        ))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let response_json: Value = serde_json::from_str(&response).ok()?;
    let access_token = response_json["access_token"].as_str()?.to_owned();
    let xbox_data = login_to_xbox(access_token).await.ok()?;
    login_to_minecraft(xbox_data).await.ok()
}
