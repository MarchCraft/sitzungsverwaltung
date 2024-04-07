use std::collections::HashMap;

use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::AccessToken;
use oauth2::AuthorizationCode;
use oauth2::ClientId;
use oauth2::ClientSecret;
use oauth2::CsrfToken;
use oauth2::HttpRequest;
use oauth2::PkceCodeChallenge;
use oauth2::RedirectUrl;
use oauth2::Scope;
use oauth2::TokenResponse;
use serde_json::json;

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct KeycloakConfig {
    pub url: String,
    pub realm: String,
    pub username: String,
    pub password: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct KeycloakUser {
    id: String,
    username: String,
    email: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    enabled: bool,
}

struct KeycloakClient {
    token: AccessToken,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
struct KeycloakRole {
    id: String,
    name: String,
}

pub async fn get_token() -> anyhow::Result<String> {
    let username_env = std::env::var("USERNAME").expect("No USERNAME set");
    let password_env = std::env::var("PASSWORD").expect("No PASSWORD set");
    let keycloak_config = KeycloakConfig {
        url: "https://login.inphima.de/auth".to_string(),
        realm: "FSCS-Intern".to_string(),
        username: username_env,
        password: password_env,
    };
    let client = KeycloakClient::new(
        keycloak_config.url.clone(),
        keycloak_config.realm.clone(),
        keycloak_config.username.clone(),
        keycloak_config.password.clone(),
    )
    .await?;

    Ok(client.token.secret().to_string())
}

impl KeycloakClient {
    async fn new(
        base_url: String,
        realm: String,
        user: String,
        password: String,
    ) -> anyhow::Result<Self> {
        let client_id = std::env::var("CLIENT_ID").expect("No CLIENT ID set");
        let client_secret = std::env::var("CLIENT_SECRET").expect("No CLIENT SECRET set");

        let client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            oauth2::AuthUrl::new(format!(
                "{}/realms/{}/protocol/openid-connect/auth",
                base_url, realm
            ))
            .unwrap(),
            Some(
                oauth2::TokenUrl::new(format!(
                    "{}/realms/{}/protocol/openid-connect/token",
                    base_url, realm
                ))
                .unwrap(),
            ),
        )
        .set_redirect_uri(RedirectUrl::new("http://localhost:8080/".to_string())?);

        let token = client
            .exchange_password(
                &oauth2::ResourceOwnerUsername::new(user.clone()),
                &oauth2::ResourceOwnerPassword::new(password.clone()),
            )
            .add_scope(Scope::new("openid".to_string()))
            .request_async(async_http_client)
            .await?
            .access_token()
            .clone();

        Ok(KeycloakClient { token })
    }
}
