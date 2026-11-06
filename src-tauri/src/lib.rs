pub mod downloader;
pub mod media;
pub mod queue;
pub mod torrent;

use downloader::{DownloadTask, EngineState, UrlMetadata};
use media::{AppUrlState, DownloadState};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{command, AppHandle, Emitter, Manager, State};

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
) -> Result<String, String> {
    let meta = downloader::probe_url(&state.client, &url).await.unwrap_or(UrlMetadata {
        url: url.clone(),
        filename: "download.bin".to_string(),
        total_size: None,
        supports_ranges: false,
        content_type: None,
        category: "docs".to_string(),
    });

    let resolved_name = filename
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(meta.filename);

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
) -> Result<Vec<DownloadTask>, String> {
    let tasks = state.tasks.read().await;
    let mut list: Vec<DownloadTask> = tasks.values().cloned().collect();
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(list)
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
                let mut buf = [0u8; 4096];
                if let Ok(n) = stream.read(&mut buf).await {
                    let req_str = String::from_utf8_lossy(&buf[..n]);

                    if req_str.starts_with("OPTIONS") {
                        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes()).await;
                        return;
                    }

                    if req_str.contains("/ping") {
                        let resp = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"alive\",\"app\":\"ZR Downloader\",\"version\":\"1.2.5\"}\r\n";
                        let _ = stream.write_all(resp.as_bytes()).await;
                        return;
                    }

                    let mut target_url = None;
                    let mut target_filename = None;
                    let mut target_quality = None;
                    let mut target_referrer = None;
                    let mut target_container = None;
                    let mut is_torrent = false;
                    let mut auto_start = false;
                    let mut is_direct_file = false;
                    if let Some(idx) = req_str.find("/open?") {
                        let query_str = &req_str[idx + 6..];
                        let end_idx = query_str.find(|c: char| c == ' ' || c == '\r' || c == '\n').unwrap_or(query_str.len());
                        let clean_query = &query_str[..end_idx];

                        for part in clean_query.split('&') {
                            if part.starts_with("url=") {
                                target_url = Some(decode_percent_encoded(&part[4..]));
                            } else if part.starts_with("filename=") {
                                target_filename = Some(decode_percent_encoded(&part[9..]));
                            } else if part.starts_with("quality=") {
                                target_quality = Some(decode_percent_encoded(&part[8..]));
                            } else if part.starts_with("autostart=") {
                                auto_start = &part[10..] == "true" || &part[10..] == "1";
                            } else if part.starts_with("isTorrent=") {
                                is_torrent = &part[10..] == "true" || &part[10..] == "1";
                            } else if part.starts_with("referrer=") {
                                target_referrer = Some(decode_percent_encoded(&part[9..]));
                            } else if part.starts_with("container=") {
                                target_container = Some(decode_percent_encoded(&part[10..]));
                            } else if part.starts_with("isDirectFile=") {
                                is_direct_file = &part[13..] == "true" || &part[13..] == "1";
                            }
                        }
                    }

                    if let Some(url) = target_url {
                        let clean = url.trim().to_string();
                        if !clean.is_empty() {
                            let is_detected_torrent = is_torrent
                                || clean.starts_with("magnet:")
                                || clean.ends_with(".torrent")
                                || clean.contains(".torrent");
                            let payload = serde_json::json!({
                                "url": clean,
                                "filename": target_filename,
                                "quality": target_quality,
                                "autoStart": auto_start,
                                "isTorrent": is_detected_torrent,
                                "referrer": target_referrer,
                                "container": target_container,
                                "isDirectFile": is_direct_file
                            });
                            let _ = handle.emit("open-url", payload.clone());
                            let _ = handle.emit("zr://open-url", payload);
                            if let Some(win) = handle.get_webview_window("main") {
                                let _ = win.unminimize();
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
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
fn get_pending_url(state: State<'_, AppUrlState>) -> Option<String> {
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

// ---------------------------------------------------------------------------
// App Entrypoint
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut pending_arg_req: Option<String> = None;
    let mut pending_arg_url: Option<String> = None;
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
            if trimmed.starts_with("zr://") || trimmed.starts_with("parrot://") || trimmed.starts_with("gmdownloader://") {
                let without_proto = if trimmed.starts_with("zr://") {
                    &trimmed["zr://".len()..]
                } else if trimmed.starts_with("parrot://") {
                    &trimmed["parrot://".len()..]
                } else {
                    &trimmed["gmdownloader://".len()..]
                };
                if let Some(pos) = without_proto.find("open?") {
                    pending_arg_req = Some(without_proto[pos..].to_string());
                    let query = &without_proto[pos + 5..];
                    for part in query.split('&') {
                        if part.starts_with("url=") {
                            pending_arg_url = Some(decode_percent_encoded(&part[4..]));
                        }
                    }
                    break;
                } else if let Some(pos) = without_proto.find("?url=") {
                    pending_arg_req = Some(format!("open{}", &without_proto[pos..]));
                    pending_arg_url = Some(decode_percent_encoded(&without_proto[pos + 5..]));
                    break;
                }
            } else if trimmed.starts_with("magnet:") || trimmed.ends_with(".torrent") || trimmed.contains(".torrent") || trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                let is_torrent = trimmed.starts_with("magnet:") || trimmed.ends_with(".torrent") || trimmed.contains(".torrent");
                let torrent_param = if is_torrent { "&isTorrent=true" } else { "" };
                pending_arg_req = Some(format!("open?url={}{}", url_encode(trimmed), torrent_param));
                pending_arg_url = Some(trimmed.to_string());
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
        })
        .manage(AppUrlState {
            pending_url: Arc::new(Mutex::new(pending_arg_url)),
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
