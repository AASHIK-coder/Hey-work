use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(
        options: core_foundation::dictionary::CFDictionaryRef,
    ) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotAsked,
    NotNeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsCheck {
    pub accessibility: PermissionStatus,
    pub screen_recording: PermissionStatus,
    pub microphone: PermissionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileStatus {
    pub exists: bool,
    pub path: String,
    pub sessions: Vec<String>, // domains with cookies
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub anthropic: bool,
    pub deepgram: bool,
    pub elevenlabs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettings {
    pub elevenlabs_voice_id: Option<String>,
}

const KEYRING_SERVICE: &str = "com.heywork.app";

fn api_env_var_for_service(service: &str) -> Option<&'static str> {
    match service {
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "deepgram" => Some("DEEPGRAM_API_KEY"),
        "elevenlabs" => Some("ELEVENLABS_API_KEY"),
        _ => None,
    }
}

fn read_api_key_secure(var_name: &str) -> Option<String> {
    if let Ok(value) = std::env::var(var_name) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }

    let entry = keyring::Entry::new(KEYRING_SERVICE, var_name).ok()?;
    let key = entry.get_password().ok()?;
    if key.trim().is_empty() {
        return None;
    }
    std::env::set_var(var_name, key.clone());
    Some(key)
}

pub fn load_api_key_for_service(service: &str) -> Option<String> {
    let var_name = api_env_var_for_service(service)?;
    read_api_key_secure(var_name)
}

fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    let base = dirs::data_dir();
    #[cfg(not(target_os = "macos"))]
    let base = dirs::data_local_dir();

    base.unwrap_or_else(|| PathBuf::from(".")).join("hey-work")
}

fn browser_profile_path() -> PathBuf {
    app_data_dir().join("heywork-chrome")
}

fn find_chrome_binary() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mac_path = PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        return mac_path.exists().then_some(mac_path);
    }

    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").ok();
        let program_files = std::env::var("ProgramFiles").ok();
        let program_files_x86 = std::env::var("ProgramFiles(x86)").ok();
        let candidates = [
            local_app_data.map(|p| PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
            program_files.map(|p| PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
            program_files_x86.map(|p| PathBuf::from(p).join("Google/Chrome/Application/chrome.exe")),
        ];
        return candidates.into_iter().flatten().find(|p| p.exists());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

// check all permissions
#[tauri::command]
pub fn check_permissions() -> PermissionsCheck {
    #[cfg(target_os = "macos")]
    {
        PermissionsCheck {
            accessibility: check_accessibility(),
            screen_recording: check_screen_recording(),
            microphone: check_microphone(),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        PermissionsCheck {
            accessibility: PermissionStatus::NotNeeded,
            screen_recording: PermissionStatus::NotNeeded,
            microphone: PermissionStatus::NotNeeded,
        }
    }
}

#[cfg(target_os = "macos")]
fn check_accessibility() -> PermissionStatus {
    if unsafe { AXIsProcessTrusted() } {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    }
}

#[cfg(target_os = "macos")]
fn check_screen_recording() -> PermissionStatus {
    // try to capture a 1x1 region - if it fails, we don't have permission
    // use a thread with timeout to avoid hanging on permission dialogs
    use core_graphics::display::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        kCGWindowListOptionOnScreenOnly, CGWindowListCreateImage,
    };
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
        let image = unsafe {
            CGWindowListCreateImage(
                rect,
                kCGWindowListOptionOnScreenOnly,
                0,
                0,
            )
        };
        let _ = tx.send(!image.is_null());
    });

    // wait up to 1 second for the check
    match rx.recv_timeout(Duration::from_secs(1)) {
        Ok(true) => PermissionStatus::Granted,
        Ok(false) => PermissionStatus::Denied,
        Err(_) => {
            // timeout - likely a permission dialog is showing
            println!("[permissions] Screen recording check timed out - may need permission");
            PermissionStatus::NotAsked
        }
    }
}

#[cfg(target_os = "macos")]
fn check_microphone() -> PermissionStatus {
    // check AVCaptureDevice authorization status with timeout
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        // use swift snippet to check - simpler than objc bindings
        let result = Command::new("swift")
            .args([
                "-e",
                r#"
                import AVFoundation
                let status = AVCaptureDevice.authorizationStatus(for: .audio)
                switch status {
                case .authorized: print("granted")
                case .denied, .restricted: print("denied")
                case .notDetermined: print("notasked")
                @unknown default: print("denied")
                }
                "#,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(result);
    });

    // wait up to 2 seconds for swift command
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("granted") {
                PermissionStatus::Granted
            } else if stdout.contains("notasked") {
                PermissionStatus::NotAsked
            } else {
                PermissionStatus::Denied
            }
        }
        Ok(Err(_)) => PermissionStatus::Denied,
        Err(_) => {
            println!("[permissions] Microphone check timed out");
            PermissionStatus::Denied
        }
    }
}

// request permission (triggers system prompt)
#[tauri::command]
pub fn request_permission(permission: String) {
    #[cfg(target_os = "macos")]
    {
        match permission.as_str() {
            "accessibility" => request_accessibility(),
            "screenRecording" => request_screen_recording(),
            "microphone" => request_microphone(),
            _ => {}
        }
    }
}

#[cfg(target_os = "macos")]
fn request_accessibility() {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    let prompt_key = CFString::new("AXTrustedCheckOptionPrompt");
    let prompt_value = CFBoolean::true_value();

    let options =
        CFDictionary::from_CFType_pairs(&[(prompt_key.as_CFType(), prompt_value.as_CFType())]);

    unsafe {
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());
    }
}

#[cfg(target_os = "macos")]
fn request_screen_recording() {
    // trigger screen recording prompt by attempting capture
    use core_graphics::display::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        kCGWindowListOptionOnScreenOnly, CGWindowListCreateImage,
    };

    let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
    unsafe {
        CGWindowListCreateImage(rect, kCGWindowListOptionOnScreenOnly, 0, 0);
    }
}

#[cfg(target_os = "macos")]
fn request_microphone() {
    // request mic access via swift
    let _ = std::process::Command::new("swift")
        .args([
            "-e",
            r#"
            import AVFoundation
            AVCaptureDevice.requestAccess(for: .audio) { _ in }
            "#,
        ])
        .spawn();
}

// open system settings to permission pane
#[tauri::command]
pub fn open_permission_settings(permission: String) {
    #[cfg(target_os = "macos")]
    {
        let url = match permission.as_str() {
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "screenRecording" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            "microphone" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            _ => return,
        };

        let _ = std::process::Command::new("open").arg(url).spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let url = match permission.as_str() {
            "accessibility" => "ms-settings:easeofaccess-keyboard",
            "screenRecording" => "ms-settings:privacy-screenrecording",
            "microphone" => "ms-settings:privacy-microphone",
            _ => return,
        };
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
}

// check browser profile status
#[tauri::command]
pub fn get_browser_profile_status() -> BrowserProfileStatus {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    
    let path_buf = browser_profile_path();
    let profile_path = path_buf.to_string_lossy().to_string();
    let path = path_buf.as_path();

    if !path.exists() {
        return BrowserProfileStatus {
            exists: false,
            path: profile_path,
            sessions: vec![],
        };
    }

    // read cookies db in a thread with timeout (can be slow if chrome is running)
    let cookies_db = path.join("Default/Cookies");
    let (tx, rx) = mpsc::channel();
    let db_clone = cookies_db.clone();
    
    thread::spawn(move || {
        let sessions = if db_clone.exists() {
            read_cookie_domains(&db_clone).unwrap_or_default()
        } else {
            vec![]
        };
        let _ = tx.send(sessions);
    });

    // wait up to 1 second for cookie reading
    let sessions = match rx.recv_timeout(Duration::from_secs(1)) {
        Ok(s) => s,
        Err(_) => {
            println!("[permissions] Cookie reading timed out - Chrome may be running");
            vec![]
        }
    };

    BrowserProfileStatus {
        exists: true,
        path: profile_path,
        sessions,
    }
}

fn read_cookie_domains(db_path: &std::path::Path) -> Result<Vec<String>, String> {
    // copy db to temp location (chrome locks it)
    let temp_path = std::env::temp_dir().join("heywork_cookies_copy.db");
    std::fs::copy(db_path, &temp_path).map_err(|e| e.to_string())?;

    let conn = rusqlite::Connection::open(&temp_path).map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT DISTINCT host_key FROM cookies ORDER BY last_access_utc DESC")
        .map_err(|e| e.to_string())?;

    let domains: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .map(|d| d.trim_start_matches('.').to_string())
        .collect();

    // cleanup temp file
    let _ = std::fs::remove_file(&temp_path);

    // deduplicate
    let mut unique: Vec<String> = vec![];
    for d in domains {
        if !unique.contains(&d) {
            unique.push(d);
        }
    }

    Ok(unique)
}

// clear cookies for a specific domain
#[tauri::command]
pub fn clear_domain_cookies(domain: String) -> Result<(), String> {
    let profile_path = browser_profile_path();
    let cookies_db = profile_path.join("Default/Cookies");

    if !cookies_db.exists() {
        return Ok(());
    }

    // copy, modify, copy back
    let temp_path = std::env::temp_dir().join("heywork_cookies_edit.db");
    std::fs::copy(&cookies_db, &temp_path).map_err(|e| e.to_string())?;

    let conn = rusqlite::Connection::open(&temp_path).map_err(|e| e.to_string())?;

    // delete cookies matching domain (with or without leading dot)
    conn.execute(
        "DELETE FROM cookies WHERE host_key = ?1 OR host_key = ?2",
        [&domain, &format!(".{}", domain)],
    )
    .map_err(|e| e.to_string())?;

    drop(conn);

    // copy back
    std::fs::copy(&temp_path, &cookies_db).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&temp_path);

    Ok(())
}

// open browser profile in chrome for manual login
#[tauri::command]
pub fn open_browser_profile() -> Result<(), String> {
    let profile_path = browser_profile_path();
    let profile_path_str = profile_path.to_string_lossy().to_string();

    // create profile dir if it doesn't exist
    let _ = std::fs::create_dir_all(&profile_path);

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([
                "-a",
                "Google Chrome",
                "--args",
                &format!("--user-data-dir={}", profile_path_str),
                "--profile-directory=Default",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        let chrome = find_chrome_binary().ok_or_else(|| "Google Chrome not found on this system".to_string())?;
        std::process::Command::new(chrome)
            .args([
                &format!("--user-data-dir={}", profile_path_str),
                "--profile-directory=Default",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// open browser profile with specific url
#[tauri::command]
pub fn open_browser_profile_url(url: String) -> Result<(), String> {
    let profile_path = browser_profile_path();
    let profile_path_str = profile_path.to_string_lossy().to_string();

    let _ = std::fs::create_dir_all(&profile_path);

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([
                "-a",
                "Google Chrome",
                "--args",
                &format!("--user-data-dir={}", profile_path_str),
                "--profile-directory=Default",
                "--no-first-run",
                "--no-default-browser-check",
                &url,
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        let chrome = find_chrome_binary().ok_or_else(|| "Google Chrome not found on this system".to_string())?;
        std::process::Command::new(chrome)
            .args([
                &format!("--user-data-dir={}", profile_path_str),
                "--profile-directory=Default",
                "--no-first-run",
                "--no-default-browser-check",
                &url,
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// reset browser profile (delete it)
#[tauri::command]
pub fn reset_browser_profile() -> Result<(), String> {
    let profile_path = browser_profile_path();
    if profile_path.exists() {
        std::fs::remove_dir_all(&profile_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// check which api keys are configured
#[tauri::command]
pub fn get_api_key_status() -> ApiKeyStatus {
    ApiKeyStatus {
        anthropic: read_api_key_secure("ANTHROPIC_API_KEY").is_some(),
        deepgram: read_api_key_secure("DEEPGRAM_API_KEY").is_some(),
        elevenlabs: read_api_key_secure("ELEVENLABS_API_KEY").is_some(),
    }
}

// get voice settings
#[tauri::command]
pub fn get_voice_settings() -> VoiceSettings {
    VoiceSettings {
        elevenlabs_voice_id: std::env::var("ELEVENLABS_VOICE_ID").ok(),
    }
}

// save voice settings
#[tauri::command]
pub fn save_voice_settings(voice_id: String) -> Result<(), String> {
    save_env_var("ELEVENLABS_VOICE_ID", &voice_id)
}

// helper to save env var to .env file (stored in app data dir for portability)
fn save_env_var(var_name: &str, value: &str) -> Result<(), String> {
    // On Windows, current_dir may be read-only (e.g. C:\Program Files\...).
    // Always write to app data dir so we have write permissions.
    let env_path = app_data_dir().join(".env");
    let _ = std::fs::create_dir_all(env_path.parent().unwrap_or(&env_path));

    let existing = std::fs::read_to_string(&env_path).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(String::from).collect();
    let mut found = false;

    for line in &mut lines {
        if line.starts_with(&format!("{}=", var_name)) {
            *line = format!("{}={}", var_name, value);
            found = true;
            break;
        }
    }

    if !found {
        lines.push(format!("{}={}", var_name, value));
    }

    std::fs::write(&env_path, lines.join("\n")).map_err(|e| e.to_string())?;
    std::env::set_var(var_name, value);

    Ok(())
}

// save API key to secure OS credential storage
#[tauri::command]
pub fn save_api_key(service: String, key: String) -> Result<(), String> {
    let var_name = api_env_var_for_service(&service).ok_or_else(|| "Unknown service".to_string())?;
    let entry = keyring::Entry::new(KEYRING_SERVICE, var_name).map_err(|e| e.to_string())?;
    entry.set_password(&key).map_err(|e| e.to_string())?;
    std::env::set_var(var_name, key);
    Ok(())
}
