use iced::futures::SinkExt;
use iced::{Subscription, stream};
use reqwest::{Client, StatusCode, Url};
use serde_json::{Value, json};
use std::hash::Hash;
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
pub async fn request_code() -> AuthCode {
    let client = Client::new();
    let mut url =
        Url::parse("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", AZURE_CLIENT_ID)
        .append_pair("scope", "XboxLive.signin offline_access");
    let response = match client.get(url).send().await {
        Ok(ok) => ok.text().await.unwrap(),
        Err(e) => panic!("{e}"),
    };
    let response_json: Value = serde_json::from_str(&response).unwrap();
    let code = response_json["user_code"].as_str().unwrap().to_owned();
    let link = response_json["verification_uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let device_code = response_json["device_code"].as_str().unwrap().to_owned();
    AuthCode {
        code,
        link,
        device_code,
    }
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
            let client = Client::new();
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
                    Ok(ok) => ok,
                    Err(e) => panic!("{e}"),
                };
                match response.status() {
                    StatusCode::OK => {
                        let response_json: Value =
                            serde_json::from_str(&response.text().await.unwrap()).unwrap();
                        let access_token =
                            response_json["access_token"].as_str().unwrap().to_string();
                        let refresh_token =
                            response_json["refresh_token"].as_str().unwrap().to_string();
                        let token = AuthToken {
                            access_token,
                            refresh_token,
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
pub async fn login_to_xbox(access_token: String) -> XboxLiveData {
    let client = Client::new();
    let xbox_live_response_request_data = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": &format!("d={}", access_token)
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let xbox_live_response = match client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&xbox_live_response_request_data)
        .send()
        .await
    {
        Ok(ok) => ok.text().await.unwrap(),
        Err(e) => panic!("{e}"),
    };
    let xbox_live_response_json: Value = serde_json::from_str(&xbox_live_response).unwrap();
    let xbox_live_token = xbox_live_response_json["Token"]
        .as_str()
        .unwrap()
        .to_owned();
    let user_hash = xbox_live_response_json["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .unwrap()
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
    let xbox_xsts_response = match client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&xbox_xsts_response_request_data)
        .send()
        .await
    {
        Ok(ok) => ok.text().await.unwrap(),
        Err(e) => panic!("{e}"),
    };
    let xbox_xsts_response_json: Value = serde_json::from_str(&xbox_xsts_response).unwrap();
    let xsts_token = xbox_xsts_response_json["Token"]
        .as_str()
        .unwrap()
        .to_owned();
    XboxLiveData {
        xsts_token,
        user_hash,
    }
}
pub async fn login_to_minecraft(xbox_data: XboxLiveData) -> MinecraftAccount {
    let client = Client::new();
    let minecraft_data_response_request_data = json!(
        {
        "identityToken": format!("XBL3.0 x={};{}", xbox_data.user_hash, xbox_data.xsts_token)
        }
    );
    let minecraft_data_response = match client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&minecraft_data_response_request_data)
        .send()
        .await
    {
        Ok(ok) => ok.text().await.unwrap(),
        Err(e) => panic!("{e}"),
    };
    let minecraft_data_json: Value = serde_json::from_str(&minecraft_data_response).unwrap();
    let token = minecraft_data_json["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let mc_profile_response = match client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(token.clone())
        .send()
        .await
    {
        Ok(ok) => ok.text().await.unwrap(),
        Err(e) => panic!("{e}"),
    };
    let mc_profile_json: Value = serde_json::from_str(&mc_profile_response).unwrap();
    let uuid = mc_profile_json["id"].as_str().unwrap().to_owned();
    let username = mc_profile_json["name"].as_str().unwrap().to_owned();
    MinecraftAccount {
        username,
        token,
        uuid,
    }
}
pub async fn login_with_refresh_token(refresh_token: String) -> Option<MinecraftAccount> {
    let client = Client::new();
    let response = match client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&scope={}&grant_type={}&refresh_token={}",
            AZURE_CLIENT_ID, "XboxLive.signin offline_access", "refresh_token", refresh_token
        ))
        .send()
        .await
    {
        Ok(ok) => ok.text().await.unwrap(),
        Err(_) => return None,
    };
    let response_json: Value = serde_json::from_str(&response).unwrap();
    let access_token = response_json["access_token"].as_str().unwrap().to_owned();
    let xbox_data = login_to_xbox(access_token).await;
    Some(login_to_minecraft(xbox_data).await)
}
