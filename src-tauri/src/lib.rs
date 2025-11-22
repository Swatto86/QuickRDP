#[tauri::command]
async fn quit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
fn show_about(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(about_window) = app_handle.get_webview_window("about") {
        about_window.show().map_err(|e| e.to_string())?;
        about_window.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("About window not found".to_string())
    }
}

#[derive(Clone, serde::Serialize)]
struct ErrorPayload {
    message: String,
    timestamp: String,
    category: Option<String>,
    details: Option<String>,
}

#[tauri::command]
fn show_error(
    app_handle: tauri::AppHandle,
    message: String,
    category: Option<String>,
    details: Option<String>,
) -> Result<(), String> {
    use chrono::Local;
    
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    let payload = ErrorPayload {
        message,
        timestamp,
        category,
        details,
    };
    
    debug_log(
        "INFO",
        "ERROR_WINDOW",
        &format!("Showing error in error window: {}", payload.message),
        payload.details.as_deref(),
    );
    
    // Emit the error event to the error window (this will work even if window is hidden)
    if let Some(error_window) = app_handle.get_webview_window("error") {
        let _ = error_window.emit("show-error", &payload);
        // Always show and focus the window when a new error occurs
        error_window.show().map_err(|e| e.to_string())?;
        error_window.unminimize().map_err(|e| e.to_string())?;
        error_window.set_focus().map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

use ldap3::{LdapConnAsync, Scope, SearchEntry};
use serde::Deserialize;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredEnumerateW, CredReadW, CredWriteW, CREDENTIALW, CRED_ENUMERATE_FLAGS,
    CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

static LAST_HIDDEN_WINDOW: Mutex<String> = Mutex::new(String::new());
static DEBUG_MODE: Mutex<bool> = Mutex::new(false);

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
}

#[derive(serde::Serialize)]
struct StoredCredentials {
    username: String,
    password: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct Host {
    hostname: String,
    description: String,
    last_connected: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct RecentConnection {
    hostname: String,
    description: String,
    timestamp: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RecentConnections {
    connections: Vec<RecentConnection>,
}

impl RecentConnections {
    fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    fn add_connection(&mut self, hostname: String, description: String) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Remove existing entry for this hostname if it exists
        self.connections.retain(|c| c.hostname != hostname);

        // Add new connection at the beginning
        self.connections.insert(0, RecentConnection {
            hostname,
            description,
            timestamp,
        });

        // Keep only the 5 most recent
        if self.connections.len() > 5 {
            self.connections.truncate(5);
        }
    }
}

fn get_recent_connections_file() -> Result<PathBuf, String> {
    let appdata_dir = std::env::var("APPDATA")
        .map_err(|_| "Failed to get APPDATA directory".to_string())?;
    let quickrdp_dir = PathBuf::from(appdata_dir).join("QuickRDP");
    std::fs::create_dir_all(&quickrdp_dir)
        .map_err(|e| format!("Failed to create QuickRDP directory: {}", e))?;
    Ok(quickrdp_dir.join("recent_connections.json"))
}

fn save_recent_connections(recent: &RecentConnections) -> Result<(), String> {
    let file_path = get_recent_connections_file()?;
    let json = serde_json::to_string_pretty(recent)
        .map_err(|e| format!("Failed to serialize recent connections: {}", e))?;
    std::fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write recent connections: {}", e))?;
    Ok(())
}

fn load_recent_connections() -> Result<RecentConnections, String> {
    let file_path = get_recent_connections_file()?;
    if !file_path.exists() {
        return Ok(RecentConnections::new());
    }
    let json = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read recent connections: {}", e))?;
    let recent: RecentConnections = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse recent connections: {}", e))?;
    Ok(recent)
}

#[tauri::command]
fn get_recent_connections() -> Result<Vec<RecentConnection>, String> {
    let recent = load_recent_connections()?;
    Ok(recent.connections)
}

#[tauri::command]
async fn save_credentials(credentials: Credentials) -> Result<(), String> {
    debug_log(
        "INFO",
        "CREDENTIALS",
        "Attempting to save credentials",
        None,
    );

    if credentials.username.is_empty() {
        let error = "Username cannot be empty";
        debug_log(
            "ERROR",
            "CREDENTIALS",
            error,
            Some("Username parameter was empty"),
        );
        return Err(error.to_string());
    }

    unsafe {
        // Convert strings to wide character format (UTF-16)
        let target_name: Vec<u16> = OsStr::new("QuickRDP")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let username: Vec<u16> = OsStr::new(&credentials.username)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // Password must be stored as UTF-16 wide string (matching how we retrieve it)
        let password_wide: Vec<u16> = OsStr::new(&credentials.password)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_name.as_ptr() as *mut u16),
            Comment: PWSTR::null(),
            LastWritten: FILETIME::default(),
            CredentialBlobSize: (password_wide.len() * 2) as u32, // Size in bytes, including null terminator
            CredentialBlob: password_wide.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR(username.as_ptr() as *mut u16),
        };

        match CredWriteW(&cred, 0) {
            Ok(_) => {
                debug_log(
                    "INFO",
                    "CREDENTIALS",
                    "Credentials saved successfully",
                    None,
                );
                Ok(())
            }
            Err(e) => {
                let error = format!("Failed to save credentials: {:?}", e);
                debug_log(
                    "ERROR",
                    "CREDENTIALS",
                    &error,
                    Some(&format!("CredWriteW error: {:?}", e)),
                );
                Err(error)
            }
        }
    }
}

#[tauri::command]
async fn get_all_hosts() -> Result<Vec<Host>, String> {
    get_hosts()
}

#[tauri::command]
async fn search_hosts(query: String) -> Result<Vec<Host>, String> {
    let hosts = get_hosts()?;
    let query = query.to_lowercase();

    let filtered_hosts: Vec<Host> = hosts
        .into_iter()
        .filter(|host| {
            host.hostname.to_lowercase().contains(&query)
                || host.description.to_lowercase().contains(&query)
        })
        .collect();

    Ok(filtered_hosts)
}

#[tauri::command]
async fn get_stored_credentials() -> Result<Option<StoredCredentials>, String> {
    debug_log(
        "INFO",
        "CREDENTIALS",
        "Attempting to retrieve stored credentials",
        None,
    );

    unsafe {
        let target_name: Vec<u16> = OsStr::new("QuickRDP")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut pcred = std::ptr::null_mut();

        match CredReadW(
            PCWSTR::from_raw(target_name.as_ptr()),
            CRED_TYPE_GENERIC,
            0,
            &mut pcred,
        ) {
            Ok(_) => {
                let cred = &*(pcred as *const CREDENTIALW);
                let username = if !cred.UserName.is_null() {
                    match PWSTR::from_raw(cred.UserName.0).to_string() {
                        Ok(u) => u,
                        Err(e) => {
                            let error = format!("Failed to read username: {:?}", e);
                            debug_log(
                                "ERROR",
                                "CREDENTIALS",
                                &error,
                                Some(&format!("Username decoding error: {:?}", e)),
                            );
                            return Err(error);
                        }
                    }
                } else {
                    String::new()
                };

                // Password is stored as UTF-16 wide string, so we need to decode it properly
                let password_bytes = std::slice::from_raw_parts(
                    cred.CredentialBlob,
                    cred.CredentialBlobSize as usize,
                );

                // Convert bytes to u16 array for UTF-16 decoding
                let password_wide: Vec<u16> = password_bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                // Decode UTF-16, removing the null terminator if present
                let password = match String::from_utf16(&password_wide) {
                    Ok(p) => p.trim_end_matches('\0').to_string(),
                    Err(e) => {
                        let error = format!("Failed to decode password from UTF-16: {:?}", e);
                        debug_log(
                            "ERROR",
                            "CREDENTIALS",
                            &error,
                            Some(&format!("Password decoding error: {:?}", e)),
                        );
                        return Err(error);
                    }
                };

                debug_log(
                    "INFO",
                    "CREDENTIALS",
                    &format!(
                        "Successfully retrieved stored credentials for user: {}",
                        username
                    ),
                    Some(&format!("Password length: {} characters", password.len())),
                );
                Ok(Some(StoredCredentials { username, password }))
            }
            Err(e) => {
                debug_log(
                    "INFO",
                    "CREDENTIALS",
                    "No stored credentials found",
                    Some(&format!("CredReadW returned error: {:?}", e)),
                );
                Ok(None)
            }
        }
    }
}

#[tauri::command]
async fn delete_credentials() -> Result<(), String> {
    unsafe {
        let target_name: Vec<u16> = OsStr::new("QuickRDP")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        CredDeleteW(PCWSTR::from_raw(target_name.as_ptr()), CRED_TYPE_GENERIC, 0)
            .map_err(|e| format!("Failed to delete credentials: {:?}", e))?;
    }
    Ok(())
}

#[tauri::command]
async fn toggle_visible_window(app_handle: tauri::AppHandle) -> Result<(), tauri::Error> {
    let login_window = app_handle
        .get_webview_window("login")
        .expect("login window exists");
    let main_window = app_handle
        .get_webview_window("main")
        .expect("main window exists");

    let login_visible = login_window.is_visible()?;
    let main_visible = main_window.is_visible()?;

    // First, determine which window should be shown
    if login_visible {
        // If login is visible, hide it
        login_window.hide()?;
    } else if main_visible {
        // If main is visible, hide it
        main_window.hide()?;
    } else {
        // If neither is visible, show login window
        // Make sure main window is hidden first
        main_window.hide()?;
        login_window.unminimize()?; // First unminimize if needed
        login_window.show()?; // Then show
        login_window.set_focus()?; // Finally bring to front
    }

    Ok(())
}

#[tauri::command]
async fn close_login_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    debug_log("DEBUG", "WINDOW", "Closing login window", None);
    if let Some(window) = app_handle.get_webview_window("login") {
        // Update LAST_HIDDEN_WINDOW before hiding
        if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
            *last_hidden = "login".to_string();
        }
        window.hide().map_err(|e| e.to_string())?;
        debug_log("DEBUG", "WINDOW", "Login window closed successfully", None);
    }
    Ok(())
}

#[tauri::command]
async fn close_login_and_prepare_main(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("login") {
        // Update LAST_HIDDEN_WINDOW to "main" so tray click shows main window
        if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
            *last_hidden = "main".to_string();
        }
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn get_login_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("login") {
        window.hide().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Login window not found".to_string())
    }
}

#[tauri::command]
async fn show_login_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    debug_log("DEBUG", "WINDOW", "Showing login window", None);
    if let Some(login_window) = app_handle.get_webview_window("login") {
        // First hide main window if it's visible
        if let Some(main_window) = app_handle.get_webview_window("main") {
            main_window.hide().map_err(|e| e.to_string())?;
        }

        // Update LAST_HIDDEN_WINDOW to "login"
        if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
            *last_hidden = "login".to_string();
        }

        login_window.unminimize().map_err(|e| e.to_string())?;
        login_window.show().map_err(|e| e.to_string())?;
        login_window.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Login window not found".to_string())
    }
}

#[tauri::command]
async fn switch_to_main_window(app_handle: tauri::AppHandle) -> Result<(), tauri::Error> {
    let login_window = app_handle.get_webview_window("login").unwrap();
    let main_window = app_handle.get_webview_window("main").unwrap();

    // First show main window, then hide login window to prevent flicker
    main_window.unminimize()?;
    main_window.show()?;
    main_window.set_focus()?;

    // Update LAST_HIDDEN_WINDOW before hiding login window
    if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
        *last_hidden = "main".to_string();
    }

    login_window.hide()?;

    Ok(())
}

#[tauri::command]
async fn hide_main_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

#[tauri::command]
async fn show_hosts_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(hosts_window) = app_handle.get_webview_window("hosts") {
        // First hide main window
        if let Some(main_window) = app_handle.get_webview_window("main") {
            main_window.hide().map_err(|e| e.to_string())?;
        }

        // Make sure login window is also hidden
        if let Some(login_window) = app_handle.get_webview_window("login") {
            login_window.hide().map_err(|e| e.to_string())?;
        }

        // Now show hosts window
        hosts_window.unminimize().map_err(|e| e.to_string())?;
        hosts_window.show().map_err(|e| e.to_string())?;
        hosts_window.set_focus().map_err(|e| e.to_string())?;

        // Update LAST_HIDDEN_WINDOW
        if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
            *last_hidden = "hosts".to_string();
        }

        Ok(())
    } else {
        Err("Hosts window not found".to_string())
    }
}

#[tauri::command]
async fn hide_hosts_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("hosts") {
        window.hide().map_err(|e| e.to_string())?;

        // Show main window again and update LAST_HIDDEN_WINDOW
        if let Some(main_window) = app_handle.get_webview_window("main") {
            if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
                *last_hidden = "main".to_string();
            }
            main_window.show().map_err(|e| e.to_string())?;
            main_window.set_focus().map_err(|e| e.to_string())?;
        }
        Ok(())
    } else {
        Err("Hosts window not found".to_string())
    }
}

#[tauri::command]
fn get_hosts() -> Result<Vec<Host>, String> {
    debug_log("DEBUG", "CSV_OPERATIONS", "Reading hosts from CSV", None);
    let path = std::path::Path::new("hosts.csv");
    if !path.exists() {
        debug_log("INFO", "CSV_OPERATIONS", "hosts.csv does not exist, returning empty list", None);
        return Ok(Vec::new());
    }

    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read CSV: {}", e))?;

    let mut hosts = Vec::new();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(contents.as_bytes());

    for result in reader.records() {
        match result {
            Ok(record) => {
                if record.len() >= 2 {
                    let last_connected = if record.len() >= 3 && !record[2].is_empty() {
                        Some(record[2].to_string())
                    } else {
                        None
                    };
                    hosts.push(Host {
                        hostname: record[0].to_string(),
                        description: record[1].to_string(),
                        last_connected,
                    });
                }
            }
            Err(e) => return Err(format!("Failed to parse CSV record: {}", e)),
        }
    }

    debug_log(
        "DEBUG",
        "CSV_OPERATIONS",
        &format!("Successfully loaded {} hosts from CSV", hosts.len()),
        None,
    );
    Ok(hosts)
}

#[tauri::command]
fn save_host(host: Host) -> Result<(), String> {
    debug_log(
        "INFO",
        "CSV_OPERATIONS",
        &format!("Saving host: {} - {}", host.hostname, host.description),
        None,
    );
    
    // Create hosts.csv if it doesn't exist
    if !std::path::Path::new("hosts.csv").exists() {
        let mut wtr = csv::WriterBuilder::new()
            .from_path("hosts.csv")
            .map_err(|e| format!("Failed to create hosts.csv: {}", e))?;

        wtr.write_record(&["hostname", "description"])
            .map_err(|e| format!("Failed to write CSV header: {}", e))?;

        wtr.flush()
            .map_err(|e| format!("Failed to flush CSV writer: {}", e))?;
    }

    let mut hosts = get_hosts()?;

    // Check if hostname is empty or invalid
    if host.hostname.trim().is_empty() {
        return Err("Hostname cannot be empty".to_string());
    }

    // Update or add the host
    if let Some(idx) = hosts.iter().position(|h| h.hostname == host.hostname) {
        hosts[idx] = host;
    } else {
        hosts.push(host);
    }

    let mut wtr = csv::WriterBuilder::new()
        .from_path("hosts.csv")
        .map_err(|e| format!("Failed to create CSV writer: {}", e))?;

    // Write header
    wtr.write_record(&["hostname", "description", "last_connected"])
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // Write records
    for host in hosts {
        debug_log(
            "DEBUG",
            "CSV_OPERATIONS",
            &format!("Writing host to CSV: {} - {}", host.hostname, host.description),
            None,
        );
        wtr.write_record(&[
            &host.hostname,
            &host.description,
            &host.last_connected.unwrap_or_default(),
        ])
        .map_err(|e| format!("Failed to write CSV record: {}", e))?;
    }

    wtr.flush()
        .map_err(|e| format!("Failed to flush CSV writer: {}", e))?;

    Ok(())
}

#[tauri::command]
fn delete_host(hostname: String) -> Result<(), String> {
    debug_log(
        "INFO",
        "CSV_OPERATIONS",
        &format!("Deleting host: {}", hostname),
        None,
    );
    
    let hosts: Vec<Host> = get_hosts()?
        .into_iter()
        .filter(|h| h.hostname != hostname)
        .collect();

    let mut wtr = csv::WriterBuilder::new()
        .from_path("hosts.csv")
        .map_err(|e| format!("Failed to create CSV writer: {}", e))?;

    // Write header
    wtr.write_record(&["hostname", "description", "last_connected"])
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // Write records
    for host in hosts {
        wtr.write_record(&[
            &host.hostname,
            &host.description,
            &host.last_connected.unwrap_or_default(),
        ])
        .map_err(|e| format!("Failed to write CSV record: {}", e))?;
    }

    wtr.flush()
        .map_err(|e| format!("Failed to flush CSV writer: {}", e))?;

    Ok(())
}

fn update_last_connected(hostname: &str) -> Result<(), String> {
    // Get current timestamp in UK format (DD/MM/YYYY HH:MM:SS)
    use chrono::Local;
    
    let timestamp = Local::now().format("%d/%m/%Y %H:%M:%S").to_string();
    
    debug_log(
        "INFO",
        "TIMESTAMP_UPDATE",
        &format!("Updating last connected for {} to {}", hostname, timestamp),
        None,
    );

    // Read all hosts
    let mut hosts = get_hosts()?;
    
    // Find and update the host
    let mut found = false;
    for host in &mut hosts {
        if host.hostname == hostname {
            host.last_connected = Some(timestamp.clone());
            found = true;
            break;
        }
    }
    
    if !found {
        return Err(format!("Host {} not found in hosts list", hostname));
    }
    
    // Write back to CSV
    let mut wtr = csv::WriterBuilder::new()
        .from_path("hosts.csv")
        .map_err(|e| format!("Failed to create CSV writer: {}", e))?;

    wtr.write_record(&["hostname", "description", "last_connected"])
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    for host in hosts {
        wtr.write_record(&[
            &host.hostname,
            &host.description,
            &host.last_connected.unwrap_or_default(),
        ])
        .map_err(|e| format!("Failed to write CSV record: {}", e))?;
    }

    wtr.flush()
        .map_err(|e| format!("Failed to flush CSV writer: {}", e))?;
    
    debug_log(
        "INFO",
        "TIMESTAMP_UPDATE",
        &format!("Successfully updated last connected for {}", hostname),
        None,
    );

    Ok(())
}

#[tauri::command]
async fn launch_rdp(host: Host) -> Result<(), String> {
    debug_log(
        "INFO",
        "RDP_LAUNCH",
        &format!("Starting RDP launch for host: {}", host.hostname),
        None,
    );

    // First check for per-host credentials, fall back to global credentials
    let credentials = match get_host_credentials(host.hostname.clone()).await? {
        Some(creds) => {
            debug_log(
                "INFO",
                "RDP_LAUNCH",
                &format!("Using per-host credentials for {}", host.hostname),
                None,
            );
            creds
        }
        None => {
            debug_log(
                "INFO",
                "RDP_LAUNCH",
                &format!(
                    "No per-host credentials found for {}, using global credentials",
                    host.hostname
                ),
                None,
            );
            match get_stored_credentials().await? {
                Some(creds) => creds,
                None => {
                    let error =
                        "No credentials found. Please save credentials in the login window first.";
                    debug_log(
                        "ERROR",
                        "RDP_LAUNCH",
                        error,
                        Some("Neither per-host nor global credentials are available"),
                    );
                    return Err(error.to_string());
                }
            }
        }
    };

    // Parse username to extract domain and username components BEFORE saving credentials
    // Supports formats: "DOMAIN\username", "username@domain.com", or "username"
    let (domain, username) = if credentials.username.contains('\\') {
        // Format: DOMAIN\username
        let parts: Vec<&str> = credentials.username.splitn(2, '\\').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (String::new(), credentials.username.clone())
        }
    } else if credentials.username.contains('@') {
        // Format: username@domain.com
        let parts: Vec<&str> = credentials.username.splitn(2, '@').collect();
        if parts.len() == 2 {
            (parts[1].to_string(), parts[0].to_string())
        } else {
            (String::new(), credentials.username.clone())
        }
    } else {
        // Format: just username (no domain)
        (String::new(), credentials.username.clone())
    };

    debug_log(
        "INFO",
        "RDP_LAUNCH",
        &format!(
            "Parsed credentials - Domain: '{}', Username: '{}'",
            domain, username
        ),
        Some(&format!(
            "Domain: '{}', Username: '{}', Password length: {}",
            domain, username, credentials.password.len()
        )),
    );

    // If per-host credentials don't exist, we need to save the global credentials to TERMSRV/{hostname}
    // If per-host credentials exist, they're already saved at TERMSRV/{hostname}
    if get_host_credentials(host.hostname.clone()).await?.is_none() {
        debug_log(
            "INFO",
            "RDP_LAUNCH",
            &format!(
                "Saving global credentials to TERMSRV/{} for RDP SSO",
                host.hostname
            ),
            None,
        );

        unsafe {
            // Convert password to wide string (UTF-16) as Windows expects
            let password_wide: Vec<u16> = OsStr::new(&credentials.password)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let target_name: Vec<u16> = OsStr::new(&format!("TERMSRV/{}", host.hostname))
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            // Use FULL username including domain for TERMSRV (e.g., DOMAIN\username)
            let termsrv_username = if !domain.is_empty() {
                format!("{}\\{}", domain, username)
            } else {
                username.clone()
            };
            let username_wide: Vec<u16> = OsStr::new(&termsrv_username)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let cred = CREDENTIALW {
                Flags: CRED_FLAGS(0),
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target_name.as_ptr() as *mut u16),
                Comment: PWSTR::null(),
                LastWritten: FILETIME::default(),
                CredentialBlobSize: (password_wide.len() * 2) as u32, // Size in bytes, including null terminator
                CredentialBlob: password_wide.as_ptr() as *mut u8,
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                AttributeCount: 0,
                Attributes: std::ptr::null_mut(),
                TargetAlias: PWSTR::null(),
                UserName: PWSTR(username_wide.as_ptr() as *mut u16),
            };

            match CredWriteW(&cred, 0) {
                Ok(_) => {
                    debug_log(
                        "INFO",
                        "RDP_LAUNCH",
                        &format!(
                            "Successfully saved credentials to TERMSRV/{} with username: {}",
                            host.hostname, termsrv_username
                        ),
                        None,
                    );
                }
                Err(e) => {
                    let error = format!("Failed to save RDP credentials: {:?}", e);
                    debug_log(
                        "ERROR",
                        "RDP_LAUNCH",
                        &error,
                        Some(&format!(
                            "CredWriteW error for host {}: {:?}",
                            host.hostname, e
                        )),
                    );
                    return Err(error);
                }
            }
        }
    } else {
        debug_log(
            "INFO",
            "RDP_LAUNCH",
            &format!(
                "Using existing per-host credentials at TERMSRV/{}",
                host.hostname
            ),
            None,
        );
    }

    // Get AppData\Roaming directory and create QuickRDP\Connections folder
    let appdata_dir =
        std::env::var("APPDATA").map_err(|_| "Failed to get APPDATA directory".to_string())?;
    let connections_dir = PathBuf::from(&appdata_dir)
        .join("QuickRDP")
        .join("Connections");

    debug_log(
        "DEBUG",
        "RDP_LAUNCH",
        &format!("Connections directory: {:?}", connections_dir),
        Some(&format!("AppData directory: {}", appdata_dir)),
    );

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&connections_dir)
        .map_err(|e| format!("Failed to create connections directory: {}", e))?;

    // Create filename using hostname
    let rdp_filename = format!("{}.rdp", host.hostname);
    let rdp_path = connections_dir.join(&rdp_filename);

    // Create RDP file content (no leading spaces, proper CRLF line endings)
    let rdp_content = format!(
        "screen mode id:i:2\r\n\
desktopwidth:i:1920\r\n\
desktopheight:i:1080\r\n\
session bpp:i:32\r\n\
full address:s:{}\r\n\
compression:i:1\r\n\
keyboardhook:i:2\r\n\
audiocapturemode:i:1\r\n\
videoplaybackmode:i:1\r\n\
connection type:i:2\r\n\
networkautodetect:i:1\r\n\
bandwidthautodetect:i:1\r\n\
enableworkspacereconnect:i:1\r\n\
disable wallpaper:i:0\r\n\
allow desktop composition:i:0\r\n\
allow font smoothing:i:0\r\n\
disable full window drag:i:1\r\n\
disable menu anims:i:1\r\n\
disable themes:i:0\r\n\
disable cursor setting:i:0\r\n\
bitmapcachepersistenable:i:1\r\n\
audiomode:i:0\r\n\
redirectprinters:i:1\r\n\
redirectcomports:i:0\r\n\
redirectsmartcards:i:1\r\n\
redirectclipboard:i:1\r\n\
redirectposdevices:i:0\r\n\
autoreconnection enabled:i:1\r\n\
authentication level:i:2\r\n\
prompt for credentials:i:0\r\n\
negotiate security layer:i:1\r\n\
remoteapplicationmode:i:0\r\n\
alternate shell:s:\r\n\
shell working directory:s:\r\n\
gatewayhostname:s:\r\n\
gatewayusagemethod:i:4\r\n\
gatewaycredentialssource:i:4\r\n\
gatewayprofileusagemethod:i:0\r\n\
promptcredentialonce:i:1\r\n\
use redirection server name:i:0\r\n\
rdgiskdcproxy:i:0\r\n\
kdcproxyname:s:\r\n\
username:s:{}\r\n\
domain:s:{}\r\n\
enablecredsspsupport:i:1\r\n\
public mode:i:0\r\n\
cert ignore:i:1\r\n",
        host.hostname, username, domain
    );

    debug_log(
        "INFO",
        "RDP_LAUNCH",
        &format!("Writing RDP file to: {:?}", rdp_path),
        Some(&format!(
            "RDP content length: {} bytes, File: {:?}",
            rdp_content.len(),
            rdp_path
        )),
    );

    // Write the RDP file with explicit UTF-8 encoding
    match std::fs::write(&rdp_path, rdp_content.as_bytes()) {
        Ok(_) => {
            debug_log(
                "INFO",
                "RDP_LAUNCH",
                &format!("RDP file written successfully to {:?}", rdp_path),
                None,
            );
        }
        Err(e) => {
            let error = format!("Failed to write RDP file: {}", e);
            debug_log(
                "ERROR",
                "RDP_LAUNCH",
                &error,
                Some(&format!("File write error: {:?}", e)),
            );
            return Err(error);
        }
    }

    // Launch RDP file using Windows ShellExecuteW API
    // Opening the .rdp file directly (like double-clicking in Explorer)
    debug_log(
        "INFO",
        "RDP_LAUNCH",
        "Attempting to open RDP file with ShellExecuteW",
        Some(&format!("Target file: {:?}", rdp_path)),
    );

    unsafe {
        let operation = HSTRING::from("open");
        let file = HSTRING::from(rdp_path.to_string_lossy().as_ref());

        let result = ShellExecuteW(
            None,          // hwnd
            &operation,    // lpOperation - "open"
            &file,         // lpFile - path to .rdp file
            None,          // lpParameters - none needed
            None,          // lpDirectory
            SW_SHOWNORMAL, // nShowCmd
        );

        // ShellExecuteW returns a value > 32 on success
        if result.0 as i32 <= 32 {
            let error = format!("Failed to open RDP file. Error code: {}", result.0);
            debug_log(
                "ERROR",
                "RDP_LAUNCH",
                &error,
                Some(&format!(
                    "ShellExecuteW returned error code: {}. File: {:?}",
                    result.0, rdp_path
                )),
            );
            return Err(error);
        }
    }

    debug_log(
        "INFO",
        "RDP_LAUNCH",
        &format!(
            "Successfully launched RDP connection to {} using file: {:?}",
            host.hostname, rdp_path
        ),
        Some(&format!("RDP client invoked for hostname: {}", host.hostname)),
    );

    // Save to recent connections
    if let Ok(mut recent) = load_recent_connections() {
        recent.add_connection(host.hostname.clone(), host.description.clone());
        let _ = save_recent_connections(&recent);
    }

    // Update last connected timestamp in hosts.csv
    if let Err(e) = update_last_connected(&host.hostname) {
        debug_log(
            "WARN",
            "RDP_LAUNCH",
            &format!("Failed to update last connected timestamp: {}", e),
            None,
        );
        // Don't fail the RDP launch if timestamp update fails
    }

    // RDP file is now persistent in AppData\Roaming\QuickRDP\Connections
    // No cleanup needed - file can be reused for future connections

    Ok(())
}

fn debug_log(level: &str, category: &str, message: &str, error_details: Option<&str>) {
    let debug_enabled = DEBUG_MODE.lock().map(|flag| *flag).unwrap_or(false);

    if !debug_enabled {
        return;
    }

    // Use AppData\Roaming\QuickRDP for reliable write permissions
    let log_file = if let Ok(appdata_dir) = std::env::var("APPDATA") {
        let quickrdp_dir = PathBuf::from(appdata_dir).join("QuickRDP");
        // Create directory if it doesn't exist
        let _ = std::fs::create_dir_all(&quickrdp_dir);
        quickrdp_dir.join("QuickRDP_Debug.log")
    } else {
        // Fallback to current directory if APPDATA not available
        PathBuf::from("QuickRDP_Debug.log")
    };

    // Check if file is new (to add header)
    let is_new_file = !log_file.exists();

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_file) {
        // Write header if this is a new file
        if is_new_file {
            let _ = writeln!(file, "{}", "=".repeat(80));
            let _ = writeln!(file, "QuickRDP Debug Log");
            let _ = writeln!(file, "{}", "=".repeat(80));
            let _ = writeln!(file, "This file contains detailed application logs and debugging information.");
            let _ = writeln!(file, "Generated when running QuickRDP with --debug or --debug-log argument.");
            let _ = writeln!(file, "");
            let _ = writeln!(file, "To enable debug logging, run: QuickRDP.exe --debug");
            let _ = writeln!(file, "");
            let _ = writeln!(file, "Log Levels:");
            let _ = writeln!(file, "  - INFO:  General informational messages");
            let _ = writeln!(file, "  - WARN:  Warning messages that may require attention");
            let _ = writeln!(file, "  - ERROR: Error messages indicating failures");
            let _ = writeln!(file, "  - DEBUG: Detailed debugging information");
            let _ = writeln!(file, "");
            let _ = writeln!(file, "{}", "=".repeat(80));
            let _ = writeln!(file, "");
        }

        // Format timestamp as human-readable date/time
        use chrono::Local;
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        // Format log level with color indicators (using text symbols)
        let level_indicator = match level {
            "ERROR" => "[!]",
            "WARN" => "[*]",
            "INFO" => "[i]",
            "DEBUG" => "[d]",
            _ => "[?]",
        };

        // Build the log entry with improved formatting
        let mut log_entry = format!("\n{} {} [{:8}] [{}]\n", timestamp, level_indicator, level, category);
        log_entry.push_str(&format!("Message: {}\n", message));

        if let Some(details) = error_details {
            log_entry.push_str(&format!("Details: {}\n", details));
        }

        // Add context information based on category
        match category {
            "RDP_LAUNCH" => {
                if let Ok(appdata_dir) = std::env::var("APPDATA") {
                    let connections_dir = PathBuf::from(appdata_dir).join("QuickRDP").join("Connections");
                    log_entry.push_str(&format!("RDP Files Directory: {:?}\n", connections_dir));
                }
            }
            "CREDENTIALS" => {
                log_entry.push_str("Credential Storage: Windows Credential Manager\n");
            }
            "LDAP_CONNECTION" | "LDAP_BIND" | "LDAP_SEARCH" => {
                log_entry.push_str("LDAP Port: 389\n");
            }
            _ => {}
        }

        // Add possible reasons for errors
        if level == "ERROR" {
            log_entry.push_str("\nPossible Causes:\n");
            match category {
                "LDAP_CONNECTION" => {
                    log_entry.push_str("  • LDAP server is not reachable or incorrect server name\n");
                    log_entry.push_str("  • Port 389 is blocked by firewall\n");
                    log_entry.push_str("  • Network connectivity issues\n");
                    log_entry.push_str("  • DNS resolution failure for server name\n");
                    log_entry.push_str("\nTroubleshooting Steps:\n");
                    log_entry.push_str("  1. Verify server name is correct\n");
                    log_entry.push_str("  2. Test network connectivity: ping <server>\n");
                    log_entry.push_str("  3. Check firewall rules for port 389\n");
                    log_entry.push_str("  4. Verify DNS resolution: nslookup <server>\n");
                }
                "LDAP_BIND" => {
                    log_entry.push_str("  • Invalid credentials (username or password)\n");
                    log_entry.push_str("  • Account is locked or disabled\n");
                    log_entry.push_str("  • Username format is incorrect\n");
                    log_entry.push_str("  • Insufficient permissions for LDAP queries\n");
                    log_entry.push_str("  • Anonymous bind is disabled on the domain controller\n");
                    log_entry.push_str("\nTroubleshooting Steps:\n");
                    log_entry.push_str("  1. Verify credentials are correct\n");
                    log_entry.push_str("  2. Try different username formats: DOMAIN\\username or username@domain.com\n");
                    log_entry.push_str("  3. Check if account is locked or disabled in Active Directory\n");
                    log_entry.push_str("  4. Verify account has permission to query AD\n");
                }
                "LDAP_SEARCH" => {
                    log_entry.push_str("  • Base DN is incorrect or domain name is wrong\n");
                    log_entry.push_str("  • LDAP filter syntax error\n");
                    log_entry.push_str("  • Insufficient permissions to search the directory\n");
                    log_entry.push_str("  • No Windows Server computers found in the domain\n");
                    log_entry.push_str("  • Connection was lost during search\n");
                    log_entry.push_str("\nTroubleshooting Steps:\n");
                    log_entry.push_str("  1. Verify domain name is correct\n");
                    log_entry.push_str("  2. Check LDAP filter syntax\n");
                    log_entry.push_str("  3. Verify account has read permissions on computer objects\n");
                }
                "CREDENTIALS" => {
                    log_entry.push_str("  • Windows Credential Manager access denied\n");
                    log_entry.push_str("  • Credential storage is corrupted\n");
                    log_entry.push_str("  • Insufficient permissions to access credentials\n");
                    log_entry.push_str("\nTroubleshooting Steps:\n");
                    log_entry.push_str("  1. Run application as administrator\n");
                    log_entry.push_str("  2. Check Windows Credential Manager (Control Panel > Credential Manager)\n");
                    log_entry.push_str("  3. Try removing and re-adding credentials\n");
                }
                "RDP_LAUNCH" => {
                    log_entry.push_str("  • mstsc.exe (RDP client) is not available or corrupted\n");
                    log_entry.push_str("  • RDP file creation failed (permissions or disk space)\n");
                    log_entry.push_str("  • RDP file directory is not accessible\n");
                    log_entry.push_str("  • Malformed RDP file content\n");
                    log_entry.push_str("\nTroubleshooting Steps:\n");
                    log_entry.push_str("  1. Verify mstsc.exe exists in System32\n");
                    log_entry.push_str("  2. Check disk space in AppData folder\n");
                    log_entry.push_str("  3. Verify file permissions in %APPDATA%\\QuickRDP\\Connections\n");
                    log_entry.push_str("  4. Try running as administrator\n");
                }
                "CSV_OPERATIONS" => {
                    log_entry.push_str("  • File permissions issue\n");
                    log_entry.push_str("  • Disk space is full\n");
                    log_entry.push_str("  • File is locked by another process\n");
                    log_entry.push_str("  • Invalid CSV format or corrupted file\n");
                    log_entry.push_str("\nTroubleshooting Steps:\n");
                    log_entry.push_str("  1. Close any programs that may have hosts.csv open\n");
                    log_entry.push_str("  2. Check disk space\n");
                    log_entry.push_str("  3. Verify file permissions\n");
                    log_entry.push_str("  4. Check if antivirus is blocking file access\n");
                }
                "HOST_CREDENTIALS" => {
                    log_entry.push_str("  • Failed to save/retrieve per-host credentials\n");
                    log_entry.push_str("  • Credential format is invalid\n");
                    log_entry.push_str("  • Permission denied\n");
                    log_entry.push_str("\nTroubleshooting Steps:\n");
                    log_entry.push_str("  1. Check Windows Credential Manager for TERMSRV/* entries\n");
                    log_entry.push_str("  2. Try running as administrator\n");
                    log_entry.push_str("  3. Verify hostname is valid\n");
                }
                _ => {
                    log_entry.push_str("  • Check system event logs for more details\n");
                    log_entry.push_str("  • Verify application has necessary permissions\n");
                    log_entry.push_str("  • Try running as administrator\n");
                }
            }
        }

        // Add warning context
        if level == "WARN" {
            log_entry.push_str("\nRecommendation: This warning may not prevent operation but should be investigated.\n");
        }

        log_entry.push_str(&format!("{}\n", "-".repeat(80)));

        if let Err(e) = write!(file, "{}", log_entry) {
            eprintln!("Failed to write to debug log file: {}", e);
        }
    } else {
        eprintln!("Failed to open debug log file: {:?}", log_file);
    }
}

fn set_debug_mode(enabled: bool) {
    if let Ok(mut flag) = DEBUG_MODE.lock() {
        *flag = enabled;
    }
}

#[tauri::command]
async fn scan_domain(
    app_handle: tauri::AppHandle,
    domain: String,
    server: String,
) -> Result<String, String> {
    debug_log(
        "INFO",
        "LDAP_SCAN",
        &format!(
            "scan_domain command called with domain: {}, server: {}",
            domain, server
        ),
        None,
    );

    // Get the hosts window and set it to always on top temporarily
    let hosts_window = match app_handle.get_webview_window("hosts") {
        Some(window) => {
            debug_log("INFO", "LDAP_SCAN", "Hosts window found", None);
            window
        }
        None => {
            let error = "Failed to get hosts window";
            debug_log(
                "ERROR",
                "LDAP_SCAN",
                error,
                Some("Hosts window does not exist or is not accessible"),
            );
            return Err(error.to_string());
        }
    };

    // Set window to always on top
    if let Err(e) = hosts_window.set_always_on_top(true) {
        let error = "Failed to set window always on top";
        debug_log(
            "WARN",
            "LDAP_SCAN",
            error,
            Some(&format!("Window operation error: {:?}", e)),
        );
        // Continue anyway, this is not critical
    }

    // Perform the LDAP scan
    let result = scan_domain_ldap(domain, server).await;

    // Reset always on top after command completes
    let _ = hosts_window.set_always_on_top(false);

    result
}

async fn scan_domain_ldap(domain: String, server: String) -> Result<String, String> {
    debug_log(
        "INFO",
        "LDAP_SCAN",
        &format!(
            "Starting LDAP scan for domain: {} on server: {}",
            domain, server
        ),
        Some(&format!("Domain: {}, Server: {}", domain, server)),
    );

    // Validate inputs
    if domain.is_empty() {
        let error = "Domain name is empty";
        debug_log(
            "ERROR",
            "LDAP_SCAN",
            error,
            Some("Domain parameter was empty or whitespace"),
        );
        return Err(error.to_string());
    }

    if server.is_empty() {
        let error = "Server name is empty";
        debug_log(
            "ERROR",
            "LDAP_SCAN",
            error,
            Some("Server parameter was empty or whitespace"),
        );
        return Err(error.to_string());
    }

    // Build the LDAP URL
    let ldap_url = format!("ldap://{}:389", server);
    debug_log(
        "INFO",
        "LDAP_CONNECTION",
        &format!("Attempting to connect to: {}", ldap_url),
        None,
    );

    // Connect to LDAP server
    let (conn, mut ldap) = match LdapConnAsync::new(&ldap_url).await {
        Ok(conn) => {
            debug_log(
                "INFO",
                "LDAP_CONNECTION",
                "LDAP connection established successfully",
                None,
            );
            conn
        }
        Err(e) => {
            let error_msg = format!("Failed to connect to LDAP server {}: {}", server, e);
            debug_log(
                "ERROR",
                "LDAP_CONNECTION",
                &error_msg,
                Some(&format!(
                    "Connection error: {:?}. Check if server is reachable and port 389 is open.",
                    e
                )),
            );
            return Err(error_msg);
        }
    };

    // Drive the connection in the background
    ldap3::drive!(conn);

    // Corporate AD environments require authenticated bind for searches
    // Skip anonymous bind and go straight to authenticated bind
    debug_log(
        "INFO",
        "LDAP_BIND",
        "Retrieving stored credentials for LDAP authentication",
        None,
    );

    // Get stored credentials
    let credentials = match get_stored_credentials().await {
        Ok(Some(creds)) => {
            debug_log(
                "INFO",
                "CREDENTIALS",
                &format!(
                    "Retrieved stored credentials for LDAP: username={}, password_len={}",
                    creds.username,
                    creds.password.len()
                ),
                None,
            );
            creds
        }
        Ok(None) => {
            let error = "No stored credentials found. Please save your domain credentials in the login window first.";
            debug_log("ERROR", "CREDENTIALS", error, Some("No credentials found in Windows Credential Manager. User must save credentials in login window before scanning."));
            return Err(error.to_string());
        }
        Err(e) => {
            let error = format!("Failed to retrieve credentials: {}", e);
            debug_log(
                "ERROR",
                "CREDENTIALS",
                &error,
                Some(&format!("Credential retrieval error: {:?}", e)),
            );
            return Err(error);
        }
    };

    // Format the username for LDAP binding
    // Support multiple formats: username, DOMAIN\username, or username@domain.com
    let bind_dn = if credentials.username.contains('@') || credentials.username.contains('\\') {
        credentials.username.clone()
    } else {
        // If just username, append @domain
        format!("{}@{}", credentials.username, domain)
    };

    debug_log(
        "INFO",
        "LDAP_BIND",
        &format!(
            "Attempting authenticated LDAP bind with username: {}",
            bind_dn
        ),
        Some(&format!("Bind DN: {}", bind_dn)),
    );

    // Perform authenticated bind
    match ldap.simple_bind(&bind_dn, &credentials.password).await {
        Ok(result) => {
            debug_log(
                "INFO",
                "LDAP_BIND",
                "Authenticated LDAP bind successful",
                Some(&format!("Bind result: {:?}", result)),
            );
        }
        Err(e) => {
            let error = format!("Authenticated LDAP bind failed: {}. Please verify your credentials have permission to query Active Directory.", e);
            debug_log("ERROR", "LDAP_BIND", &error, Some(&format!("Bind error: {:?}. Check username format (try DOMAIN\\username or username@domain.com) and password.", e)));
            return Err(error);
        }
    }

    // Build the search base DN from domain
    // e.g., "domain.com" -> "DC=domain,DC=com"
    let base_dn = domain
        .split('.')
        .map(|part| format!("DC={}", part))
        .collect::<Vec<String>>()
        .join(",");

    debug_log(
        "INFO",
        "LDAP_SEARCH",
        &format!("Searching base DN: {}", base_dn),
        Some(&format!("Base DN: {}, Filter: (&(objectClass=computer)(operatingSystem=Windows Server*)(dNSHostName=*))", base_dn)),
    );

    // Search for Windows Server computers
    // LDAP filter for computer objects with Windows Server operating system
    let filter = "(&(objectClass=computer)(operatingSystem=Windows Server*)(dNSHostName=*))";
    let attrs = vec!["dNSHostName", "description", "operatingSystem"];

    debug_log(
        "INFO",
        "LDAP_SEARCH",
        &format!("Using LDAP filter: {}", filter),
        None,
    );

    let (rs, _res) = match ldap.search(&base_dn, Scope::Subtree, filter, attrs).await {
        Ok(result) => match result.success() {
            Ok(search_result) => {
                debug_log(
                    "INFO",
                    "LDAP_SEARCH",
                    &format!(
                        "LDAP search completed, found {} entries",
                        search_result.0.len()
                    ),
                    None,
                );
                search_result
            }
            Err(e) => {
                let error = format!("LDAP search failed: {}", e);
                debug_log(
                    "ERROR",
                    "LDAP_SEARCH",
                    &error,
                    Some(&format!("Search result error: {:?}", e)),
                );
                return Err(error);
            }
        },
        Err(e) => {
            let error = format!("Failed to search LDAP: {}", e);
            debug_log(
                "ERROR",
                "LDAP_SEARCH",
                &error,
                Some(&format!("Search execution error: {:?}", e)),
            );
            return Err(error);
        }
    };

    debug_log(
        "INFO",
        "LDAP_SEARCH",
        &format!("Found {} entries from LDAP", rs.len()),
        Some(&format!("Entry count: {}", rs.len())),
    );

    // Parse results
    let mut hosts = Vec::new();
    for entry in rs {
        let search_entry = SearchEntry::construct(entry);

        // Get the dNSHostName attribute
        if let Some(hostname_values) = search_entry.attrs.get("dNSHostName") {
            if let Some(hostname) = hostname_values.first() {
                // Get description if available
                let description = search_entry
                    .attrs
                    .get("description")
                    .and_then(|v| v.first())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                debug_log(
                    "INFO",
                    "LDAP_SEARCH",
                    &format!("Found host: {} - {}", hostname, description),
                    Some(&format!("Hostname: {}, Description: {}", hostname, description)),
                );

                hosts.push(Host {
                    hostname: hostname.to_string(),
                    description,
                    last_connected: None,
                });
            }
        } else {
            debug_log(
                "WARN",
                "LDAP_SEARCH",
                "LDAP entry found but missing dNSHostName attribute",
                None,
            );
        }
    }

    // Unbind from LDAP
    let _ = ldap.unbind().await;
    debug_log("INFO", "LDAP_CONNECTION", "LDAP connection closed", None);

    // Write results to CSV
    if hosts.is_empty() {
        let error = "No Windows Servers found in the domain.";
        debug_log("ERROR", "LDAP_SEARCH", error, Some("Search completed but no hosts were found. Check if filter matches any computers in the domain."));
        return Err(error.to_string());
    }

    debug_log(
        "INFO",
        "CSV_OPERATIONS",
        &format!("Writing {} hosts to CSV file", hosts.len()),
        None,
    );

    // Write to CSV file
    let mut wtr = match csv::WriterBuilder::new().from_path("hosts.csv") {
        Ok(writer) => writer,
        Err(e) => {
            let error = format!("Failed to create CSV writer: {}", e);
            debug_log(
                "ERROR",
                "CSV_OPERATIONS",
                &error,
                Some(&format!("CSV writer creation error: {:?}", e)),
            );
            return Err(error);
        }
    };

    // Write header
    if let Err(e) = wtr.write_record(&["hostname", "description"]) {
        let error = format!("Failed to write CSV header: {}", e);
        debug_log(
            "ERROR",
            "CSV_OPERATIONS",
            &error,
            Some(&format!("CSV write error: {:?}", e)),
        );
        return Err(error);
    }

    // Write records
    for host in &hosts {
        if let Err(e) = wtr.write_record(&[&host.hostname, &host.description]) {
            let error = format!("Failed to write CSV record: {}", e);
            debug_log(
                "ERROR",
                "CSV_OPERATIONS",
                &error,
                Some(&format!(
                    "CSV write error for host {}: {:?}",
                    host.hostname, e
                )),
            );
            return Err(error);
        }
    }

    if let Err(e) = wtr.flush() {
        let error = format!("Failed to flush CSV writer: {}", e);
        debug_log(
            "ERROR",
            "CSV_OPERATIONS",
            &error,
            Some(&format!("CSV flush error: {:?}", e)),
        );
        return Err(error);
    }

    debug_log(
        "INFO",
        "LDAP_SCAN",
        &format!(
            "Successfully completed scan and wrote {} hosts to CSV",
            hosts.len()
        ),
        Some(&format!("Total hosts written: {}", hosts.len())),
    );

    Ok(format!(
        "Successfully found {} Windows Server(s).",
        hosts.len()
    ))
}

#[tauri::command]
async fn save_host_credentials(host: Host, credentials: Credentials) -> Result<(), String> {
    debug_log(
        "INFO",
        "HOST_CREDENTIALS",
        &format!("Saving credentials for host: {}", host.hostname),
        None,
    );

    // Parse username to extract just the username part (not DOMAIN\username)
    let username = if credentials.username.contains('\\') {
        let parts: Vec<&str> = credentials.username.splitn(2, '\\').collect();
        if parts.len() == 2 {
            parts[1].to_string()
        } else {
            credentials.username.clone()
        }
    } else if credentials.username.contains('@') {
        let parts: Vec<&str> = credentials.username.splitn(2, '@').collect();
        if parts.len() == 2 {
            parts[0].to_string()
        } else {
            credentials.username.clone()
        }
    } else {
        credentials.username.clone()
    };

    debug_log(
        "INFO",
        "HOST_CREDENTIALS",
        &format!("Parsed username for TERMSRV: {}", username),
        None,
    );

    unsafe {
        let password_wide: Vec<u16> = OsStr::new(&credentials.password)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let target_name: Vec<u16> = OsStr::new(&format!("TERMSRV/{}", host.hostname))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let username_wide: Vec<u16> = OsStr::new(&username)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_name.as_ptr() as *mut u16),
            Comment: PWSTR::null(),
            LastWritten: FILETIME::default(),
            CredentialBlobSize: (password_wide.len() * 2) as u32, // Size in bytes, including null terminator
            CredentialBlob: password_wide.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR(username_wide.as_ptr() as *mut u16),
        };

        match CredWriteW(&cred, 0) {
            Ok(_) => {
                debug_log(
                    "INFO",
                    "HOST_CREDENTIALS",
                    &format!(
                        "Successfully saved credentials for host: {} (username: {})",
                        host.hostname, username
                    ),
                    None,
                );
                Ok(())
            }
            Err(e) => {
                let error = format!(
                    "Failed to save credentials for host {}: {:?}",
                    host.hostname, e
                );
                debug_log(
                    "ERROR",
                    "HOST_CREDENTIALS",
                    &error,
                    Some(&format!("CredWriteW error: {:?}", e)),
                );
                Err(error)
            }
        }
    }
}

#[tauri::command]
async fn get_host_credentials(hostname: String) -> Result<Option<StoredCredentials>, String> {
    debug_log(
        "INFO",
        "HOST_CREDENTIALS",
        &format!("Retrieving credentials for host: {}", hostname),
        None,
    );

    unsafe {
        let target_name: Vec<u16> = OsStr::new(&format!("TERMSRV/{}", hostname))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut pcred = std::ptr::null_mut();

        match CredReadW(
            PCWSTR::from_raw(target_name.as_ptr()),
            CRED_TYPE_GENERIC,
            0,
            &mut pcred,
        ) {
            Ok(_) => {
                let cred = &*(pcred as *const CREDENTIALW);
                let username = if !cred.UserName.is_null() {
                    PWSTR::from_raw(cred.UserName.0).to_string().map_err(|e| {
                        debug_log(
                            "ERROR",
                            "HOST_CREDENTIALS",
                            &format!("Failed to decode username for host {}", hostname),
                            Some(&format!("Error: {:?}", e)),
                        );
                        format!("Failed to read username: {:?}", e)
                    })?
                } else {
                    String::new()
                };

                // Password is stored as UTF-16 wide string, so we need to decode it properly
                let password_bytes = std::slice::from_raw_parts(
                    cred.CredentialBlob,
                    cred.CredentialBlobSize as usize,
                );

                // Convert bytes to u16 array for UTF-16 decoding
                let password_wide: Vec<u16> = password_bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                // Decode UTF-16, removing the null terminator if present
                let password = String::from_utf16(&password_wide)
                    .map_err(|e| {
                        debug_log(
                            "ERROR",
                            "HOST_CREDENTIALS",
                            &format!("Failed to decode password for host {}", hostname),
                            Some(&format!("UTF-16 decode error: {:?}", e)),
                        );
                        format!("Failed to decode password from UTF-16: {:?}", e)
                    })?
                    .trim_end_matches('\0')
                    .to_string();

                debug_log("INFO", "HOST_CREDENTIALS", &format!("Successfully retrieved credentials for host: {} (username: {}, password_len: {})", hostname, username, password.len()), None);
                Ok(Some(StoredCredentials { username, password }))
            }
            Err(_) => {
                debug_log(
                    "INFO",
                    "HOST_CREDENTIALS",
                    &format!("No stored credentials found for host: {}", hostname),
                    None,
                );
                Ok(None)
            }
        }
    }
}

#[tauri::command]
async fn delete_all_hosts() -> Result<(), String> {
    // Create empty file to clear all contents
    std::fs::write("hosts.csv", "hostname,description\n")
        .map_err(|e| format!("Failed to clear hosts file: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn reset_application() -> Result<String, String> {
    debug_log(
        "WARN",
        "RESET",
        "Application reset initiated - deleting all credentials and data",
        None,
    );

    let mut report = String::from("=== QuickRDP Application Reset ===\n\n");

    // 1. Delete all QuickRDP credentials
    match delete_credentials().await {
        Ok(_) => {
            report.push_str("✓ Deleted global QuickRDP credentials\n");
            debug_log("INFO", "RESET", "Deleted global credentials", None);
        }
        Err(e) => {
            report.push_str(&format!("✗ Failed to delete global credentials: {}\n", e));
            debug_log(
                "ERROR",
                "RESET",
                "Failed to delete global credentials",
                Some(&e),
            );
        }
    }

    // 2. Enumerate and delete all TERMSRV/* credentials
    unsafe {
        let filter: Vec<u16> = OsStr::new("TERMSRV/*")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut count: u32 = 0;
        let mut pcreds: *mut *mut CREDENTIALW = std::ptr::null_mut();

        match CredEnumerateW(
            PCWSTR::from_raw(filter.as_ptr()),
            CRED_ENUMERATE_FLAGS(0),
            &mut count,
            &mut pcreds as *mut *mut *mut CREDENTIALW,
        ) {
            Ok(_) => {
                debug_log(
                    "INFO",
                    "RESET",
                    &format!("Found {} TERMSRV credentials to delete", count),
                    None,
                );
                report.push_str(&format!("\nFound {} RDP host credentials:\n", count));

                // Iterate through credentials and delete them
                for i in 0..count {
                    let cred_ptr = *pcreds.offset(i as isize);
                    let cred = &*cred_ptr;

                    if let Ok(target_name) = PWSTR::from_raw(cred.TargetName.0).to_string() {
                        report.push_str(&format!("  - {}\n", target_name));

                        let target_name_wide: Vec<u16> = OsStr::new(&target_name)
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();

                        match CredDeleteW(
                            PCWSTR::from_raw(target_name_wide.as_ptr()),
                            CRED_TYPE_GENERIC,
                            0,
                        ) {
                            Ok(_) => {
                                debug_log(
                                    "INFO",
                                    "RESET",
                                    &format!("Deleted credential: {}", target_name),
                                    None,
                                );
                            }
                            Err(e) => {
                                report.push_str(&format!("    ✗ Failed to delete: {:?}\n", e));
                                debug_log(
                                    "ERROR",
                                    "RESET",
                                    &format!("Failed to delete {}", target_name),
                                    Some(&format!("{:?}", e)),
                                );
                            }
                        }
                    }
                }

                report.push_str(&format!("✓ Processed {} RDP host credentials\n", count));
            }
            Err(e) => {
                report.push_str(&format!(
                    "✗ No TERMSRV credentials found or error: {:?}\n",
                    e
                ));
                debug_log(
                    "INFO",
                    "RESET",
                    "No TERMSRV credentials found",
                    Some(&format!("{:?}", e)),
                );
            }
        }
    }

    // 3. Delete all RDP files in AppData\Roaming\QuickRDP\Connections
    if let Ok(appdata_dir) = std::env::var("APPDATA") {
        let connections_dir = PathBuf::from(appdata_dir)
            .join("QuickRDP")
            .join("Connections");

        report.push_str(&format!("\nRDP Files in {:?}:\n", connections_dir));

        if connections_dir.exists() {
            match std::fs::read_dir(&connections_dir) {
                Ok(entries) => {
                    let mut deleted_count = 0;
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("rdp") {
                            match std::fs::remove_file(&path) {
                                Ok(_) => {
                                    report.push_str(&format!(
                                        "  ✓ Deleted: {:?}\n",
                                        path.file_name().unwrap_or_default()
                                    ));
                                    deleted_count += 1;
                                    debug_log(
                                        "INFO",
                                        "RESET",
                                        &format!("Deleted RDP file: {:?}", path),
                                        None,
                                    );
                                }
                                Err(e) => {
                                    report.push_str(&format!(
                                        "  ✗ Failed to delete {:?}: {}\n",
                                        path.file_name().unwrap_or_default(),
                                        e
                                    ));
                                    debug_log(
                                        "ERROR",
                                        "RESET",
                                        &format!("Failed to delete RDP file: {:?}", path),
                                        Some(&format!("{}", e)),
                                    );
                                }
                            }
                        }
                    }
                    report.push_str(&format!("✓ Deleted {} RDP files\n", deleted_count));
                }
                Err(e) => {
                    report.push_str(&format!("✗ Failed to read connections directory: {}\n", e));
                    debug_log(
                        "ERROR",
                        "RESET",
                        "Failed to read connections directory",
                        Some(&format!("{}", e)),
                    );
                }
            }
        } else {
            report.push_str("  (Connections directory does not exist)\n");
        }
    }

    // 4. Delete hosts.csv
    match delete_all_hosts().await {
        Ok(_) => {
            report.push_str("\n✓ Cleared hosts.csv\n");
            debug_log("INFO", "RESET", "Cleared hosts.csv", None);
        }
        Err(e) => {
            report.push_str(&format!("\n✗ Failed to clear hosts.csv: {}\n", e));
            debug_log("ERROR", "RESET", "Failed to clear hosts.csv", Some(&e));
        }
    }

    // 5. Delete recent_connections.json
    if let Ok(appdata_dir) = std::env::var("APPDATA") {
        let recent_file = PathBuf::from(appdata_dir)
            .join("QuickRDP")
            .join("recent_connections.json");

        if recent_file.exists() {
            match std::fs::remove_file(&recent_file) {
                Ok(_) => {
                    report.push_str("✓ Deleted recent connections history\n");
                    debug_log(
                        "INFO",
                        "RESET",
                        "Deleted recent_connections.json",
                        None,
                    );
                }
                Err(e) => {
                    report.push_str(&format!(
                        "✗ Failed to delete recent connections: {}\n",
                        e
                    ));
                    debug_log(
                        "ERROR",
                        "RESET",
                        "Failed to delete recent_connections.json",
                        Some(&format!("{}", e)),
                    );
                }
            }
        } else {
            report.push_str("✓ No recent connections to delete\n");
        }
    }

    report.push_str("\n=== Reset Complete ===\n");
    report.push_str("The application has been reset to its initial state.\n");
    report.push_str("Please restart the application.\n");

    debug_log("WARN", "RESET", "Application reset completed", None);

    Ok(report)
}

const REGISTRY_RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "QuickRDP";

#[tauri::command]
fn check_autostart() -> Result<bool, String> {
    unsafe {
        let key_path: Vec<u16> = OsStr::new(REGISTRY_RUN_KEY)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey = HKEY::default();

        // Open the registry key
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(key_path.as_ptr()),
            0,
            KEY_READ,
            &mut hkey as *mut HKEY,
        );

        if result.is_err() {
            return Ok(false);
        }

        let value_name: Vec<u16> = OsStr::new(APP_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut data_size: u32 = 0;

        // Query the value to check if it exists
        let query_result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(value_name.as_ptr()),
            None,
            None,
            None,
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey);

        Ok(query_result.is_ok())
    }
}

#[tauri::command]
fn toggle_autostart() -> Result<bool, String> {
    let is_enabled = check_autostart()?;

    if is_enabled {
        // Disable autostart - remove from registry
        disable_autostart()?;
        Ok(false)
    } else {
        // Enable autostart - add to registry
        enable_autostart()?;
        Ok(true)
    }
}

fn enable_autostart() -> Result<(), String> {
    unsafe {
        // Get the current executable path
        let exe_path =
            std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

        let exe_path_str = exe_path.to_string_lossy().to_string();

        debug_log(
            "INFO",
            "AUTOSTART",
            &format!("Enabling autostart with path: {}", exe_path_str),
            Some(&format!("Executable path: {}", exe_path_str)),
        );

        let key_path: Vec<u16> = OsStr::new(REGISTRY_RUN_KEY)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey = HKEY::default();

        // Open the registry key with write access
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(key_path.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey as *mut HKEY,
        )
        .map_err(|e| format!("Failed to open registry key: {:?}", e))?;

        let value_name: Vec<u16> = OsStr::new(APP_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let value_data: Vec<u16> = OsStr::new(&exe_path_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Set the registry value
        let result = RegSetValueExW(
            hkey,
            PCWSTR::from_raw(value_name.as_ptr()),
            0,
            REG_SZ,
            Some(&value_data.align_to::<u8>().1),
        );

        let _ = RegCloseKey(hkey);

        result.map_err(|e| format!("Failed to set registry value: {:?}", e))?;

        debug_log(
            "INFO",
            "AUTOSTART",
            "Autostart enabled successfully",
            Some(&format!("Registry value set for {}", APP_NAME)),
        );
        Ok(())
    }
}

fn disable_autostart() -> Result<(), String> {
    unsafe {
        debug_log(
            "INFO",
            "AUTOSTART",
            "Disabling autostart",
            None,
        );

        let key_path: Vec<u16> = OsStr::new(REGISTRY_RUN_KEY)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey = HKEY::default();

        // Open the registry key with write access
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(key_path.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey as *mut HKEY,
        )
        .map_err(|e| format!("Failed to open registry key: {:?}", e))?;

        let value_name: Vec<u16> = OsStr::new(APP_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Delete the registry value
        let result = RegDeleteValueW(hkey, PCWSTR::from_raw(value_name.as_ptr()));

        let _ = RegCloseKey(hkey);

        result.map_err(|e| format!("Failed to delete registry value: {:?}", e))?;

        debug_log(
            "INFO",
            "AUTOSTART",
            "Autostart disabled successfully",
            Some(&format!("Registry value deleted for {}", APP_NAME)),
        );
        Ok(())
    }
}

#[tauri::command]
fn get_windows_theme() -> Result<String, String> {
    unsafe {
        // Windows theme is stored in the registry at:
        // HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize
        // Value: AppsUseLightTheme (0 = dark, 1 = light)

        let key_path: Vec<u16> =
            OsStr::new("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

        let mut hkey = HKEY::default();

        // Open the registry key
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(key_path.as_ptr()),
            0,
            KEY_READ,
            &mut hkey as *mut HKEY,
        );

        if result.is_err() {
            // If we can't read the registry, default to dark theme
            return Ok("dark".to_string());
        }

        let value_name: Vec<u16> = OsStr::new("AppsUseLightTheme")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut data_type = REG_VALUE_TYPE::default();
        let mut data: u32 = 0;
        let mut data_size: u32 = std::mem::size_of::<u32>() as u32;

        // Query the value
        let query_result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(value_name.as_ptr()),
            None,
            Some(&mut data_type),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey);

        if query_result.is_ok() {
            // 0 = dark theme, 1 (or any other value) = light theme
            if data == 0 {
                Ok("dark".to_string())
            } else {
                Ok("light".to_string())
            }
        } else {
            // Default to dark if we can't read the value
            Ok("dark".to_string())
        }
    }
}

#[tauri::command]
fn set_theme(app_handle: tauri::AppHandle, theme: String) -> Result<(), String> {
    // Save the theme preference in the app's data directory
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    std::fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    let theme_file = app_dir.join("theme.txt");
    std::fs::write(&theme_file, &theme)
        .map_err(|e| format!("Failed to write theme preference: {}", e))?;

    // Emit an event to all windows to update their theme
    for window_label in ["login", "main", "hosts", "about"] {
        if let Some(window) = app_handle.get_webview_window(window_label) {
            let _ = window.emit("theme-changed", theme.clone());
        }
    }

    // Rebuild tray menu with new theme
    if let Some(tray) = app_handle.tray_by_id("main") {
        if let Ok(menu) = build_tray_menu(&app_handle, &theme) {
            let _ = tray.set_menu(Some(menu));
        }
    }

    Ok(())
}

#[tauri::command]
fn get_theme(app_handle: tauri::AppHandle) -> Result<String, String> {
    // Try to read the saved theme preference
    let app_dir = match app_handle.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => return get_windows_theme(), // Fallback to Windows theme
    };

    let theme_file = app_dir.join("theme.txt");

    if theme_file.exists() {
        match std::fs::read_to_string(&theme_file) {
            Ok(theme) => Ok(theme.trim().to_string()),
            Err(_) => get_windows_theme(), // Fallback to Windows theme
        }
    } else {
        get_windows_theme() // Fallback to Windows theme
    }
}

// Helper function to build tray menu with theme awareness
fn build_tray_menu(app: &tauri::AppHandle, current_theme: &str) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    // Check autostart status
    let autostart_enabled = check_autostart().unwrap_or(false);
    let autostart_text = if autostart_enabled {
        "✓ Autostart with Windows"
    } else {
        "✗ Autostart with Windows"
    };
    let autostart_item = MenuItem::with_id(
        app,
        "toggle_autostart",
        &autostart_text,
        true,
        None::<&str>,
    )?;

    // Create theme menu items with checkmarks
    let theme_light = MenuItem::with_id(
        app,
        "theme_light",
        if current_theme == "light" { "✓ Light" } else { "✗ Light" },
        true,
        None::<&str>,
    )?;
    let theme_dark = MenuItem::with_id(
        app,
        "theme_dark",
        if current_theme == "dark" { "✓ Dark" } else { "✗ Dark" },
        true,
        None::<&str>,
    )?;

    let theme_submenu = Submenu::with_items(
        app,
        "Theme",
        true,
        &[&theme_light, &theme_dark],
    )?;

    // Create recent connections submenu
    let recent_connections = load_recent_connections().unwrap_or_else(|_| RecentConnections::new());
    
    let recent_submenu = if recent_connections.connections.is_empty() {
        let no_recent = MenuItem::with_id(
            app,
            "no_recent",
            "No recent connections",
            false,
            None::<&str>,
        )?;
        Submenu::with_items(
            app,
            "Recent Connections",
            true,
            &[&no_recent],
        )?
    } else {
        // Build submenu with actual recent items
        let items: Vec<_> = recent_connections.connections.iter().map(|conn| {
            let label = if conn.description.is_empty() {
                conn.hostname.clone()
            } else {
                format!("{} - {}", conn.hostname, conn.description)
            };
            let menu_id = format!("recent_{}", conn.hostname);
            MenuItem::with_id(
                app,
                &menu_id,
                &label,
                true,
                None::<&str>,
            )
        }).collect::<Result<Vec<_>, _>>()?;
        
        let item_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items.iter().map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>).collect();
        Submenu::with_items(
            app,
            "Recent Connections",
            true,
            &item_refs,
        )?
    };

    let about_item = MenuItem::with_id(app, "about", "About QuickRDP", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[&recent_submenu, &autostart_item, &theme_submenu, &about_item, &separator, &quit_item],
    ).map_err(|e| e.into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Check for --debug or --debug-log command line argument
    let args: Vec<String> = std::env::args().collect();
    let debug_enabled = args
        .iter()
        .any(|arg| arg == "--debug" || arg == "--debug-log");

    if debug_enabled {
        eprintln!("[QuickRDP] Debug mode enabled");
        eprintln!("[QuickRDP] Args: {:?}", args);

        // Show where log file will be written
        if let Ok(appdata_dir) = std::env::var("APPDATA") {
            let log_file = PathBuf::from(appdata_dir)
                .join("QuickRDP")
                .join("QuickRDP_Debug.log");
            eprintln!("[QuickRDP] Log file will be written to: {:?}", log_file);
        } else {
            eprintln!("[QuickRDP] WARNING: APPDATA not found, using current directory for log");
        }

        set_debug_mode(true);
        debug_log(
            "INFO",
            "SYSTEM",
            "Debug logging enabled via command line argument",
            Some(&format!("Command line arguments: {:?}", args)),
        );
        debug_log(
            "INFO",
            "SYSTEM",
            &format!("Application version: {}", env!("CARGO_PKG_VERSION")),
            None,
        );
        debug_log(
            "INFO",
            "SYSTEM",
            &format!("Operating System: {}", std::env::consts::OS),
            Some(&format!("Architecture: {}", std::env::consts::ARCH)),
        );
        if let Ok(current_dir) = std::env::current_dir() {
            debug_log(
                "INFO",
                "SYSTEM",
                &format!("Working directory: {:?}", current_dir),
                None,
            );
        }
        eprintln!("[QuickRDP] Debug log initialized");
    } else {
        eprintln!("[QuickRDP] Starting without debug mode. Use --debug to enable logging.");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            if debug_enabled {
                debug_log("INFO", "SYSTEM", "Tauri application setup started", None);
            }
            // Initialize the LAST_HIDDEN_WINDOW
            if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
                *last_hidden = "login".to_string();
            }

            // Get current theme for tray menu
            let current_theme = get_theme(app.app_handle().clone()).unwrap_or_else(|_| "dark".to_string());

            // Build the tray menu with theme awareness
            let menu = build_tray_menu(app.app_handle(), &current_theme)?;

            // Set up close handlers for all windows
            let app_handle = app.app_handle().clone();
            let login_window = app.get_webview_window("login").unwrap();
            login_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    println!("Close requested for login window");
                    if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
                        *last_hidden = "login".to_string();
                    }
                    let _ = app_handle.get_webview_window("login").unwrap().hide();
                    // Prevent the window from being destroyed
                    api.prevent_close();
                }
            });

            let app_handle = app.app_handle().clone();
            let main_window = app.get_webview_window("main").unwrap();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    println!("Close requested for main window");
                    if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
                        *last_hidden = "main".to_string();
                    }
                    let _ = app_handle.get_webview_window("main").unwrap().hide();
                    // Prevent the window from being destroyed
                    api.prevent_close();
                }
            });

            let app_handle = app.app_handle().clone();
            let hosts_window = app.get_webview_window("hosts").unwrap();
            hosts_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    println!("Close requested for hosts window");
                    if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
                        *last_hidden = "hosts".to_string();
                    }
                    let _ = app_handle.get_webview_window("hosts").unwrap().hide();
                    // Prevent the window from being destroyed
                    api.prevent_close();
                }
            });

            // Set up close handler for about window (just hide it)
            let app_handle = app.app_handle().clone();
            let about_window = app.get_webview_window("about").unwrap();
            about_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    println!("Close requested for about window");
                    let _ = app_handle.get_webview_window("about").unwrap().hide();
                    // Prevent the window from being destroyed
                    api.prevent_close();
                }
            });

            // Set up close handler for error window (just hide it)
            let app_handle = app.app_handle().clone();
            let error_window = app.get_webview_window("error").unwrap();
            error_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    println!("Close requested for error window");
                    let _ = app_handle.get_webview_window("error").unwrap().hide();
                    // Prevent the window from being destroyed
                    api.prevent_close();
                }
            });

            // Create the system tray
            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray_handle, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state,
                            ..
                        } => {
                            println!(
                                "Left click detected on system tray icon with state: {:?}",
                                button_state
                            );
                            // Only handle the Down state to prevent double-triggering
                            if button_state == MouseButtonState::Down {
                                let app_handle = tray_handle.app_handle().clone();

                                if let Ok(window_label) = LAST_HIDDEN_WINDOW.lock() {
                                    println!("Last hidden window: {}", window_label);

                                    let window = app_handle
                                        .get_webview_window(&window_label)
                                        .or_else(|| app_handle.get_webview_window("login"))
                                        .or_else(|| app_handle.get_webview_window("main"))
                                        .or_else(|| app_handle.get_webview_window("hosts"));

                                    if let Some(window) = window {
                                        println!("Found window: {}", window.label());

                                        tauri::async_runtime::spawn(async move {
                                            match window.is_visible() {
                                                Ok(is_visible) => {
                                                    println!(
                                                        "Window visibility status: {}",
                                                        is_visible
                                                    );
                                                    if is_visible {
                                                        println!("Attempting to hide window");
                                                        if let Err(e) = window.hide() {
                                                            println!(
                                                                "Error hiding window: {:?}",
                                                                e
                                                            );
                                                        } else {
                                                            println!("Window hidden successfully");
                                                        }
                                                    } else {
                                                        println!("Attempting to show window");
                                                        if let Err(e) = window.unminimize() {
                                                            println!(
                                                                "Error unminimizing window: {:?}",
                                                                e
                                                            );
                                                        }
                                                        if let Err(e) = window.show() {
                                                            println!(
                                                                "Error showing window: {:?}",
                                                                e
                                                            );
                                                        }
                                                        if let Err(e) = window.set_focus() {
                                                            println!(
                                                                "Error setting focus: {:?}",
                                                                e
                                                            );
                                                        }
                                                        println!("Window show sequence completed");
                                                    }
                                                }
                                                Err(e) => println!(
                                                    "Error checking window visibility: {:?}",
                                                    e
                                                ),
                                            }
                                        });
                                    } else {
                                        println!("No windows found at all!");
                                    }
                                } else {
                                    println!("Failed to acquire LAST_HIDDEN_WINDOW lock");
                                }
                            }
                        }
                        TrayIconEvent::Click {
                            button: MouseButton::Right,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            println!("Right click detected on system tray icon");
                        }
                        _ => {}
                    }
                })
                .on_menu_event(|app, event| {
                    let id_str = event.id().as_ref();
                    
                    // Check if it's a recent connection item
                    if id_str.starts_with("recent_") {
                        let hostname = id_str.strip_prefix("recent_").unwrap_or("").to_string();
                        if !hostname.is_empty() {
                            // Get the host details and launch RDP
                            tauri::async_runtime::spawn(async move {
                                // Try to get host from hosts list
                                match get_hosts() {
                                    Ok(hosts) => {
                                        if let Some(host) = hosts.into_iter().find(|h| h.hostname == hostname) {
                                            if let Err(e) = launch_rdp(host).await {
                                                eprintln!("Failed to launch RDP to {}: {}", hostname, e);
                                            }
                                        } else {
                                            // Host not in list, create a temporary host entry
                                            let host = Host {
                                                hostname: hostname.clone(),
                                                description: String::new(),
                                                last_connected: None,
                                            };
                                            if let Err(e) = launch_rdp(host).await {
                                                eprintln!("Failed to launch RDP to {}: {}", hostname, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to get hosts: {}", e);
                                    }
                                }
                            });
                        }
                        return;
                    }
                    
                    // Handle other menu events
                    match event.id() {
                        id if id == "toggle_autostart" => {
                            match toggle_autostart() {
                                Ok(_enabled) => {
                                    // Rebuild the entire menu with updated autostart status and current theme
                                    if let Some(tray) = app.tray_by_id("main") {
                                        let current_theme = get_theme(app.clone())
                                            .unwrap_or_else(|_| "dark".to_string());
                                        if let Ok(new_menu) = build_tray_menu(app, &current_theme) {
                                            let _ = tray.set_menu(Some(new_menu));
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to toggle autostart: {}", e);
                                }
                            }
                        }
                        id if id == "theme_light" => {
                            if let Err(e) = set_theme(app.clone(), "light".to_string()) {
                                eprintln!("Failed to set theme to light: {}", e);
                            }
                        }
                        id if id == "theme_dark" => {
                            if let Err(e) = set_theme(app.clone(), "dark".to_string()) {
                                eprintln!("Failed to set theme to dark: {}", e);
                            }
                        }
                        id if id == "about" => {
                            if let Err(e) = show_about(app.clone()) {
                                eprintln!("Failed to show about window: {}", e);
                            }
                        }
                        id if id == "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)
                .expect("Failed to build tray icon");

            let window = app.get_webview_window("login").unwrap();
            let main_window = app.get_webview_window("main").unwrap();
            let hosts_window = app.get_webview_window("hosts").unwrap();

            let window_clone = window.clone();
            let main_window_clone = main_window.clone();
            let hosts_window_clone = hosts_window.clone();

            tauri::async_runtime::spawn(async move {
                std::thread::sleep(std::time::Duration::from_millis(100));
                // Center login window
                window_clone.center().unwrap();
                window_clone.show().unwrap();
                window_clone.set_focus().unwrap();

                // Center main window
                main_window_clone.center().unwrap();

                // Center hosts window
                hosts_window_clone.center().unwrap();
            });

            // Register global hotkey Ctrl+Shift+R to show the main window
            // Note: We don't fail the app if hotkey registration fails
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let app_handle_for_hotkey = app.app_handle().clone();
            let shortcut_manager = app.handle().global_shortcut();
            
            // Try to unregister first in case it was registered by a previous instance
            let _ = shortcut_manager.unregister("Ctrl+Shift+R");
            
            // Set up the handler BEFORE registering (per Tauri docs)
            match shortcut_manager.on_shortcut("Ctrl+Shift+R", move |_app_handle, _shortcut, _event| {
                println!("Global hotkey Ctrl+Shift+R pressed!");
                
                let main_window = app_handle_for_hotkey.get_webview_window("main");
                
                if let Some(window) = main_window {
                    tauri::async_runtime::spawn(async move {
                        // Update last hidden window to main so tray shows correct window
                        if let Ok(mut last_hidden) = LAST_HIDDEN_WINDOW.lock() {
                            *last_hidden = "main".to_string();
                        }
                        
                        // Show and focus the window
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                        println!("Main window shown via global hotkey");
                    });
                }
            }) {
                Ok(_) => {
                    println!("Global hotkey handler registered");
                    
                    // Now register the actual shortcut
                    match shortcut_manager.register("Ctrl+Shift+R") {
                        Ok(_) => println!("Global hotkey Ctrl+Shift+R activated successfully"),
                        Err(e) => {
                            eprintln!("Warning: Failed to register global hotkey Ctrl+Shift+R: {:?}", e);
                            eprintln!("The hotkey may be in use by another application.");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to set up hotkey handler: {:?}", e);
                    eprintln!("The application will continue without the global hotkey.");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            quit_app,
            show_about,
            show_error,
            save_credentials,
            get_stored_credentials,
            delete_credentials,
            toggle_visible_window,
            close_login_window,
            close_login_and_prepare_main,
            get_login_window,
            show_login_window,
            switch_to_main_window,
            hide_main_window,
            show_hosts_window,
            get_hosts,
            get_all_hosts,
            save_host,
            delete_host,
            hide_hosts_window,
            search_hosts,
            launch_rdp,
            scan_domain,
            save_host_credentials,
            get_host_credentials,
            delete_all_hosts,
            reset_application,
            check_autostart,
            toggle_autostart,
            get_windows_theme,
            set_theme,
            get_theme,
            get_recent_connections,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
