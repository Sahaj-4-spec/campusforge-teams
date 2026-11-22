use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use chrono::{Duration, Utc};
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use url::Url;
use uuid::Uuid;

use crate::{
    auth::{issue_token, session_cookie_header},
    state::AppState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Provider {
    Google,
    Github,
    Discord,
}

impl Provider {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "google" => Some(Self::Google),
            "github" => Some(Self::Github),
            "discord" => Some(Self::Discord),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Github => "github",
            Self::Discord => "discord",
        }
    }

    fn config(self) -> Result<ProviderConfig, &'static str> {
        let (client_id_key, client_secret_key, authorization_endpoint, token_endpoint, scopes) =
            match self {
                Self::Google => (
                    "GOOGLE_CLIENT_ID",
                    "GOOGLE_CLIENT_SECRET",
                    "https://accounts.google.com/o/oauth2/v2/auth",
                    "https://oauth2.googleapis.com/token",
                    "openid email profile",
                ),
                Self::Github => (
                    "GITHUB_CLIENT_ID",
                    "GITHUB_CLIENT_SECRET",
                    "https://github.com/login/oauth/authorize",
                    "https://github.com/login/oauth/access_token",
                    "read:user user:email",
                ),
                Self::Discord => (
                    "DISCORD_CLIENT_ID",
                    "DISCORD_CLIENT_SECRET",
                    "https://discord.com/oauth2/authorize",
                    "https://discord.com/api/oauth2/token",
                    "identify email",
                ),
            };

        let client_id = std::env::var(client_id_key)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or("This sign-in provider is not configured yet.")?;
        let client_secret = std::env::var(client_secret_key)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or("This sign-in provider is not configured yet.")?;

        Ok(ProviderConfig {
            client_id,
            client_secret,
            authorization_endpoint,
            token_endpoint,
            scopes,
        })
    }
}

struct ProviderConfig {
    client_id: String,
    client_secret: String,
    authorization_endpoint: &'static str,
    token_endpoint: &'static str,
    scopes: &'static str,
}

#[derive(Debug, Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
}

#[derive(Debug)]
struct VerifiedProfile {
    provider_user_id: String,
    email: String,
    full_name: String,
    avatar_url: Option<String>,
}

#[derive(Debug, FromRow)]
struct OAuthUser {
    id: Uuid,
    email: String,
}

#[derive(Debug, Deserialize)]
struct GoogleProfile {
    sub: String,
    email: String,
    email_verified: bool,
    name: Option<String>,
    picture: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubProfile {
    id: i64,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

#[derive(Debug, Deserialize)]
struct DiscordProfile {
    id: String,
    username: String,
    global_name: Option<String>,
    email: Option<String>,
    verified: Option<bool>,
    avatar: Option<String>,
}

fn callback_uri(provider: Provider) -> Result<String, &'static str> {
    let base = std::env::var("OAUTH_REDIRECT_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:5000".to_owned());
    let mut url = Url::parse(&base).map_err(|_| "OAuth callback base URL is invalid.")?;
    let host = url.host_str().unwrap_or_default();
    let is_local_host = host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]";
    if url.scheme() != "https" && !is_local_host {
        return Err("Public OAuth callback URLs must use HTTPS.");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("OAuth callback base URL must not contain credentials.");
    }
    let prefix = url.path().trim_end_matches('/');
    url.set_path(&format!("{prefix}/api/auth/{}/callback", provider.as_str()));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

fn frontend_redirect(error: Option<&str>) -> Redirect {
    let origin = std::env::var("FRONTEND_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_owned());
    let mut url = Url::parse(&origin)
        .unwrap_or_else(|_| Url::parse("http://localhost:5173").expect("valid local fallback URL"));
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);

    if let Some(error) = error {
        url.query_pairs_mut().append_pair("oauth_error", error);
    }

    Redirect::temporary(url.as_str())
}

fn oauth_cookie_name(state_value: &str) -> String {
    let digest = Sha256::digest(state_value.as_bytes());
    format!("cf_oauth_{}", hex_prefix(&digest, 12))
}

fn hex_prefix(bytes: &[u8], count: usize) -> String {
    bytes.iter().take(count).map(|byte| format!("{byte:02x}")).collect()
}

fn state_digest(state_value: &str, browser_binding: &str) -> Vec<u8> {
    Sha256::digest(format!("{state_value}:{browser_binding}").as_bytes()).to_vec()
}

fn browser_binding_from_headers(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == cookie_name && !value.is_empty()).then(|| value.to_owned())
    })
}

async fn start(
    Path(provider_name): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let Some(provider) = Provider::parse(&provider_name) else {
        return frontend_redirect(Some("That sign-in provider is not supported.")).into_response();
    };
    let config = match provider.config() {
        Ok(config) => config,
        Err(message) => return frontend_redirect(Some(message)).into_response(),
    };
    let redirect_uri = match callback_uri(provider) {
        Ok(uri) => uri,
        Err(message) => return frontend_redirect(Some(message)).into_response(),
    };

    // Bind OAuth state to a short-lived HttpOnly browser cookie, preventing
    // an attacker from completing a login in a victim's unrelated browser.
    let state_value = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let browser_binding = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let state_hash = state_digest(&state_value, &browser_binding);
    let cookie_name = oauth_cookie_name(&state_value);
    let secure_cookie = std::env::var("OAUTH_REDIRECT_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:5000".to_owned())
        .starts_with("https://");

    if let Err(error) = sqlx::query("DELETE FROM oauth_states WHERE expires_at <= NOW()")
        .execute(&state.db)
        .await
    {
        tracing::error!(%error, "Could not clean expired OAuth state");
        return frontend_redirect(Some("Sign-in is temporarily unavailable.")).into_response();
    }

    if let Err(error) = sqlx::query(
        "INSERT INTO oauth_states (state_hash, provider, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(state_hash.as_slice())
    .bind(provider.as_str())
    .bind(Utc::now() + Duration::minutes(10))
    .execute(&state.db)
    .await
    {
        tracing::error!(%error, "Could not save OAuth state");
        return frontend_redirect(Some("Sign-in is temporarily unavailable.")).into_response();
    }

    let mut authorization_url = match Url::parse(config.authorization_endpoint) {
        Ok(url) => url,
        Err(_) => return frontend_redirect(Some("Sign-in is temporarily unavailable.")).into_response(),
    };

    authorization_url
        .query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", config.scopes)
        .append_pair("state", &state_value);

    if provider == Provider::Google {
        authorization_url.query_pairs_mut().append_pair("prompt", "select_account");
    }
    if provider == Provider::Github {
        authorization_url.query_pairs_mut().append_pair("allow_signup", "true");
    }

    let mut response = Redirect::temporary(authorization_url.as_str()).into_response();
    let secure_attribute = if secure_cookie { "; Secure" } else { "" };
    let cookie = format!(
        "{cookie_name}={browser_binding}; HttpOnly; SameSite=Lax; Path=/api/auth; Max-Age=600{secure_attribute}"
    );
    match HeaderValue::from_str(&cookie) {
        Ok(value) => {
            response.headers_mut().insert(header::SET_COOKIE, value);
            response
        }
        Err(error) => {
            tracing::error!(%error, "Could not create OAuth browser binding cookie");
            frontend_redirect(Some("Sign-in is temporarily unavailable.")).into_response()
        }
    }
}

async fn callback(
    Path(provider_name): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    let cookie_name = query.state.as_deref().map(oauth_cookie_name);
    let browser_binding = cookie_name.as_deref()
        .and_then(|name| browser_binding_from_headers(&headers, name));
    let result = match browser_binding.as_deref() {
        Some(binding) => complete_login(&state, &provider_name, query, binding).await,
        None => Err("The sign-in session expired. Please start sign-in again."),
    };
    let mut response = match result {
        Ok((token, expires_in)) => {
            let mut response = frontend_redirect(None).into_response();
            match session_cookie_header(&token, expires_in) {
                Ok(cookie) => response.headers_mut().append(header::SET_COOKIE, cookie),
                Err(_) => return frontend_redirect(Some("CampusForge could not create your session.")).into_response(),
            };
            response
        }
        Err(message) => frontend_redirect(Some(message)).into_response(),
    };
    if let Some(cookie_name) = cookie_name {
        let secure_attribute = if std::env::var("OAUTH_REDIRECT_BASE_URL")
            .unwrap_or_default().starts_with("https://") { "; Secure" } else { "" };
        let expired = format!(
            "{cookie_name}=; HttpOnly; SameSite=Lax; Path=/api/auth; Max-Age=0{secure_attribute}"
        );
        if let Ok(value) = HeaderValue::from_str(&expired) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

async fn complete_login(
    state: &AppState,
    provider_name: &str,
    query: OAuthCallbackQuery,
    browser_binding: &str,
) -> Result<(String, i64), &'static str> {
    let provider = Provider::parse(provider_name)
        .ok_or("That sign-in provider is not supported.")?;
    let config = provider.config()?;

    let state_value = query.state.as_deref()
        .ok_or("The sign-in session expired. Please try again.")?;
    consume_state(state, provider, state_value, browser_binding).await?;

    if query.error.is_some() {
        return Err("Sign-in was cancelled. You can try again.");
    }

    let code = query.code.as_deref()
        .ok_or("The identity provider did not return an authorization code.")?;
    let redirect_uri = callback_uri(provider)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("CampusForgeTeams/1.0")
        .build()
        .map_err(|_| "Sign-in is temporarily unavailable.")?;

    let token_response = client
        .post(config.token_endpoint)
        .header(ACCEPT, "application/json")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|_| "The identity provider could not complete sign-in.")?;

    if !token_response.status().is_success() {
        tracing::warn!(provider = provider.as_str(), "OAuth token exchange was rejected");
        return Err("The identity provider could not complete sign-in.");
    }

    let token_payload = token_response
        .json::<OAuthTokenResponse>()
        .await
        .map_err(|_| "The identity provider returned an unreadable token response.")?;
    let access_token = token_payload.access_token
        .filter(|value| !value.trim().is_empty())
        .ok_or("The identity provider did not issue an access token.")?;

    let profile = fetch_verified_profile(&client, provider, &access_token).await?;
    let user = find_or_create_user(state, provider, profile).await?;

    let (token, expires_in) = issue_token(user.id, &user.email, &state.jwt_secret)
        .map_err(|_| "CampusForge could not create your session.")?;

    Ok((token, expires_in))
}

async fn consume_state(
    state: &AppState,
    provider: Provider,
    state_value: &str,
    browser_binding: &str,
) -> Result<(), &'static str> {
    if state_value.len() < 32 || state_value.len() > 256 {
        return Err("The sign-in session expired. Please try again.");
    }

    let state_hash = state_digest(state_value, browser_binding);
    let stored_provider = sqlx::query_scalar::<_, String>(
        "DELETE FROM oauth_states
         WHERE state_hash = $1 AND provider = $2 AND expires_at > NOW()
         RETURNING provider",
    )
    .bind(state_hash.as_slice())
    .bind(provider.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "Could not validate OAuth state");
        "Sign-in is temporarily unavailable."
    })?;

    if stored_provider.as_deref() != Some(provider.as_str()) {
        return Err("The sign-in session expired. Please try again.");
    }

    Ok(())
}

async fn fetch_verified_profile(
    client: &reqwest::Client,
    provider: Provider,
    access_token: &str,
) -> Result<VerifiedProfile, &'static str> {
    match provider {
        Provider::Google => {
            let response = client
                .get("https://openidconnect.googleapis.com/v1/userinfo")
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(|_| "Google could not return your account details.")?;

            if !response.status().is_success() {
                return Err("Google could not verify your account details.");
            }

            let profile = response.json::<GoogleProfile>()
                .await
                .map_err(|_| "Google returned an unreadable account profile.")?;

            if !profile.email_verified || profile.email.trim().is_empty() || profile.sub.trim().is_empty() {
                return Err("Use a Google account with a verified email address.");
            }

            Ok(VerifiedProfile {
                provider_user_id: profile.sub,
                email: profile.email,
                full_name: profile.name.unwrap_or_default(),
                avatar_url: profile.picture,
            })
        }
        Provider::Github => {
            let response = client
                .get("https://api.github.com/user")
                .bearer_auth(access_token)
                .header(USER_AGENT, "CampusForgeTeams/1.0")
                .send()
                .await
                .map_err(|_| "GitHub could not return your account details.")?;

            if !response.status().is_success() {
                return Err("GitHub could not verify your account details.");
            }

            let profile = response.json::<GithubProfile>()
                .await
                .map_err(|_| "GitHub returned an unreadable account profile.")?;

            let emails_response = client
                .get("https://api.github.com/user/emails")
                .bearer_auth(access_token)
                .header(USER_AGENT, "CampusForgeTeams/1.0")
                .send()
                .await
                .map_err(|_| "GitHub could not return your verified email.")?;

            if !emails_response.status().is_success() {
                return Err("Grant GitHub email access to continue signing in.");
            }

            let emails = emails_response.json::<Vec<GithubEmail>>()
                .await
                .map_err(|_| "GitHub returned an unreadable email list.")?;

            let email = emails
                .iter()
                .find(|item| item.primary && item.verified)
                .or_else(|| emails.iter().find(|item| item.verified))
                .map(|item| item.email.clone())
                .ok_or("Your GitHub account needs a verified email address.")?;

            Ok(VerifiedProfile {
                provider_user_id: profile.id.to_string(),
                email,
                full_name: profile.name.filter(|value| !value.trim().is_empty()).unwrap_or(profile.login),
                avatar_url: profile.avatar_url,
            })
        }
        Provider::Discord => {
            let response = client
                .get("https://discord.com/api/users/@me")
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(|_| "Discord could not return your account details.")?;

            if !response.status().is_success() {
                return Err("Discord could not verify your account details.");
            }

            let profile = response.json::<DiscordProfile>()
                .await
                .map_err(|_| "Discord returned an unreadable account profile.")?;

            let email = profile.email
                .filter(|value| profile.verified.unwrap_or(false) && !value.trim().is_empty())
                .ok_or("Use a Discord account with a verified email address.")?;

            let avatar_url = profile.avatar.map(|hash| {
                let extension = if hash.starts_with("a_") { "gif" } else { "png" };
                format!("https://cdn.discordapp.com/avatars/{}/{hash}.{extension}", profile.id)
            });

            Ok(VerifiedProfile {
                provider_user_id: profile.id,
                email,
                full_name: profile.global_name.unwrap_or(profile.username),
                avatar_url,
            })
        }
    }
}

async fn find_or_create_user(
    state: &AppState,
    provider: Provider,
    profile: VerifiedProfile,
) -> Result<OAuthUser, &'static str> {
    let email = profile.email.trim().to_lowercase();
    if email.is_empty() {
        return Err("The identity provider did not return a verified email address.");
    }

    let full_name = {
        let name = profile.full_name.trim();
        if name.chars().count() < 2 {
            email.split('@').next().unwrap_or("CampusForge member").to_owned()
        } else {
            name.chars().take(80).collect::<String>()
        }
    };

    let mut tx = state.db.begin().await.map_err(|error| {
        tracing::error!(%error, "Could not start the social account transaction");
        "Sign-in is temporarily unavailable."
    })?;

    let subject_lock_digest = Sha256::digest(
        format!("{}:{}", provider.as_str(), profile.provider_user_id).as_bytes()
    );
    let subject_lock_key = i64::from_be_bytes(subject_lock_digest[..8].try_into().unwrap());
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(subject_lock_key)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Could not lock social identity");
            "Sign-in is temporarily unavailable."
        })?;

    let existing = sqlx::query_as::<_, OAuthUser>(
        "SELECT u.id, u.email
         FROM oauth_accounts oa
         JOIN users u ON u.id = oa.user_id
         WHERE oa.provider = $1 AND oa.provider_user_id = $2",
    )
    .bind(provider.as_str())
    .bind(&profile.provider_user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(%error, "Could not look up social account");
        "Sign-in is temporarily unavailable."
    })?;

    let user = if let Some(user) = existing {
        user
    } else {
        // Only verified provider emails reach this point. A match reuses the
        // existing CampusForge account instead of creating a duplicate.
        let existing_user = sqlx::query_as::<_, OAuthUser>(
            "SELECT id, email
             FROM users WHERE lower(email) = $1
             FOR UPDATE",
        )
        .bind(&email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Could not match verified social email");
            "Sign-in is temporarily unavailable."
        })?;

        let user = match existing_user {
            Some(_) => {
                return Err("A CampusForge account already uses this email. Sign in with that account first; automatic social linking is disabled.");
            }
            None => sqlx::query_as::<_, OAuthUser>(
                "INSERT INTO users (email, password_hash, full_name, avatar_url)
                 VALUES ($1, NULL, $2, $3)
                 RETURNING id, email",
            )
            .bind(&email)
            .bind(&full_name)
            .bind(&profile.avatar_url)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| {
                tracing::error!(%error, "Could not create social account");
                "An account with this email may already exist. Try signing in again."
            })?,
        };

        sqlx::query(
            "INSERT INTO oauth_accounts (provider, provider_user_id, user_id, email)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(provider.as_str())
        .bind(&profile.provider_user_id)
        .bind(user.id)
        .bind(&email)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Could not save social identity");
            "This provider is already connected to another sign-in. Contact support if this continues."
        })?;

        user
    };

    tx.commit().await.map_err(|error| {
        tracing::error!(%error, "Could not commit social account transaction");
        "Sign-in is temporarily unavailable."
    })?;

    Ok(user)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{provider}/start", get(start))
        .route("/{provider}/callback", get(callback))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_supported_providers_are_accepted() {
        assert_eq!(Provider::parse("google"), Some(Provider::Google));
        assert_eq!(Provider::parse("github"), Some(Provider::Github));
        assert_eq!(Provider::parse("discord"), Some(Provider::Discord));
        assert_eq!(Provider::parse("facebook"), None);
    }

    #[test]
    fn oauth_state_hash_is_not_the_original_value() {
        let state = "a-long-random-state-value-used-for-a-test";
        let digest = state_digest(state, "browser-binding");
        assert_ne!(digest.as_slice(), state.as_bytes());
        assert_eq!(digest.len(), 32);
    }
}
