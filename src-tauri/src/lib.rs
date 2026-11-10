pub mod downloader;
pub mod media;
pub mod queue;
pub mod torrent;

use downloader::{DownloadTask, EngineState, UrlMetadata};
use media::{AppUrlState, DownloadState};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{command, AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ---------------------------------------------------------------------------
// HTTP Downloader Commands
// ---------------------------------------------------------------------------

#[command]
async fn probe_http_url(
    state: State<'_, EngineState>,
    url: String,
) -> Result<UrlMetadata, String> {
    downloader::probe_url(&state.client, &url).await
}

#[command]
async fn start_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    url: String,
    filename: Option<String>,
    save_folder: Option<String>,
    chunk_count: Option<usize>,
    low_disk_space: Option<bool>,
) -> Result<String, String> {
    let meta = downloader::probe_url(&state.client, &url).await.unwrap_or(UrlMetadata {
        url: url.clone(),
        filename: "download.bin".to_string(),
        total_size: None,
        supports_ranges: false,
        content_type: None,
        category: "docs".to_string(),
    });

    let mut resolved_name = filename
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(meta.filename.clone());
    if resolved_name.starts_with("@[") && resolved_name.ends_with(']') {
        resolved_name = resolved_name[2..resolved_name.len() - 1].trim().to_string();
    }
    resolved_name = resolved_name
        .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '[' || c == ']')
        .trim()
        .to_string();
    if let Some(pos) = resolved_name.rfind(|c| c == '/' || c == '\\') {
        resolved_name = resolved_name[pos + 1..].to_string();
    }
    if !resolved_name.contains('.') {
        let ext = if let Some(pos) = meta.filename.rfind('.') {
            &meta.filename[pos..]
        } else if let Some(ref ct) = meta.content_type {
            if ct.contains("mp4") {
                ".mp4"
            } else if ct.contains("webm") {
                ".webm"
            } else if ct.contains("audio/mpeg") || ct.contains("mp3") {
                ".mp3"
            } else {
                ".bin"
            }
        } else {
            ".mp4"
        };
        resolved_name = format!("{}{}", resolved_name, ext);
    }

    let resolved_folder = save_folder
        .filter(|f| !f.trim().is_empty())
        .unwrap_or_else(|| {
            dirs_or_default_downloads()
        });

    let save_path = PathBuf::from(&resolved_folder).join(&resolved_name).to_string_lossy().to_string();
    let total_bytes = meta.total_size.unwrap_or(0);
    let chunks = if !meta.supports_ranges || total_bytes == 0 {
        1
    } else {
        chunk_count.unwrap_or(if total_bytes > 2 * 1024 * 1024 { 8 } else { 1 }).clamp(1, 32)
    };
    let category = downloader::detect_category_with_mime(&resolved_name, meta.content_type.as_deref());

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let task = DownloadTask {
        id: task_id.clone(),
        url,
        filename: resolved_name,
        save_path,
        total_bytes,
        downloaded_bytes: 0,
        percent: 0.0,
        speed_bps: 0,
        speed_formatted: "0 B/s".to_string(),
        eta_formatted: "--".to_string(),
        status: "queued".to_string(),
        category,
        chunk_count: chunks,
        is_resumable: meta.supports_ranges,
        low_disk_space: low_disk_space.unwrap_or(false),
        chunks: Vec::new(),
        speed_history: Vec::new(),
        error_message: None,
        created_at: now,
    };

    queue::enqueue_download(app, state.inner().clone(), task).await
}

#[command]
async fn pause_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
) -> Result<(), String> {
    let _ = media::cancel_download(download_state, Some(task_id.clone())).await;
    queue::pause_download(app, state.inner().clone(), task_id).await
}

#[command]
async fn resume_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
) -> Result<(), String> {
    // 1. Check if this is a media (yt-dlp) task with saved options
    let maybe_opts = {
        let opts_lock = download_state.task_options.lock().unwrap();
        opts_lock.get(&task_id).cloned()
    };

    if let Some(mut opts) = maybe_opts {
        opts.task_id = Some(task_id.clone());
        download_state.cancelled_tasks.lock().unwrap().insert(task_id.clone(), false);
        let _ = media::start_download(app, download_state, opts).await;
        return Ok(());
    }

    // 2. Otherwise it's an HTTP engine task
    queue::resume_download(app, state.inner().clone(), task_id).await
}

#[command]
async fn cancel_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
) -> Result<(), String> {
    let _ = media::cancel_download(download_state, Some(task_id.clone())).await;
    queue::cancel_download(app, state.inner().clone(), task_id).await
}

#[command]
async fn remove_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
    delete_file: Option<bool>,
) -> Result<(), String> {
    let _ = media::cancel_download(download_state, Some(task_id.clone())).await;
    queue::remove_download(app, state.inner().clone(), task_id, delete_file.unwrap_or(false)).await
}

#[command]
async fn get_all_downloads(
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
) -> Result<Vec<DownloadTask>, String> {
    let mut list: Vec<DownloadTask> = {
        let tasks = state.tasks.read().await;
        tasks.values().cloned().collect()
    };
    {
        let media_tasks = download_state.tasks.lock().unwrap();
        for t in media_tasks.values() {
            if let Some(pos) = list.iter().position(|existing| existing.id == t.id) {
                list[pos] = t.clone();
            } else {
                list.push(t.clone());
            }
        }
    }
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(list)
}

#[command]
async fn get_download_task(
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
) -> Result<Option<DownloadTask>, String> {
    if task_id.starts_with("media-") {
        {
            let media_tasks = download_state.tasks.lock().unwrap();
            if let Some(t) = media_tasks.get(&task_id) {
                return Ok(Some(t.clone()));
            }
        }
        let tasks = state.tasks.read().await;
        if let Some(t) = tasks.get(&task_id) {
            return Ok(Some(t.clone()));
        }
    } else {
        {
            let tasks = state.tasks.read().await;
            if let Some(t) = tasks.get(&task_id) {
                return Ok(Some(t.clone()));
            }
        }
        let media_tasks = download_state.tasks.lock().unwrap();
        if let Some(t) = media_tasks.get(&task_id) {
            return Ok(Some(t.clone()));
        }
    }
    Ok(None)
}

#[command]
fn get_default_download_dir() -> String {
    dirs_or_default_downloads()
}

fn dirs_or_default_downloads() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(user_profile) = env::var("USERPROFILE") {
            let path = PathBuf::from(user_profile).join("Downloads");
            if path.exists() {
                return path.to_string_lossy().to_string();
            }
        }
    }

    if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join("Downloads");
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }

    env::temp_dir().to_string_lossy().to_string()
}

// ---------------------------------------------------------------------------
// Protocol & Local Bridge Server (for Browser Extension Interception)
// ---------------------------------------------------------------------------

pub fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 2);
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

pub fn decode_percent_encoded(input: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex_str = [h1, h2];
                if let Ok(hex_str) = std::str::from_utf8(&hex_str) {
                    if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                        bytes.push(val);
                        continue;
                    }
                }
                bytes.push(b'%');
                bytes.push(h1);
                bytes.push(h2);
            } else {
                bytes.push(b'%');
            }
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn parse_open_query(query_str: &str) -> Option<serde_json::Value> {
    let mut target_url = None;
    let mut target_filename = None;
    let mut target_quality = None;
    let mut target_referrer = None;
    let mut target_container = None;
    let mut target_format_id = None;
    let mut is_torrent = false;
    let mut auto_start = false;
    let mut is_direct_file = false;
    let mut low_disk_space = false;

    for part in query_str.split('&') {
        if part.is_empty() {
            continue;
        }
        let lower = part.to_ascii_lowercase();
        if lower.starts_with("url=") {
            target_url = Some(decode_percent_encoded(&part[4..]));
        } else if lower.starts_with("filename=") {
            target_filename = Some(decode_percent_encoded(&part[9..]));
        } else if lower.starts_with("quality=") {
            target_quality = Some(decode_percent_encoded(&part[8..]));
        } else if lower.starts_with("autostart=") {
            let val = &part[10..];
            auto_start = val == "true" || val == "1";
        } else if lower.starts_with("istorrent=") {
            let val = &part[10..];
            is_torrent = val == "true" || val == "1";
        } else if lower.starts_with("referrer=") {
            target_referrer = Some(decode_percent_encoded(&part[9..]));
        } else if lower.starts_with("container=") {
            target_container = Some(decode_percent_encoded(&part[10..]));
        } else if lower.starts_with("formatid=") {
            target_format_id = Some(decode_percent_encoded(&part[9..]));
        } else if lower.starts_with("isdirectfile=") {
            let val = &part[13..];
            is_direct_file = val == "true" || val == "1";
        } else if lower.starts_with("lowdiskspace=") {
            let val = &part[13..];
            low_disk_space = val == "true" || val == "1";
        }
    }

    if let Some(url) = target_url {
        let clean = url.trim().to_string();
        if !clean.is_empty() {
            let is_detected_torrent = is_torrent
                || clean.starts_with("magnet:")
                || clean.ends_with(".torrent")
                || clean.contains(".torrent");
            return Some(serde_json::json!({
                "url": clean,
                "filename": target_filename,
                "quality": target_quality,
                "autoStart": auto_start,
                "isTorrent": is_detected_torrent,
                "referrer": target_referrer,
                "container": target_container,
                "formatId": target_format_id,
                "isDirectFile": is_direct_file,
                "lowDiskSpace": low_disk_space
            }));
        }
    }
    None
}

static FORMATS_CACHE: std::sync::OnceLock<Mutex<HashMap<String, (std::time::Instant, String)>>> = std::sync::OnceLock::new();

async fn start_local_bridge(app_handle: AppHandle) {
    let mut listener = None;
    for _ in 0..15 {
        match tokio::net::TcpListener::bind("127.0.0.1:9666").await {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }
    }

    let listener = match listener {
        Some(l) => l,
        None => {
            eprintln!("ZR Downloader: Could not bind local bridge port 9666 after retries");
            return;
        }
    };

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 16384];
                if let Ok(n) = stream.read(&mut buf).await {
                    let req_str = String::from_utf8_lossy(&buf[..n]);

                    if req_str.starts_with("OPTIONS") {
                        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes()).await;
                        return;
                    }

                    let first_line = req_str.lines().next().unwrap_or("").to_string();
                    if first_line.starts_with("GET /formats?") {
                        // Real, accurate quality list for the browser extension (IDM-style), powered by yt-dlp.
                        let q = &first_line["GET /formats?".len()..];
                        let q = q.split(" HTTP/").next().unwrap_or(q);
                        let mut target = String::new();
                        for part in q.split('&') {
                            if let Some(v) = part.strip_prefix("url=") {
                                target = decode_percent_encoded(v);
                            }
                        }
                        let (status_line, body) = if target.trim().is_empty() {
                            ("400 Bad Request", "{\"error\":\"missing_url\"}".to_string())
                        } else {
                            let cache = FORMATS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
                            let cached = cache.lock().ok().and_then(|m| {
                                m.get(&target).and_then(|(t, body)| {
                                    if t.elapsed() < std::time::Duration::from_secs(600) { Some(body.clone()) } else { None }
                                })
                            });
                            if let Some(body) = cached {
                                ("200 OK", body)
                            } else {
                                match tokio::time::timeout(
                                    std::time::Duration::from_secs(60),
                                    media::fetch_media_info(target.clone(), None, None, None, None),
                                )
                                .await
                                {
                                    Ok(Ok(info)) => {
                                        let body = serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string());
                                        if let Ok(mut m) = cache.lock() {
                                            if m.len() > 64 { m.clear(); }
                                            m.insert(target.clone(), (std::time::Instant::now(), body.clone()));
                                        }
                                        ("200 OK", body)
                                    }
                                    Ok(Err(e)) => ("502 Bad Gateway", serde_json::json!({ "error": e }).to_string()),
                                    Err(_) => ("504 Gateway Timeout", "{\"error\":\"timeout\"}".to_string()),
                                }
                            }
                        };
                        let resp = format!(
                            "HTTP/1.1 {}\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status_line,
                            body.as_bytes().len(),
                            body
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                        return;
                    }

                    if first_line.starts_with("GET /ping") || req_str.contains("/ping") && !first_line.contains("/open?") {
                        let resp = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"alive\",\"app\":\"ZR Downloader\",\"version\":\"1.2.9\"}\r\n";
                        let _ = stream.write_all(resp.as_bytes()).await;
                        return;
                    }

                    let mut payload_opt = None;
                    if let Some(idx) = req_str.find("/open?") {
                        let query_str = &req_str[idx + 6..];
                        let end_idx = query_str.find(|c: char| c == ' ' || c == '\r' || c == '\n').unwrap_or(query_str.len());
                        let clean_query = &query_str[..end_idx];
                        payload_opt = parse_open_query(clean_query);
                    }

                    if let Some(payload) = payload_opt {
                        let _ = handle.emit("open-url", payload);
                        if let Some(win) = handle.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_always_on_top(true);
                            let _ = win.set_focus();
                            let _ = win.set_always_on_top(false);
                        }
                        let body = "{\"status\":\"ok\",\"received\":true,\"app\":\"ZR Downloader\"}";
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                    } else {
                        let body = "{\"error\":\"missing_url\"}";
                        let resp = format!(
                            "HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                    }
                }
            });
        }
    }
}

#[cfg(target_os = "windows")]
fn register_windows_protocols() {
    if let Ok(exe_path) = env::current_exe() {
        let exe_str = exe_path.to_string_lossy();
        let cmd = format!("\"{}\" \"%1\"", exe_str);

        // Register zr:// protocol
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\zr", "/ve", "/d", "URL:ZR Downloader Protocol", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\zr", "/v", "URL Protocol", "/d", "", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\zr\\shell\\open\\command", "/ve", "/d", &cmd, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        // Also register gdm:// for backward compatibility with older extensions/shortcuts
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\gdm", "/ve", "/d", "URL:ZR Downloader Protocol", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\gdm", "/v", "URL Protocol", "/d", "", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\gdm\\shell\\open\\command", "/ve", "/d", &cmd, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        // Also register parrot:// for backward compatibility with the extension
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\parrot", "/ve", "/d", "URL:Parrot Protocol", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\parrot", "/v", "URL Protocol", "/d", "", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\parrot\\shell\\open\\command", "/ve", "/d", &cmd, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        // Register magnet:// protocol handler for BitTorrent
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\magnet", "/ve", "/d", "URL:BitTorrent Magnet Link", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\magnet", "/v", "URL Protocol", "/d", "", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\magnet\\shell\\open\\command", "/ve", "/d", &cmd, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        // Register .torrent file association
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\.torrent", "/ve", "/d", "ZRDownloader.Torrent", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\ZRDownloader.Torrent", "/ve", "/d", "Torrent File", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("reg")
            .args(["add", "HKCU\\Software\\Classes\\ZRDownloader.Torrent\\shell\\open\\command", "/ve", "/d", &cmd, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
}

#[command]
fn get_pending_url(state: State<'_, AppUrlState>) -> Option<serde_json::Value> {
    let mut lock = state.pending_url.lock().ok()?;
    lock.take()
}

#[command]
async fn ytdlp_get_info(url: String) -> Result<media::MediaInfo, String> {
    media::fetch_media_info(url, None, None, None, None).await
}

#[command]
async fn ytdlp_download(
    app: AppHandle,
    state: State<'_, DownloadState>,
    options: media::DownloadOptions,
) -> Result<String, String> {
    media::start_download(app, state, options).await
}

#[command]
async fn open_download_window(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
    title: Option<String>,
    initial_task: Option<serde_json::Value>,
) -> Result<(), String> {
    let resolved_title = title.clone().unwrap_or_else(|| {
        initial_task
            .as_ref()
            .and_then(|v| v.get("filename"))
            .and_then(|v| v.as_str())
            .unwrap_or("ZR Downloader")
            .to_string()
    });
    if let Some(ref val) = initial_task {
        let is_media = task_id.starts_with("media-");
        if is_media {
            if let Ok(mut tasks_lock) = download_state.tasks.lock() {
                if !tasks_lock.contains_key(&task_id) {
                    let t = DownloadTask {
                        id: task_id.clone(),
                        url: val.get("url").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        filename: val.get("filename").and_then(|v| v.as_str()).unwrap_or("Download.mp4").to_string(),
                        save_path: val.get("save_path").or_else(|| val.get("savePath")).and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        total_bytes: val.get("total_bytes").or_else(|| val.get("totalBytes")).and_then(|v| v.as_u64()).unwrap_or(0),
                        downloaded_bytes: val.get("downloaded_bytes").or_else(|| val.get("downloadedBytes")).and_then(|v| v.as_u64()).unwrap_or(0),
                        percent: val.get("percent").and_then(|v| v.as_f64()).unwrap_or(1.0),
                        speed_bps: val.get("speed_bps").or_else(|| val.get("speedBps")).and_then(|v| v.as_u64()).unwrap_or(0),
                        speed_formatted: val.get("speed_formatted").or_else(|| val.get("speedFormatted")).and_then(|v| v.as_str()).unwrap_or("Connecting...").to_string(),
                        eta_formatted: val.get("eta_formatted").or_else(|| val.get("etaFormatted")).and_then(|v| v.as_str()).unwrap_or("--").to_string(),
                        status: val.get("status").and_then(|v| v.as_str()).unwrap_or("downloading").to_string(),
                        category: val.get("category").and_then(|v| v.as_str()).unwrap_or("video").to_string(),
                        chunk_count: 1,
                        is_resumable: true,
                        low_disk_space: val.get("low_disk_space").or_else(|| val.get("lowDiskSpace")).and_then(|v| v.as_bool()).unwrap_or(false),
                        chunks: Vec::new(),
                        speed_history: Vec::new(),
                        error_message: None,
                        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    };
                    tasks_lock.insert(task_id.clone(), t);
                }
            }
        } else {
            let mut engine_tasks = state.tasks.write().await;
            if !engine_tasks.contains_key(&task_id) {
                let t = DownloadTask {
                    id: task_id.clone(),
                    url: val.get("url").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    filename: val.get("filename").and_then(|v| v.as_str()).unwrap_or("Download.mp4").to_string(),
                    save_path: val.get("save_path").or_else(|| val.get("savePath")).and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    total_bytes: val.get("total_bytes").or_else(|| val.get("totalBytes")).and_then(|v| v.as_u64()).unwrap_or(0),
                    downloaded_bytes: val.get("downloaded_bytes").or_else(|| val.get("downloadedBytes")).and_then(|v| v.as_u64()).unwrap_or(0),
                    percent: val.get("percent").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    speed_bps: val.get("speed_bps").or_else(|| val.get("speedBps")).and_then(|v| v.as_u64()).unwrap_or(0),
                    speed_formatted: val.get("speed_formatted").or_else(|| val.get("speedFormatted")).and_then(|v| v.as_str()).unwrap_or("Connecting...").to_string(),
                    eta_formatted: val.get("eta_formatted").or_else(|| val.get("etaFormatted")).and_then(|v| v.as_str()).unwrap_or("--").to_string(),
                    status: val.get("status").and_then(|v| v.as_str()).unwrap_or("downloading").to_string(),
                    category: val.get("category").and_then(|v| v.as_str()).unwrap_or("video").to_string(),
                    chunk_count: 1,
                    is_resumable: true,
                    low_disk_space: val.get("low_disk_space").or_else(|| val.get("lowDiskSpace")).and_then(|v| v.as_bool()).unwrap_or(false),
                    chunks: Vec::new(),
                    speed_history: Vec::new(),
                    error_message: None,
                    created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                };
                engine_tasks.insert(task_id.clone(), t);
            }
        }
    }
    let clean_label = format!("dl_{}", task_id.replace(|c: char| !c.is_alphanumeric(), "_"));
    if let Some(w) = app.get_webview_window(&clean_label) {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }

    let url_str = format!("index.html?popup=true&taskId={}", task_id);
    let win = WebviewWindowBuilder::new(
        &app,
        &clean_label,
        WebviewUrl::App(url_str.into()),
    )
    .title(&resolved_title)
    .inner_size(520.0, 420.0)
    .min_inner_size(480.0, 360.0)
    .resizable(true)
    .decorations(true)
    .center()
    .build()
    .map_err(|e| format!("Window spawn error: {}", e))?;

    let _ = win.show();
    let _ = win.set_focus();

    Ok(())
}


// ---------------------------------------------------------------------------
// App Entrypoint
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut pending_arg_req: Option<String> = None;
    let mut pending_arg_payload: Option<serde_json::Value> = None;
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        for arg in &args[1..] {
            let mut trimmed = arg.trim();
            if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
                trimmed = &trimmed[1..trimmed.len() - 1].trim();
            }
            if trimmed.starts_with("@[") && trimmed.ends_with(']') && trimmed.len() >= 3 {
                trimmed = &trimmed[2..trimmed.len() - 1].trim();
            }
            if trimmed.starts_with("zr://") || trimmed.starts_with("gdm://") || trimmed.starts_with("parrot://") || trimmed.starts_with("gmdownloader://") {
                let without_proto = if trimmed.starts_with("zr://") {
                    &trimmed["zr://".len()..]
                } else if trimmed.starts_with("gdm://") {
                    &trimmed["gdm://".len()..]
                } else if trimmed.starts_with("parrot://") {
                    &trimmed["parrot://".len()..]
                } else {
                    &trimmed["gmdownloader://".len()..]
                };
                if let Some(pos) = without_proto.find("open?") {
                    pending_arg_req = Some(without_proto[pos..].to_string());
                    let query = &without_proto[pos + 5..];
                    pending_arg_payload = parse_open_query(query);
                    break;
                } else if let Some(pos) = without_proto.find("?url=") {
                    let query = &without_proto[pos + 1..];
                    pending_arg_req = Some(format!("open?{}", query));
                    pending_arg_payload = parse_open_query(query);
                    break;
                }
            } else if trimmed.starts_with("magnet:") || trimmed.ends_with(".torrent") || trimmed.contains(".torrent") || trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                let is_torrent = trimmed.starts_with("magnet:") || trimmed.ends_with(".torrent") || trimmed.contains(".torrent");
                let torrent_param = if is_torrent { "&isTorrent=true" } else { "" };
                pending_arg_req = Some(format!("open?url={}{}", url_encode(trimmed), torrent_param));
                pending_arg_payload = Some(serde_json::json!({
                    "url": trimmed,
                    "isTorrent": is_torrent,
                    "autoStart": false
                }));
                break;
            }
        }
    }

    if let Some(ref req_path) = pending_arg_req {
        if let Ok(addr) = "127.0.0.1:9666".parse() {
            if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
                &addr,
                std::time::Duration::from_millis(350),
            ) {
                use std::io::Write;
                let req = format!(
                    "GET /{} HTTP/1.1\r\nHost: 127.0.0.1:9666\r\nConnection: close\r\n\r\n",
                    req_path
                );
                let _ = stream.write_all(req.as_bytes());
                std::process::exit(0);
            }
        }
    }

    tauri::Builder::default()
        .manage(DownloadState {
            active_pids: Arc::new(Mutex::new(HashMap::new())),
            cancelled_tasks: Arc::new(Mutex::new(HashMap::new())),
            task_options: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        })
        .manage(AppUrlState {
            pending_url: Arc::new(Mutex::new(pending_arg_payload)),
        })
        .manage(EngineState::new())
        .manage(torrent::TorrentState::default())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            #[cfg(target_os = "windows")]
            register_windows_protocols();

            // Setup System Tray
            if let (Ok(show_i), Ok(quit_i)) = (
                MenuItem::with_id(app, "show", "Open ZR Downloader", true, None::<&str>),
                MenuItem::with_id(app, "quit", "Exit ZR Downloader", true, None::<&str>),
            ) {
                if let Ok(tray_menu) = Menu::with_items(app, &[&show_i, &quit_i]) {
                    if let Some(icon) = app.default_window_icon() {
                        let _ = TrayIconBuilder::new()
                            .tooltip("ZR Downloader")
                            .icon(icon.clone())
                            .menu(&tray_menu)
                            .show_menu_on_left_click(false)
                            .on_menu_event(|app, event| {
                                match event.id.as_ref() {
                                    "show" => {
                                        if let Some(window) = app.get_webview_window("main") {
                                            let _ = window.show();
                                            let _ = window.unminimize();
                                            let _ = window.set_focus();
                                        }
                                    }
                                    "quit" => {
                                        app.exit(0);
                                    }
                                    _ => {}
                                }
                            })
                            .on_tray_icon_event(|tray, event| {
                                if let TrayIconEvent::Click {
                                    button: MouseButton::Left,
                                    button_state: MouseButtonState::Up,
                                    ..
                                } = event
                                {
                                    let app = tray.app_handle();
                                    if let Some(window) = app.get_webview_window("main") {
                                        let _ = window.show();
                                        let _ = window.unminimize();
                                        let _ = window.set_focus();
                                    }
                                }
                            })
                            .build(app);
                    }
                }
            }

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                start_local_bridge(handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // HTTP Downloader Commands
            probe_http_url,
            start_http_download,
            pause_http_download,
            resume_http_download,
            cancel_http_download,
            remove_http_download,
            get_all_downloads,
            get_default_download_dir,
            // Media Downloader Commands
            media::fetch_media_info,
            media::start_download,
            media::cancel_download,
            media::check_tool_updates,
            media::update_tool,
            media::open_folder,
            media::open_file,
            media::install_app_update,
            get_pending_url,
            ytdlp_get_info,
            ytdlp_download,
            open_download_window,
            get_download_task,
            // BitTorrent Engine Commands (librqbit)
            torrent::add_torrent,
            torrent::get_torrent_details,
            torrent::pause_torrent,
            torrent::resume_torrent,
            torrent::remove_torrent,
            torrent::get_torrent_peers,
            torrent::get_torrent_pieces
        ])
        .run(tauri::generate_context!())
        .expect("error while running ZR Downloader");
}
