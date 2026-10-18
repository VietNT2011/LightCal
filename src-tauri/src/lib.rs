use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::Utc;
use rand::{distr::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use url::Url;

const KEYRING_SERVICE: &str = "com.vietnguyen.lightcal";
const KEYRING_USER: &str = "google-refresh-token";

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    google_client_id: String,
    account_email: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    configured: bool,
    connected: bool,
    account_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarEvent {
    id: String,
    title: String,
    start: String,
    end: String,
    all_day: bool,
    location: Option<String>,
    color: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventInput {
    id: Option<String>,
    title: String,
    start: String,
    end: String,
    all_day: bool,
    location: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventCache {
    events: Vec<CalendarEvent>,
    synced_at: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PrimaryCalendar {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GoogleEventsResponse {
    #[serde(default)]
    items: Vec<GoogleEvent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleEvent {
    id: String,
    summary: Option<String>,
    location: Option<String>,
    color_id: Option<String>,
    start: GoogleEventTime,
    end: GoogleEventTime,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleEventTime {
    date_time: Option<String>,
    date: Option<String>,
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("settings.json"))
        .map_err(|error| error.to_string())
}

fn cache_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map(|path| path.join("events.json"))
        .map_err(|error| error.to_string())
}

fn load_config(app: &tauri::AppHandle) -> AppConfig {
    config_path(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|error| error.to_string())
}

fn has_refresh_token() -> bool {
    keyring_entry().and_then(|entry| entry.get_password().map_err(|error| error.to_string())).is_ok()
}

fn random_string(length: usize) -> String {
    rand::rng().sample_iter(&Alphanumeric).take(length).map(char::from).collect()
}

fn wait_for_oauth(listener: TcpListener, expected_state: String) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(180) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buffer = [0_u8; 8192];
                let size = stream.read(&mut buffer).map_err(|error| error.to_string())?;
                let request = String::from_utf8_lossy(&buffer[..size]);
                let target = request.lines().next().and_then(|line| line.split_whitespace().nth(1)).ok_or("Phản hồi OAuth không hợp lệ")?;
                let callback = Url::parse(&format!("http://localhost{target}")).map_err(|error| error.to_string())?;
                let mut code = None;
                let mut state = None;
                let mut oauth_error = None;
                for (key, value) in callback.query_pairs() {
                    match key.as_ref() {
                        "code" => code = Some(value.into_owned()),
                        "state" => state = Some(value.into_owned()),
                        "error" => oauth_error = Some(value.into_owned()),
                        _ => {}
                    }
                }
                let success = oauth_error.is_none() && state.as_deref() == Some(expected_state.as_str()) && code.is_some();
                let message = if success { "Đã kết nối Google Calendar. Bạn có thể đóng tab này và quay lại LightCal." } else { "Không thể kết nối Google Calendar. Hãy quay lại LightCal và thử lại." };
                let body = format!("<!doctype html><meta charset=\"utf-8\"><title>LightCal</title><style>body{{font:16px Segoe UI,sans-serif;display:grid;place-items:center;min-height:90vh;background:#f7f6f2;color:#292925}}main{{text-align:center;max-width:480px;padding:32px}}h1{{color:#5f52cc}}</style><main><h1>LightCal</h1><p>{message}</p></main>");
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes());
                if let Some(error) = oauth_error { return Err(format!("Google từ chối đăng nhập: {error}")); }
                if state.as_deref() != Some(expected_state.as_str()) { return Err("Mã xác thực OAuth không khớp".into()); }
                return code.ok_or_else(|| "Google không trả về mã xác thực".into());
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(120)),
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Đăng nhập Google đã hết thời gian chờ".into())
}

#[tauri::command]
fn get_app_status(app: tauri::AppHandle) -> AppStatus {
    let config = load_config(&app);
    AppStatus { configured: !config.google_client_id.is_empty(), connected: has_refresh_token(), account_email: config.account_email }
}

#[tauri::command]
fn save_google_client_id(app: tauri::AppHandle, client_id: String) -> Result<AppStatus, String> {
    let client_id = client_id.trim();
    if !client_id.ends_with(".apps.googleusercontent.com") {
        return Err("Client ID không đúng định dạng Google".into());
    }
    let mut config = load_config(&app);
    config.google_client_id = client_id.to_owned();
    save_config(&app, &config)?;
    Ok(AppStatus { configured: true, connected: has_refresh_token(), account_email: config.account_email })
}

#[tauri::command]
async fn connect_google(app: tauri::AppHandle) -> Result<AppStatus, String> {
    let mut config = load_config(&app);
    if config.google_client_id.is_empty() { return Err("Hãy lưu OAuth Client ID trước".into()); }

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let verifier = random_string(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let oauth_state = random_string(32);
    let mut auth_url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(|error| error.to_string())?;
    auth_url.query_pairs_mut()
        .append_pair("client_id", &config.google_client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "https://www.googleapis.com/auth/calendar.events")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &oauth_state);
    tauri_plugin_opener::open_url(auth_url.as_str(), None::<&str>).map_err(|error| error.to_string())?;

    let code = tauri::async_runtime::spawn_blocking(move || wait_for_oauth(listener, oauth_state)).await.map_err(|error| error.to_string())??;
    let token = reqwest::Client::new().post("https://oauth2.googleapis.com/token").form(&[
        ("client_id", config.google_client_id.as_str()), ("code", code.as_str()),
        ("code_verifier", verifier.as_str()), ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri.as_str()),
    ]).send().await.map_err(|error| error.to_string())?.error_for_status().map_err(|error| format!("Google không chấp nhận mã xác thực: {error}"))?.json::<TokenResponse>().await.map_err(|error| error.to_string())?;

    let refresh_token = token.refresh_token.ok_or("Google không cấp refresh token; hãy thu hồi quyền LightCal rồi thử lại")?;
    keyring_entry()?.set_password(&refresh_token).map_err(|error| error.to_string())?;
    let primary = reqwest::Client::new().get("https://www.googleapis.com/calendar/v3/calendars/primary").bearer_auth(&token.access_token).send().await.map_err(|error| error.to_string())?.error_for_status().map_err(|error| error.to_string())?.json::<PrimaryCalendar>().await.map_err(|error| error.to_string())?;
    config.account_email = Some(primary.id.clone());
    save_config(&app, &config)?;
    Ok(AppStatus { configured: true, connected: true, account_email: Some(primary.id) })
}

async fn refreshed_access_token(client_id: &str) -> Result<String, String> {
    let refresh_token = keyring_entry()?.get_password().map_err(|_| "Bạn chưa kết nối tài khoản Google".to_string())?;
    let token = reqwest::Client::new().post("https://oauth2.googleapis.com/token").form(&[
        ("client_id", client_id), ("refresh_token", refresh_token.as_str()), ("grant_type", "refresh_token"),
    ]).send().await.map_err(|error| error.to_string())?.error_for_status().map_err(|error| format!("Không thể làm mới phiên Google: {error}"))?.json::<TokenResponse>().await.map_err(|error| error.to_string())?;
    Ok(token.access_token)
}

fn calendar_color(color_id: Option<&str>) -> String {
    match color_id { Some("1") => "#7986cb", Some("2") => "#33b679", Some("3") => "#8e24aa", Some("4") => "#e67c73", Some("5") => "#f6bf26", Some("6") => "#f4511e", Some("7") => "#039be5", Some("8") => "#616161", Some("9") => "#3f51b5", Some("10") => "#0b8043", Some("11") => "#d50000", _ => "#5f52cc" }.into()
}

fn map_google_event(event: GoogleEvent) -> CalendarEvent {
    let all_day = event.start.date_time.is_none();
    let start = event.start.date_time.or_else(|| event.start.date.map(|date| format!("{date}T00:00:00"))).unwrap_or_default();
    let end = event.end.date_time.or_else(|| event.end.date.map(|date| format!("{date}T00:00:00"))).unwrap_or_else(|| start.clone());
    CalendarEvent { id: event.id, title: event.summary.unwrap_or_else(|| "(Không có tiêu đề)".into()), start, end, all_day, location: event.location, color: calendar_color(event.color_id.as_deref()) }
}

#[tauri::command]
async fn sync_calendar(app: tauri::AppHandle, time_min: String, time_max: String) -> Result<EventCache, String> {
    let config = load_config(&app);
    if config.google_client_id.is_empty() { return Err("Chưa cấu hình Google OAuth Client ID".into()); }
    let access_token = refreshed_access_token(&config.google_client_id).await?;
    let response = reqwest::Client::new().get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
        .bearer_auth(access_token)
        .query(&[("timeMin", time_min.as_str()), ("timeMax", time_max.as_str()), ("singleEvents", "true"), ("orderBy", "startTime"), ("maxResults", "250")])
        .send().await.map_err(|error| error.to_string())?.error_for_status().map_err(|error| format!("Không thể đọc Google Calendar: {error}"))?.json::<GoogleEventsResponse>().await.map_err(|error| error.to_string())?;
    let events = response.items.into_iter().map(map_google_event).collect();
    let cache = EventCache { events, synced_at: Utc::now().to_rfc3339() };
    let path = cache_path(&app)?;
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|error| error.to_string())?; }
    fs::write(path, serde_json::to_string(&cache).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    Ok(cache)
}

#[tauri::command]
async fn save_calendar_event(app: tauri::AppHandle, event: EventInput) -> Result<CalendarEvent, String> {
    if event.title.trim().is_empty() { return Err("Tên sự kiện không được để trống".into()); }
    let config = load_config(&app);
    if config.google_client_id.is_empty() { return Err("Chưa cấu hình Google OAuth Client ID".into()); }
    let access_token = refreshed_access_token(&config.google_client_id).await?;
    let time_body = if event.all_day {
        serde_json::json!({ "start": { "date": event.start }, "end": { "date": event.end } })
    } else {
        serde_json::json!({ "start": { "dateTime": event.start }, "end": { "dateTime": event.end } })
    };
    let mut body = serde_json::json!({
        "summary": event.title.trim(),
        "location": event.location.as_deref().unwrap_or("").trim(),
    });
    body.as_object_mut().expect("event body must be an object").extend(time_body.as_object().expect("time body must be an object").clone());
    let client = reqwest::Client::new();
    let response = if let Some(id) = event.id.filter(|id| !id.is_empty()) {
        let mut event_url = Url::parse("https://www.googleapis.com/calendar/v3/calendars/primary/events/").map_err(|error| error.to_string())?;
        event_url.path_segments_mut().map_err(|_| "URL Google Calendar không hợp lệ")?.push(&id);
        client.patch(event_url).bearer_auth(access_token).json(&body).send().await
    } else {
        client.post("https://www.googleapis.com/calendar/v3/calendars/primary/events").bearer_auth(access_token).json(&body).send().await
    }.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| format!("Không thể lưu sự kiện: {error}"))?
        .json::<GoogleEvent>().await.map_err(|error| error.to_string())?;
    Ok(map_google_event(response))
}

#[tauri::command]
fn load_cached_events(app: tauri::AppHandle) -> Option<EventCache> {
    cache_path(&app).ok().and_then(|path| fs::read_to_string(path).ok()).and_then(|content| serde_json::from_str(&content).ok())
}

#[tauri::command]
fn disconnect_google(app: tauri::AppHandle) -> Result<AppStatus, String> {
    if let Ok(entry) = keyring_entry() { let _ = entry.delete_credential(); }
    let mut config = load_config(&app);
    config.account_email = None;
    save_config(&app, &config)?;
    Ok(AppStatus { configured: !config.google_client_id.is_empty(), connected: false, account_email: None })
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main_window(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Mở LightCal", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("missing app icon").clone())
                .tooltip("LightCal")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![get_app_status, save_google_client_id, connect_google, sync_calendar, save_calendar_event, load_cached_events, disconnect_google])
        .run(tauri::generate_context!())
        .expect("error while running LightCal");
}
