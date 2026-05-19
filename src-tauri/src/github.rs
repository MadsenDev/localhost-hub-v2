use serde::{Deserialize, Serialize};

// Set your GitHub OAuth App client_id here.
// Create one at https://github.com/settings/developers — enable "Device Authorization Flow".
// The client_id is public — device flow requires no client secret.
pub const CLIENT_ID: &str = env!("GITHUB_CLIENT_ID");

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

pub async fn request_device_code() -> Result<DeviceCodeResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .header("User-Agent", "localhost-hub")
        .form(&[("client_id", CLIENT_ID), ("scope", "repo user read:org")])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let code: DeviceCodeResponse = resp.json().await.map_err(|e| e.to_string())?;
    let _ = open::that(&code.verification_uri);
    Ok(code)
}

/// Returns (access_token, GitHubUser). The token stays in Rust — callers save it to config.
pub async fn poll_token(device_code: &str) -> Result<(String, GitHubUser), String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "localhost-hub")
        .form(&[
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let token_resp: DeviceTokenResponse = resp.json().await.map_err(|e| e.to_string())?;

    if let Some(err) = token_resp.error {
        return Err(err);
    }

    let token = token_resp.access_token.ok_or("no access_token in response")?;
    let user = fetch_user(&token).await?;
    Ok((token, user))
}

pub async fn fetch_user(token: &str) -> Result<GitHubUser, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "localhost-hub")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    resp.json::<GitHubUser>().await.map_err(|e| e.to_string())
}
