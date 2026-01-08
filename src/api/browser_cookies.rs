use axum::Json;
use axum_auth::AuthBearer;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use tracing::{info, warn};

use super::error::ApiError;
use crate::config::CLEWDR_CONFIG;

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub browser: String,
    pub profile: String,
}

#[derive(Debug, Serialize)]
pub struct BrowserCookieResponse {
    pub cookies: Vec<BrowserCookie>,
    pub errors: Vec<String>,
}

/// Get default browser profile paths
fn get_browser_profile_paths() -> Vec<(String, PathBuf)> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let local_app_data = PathBuf::from(local_app_data);
            // Chrome
            paths.push((
                "Chrome".to_string(),
                local_app_data.join("Google/Chrome/User Data"),
            ));
            // Edge
            paths.push((
                "Edge".to_string(),
                local_app_data.join("Microsoft/Edge/User Data"),
            ));
            // Brave
            paths.push((
                "Brave".to_string(),
                local_app_data.join("BraveSoftware/Brave-Browser/User Data"),
            ));
        }
        if let Some(app_data) = std::env::var_os("APPDATA") {
            let app_data = PathBuf::from(app_data);
            // Firefox
            paths.push(("Firefox".to_string(), app_data.join("Mozilla/Firefox/Profiles")));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            // Chrome
            paths.push((
                "Chrome".to_string(),
                home.join("Library/Application Support/Google/Chrome"),
            ));
            // Edge
            paths.push((
                "Edge".to_string(),
                home.join("Library/Application Support/Microsoft Edge"),
            ));
            // Brave
            paths.push((
                "Brave".to_string(),
                home.join("Library/Application Support/BraveSoftware/Brave-Browser"),
            ));
            // Firefox
            paths.push((
                "Firefox".to_string(),
                home.join("Library/Application Support/Firefox/Profiles"),
            ));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            // Chrome
            paths.push(("Chrome".to_string(), home.join(".config/google-chrome")));
            // Chromium
            paths.push(("Chromium".to_string(), home.join(".config/chromium")));
            // Edge
            paths.push((
                "Edge".to_string(),
                home.join(".config/microsoft-edge"),
            ));
            // Brave
            paths.push((
                "Brave".to_string(),
                home.join(".config/BraveSoftware/Brave-Browser"),
            ));
            // Firefox
            paths.push(("Firefox".to_string(), home.join(".mozilla/firefox")));
        }
    }

    paths
}

/// Extract cookies from Firefox's cookies.sqlite
fn extract_firefox_cookies(
    profile_path: &PathBuf,
    domain_filter: &str,
) -> Result<Vec<BrowserCookie>, String> {
    // Find cookies.sqlite in profile
    let cookies_db = profile_path.join("cookies.sqlite");
    if !cookies_db.exists() {
        return Err(format!("cookies.sqlite not found at {:?}", cookies_db));
    }

    // Copy the database to a temp file since Firefox locks it
    let temp_dir = std::env::temp_dir();
    let temp_db = temp_dir.join(format!("clewdr_firefox_cookies_{}.sqlite", std::process::id()));

    std::fs::copy(&cookies_db, &temp_db)
        .map_err(|e| format!("Failed to copy cookies database: {}", e))?;

    let result = extract_from_sqlite(&temp_db, domain_filter, "Firefox", profile_path);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_db);

    result
}

/// Extract cookies from Chromium-based browsers
fn extract_chromium_cookies(
    browser_name: &str,
    profile_path: &PathBuf,
    domain_filter: &str,
) -> Result<Vec<BrowserCookie>, String> {
    // Find Cookies database
    let cookies_db = profile_path.join("Cookies");
    if !cookies_db.exists() {
        // Try Network/Cookies for newer Chrome versions
        let network_cookies = profile_path.join("Network/Cookies");
        if network_cookies.exists() {
            return extract_chromium_cookies_inner(browser_name, &network_cookies, profile_path, domain_filter);
        }
        return Err(format!("Cookies database not found at {:?}", cookies_db));
    }

    extract_chromium_cookies_inner(browser_name, &cookies_db, profile_path, domain_filter)
}

fn extract_chromium_cookies_inner(
    browser_name: &str,
    cookies_db: &PathBuf,
    profile_path: &PathBuf,
    domain_filter: &str,
) -> Result<Vec<BrowserCookie>, String> {
    // Copy the database to a temp file since the browser locks it
    let temp_dir = std::env::temp_dir();
    let temp_db = temp_dir.join(format!("clewdr_chrome_cookies_{}.sqlite", std::process::id()));

    std::fs::copy(cookies_db, &temp_db)
        .map_err(|e| format!("Failed to copy cookies database: {}", e))?;

    // Note: Chromium cookies are encrypted on Windows and macOS
    // On Linux, they may be encrypted with the system keyring or stored in plaintext
    // For now, we'll try to read them and note that decryption may be needed
    let result = extract_from_sqlite_chromium(&temp_db, domain_filter, browser_name, profile_path);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_db);

    result
}

fn extract_from_sqlite(
    db_path: &PathBuf,
    domain_filter: &str,
    browser: &str,
    profile_path: &PathBuf,
) -> Result<Vec<BrowserCookie>, String> {
    use rusqlite::Connection;

    let conn = Connection::open(db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let mut stmt = conn
        .prepare(
            "SELECT name, value, host FROM moz_cookies WHERE host LIKE ?1 OR host LIKE ?2",
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let domain_pattern = format!("%{}", domain_filter);
    let domain_pattern2 = format!(".{}", domain_filter);

    let cookie_iter = stmt
        .query_map([&domain_pattern, &domain_pattern2], |row| {
            Ok(BrowserCookie {
                name: row.get(0)?,
                value: row.get(1)?,
                domain: row.get(2)?,
                path: "/".to_string(),
                browser: browser.to_string(),
                profile: profile_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| format!("Failed to query: {}", e))?;

    let mut cookies = Vec::new();
    for cookie in cookie_iter {
        if let Ok(c) = cookie {
            cookies.push(c);
        }
    }

    Ok(cookies)
}

fn extract_from_sqlite_chromium(
    db_path: &PathBuf,
    domain_filter: &str,
    browser: &str,
    profile_path: &PathBuf,
) -> Result<Vec<BrowserCookie>, String> {
    use rusqlite::Connection;

    let conn = Connection::open(db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Chromium uses encrypted_value column, but on some systems value column has plaintext
    let mut stmt = conn
        .prepare(
            "SELECT name, value, host_key, path FROM cookies WHERE host_key LIKE ?1 OR host_key LIKE ?2",
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let domain_pattern = format!("%{}", domain_filter);
    let domain_pattern2 = format!(".{}", domain_filter);

    let cookie_iter = stmt
        .query_map([&domain_pattern, &domain_pattern2], |row| {
            let value: String = row.get(1)?;
            Ok(BrowserCookie {
                name: row.get(0)?,
                value,
                domain: row.get(2)?,
                path: row.get(3)?,
                browser: browser.to_string(),
                profile: profile_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| format!("Failed to query: {}", e))?;

    let mut cookies = Vec::new();
    for cookie in cookie_iter {
        if let Ok(c) = cookie {
            // Skip cookies with empty values (likely encrypted)
            if !c.value.is_empty() {
                cookies.push(c);
            }
        }
    }

    if cookies.is_empty() {
        return Err(format!(
            "{} cookies are encrypted. Please export cookies manually using a browser extension.",
            browser
        ));
    }

    Ok(cookies)
}

/// Find profile directories for a browser
fn find_profiles(browser_path: &PathBuf, browser_name: &str) -> Vec<PathBuf> {
    let mut profiles = Vec::new();

    if !browser_path.exists() {
        return profiles;
    }

    if browser_name == "Firefox" {
        // Firefox profiles are in subdirectories
        if let Ok(entries) = std::fs::read_dir(browser_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("cookies.sqlite").exists() {
                    profiles.push(path);
                }
            }
        }
    } else {
        // Chromium-based browsers have Default and Profile N directories
        let default_profile = browser_path.join("Default");
        if default_profile.exists() {
            profiles.push(default_profile);
        }

        // Check for numbered profiles
        if let Ok(entries) = std::fs::read_dir(browser_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().map(|s| s.to_string_lossy().to_string());
                if let Some(name) = name {
                    if name.starts_with("Profile ") && path.is_dir() {
                        profiles.push(path);
                    }
                }
            }
        }
    }

    profiles
}

/// API endpoint to extract browser cookies for Claude
pub async fn api_get_browser_cookies(
    AuthBearer(t): AuthBearer,
) -> Result<Json<BrowserCookieResponse>, ApiError> {
    if !CLEWDR_CONFIG.load().admin_auth(&t) {
        return Err(ApiError::unauthorized());
    }

    let mut all_cookies = Vec::new();
    let mut errors = Vec::new();
    let domain_filter = "claude.ai";

    let browser_paths = get_browser_profile_paths();

    for (browser_name, browser_path) in browser_paths {
        if !browser_path.exists() {
            continue;
        }

        let profiles = find_profiles(&browser_path, &browser_name);
        if profiles.is_empty() {
            continue;
        }

        for profile_path in profiles {
            let result = if browser_name == "Firefox" {
                extract_firefox_cookies(&profile_path, domain_filter)
            } else {
                extract_chromium_cookies(&browser_name, &profile_path, domain_filter)
            };

            match result {
                Ok(cookies) => {
                    info!(
                        "Found {} cookies from {} profile {:?}",
                        cookies.len(),
                        browser_name,
                        profile_path
                    );
                    all_cookies.extend(cookies);
                }
                Err(e) => {
                    warn!(
                        "Failed to extract cookies from {} profile {:?}: {}",
                        browser_name, profile_path, e
                    );
                    errors.push(format!("{} ({:?}): {}", browser_name, profile_path.file_name().unwrap_or_default(), e));
                }
            }
        }
    }

    Ok(Json(BrowserCookieResponse {
        cookies: all_cookies,
        errors,
    }))
}

/// API endpoint to get the session cookie value formatted for ClewdR
pub async fn api_get_browser_session_cookie(
    AuthBearer(t): AuthBearer,
) -> Result<Json<Value>, ApiError> {
    if !CLEWDR_CONFIG.load().admin_auth(&t) {
        return Err(ApiError::unauthorized());
    }

    let response = api_get_browser_cookies(AuthBearer(t.clone())).await?;
    let cookies = response.0.cookies;

    // Look for the sessionKey cookie
    let session_cookie = cookies
        .iter()
        .find(|c| c.name == "sessionKey" || c.name.starts_with("sk-ant-sid"));

    match session_cookie {
        Some(cookie) => Ok(Json(json!({
            "found": true,
            "cookie": cookie.value,
            "browser": cookie.browser,
            "profile": cookie.profile,
            "errors": response.0.errors,
        }))),
        None => Ok(Json(json!({
            "found": false,
            "cookie": null,
            "errors": response.0.errors,
            "message": "No Claude session cookie found. Make sure you're logged into Claude.ai in your browser."
        }))),
    }
}
