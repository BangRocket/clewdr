use std::sync::LazyLock;

use axum::{
    Json,
    extract::Query,
    response::{IntoResponse, Redirect, Response},
};
use moka::sync::Cache;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
    basic::BasicClient,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;
use wreq::StatusCode;

use crate::config::CLEWDR_CONFIG;

// OAuth URLs
const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

// Store PKCE verifiers and CSRF tokens temporarily (5 minute TTL)
static OAUTH_STATE_CACHE: LazyLock<Cache<String, OAuthState>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(100)
        .time_to_live(std::time::Duration::from_secs(300))
        .build()
});

// Store authenticated sessions (24 hour TTL)
static SESSION_CACHE: LazyLock<Cache<String, OAuthSession>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(1000)
        .time_to_live(std::time::Duration::from_secs(86400))
        .build()
});

#[derive(Clone)]
struct OAuthState {
    pkce_verifier: String,
    provider: String,
}

#[derive(Clone, Serialize)]
pub struct OAuthSession {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub provider: String,
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
    email: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct GoogleUser {
    id: String,
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
}

/// Get available OAuth providers
pub async fn api_oauth_providers() -> Json<serde_json::Value> {
    let config = CLEWDR_CONFIG.load();
    Json(json!({
        "github": config.github_oauth_configured(),
        "google": config.google_oauth_configured(),
    }))
}

/// Initiate GitHub OAuth login
pub async fn api_oauth_github_login() -> Response {
    let config = CLEWDR_CONFIG.load();

    if !config.github_oauth_configured() {
        return (StatusCode::NOT_FOUND, "GitHub OAuth not configured").into_response();
    }

    let client_id = config.github_client_id.as_ref().unwrap();
    let redirect_uri = build_redirect_uri("github");

    let client = BasicClient::new(ClientId::new(client_id.clone()))
        .set_auth_uri(AuthUrl::new(GITHUB_AUTH_URL.to_string()).unwrap())
        .set_token_uri(TokenUrl::new(GITHUB_TOKEN_URL.to_string()).unwrap())
        .set_redirect_uri(RedirectUrl::new(redirect_uri).unwrap());

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user:email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Store state for callback
    OAUTH_STATE_CACHE.insert(
        csrf_token.secret().clone(),
        OAuthState {
            pkce_verifier: pkce_verifier.secret().clone(),
            provider: "github".to_string(),
        },
    );

    Redirect::temporary(auth_url.as_str()).into_response()
}

/// Initiate Google OAuth login
pub async fn api_oauth_google_login() -> Response {
    let config = CLEWDR_CONFIG.load();

    if !config.google_oauth_configured() {
        return (StatusCode::NOT_FOUND, "Google OAuth not configured").into_response();
    }

    let client_id = config.google_client_id.as_ref().unwrap();
    let redirect_uri = build_redirect_uri("google");

    let client = BasicClient::new(ClientId::new(client_id.clone()))
        .set_auth_uri(AuthUrl::new(GOOGLE_AUTH_URL.to_string()).unwrap())
        .set_token_uri(TokenUrl::new(GOOGLE_TOKEN_URL.to_string()).unwrap())
        .set_redirect_uri(RedirectUrl::new(redirect_uri).unwrap());

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Store state for callback
    OAUTH_STATE_CACHE.insert(
        csrf_token.secret().clone(),
        OAuthState {
            pkce_verifier: pkce_verifier.secret().clone(),
            provider: "google".to_string(),
        },
    );

    Redirect::temporary(auth_url.as_str()).into_response()
}

/// Handle GitHub OAuth callback
pub async fn api_oauth_github_callback(
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    let state = match OAUTH_STATE_CACHE.get(&query.state) {
        Some(s) => s,
        None => {
            return redirect_with_error("Invalid or expired state");
        }
    };

    if state.provider != "github" {
        return redirect_with_error("Provider mismatch");
    }

    // Remove state from cache (one-time use)
    OAUTH_STATE_CACHE.invalidate(&query.state);

    let config = CLEWDR_CONFIG.load();
    let client_id = config.github_client_id.as_ref().unwrap();
    let client_secret = config.github_client_secret.as_ref().unwrap();
    let redirect_uri = build_redirect_uri("github");

    let client = BasicClient::new(ClientId::new(client_id.clone()))
        .set_client_secret(ClientSecret::new(client_secret.clone()))
        .set_auth_uri(AuthUrl::new(GITHUB_AUTH_URL.to_string()).unwrap())
        .set_token_uri(TokenUrl::new(GITHUB_TOKEN_URL.to_string()).unwrap())
        .set_redirect_uri(RedirectUrl::new(redirect_uri).unwrap());

    let http_client = oauth2::reqwest::Client::new();
    let token_result = client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(PkceCodeVerifier::new(state.pkce_verifier))
        .request_async(&http_client)
        .await;

    let token = match token_result {
        Ok(t) => t,
        Err(e) => {
            error!("GitHub token exchange failed: {:?}", e);
            return redirect_with_error("Token exchange failed");
        }
    };

    // Fetch user info from GitHub
    let wreq_client = wreq::Client::new();
    let user_response = wreq_client
        .get(GITHUB_USER_URL)
        .header("Authorization", format!("Bearer {}", token.access_token().secret()))
        .header("User-Agent", "ClewdR")
        .send()
        .await;

    let user: GitHubUser = match user_response {
        Ok(resp) => match resp.json().await {
            Ok(u) => u,
            Err(e) => {
                error!("Failed to parse GitHub user: {:?}", e);
                return redirect_with_error("Failed to get user info");
            }
        },
        Err(e) => {
            error!("GitHub user request failed: {:?}", e);
            return redirect_with_error("Failed to get user info");
        }
    };

    // Create session
    let session_token = Uuid::new_v4().to_string();
    let session = OAuthSession {
        user_id: user.id.to_string(),
        username: user.login,
        email: user.email,
        provider: "github".to_string(),
        avatar_url: user.avatar_url,
    };

    SESSION_CACHE.insert(session_token.clone(), session);
    info!("GitHub OAuth login successful");

    // Redirect to frontend with session token
    Redirect::temporary(&format!("/?oauth_token={}", session_token)).into_response()
}

/// Handle Google OAuth callback
pub async fn api_oauth_google_callback(
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    let state = match OAUTH_STATE_CACHE.get(&query.state) {
        Some(s) => s,
        None => {
            return redirect_with_error("Invalid or expired state");
        }
    };

    if state.provider != "google" {
        return redirect_with_error("Provider mismatch");
    }

    // Remove state from cache (one-time use)
    OAUTH_STATE_CACHE.invalidate(&query.state);

    let config = CLEWDR_CONFIG.load();
    let client_id = config.google_client_id.as_ref().unwrap();
    let client_secret = config.google_client_secret.as_ref().unwrap();
    let redirect_uri = build_redirect_uri("google");

    let client = BasicClient::new(ClientId::new(client_id.clone()))
        .set_client_secret(ClientSecret::new(client_secret.clone()))
        .set_auth_uri(AuthUrl::new(GOOGLE_AUTH_URL.to_string()).unwrap())
        .set_token_uri(TokenUrl::new(GOOGLE_TOKEN_URL.to_string()).unwrap())
        .set_redirect_uri(RedirectUrl::new(redirect_uri).unwrap());

    let http_client = oauth2::reqwest::Client::new();
    let token_result = client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(PkceCodeVerifier::new(state.pkce_verifier))
        .request_async(&http_client)
        .await;

    let token = match token_result {
        Ok(t) => t,
        Err(e) => {
            error!("Google token exchange failed: {:?}", e);
            return redirect_with_error("Token exchange failed");
        }
    };

    // Fetch user info from Google
    let wreq_client = wreq::Client::new();
    let user_response = wreq_client
        .get(GOOGLE_USER_URL)
        .header("Authorization", format!("Bearer {}", token.access_token().secret()))
        .send()
        .await;

    let user: GoogleUser = match user_response {
        Ok(resp) => match resp.json().await {
            Ok(u) => u,
            Err(e) => {
                error!("Failed to parse Google user: {:?}", e);
                return redirect_with_error("Failed to get user info");
            }
        },
        Err(e) => {
            error!("Google user request failed: {:?}", e);
            return redirect_with_error("Failed to get user info");
        }
    };

    // Create session
    let session_token = Uuid::new_v4().to_string();
    let session = OAuthSession {
        user_id: user.id,
        username: user.name.unwrap_or_else(|| "Unknown".to_string()),
        email: user.email,
        provider: "google".to_string(),
        avatar_url: user.picture,
    };

    SESSION_CACHE.insert(session_token.clone(), session);
    info!("Google OAuth login successful");

    // Redirect to frontend with session token
    Redirect::temporary(&format!("/?oauth_token={}", session_token)).into_response()
}

/// Validate an OAuth session token
pub async fn api_oauth_validate(
    axum_auth::AuthBearer(token): axum_auth::AuthBearer,
) -> Response {
    if let Some(session) = SESSION_CACHE.get(&token) {
        (StatusCode::OK, Json(session)).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid session").into_response()
    }
}

/// Logout - invalidate session
pub async fn api_oauth_logout(
    axum_auth::AuthBearer(token): axum_auth::AuthBearer,
) -> StatusCode {
    SESSION_CACHE.invalidate(&token);
    StatusCode::OK
}

/// Check if an OAuth token is valid for admin access
pub fn oauth_admin_auth(token: &str) -> bool {
    SESSION_CACHE.get(token).is_some()
}

fn build_redirect_uri(provider: &str) -> String {
    // In production, this should be configurable
    // For now, assume the app is running on localhost or behind a reverse proxy
    format!("/api/auth/oauth/{}/callback", provider)
}

fn redirect_with_error(error: &str) -> Response {
    Redirect::temporary(&format!("/?oauth_error={}", urlencoding::encode(error))).into_response()
}
