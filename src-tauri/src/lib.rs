pub mod aria2_rpc;
pub mod clipper;
pub mod downloader;
pub mod ftp_downloader;
pub mod media;
pub mod queue;
pub mod scheduler;
pub mod settings;
pub mod torrent;
pub mod url_utils;

use downloader::{DownloadTask, EngineState, UrlMetadata};
use media::{AppUrlState, DownloadState};
use scheduler::{SchedulerConfig, SchedulerState};
use settings::{AppSettings, SettingsState};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{command, AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

pub static ACTIVE_BRIDGE_PORT: AtomicU16 = AtomicU16::new(9666);

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct PersistedTask {
    task: DownloadTask,
    #[serde(default)]
    options: Option<media::DownloadOptions>,
    #[serde(default)]
    is_media: bool,
}

static LAST_SAVED_TASKS_JSON: std::sync::OnceLock<Mutex<String>> = std::sync::OnceLock::new();

fn tasks_file_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("tasks.json"))
}

fn read_persisted_tasks_file(app: &AppHandle) -> Vec<PersistedTask> {
    let Some(path) = tasks_file_path(app) else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<PersistedTask>>(&content).unwrap_or_default()
}

fn write_persisted_tasks_if_changed(app: &AppHandle, snapshot: &[PersistedTask]) {
    let Some(target) = tasks_file_path(app) else {
        return;
    };
    let Ok(json) = serde_json::to_string_pretty(snapshot) else {
        return;
    };
    let cache = LAST_SAVED_TASKS_JSON.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut prev) = cache.lock() {
        // Always write atomically when empty so deleted tasks never stay in tasks.json.
        if !snapshot.is_empty() && *prev == json && target.exists() {
            return;
        }
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = target.with_extension("json.tmp");
        if std::fs::write(&tmp_path, json.as_bytes()).is_ok() {
            if std::fs::rename(&tmp_path, &target).is_err() {
                let _ = std::fs::copy(&tmp_path, &target);
                let _ = std::fs::remove_file(&tmp_path);
            }
            *prev = json;
        }
    }
}

pub async fn save_all_tasks(app: &AppHandle) {
    let engine_state = app.state::<EngineState>();
    let download_state = app.state::<DownloadState>();

    let mut merged: HashMap<String, PersistedTask> = HashMap::new();
    {
        let http_tasks = engine_state.tasks.read().await;
        for (id, t) in http_tasks.iter() {
            merged.insert(
                id.clone(),
                PersistedTask {
                    task: t.clone(),
                    options: None,
                    is_media: false,
                },
            );
        }
    }
    {
        let media_tasks = download_state.tasks.lock().unwrap();
        let media_opts = download_state.task_options.lock().unwrap();
        for (id, t) in media_tasks.iter() {
            let opts = media_opts.get(id).cloned();
            merged.insert(
                id.clone(),
                PersistedTask {
                    task: t.clone(),
                    options: opts,
                    is_media: true,
                },
            );
        }
    }

    let mut list: Vec<PersistedTask> = merged.into_values().collect();
    list.sort_by(|a, b| b.task.created_at.cmp(&a.task.created_at));
    write_persisted_tasks_if_changed(app, &list);
}

pub fn spawn_save_all_tasks(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        save_all_tasks(&handle).await;
    });
}

fn save_all_tasks_sync(app: &AppHandle) {
    if tokio::runtime::Handle::try_current().is_ok() {
        // Inside an async runtime worker: try non-blocking read to avoid nested block_on panic
        let engine_state = app.state::<EngineState>();
        let download_state = app.state::<DownloadState>();
        let mut merged: HashMap<String, PersistedTask> = HashMap::new();
        if let Ok(http_tasks) = engine_state.tasks.try_read() {
            for (id, t) in http_tasks.iter() {
                merged.insert(
                    id.clone(),
                    PersistedTask {
                        task: t.clone(),
                        options: None,
                        is_media: false,
                    },
                );
            }
        }
        {
            let media_tasks = download_state.tasks.lock().unwrap();
            let media_opts = download_state.task_options.lock().unwrap();
            for (id, t) in media_tasks.iter() {
                merged.insert(
                    id.clone(),
                    PersistedTask {
                        task: t.clone(),
                        options: media_opts.get(id).cloned(),
                        is_media: true,
                    },
                );
            }
        }
        let mut list: Vec<PersistedTask> = merged.into_values().collect();
        list.sort_by(|a, b| b.task.created_at.cmp(&a.task.created_at));
        write_persisted_tasks_if_changed(app, &list);
    } else {
        tauri::async_runtime::block_on(save_all_tasks(app));
    }
}

// ---------------------------------------------------------------------------
// HTTP Downloader Commands
// ---------------------------------------------------------------------------

#[command]
async fn probe_http_url(
    state: State<'_, EngineState>,
    settings_state: State<'_, SettingsState>,
    url: String,
    auth_username: Option<String>,
    auth_password: Option<String>,
) -> Result<UrlMetadata, String> {
    let app_settings = settings_state.get().await;
    let unwrapped = url_utils::unwrap_custom_url_scheme(&url);
    let effective_url = url_utils::apply_mirrors(
        &unwrapped,
        app_settings.effective_gh_mirror(),
        app_settings.effective_hf_mirror(),
    );
    if effective_url.starts_with("ftp://") {
        return ftp_downloader::probe_ftp_url(&effective_url).await;
    }
    let client = state.current_client().await;
    let site_auth = settings::find_credentials_for_url(&app_settings, &effective_url)
        .map(|(u, p)| (u.to_string(), p.to_string()));
    let explicit_user = auth_username.unwrap_or_default();
    let explicit_pass = auth_password.unwrap_or_default();
    let auth_ref = if !explicit_user.trim().is_empty() {
        Some((explicit_user.trim(), explicit_pass.as_str()))
    } else {
        site_auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str()))
    };
    downloader::probe_url_with_auth(&client, &effective_url, auth_ref).await
}

#[allow(clippy::too_many_arguments)]
#[command]
async fn start_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    settings_state: State<'_, SettingsState>,
    url: String,
    filename: Option<String>,
    save_folder: Option<String>,
    chunk_count: Option<usize>,
    low_disk_space: Option<bool>,
    auto_connections: Option<bool>,
    speed_limit_kb: Option<u64>,
    priority: Option<String>,
    expected_checksum: Option<String>,
    origin_page_url: Option<String>,
    post_download_action: Option<String>,
    post_download_program: Option<String>,
    auth_username: Option<String>,
    auth_password: Option<String>,
) -> Result<String, String> {
    let app_settings = settings_state.get().await;
    let unwrapped = url_utils::unwrap_custom_url_scheme(&url);
    let url = url_utils::apply_mirrors(
        &unwrapped,
        app_settings.effective_gh_mirror(),
        app_settings.effective_hf_mirror(),
    );

    if url.starts_with("ftp://") {
        let meta = ftp_downloader::probe_ftp_url(&url).await.unwrap_or(UrlMetadata {
            url: url.clone(),
            filename: "download.bin".to_string(),
            total_size: None,
            supports_ranges: false,
            content_type: None,
            category: "docs".to_string(),
            last_modified: None,
        });
        let resolved_name = filename.filter(|n| !n.trim().is_empty()).unwrap_or(meta.filename);
        let save_dir = save_folder.unwrap_or_else(|| {
            if !app_settings.default_download_dir.trim().is_empty() {
                app_settings.default_download_dir.clone()
            } else {
                std::env::temp_dir().to_string_lossy().to_string()
            }
        });
        let final_path = format!("{}/{}", save_dir.trim_end_matches(['/', '\\']), resolved_name);
        let task_id = uuid::Uuid::new_v4().to_string();
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task = DownloadTask {
            id: task_id.clone(),
            url: url.clone(),
            filename: resolved_name,
            save_path: final_path.clone(),
            total_bytes: meta.total_size.unwrap_or(0),
            status: "downloading".to_string(),
            category: "docs".to_string(),
            priority: priority.unwrap_or_else(|| "normal".to_string()),
            chunk_count: 1,
            is_resumable: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            ..Default::default()
        };
        state.tasks.write().await.insert(task_id.clone(), task.clone());
        let app_c = app.clone();
        let cf_c = cancel_flag.clone();
        tauri::async_runtime::spawn(async move {
            let _ = ftp_downloader::download_ftp_task(app_c, task, cf_c).await;
        });
        spawn_save_all_tasks(&app);
        return Ok(task_id);
    }

    let client = state.current_client().await;
    let site_auth = settings::find_credentials_for_url(&app_settings, &url)
        .map(|(u, p)| (u.to_string(), p.to_string()));
    let u_val = auth_username.clone().unwrap_or_default();
    let p_val = auth_password.clone().unwrap_or_default();
    let auth_ref = if !u_val.trim().is_empty() {
        Some((u_val.trim(), p_val.as_str()))
    } else {
        site_auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str()))
    };

    let has_origin_page = origin_page_url
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let meta = match downloader::probe_url_with_auth(&client, &url, auth_ref).await {
        Ok(m) => m,
        Err(e) => {
            // A definite "gone / not found / unauthorized" answer means there is nothing to
            // download, so tell the user instead of queueing a task that can only fail.
            // Network hiccups and temporary errors still fall through and retry in the engine.
            // Links that can be refreshed from their origin page are always kept.
            if !has_origin_page
                && (e.contains("HTTP 404") || e.contains("HTTP 410") || e.contains("HTTP 401"))
            {
                return Err(e);
            }
            UrlMetadata {
                url: url.clone(),
                filename: "download.bin".to_string(),
                total_size: None,
                supports_ranges: false,
                content_type: None,
                category: "docs".to_string(),
                last_modified: None,
            }
        }
    };

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
    resolved_name = downloader::sanitize_filename(&resolved_name);
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
            ".bin"
        };
        resolved_name = format!("{}{}", resolved_name, ext);
    }

    let resolved_folder = save_folder
        .filter(|f| !f.trim().is_empty())
        .unwrap_or_else(|| {
            if !app_settings.default_download_dir.trim().is_empty() {
                app_settings.default_download_dir.clone()
            } else {
                dirs_or_default_downloads()
            }
        });

    let (final_filename, save_path) = match app_settings.name_conflict.as_str() {
        "overwrite" => {
            let p = PathBuf::from(&resolved_folder).join(&resolved_name);
            (resolved_name.clone(), p.to_string_lossy().to_string())
        }
        "ask" => {
            let target_p = PathBuf::from(&resolved_folder).join(&resolved_name);
            if target_p.exists() {
                if let Ok(m) = std::fs::metadata(&target_p) {
                    let mtime = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let _ = app.emit(
                        "name-conflict-detected",
                        serde_json::json!({
                            "folder": resolved_folder,
                            "filename": resolved_name,
                            "existingSize": m.len(),
                            "existingModified": mtime,
                        }),
                    );
                }
            }
            let unique = build_unique_filename(&resolved_folder, &resolved_name);
            let p = PathBuf::from(&resolved_folder).join(&unique);
            (unique, p.to_string_lossy().to_string())
        }
        _ => {
            let unique = build_unique_filename(&resolved_folder, &resolved_name);
            let p = PathBuf::from(&resolved_folder).join(&unique);
            (unique, p.to_string_lossy().to_string())
        }
    };
    let total_bytes = meta.total_size.unwrap_or(0);
    let is_auto = auto_connections.unwrap_or_else(|| app_settings.connection_mode == "auto");
    let default_conns = app_settings.default_connections.clamp(1, 32);
    let chunks = if !meta.supports_ranges || total_bytes == 0 {
        1
    } else if is_auto {
        8
    } else {
        chunk_count
            .unwrap_or(if total_bytes > 2 * 1024 * 1024 {
                default_conns
            } else {
                1
            })
            .clamp(1, 32)
    };
    let category =
        downloader::detect_category_with_mime(&final_filename, meta.content_type.as_deref());

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let task = DownloadTask {
        id: task_id.clone(),
        url,
        filename: final_filename,
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
        low_disk_space: low_disk_space.unwrap_or(app_settings.low_disk_space),
        chunks: Vec::new(),
        speed_history: Vec::new(),
        error_message: None,
        created_at: now,
        priority: priority.unwrap_or_else(|| "normal".to_string()),
        queue_order: chrono::Utc::now().timestamp_millis(),
        auto_connections: is_auto && meta.supports_ranges && total_bytes > 2 * 1024 * 1024,
        speed_limit_kb: speed_limit_kb.unwrap_or(0),
        expected_checksum: expected_checksum.unwrap_or_default(),
        origin_page_url: origin_page_url.unwrap_or_default(),
        post_download_action: post_download_action.unwrap_or_default(),
        post_download_program: post_download_program.unwrap_or_default(),
        auth_username: u_val,
        auth_password: p_val,
        last_modified: meta.last_modified.clone(),
        ..Default::default()
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
    let _ = media::cancel_download(app.clone(), download_state, Some(task_id.clone())).await;
    let res = queue::pause_download(app.clone(), state.inner().clone(), task_id).await;
    save_all_tasks(&app).await;
    res
}

#[command]
async fn resume_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
) -> Result<(), String> {
    // 1. Check if this is a media (yt-dlp) task with saved options
    let has_media_opts = {
        let opts_lock = download_state.task_options.lock().unwrap();
        opts_lock.contains_key(&task_id)
    };

    if has_media_opts {
        media::resume_media_download(app, download_state, task_id)
            .await
            .map(|_| ())?;
        return Ok(());
    }

    // 2. HTTP task: ensure task exists in EngineState (rebuild from tasks.json if needed)
    let exists_in_memory = {
        let tasks = state.tasks.read().await;
        tasks.contains_key(&task_id)
    };
    if !exists_in_memory {
        let persisted = read_persisted_tasks_file(&app);
        if let Some(entry) = persisted.into_iter().find(|p| p.task.id == task_id) {
            if let Some(opts) = entry.options {
                {
                    let mut t_lock = download_state.tasks.lock().unwrap();
                    t_lock.insert(task_id.clone(), entry.task);
                }
                {
                    let mut o_lock = download_state.task_options.lock().unwrap();
                    o_lock.insert(task_id.clone(), opts);
                }
                media::resume_media_download(app, download_state, task_id)
                    .await
                    .map(|_| ())?;
                return Ok(());
            } else {
                let mut tasks = state.tasks.write().await;
                tasks.insert(task_id.clone(), entry.task);
            }
        }
    }

    // Hydrate from adjacent .zr_sparse_meta_<id>.json and verify Range support before resuming.
    // Requirement A: If server does NOT support resume when partial bytes exist, error instead of restarting from 0%.
    let maybe_task = {
        let tasks = state.tasks.read().await;
        tasks.get(&task_id).cloned()
    };

    if let Some(mut task) = maybe_task {
        downloader::hydrate_task_from_sparse_meta(&mut task).await;
        if task.downloaded_bytes > 0 {
            let client = state.current_client().await;
            let probe_supports_range = match downloader::probe_url(&client, &task.url).await {
                Ok(meta) => meta.supports_ranges,
                Err(_) => task.is_resumable,
            };
            if !probe_supports_range {
                // If the task was previously in error or was non-resumable, user is clicking "Retry":
                // Restart cleanly from 0% since server cannot accept partial ranges.
                if task.status == "error" || !task.is_resumable {
                    let meta_path = downloader::sparse_meta_path_for(&task.save_path, &task.id);
                    let _ = std::fs::remove_file(meta_path);
                    if let Some(parent) = std::path::Path::new(&task.save_path).parent() {
                        let tmp_dir = parent.join(format!(".zr_tmp_{}", task.id));
                        let _ = std::fs::remove_dir_all(tmp_dir);
                    }
                    task.downloaded_bytes = 0;
                    task.percent = 0.0;
                    task.is_resumable = false;
                    task.error_message = None;
                    task.error_log = Vec::new();
                    task.status = "queued".to_string();
                } else {
                    let msg = "Server does not support resume".to_string();
                    let log_lines = vec![
                        format!("URL: {}", task.url),
                        "Detail: HTTP HEAD/Range probe on resume indicated the server no longer accepts partial byte ranges.".to_string(),
                    ];
                    task.is_resumable = false;
                    task.status = "error".to_string();
                    task.speed_bps = 0;
                    task.speed_formatted = "0 B/s".to_string();
                    task.eta_formatted = "--".to_string();
                    task.error_message = Some(msg.clone());
                    task.error_log = log_lines.clone();
                    append_failure_log(&app, &task_id, &task.filename, &task.url, &msg, &log_lines);
                    {
                        let mut tasks = state.tasks.write().await;
                        tasks.insert(task_id.clone(), task.clone());
                    }
                    let _ = app.emit(
                        "download-error",
                        serde_json::json!({ "id": task_id, "error": msg, "errorLog": log_lines }),
                    );
                    let _ = app.emit("download-progress", task);
                    save_all_tasks(&app).await;
                    return Err("Server does not support resume".to_string());
                }
            }
        }
        let mut tasks = state.tasks.write().await;
        tasks.insert(task_id.clone(), task);
    }

    queue::resume_download(app, state.inner().clone(), task_id).await
}

#[command]
async fn relink_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    settings_state: State<'_, SettingsState>,
    task_id: String,
    new_url: String,
    headers: Option<HashMap<String, String>>,
    proxy: Option<String>,
) -> Result<(), String> {
    let clean_url = new_url.trim().to_string();
    if clean_url.is_empty() {
        return Err("New URL cannot be empty".to_string());
    }

    // 1. Check if task exists and grab old size
    let old_total_bytes = {
        let tasks = state.tasks.read().await;
        let t = tasks
            .get(&task_id)
            .ok_or_else(|| format!("Task {} not found", task_id))?;
        t.total_bytes
    };

    // 2. Build client with proxy if specified, or use default client
    let app_settings = settings_state.get().await;
    let client = if let Some(ref p_str) = proxy {
        if !p_str.trim().is_empty() {
            let mut b = reqwest::Client::builder();
            if !app_settings.verify_ssl {
                b = b.danger_accept_invalid_certs(true);
            }
            if let Ok(p) = reqwest::Proxy::all(p_str.trim()) {
                b = b.proxy(p);
            }
            b.build()
                .unwrap_or_else(|_| settings::build_reqwest_client(&app_settings))
        } else {
            state.current_client().await
        }
    } else {
        state.current_client().await
    };

    // 3. Probe the new URL
    let mut probe_req = client
        .get(&clean_url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .timeout(std::time::Duration::from_secs(10));
    if let Some(ref hdrs) = headers {
        for (k, v) in hdrs {
            if let (Ok(hn), Ok(hv)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                probe_req = probe_req.header(hn, hv);
            }
        }
    }

    let meta = match probe_req.send().await {
        Ok(resp) => {
            if !resp.status().is_success() && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(format!("Server returned HTTP {}", resp.status()));
            }
            let is_range = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT
                || resp
                    .headers()
                    .get(reqwest::header::ACCEPT_RANGES)
                    .map(|v| v.to_str().unwrap_or("").contains("bytes"))
                    .unwrap_or(false);

            let mut total_size = None;
            if let Some(cl) = resp.headers().get(reqwest::header::CONTENT_LENGTH) {
                if let Ok(s) = cl.to_str() {
                    if let Ok(sz) = s.parse::<u64>() {
                        total_size = Some(sz);
                    }
                }
            }
            if total_size == Some(1) || total_size.is_none() {
                if let Some(cr) = resp.headers().get("content-range") {
                    if let Ok(cr_str) = cr.to_str() {
                        if let Some(pos) = cr_str.rfind('/') {
                            if let Ok(sz) = cr_str[pos + 1..].trim().parse::<u64>() {
                                total_size = Some(sz);
                            }
                        }
                    }
                }
            }
            (is_range, total_size)
        }
        Err(e) => return Err(format!("Failed to probe new URL: {}", e)),
    };

    let (supports_ranges, new_total_size) = meta;

    if !supports_ranges {
        return Err(
            "The new URL does not support byte ranges (resuming is not possible).".to_string(),
        );
    }

    if let Some(new_sz) = new_total_size {
        if old_total_bytes > 0 && new_sz != old_total_bytes {
            return Err(format!(
                "Size mismatch: current download has {} bytes, but the new URL has {} bytes.",
                old_total_bytes, new_sz
            ));
        }
    }

    // 4. Pause the task if currently active
    let _ = queue::pause_download(app.clone(), state.inner().clone(), task_id.clone()).await;

    // 5. Update task: swap URL, headers, proxy, set status paused so user can resume
    {
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.url = clean_url;
            t.custom_headers = headers;
            t.proxy = proxy;
            t.status = "paused".to_string();
            t.speed_bps = 0;
            t.speed_formatted = "Paused".to_string();
            let _ = app.emit("download-progress", t.clone());
        }
    }

    save_all_tasks(&app).await;
    Ok(())
}

#[command]
async fn cancel_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
) -> Result<(), String> {
    let save_folder_opt = {
        let tasks = download_state.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.get(&task_id).map(|t| t.save_path.clone())
    }.or_else(|| {
        let opts = download_state.task_options.lock().unwrap_or_else(|e| e.into_inner());
        opts.get(&task_id).map(|o| o.save_folder.clone())
    });
    let _ = media::cancel_download(app.clone(), download_state, Some(task_id.clone())).await;
    let res = queue::cancel_download(app.clone(), state.inner().clone(), task_id.clone()).await;
    if let Some(sp) = save_folder_opt {
        media::remove_clip_working_dir(&sp, &task_id);
    }
    save_all_tasks(&app).await;
    res
}

#[command]
async fn remove_http_download(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    settings_state: State<'_, SettingsState>,
    task_id: String,
    delete_file: Option<bool>,
) -> Result<(), String> {
    let app_settings = settings_state.get().await;
    let should_delete = delete_file.unwrap_or(app_settings.delete_files_on_remove);
    let ds_tasks = download_state.tasks.clone();
    let ds_opts = download_state.task_options.clone();
    let _ = media::cancel_download(app.clone(), download_state.clone(), Some(task_id.clone())).await;
    let removed_media = {
        let mut m_tasks = ds_tasks.lock().unwrap_or_else(|e| e.into_inner());
        m_tasks.remove(&task_id)
    };
    let removed_opts = {
        let mut m_opts = ds_opts.lock().unwrap_or_else(|e| e.into_inner());
        m_opts.remove(&task_id)
    };
    if let Some(ref mt) = removed_media {
        media::remove_clip_working_dir(&mt.save_path, &task_id);
        if should_delete {
            media::delete_task_files_and_partials(&mt.save_path, &mt.filename);
        }
    } else if let Some(ref opt) = removed_opts {
        media::remove_clip_working_dir(&opt.save_folder, &task_id);
    }
    let res = queue::remove_download(app.clone(), state.inner().clone(), task_id.clone(), should_delete).await;
    media::forget_task(&download_state, &task_id);
    save_all_tasks(&app).await;
    res
}

#[command]
async fn get_all_downloads(
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    settings_state: State<'_, SettingsState>,
) -> Result<Vec<DownloadTask>, String> {
    let app_settings = settings_state.get().await;
    let mut list: Vec<DownloadTask> = {
        let mut tasks = state.tasks.write().await;
        if app_settings.remove_missing_tasks {
            tasks.retain(|_, t| {
                if matches!(t.status.as_str(), "completed" | "completed_with_warning") {
                    Path::new(&t.save_path).exists()
                } else {
                    true
                }
            });
        }
        tasks.values().cloned().collect()
    };
    {
        let mut media_tasks = download_state.tasks.lock().unwrap();
        if app_settings.remove_missing_tasks {
            media_tasks.retain(|_, t| {
                if matches!(t.status.as_str(), "completed" | "completed_with_warning") {
                    Path::new(&t.save_path).exists()
                } else {
                    true
                }
            });
        }
        for t in media_tasks.values() {
            if let Some(pos) = list.iter().position(|existing| existing.id == t.id) {
                list[pos] = t.clone();
            } else {
                list.push(t.clone());
            }
        }
    }
    list.sort_by(|a, b| {
        a.queue_order
            .cmp(&b.queue_order)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    Ok(list)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateCheckResult {
    is_duplicate: bool,
    reason: String,
    existing_task_id: Option<String>,
    existing_path: Option<String>,
    suggested_filename: String,
}

fn build_unique_filename(folder: &str, filename: &str) -> String {
    let dir = Path::new(folder);
    let path = dir.join(filename);
    if !path.exists() {
        return filename.to_string();
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    for i in 1..1000 {
        let candidate = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{}_{}", chrono::Utc::now().timestamp(), filename)
}

#[command]
fn get_active_bridge_port() -> u16 {
    ACTIVE_BRIDGE_PORT.load(Ordering::Relaxed)
}

#[command]
fn get_active_aria2_rpc_status() -> (bool, u16) {
    (
        aria2_rpc::ARIA2_RPC_RUNNING.load(Ordering::Relaxed),
        aria2_rpc::ACTIVE_ARIA2_RPC_PORT.load(Ordering::Relaxed),
    )
}

#[command]
async fn regenerate_aria2_token(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
) -> Result<String, String> {
    let new_token = uuid::Uuid::new_v4().simple().to_string();
    let mut current = settings_state.get().await;
    current.aria2_rpc_token = new_token.clone();
    settings_state.save(current.clone()).await?;
    let _ = app.emit("app-settings-updated", current);
    Ok(new_token)
}

pub async fn rpc_pause_download(app: &AppHandle, task_id: &str) -> Result<(), String> {
    let dl_state = app.state::<DownloadState>();
    let _ = media::cancel_download(app.clone(), dl_state, Some(task_id.to_string())).await;
    let engine = app.state::<EngineState>();
    let res = queue::pause_download(app.clone(), engine.inner().clone(), task_id.to_string()).await;
    spawn_save_all_tasks(app);
    res
}

pub async fn rpc_resume_download(app: &AppHandle, task_id: &str) -> Result<(), String> {
    let dl_state = app.state::<DownloadState>();
    let engine = app.state::<EngineState>();
    resume_http_download(app.clone(), engine, dl_state, task_id.to_string()).await
}

pub async fn rpc_remove_download(app: &AppHandle, task_id: &str) -> Result<(), String> {
    let dl_state = app.state::<DownloadState>();
    let engine = app.state::<EngineState>();
    let settings = app.state::<SettingsState>();
    remove_http_download(app.clone(), engine, dl_state, settings, task_id.to_string(), None).await
}

pub async fn start_http_download_inner(
    app: &AppHandle,
    state: &EngineState,
    settings_state: &SettingsState,
    raw_url: String,
    out_name: String,
    save_folder: Option<String>,
    custom_headers: Option<HashMap<String, String>>,
) -> Result<DownloadTask, String> {
    let app_settings = settings_state.get().await;
    let unwrapped = url_utils::unwrap_custom_url_scheme(&raw_url);
    let url = url_utils::apply_mirrors(
        &unwrapped,
        app_settings.effective_gh_mirror(),
        app_settings.effective_hf_mirror(),
    );
    let client = state.current_client().await;
    let site_auth = settings::find_credentials_for_url(&app_settings, &url)
        .map(|(u, p)| (u.to_string(), p.to_string()));
    let auth_ref = site_auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let meta = downloader::probe_url_with_auth(&client, &url, auth_ref).await.unwrap_or(UrlMetadata {
        url: url.clone(),
        filename: if !out_name.is_empty() { out_name.clone() } else { "download.bin".to_string() },
        total_size: None,
        supports_ranges: false,
        content_type: None,
        category: "docs".to_string(),
        last_modified: None,
    });

    let resolved_name = if !out_name.trim().is_empty() {
        downloader::sanitize_filename(&out_name)
    } else {
        meta.filename.clone()
    };

    let save_dir = save_folder.unwrap_or_else(|| {
        if !app_settings.default_download_dir.trim().is_empty() {
            app_settings.default_download_dir.clone()
        } else {
            std::env::temp_dir().to_string_lossy().to_string()
        }
    });

    let final_path = format!("{}/{}", save_dir.trim_end_matches(['/', '\\']), resolved_name);
    let task_id = uuid::Uuid::new_v4().to_string();

    let task = DownloadTask {
        id: task_id.clone(),
        url: url.clone(),
        filename: resolved_name,
        save_path: final_path.clone(),
        total_bytes: meta.total_size.unwrap_or(0),
        status: "downloading".to_string(),
        category: meta.category,
        priority: "normal".to_string(),
        chunk_count: if meta.supports_ranges { app_settings.default_connections } else { 1 },
        is_resumable: meta.supports_ranges,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_modified: meta.last_modified,
        custom_headers,
        proxy: app_settings.effective_proxy_url(),
        ..Default::default()
    };

    state.tasks.write().await.insert(task_id.clone(), task.clone());
    let _ = queue::enqueue_download(app.clone(), state.clone(), task.clone()).await;
    spawn_save_all_tasks(app);

    Ok(task)
}

#[command]
async fn regenerate_bridge_token(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
) -> Result<String, String> {
    let new_token = settings::generate_bridge_token();
    let mut current = settings_state.get().await;
    current.bridge_pair_token = new_token.clone();
    settings_state.save(current.clone()).await?;
    let _ = app.emit("app-settings-updated", current);
    Ok(new_token)
}

#[command]
async fn get_app_settings(state: State<'_, SettingsState>) -> Result<AppSettings, String> {
    Ok(state.get().await)
}

#[command]
async fn save_app_settings(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    engine_state: State<'_, EngineState>,
    new_settings: AppSettings,
) -> Result<AppSettings, String> {
    let mut normalized = new_settings;
    normalized.max_active_downloads = normalized.max_active_downloads.clamp(1, 10);
    normalized.default_connections = normalized.default_connections.clamp(1, 32);
    settings_state.save(normalized.clone()).await?;
    engine_state.apply_settings(&normalized).await;
    let _ = settings::apply_start_with_windows(normalized.start_with_windows);
    let _ = app.emit("app-settings-updated", normalized.clone());
    queue::process_queue(&app).await;
    Ok(normalized)
}

#[command]
async fn toggle_global_speed_limit(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    engine_state: State<'_, EngineState>,
    enabled: bool,
) -> Result<AppSettings, String> {
    let mut current = settings_state.get().await;
    current.speed_limit_enabled = enabled;
    settings_state.save(current.clone()).await?;
    engine_state.apply_settings(&current).await;
    let _ = app.emit("app-settings-updated", current.clone());
    Ok(current)
}

#[command]
async fn get_scheduler_config(state: State<'_, SchedulerState>) -> Result<SchedulerConfig, String> {
    Ok(state.get().await)
}

#[command]
async fn save_scheduler_config(
    app: AppHandle,
    state: State<'_, SchedulerState>,
    config: SchedulerConfig,
) -> Result<(), String> {
    state.save(config.clone()).await?;
    let _ = app.emit("scheduler-config-updated", config);
    Ok(())
}

#[command]
async fn cancel_scheduled_shutdown(state: State<'_, SchedulerState>) -> Result<(), String> {
    scheduler::cancel_os_shutdown(state.inner())
}

#[command]
async fn pause_all_downloads_cmd(app: AppHandle) -> Result<(), String> {
    queue::pause_all_downloads(&app).await;
    Ok(())
}

#[command]
async fn resume_all_downloads_cmd(app: AppHandle) -> Result<(), String> {
    queue::resume_scheduled_queue(&app).await;
    Ok(())
}

#[command]
async fn set_task_priority(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
    priority: String,
) -> Result<(), String> {
    let norm = match priority.trim().to_lowercase().as_str() {
        "high" => "high".to_string(),
        "low" => "low".to_string(),
        _ => "normal".to_string(),
    };
    {
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.priority = norm.clone();
            let _ = app.emit("download-progress", t.clone());
        }
    }
    {
        let mut mtasks = download_state.tasks.lock().unwrap();
        if let Some(t) = mtasks.get_mut(&task_id) {
            t.priority = norm.clone();
            let _ = app.emit("download-progress", t.clone());
        }
        let mut mopts = download_state.task_options.lock().unwrap();
        if let Some(o) = mopts.get_mut(&task_id) {
            o.priority = norm;
        }
    }
    save_all_tasks(&app).await;
    queue::process_queue(&app).await;
    Ok(())
}

#[command]
async fn reorder_tasks(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    {
        let mut tasks = state.tasks.write().await;
        for (idx, tid) in ordered_ids.iter().enumerate() {
            if let Some(t) = tasks.get_mut(tid) {
                t.queue_order = (idx as i64) + 1;
            }
        }
    }
    {
        let mut mtasks = download_state.tasks.lock().unwrap();
        let mut mopts = download_state.task_options.lock().unwrap();
        for (idx, tid) in ordered_ids.iter().enumerate() {
            let order_val = (idx as i64) + 1;
            if let Some(t) = mtasks.get_mut(tid) {
                t.queue_order = order_val;
            }
            if let Some(o) = mopts.get_mut(tid) {
                o.queue_order = order_val;
            }
        }
    }
    save_all_tasks(&app).await;
    queue::process_queue(&app).await;
    Ok(())
}

#[command]
async fn update_task_connections(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
    connections: usize,
    auto_connections: bool,
) -> Result<(), String> {
    let clamped = connections.clamp(1, 32);
    {
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.chunk_count = clamped;
            t.auto_connections = auto_connections;
            let _ = app.emit("download-progress", t.clone());
        }
    }
    {
        let mut mtasks = download_state.tasks.lock().unwrap();
        if let Some(t) = mtasks.get_mut(&task_id) {
            t.chunk_count = clamped;
            t.auto_connections = auto_connections;
            let _ = app.emit("download-progress", t.clone());
        }
        let mut mopts = download_state.task_options.lock().unwrap();
        if let Some(o) = mopts.get_mut(&task_id) {
            o.aria2_connections = (clamped as u32).clamp(1, 16);
        }
    }
    save_all_tasks(&app).await;
    Ok(())
}

#[command]
async fn set_task_speed_limit(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    task_id: String,
    speed_limit_kb: u64,
) -> Result<(), String> {
    state.set_task_speed_limit(&task_id, speed_limit_kb).await;
    {
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.speed_limit_kb = speed_limit_kb;
            let _ = app.emit("download-progress", t.clone());
        }
    }
    {
        let mut mtasks = download_state.tasks.lock().unwrap();
        if let Some(t) = mtasks.get_mut(&task_id) {
            t.speed_limit_kb = speed_limit_kb;
            let _ = app.emit("download-progress", t.clone());
        }
        let mut mopts = download_state.task_options.lock().unwrap();
        if let Some(o) = mopts.get_mut(&task_id) {
            o.limit_speed = speed_limit_kb > 0;
            o.speed_limit_kb = speed_limit_kb as u32;
        }
    }
    save_all_tasks(&app).await;
    Ok(())
}

#[command]
async fn check_duplicate_download(
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    settings_state: State<'_, SettingsState>,
    url: String,
    filename: String,
    save_folder: Option<String>,
    expected_size: Option<u64>,
) -> Result<DuplicateCheckResult, String> {
    let norm_url = url.trim().to_string();
    let app_settings = settings_state.get().await;
    let folder = save_folder
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            if !app_settings.default_download_dir.trim().is_empty() {
                app_settings.default_download_dir.clone()
            } else {
                dirs_or_default_downloads()
            }
        });
    let suggested = build_unique_filename(&folder, &filename);

    {
        let tasks = state.tasks.read().await;
        for t in tasks.values() {
            let same_url = !norm_url.is_empty() && t.url.trim() == norm_url;
            let same_file_size = !filename.is_empty()
                && t.filename.eq_ignore_ascii_case(filename.trim())
                && expected_size.map(|sz| sz > 0 && sz == t.total_bytes).unwrap_or(false);
            if same_url || same_file_size {
                return Ok(DuplicateCheckResult {
                    is_duplicate: true,
                    reason: if same_url {
                        "This URL is already in your download list.".to_string()
                    } else {
                        "A file with the same name and size already exists in the list.".to_string()
                    },
                    existing_task_id: Some(t.id.clone()),
                    existing_path: Some(t.save_path.clone()),
                    suggested_filename: suggested,
                });
            }
        }
    }

    {
        let mtasks = download_state.tasks.lock().unwrap();
        for t in mtasks.values() {
            let same_url = !norm_url.is_empty() && t.url.trim() == norm_url;
            let same_file_size = !filename.is_empty()
                && t.filename.eq_ignore_ascii_case(filename.trim())
                && expected_size.map(|sz| sz > 0 && sz == t.total_bytes).unwrap_or(false);
            if same_url || same_file_size {
                return Ok(DuplicateCheckResult {
                    is_duplicate: true,
                    reason: if same_url {
                        "This URL is already in your download list.".to_string()
                    } else {
                        "A file with the same name and size already exists in the list.".to_string()
                    },
                    existing_task_id: Some(t.id.clone()),
                    existing_path: Some(t.save_path.clone()),
                    suggested_filename: suggested,
                });
            }
        }
    }

    if !filename.trim().is_empty() {
        let candidate_path = Path::new(&folder).join(filename.trim());
        if candidate_path.exists() {
            let existing_sz = std::fs::metadata(&candidate_path).map(|m| m.len()).unwrap_or(0);
            let matches_size = expected_size.map(|sz| sz > 0 && sz == existing_sz).unwrap_or(true);
            if matches_size {
                return Ok(DuplicateCheckResult {
                    is_duplicate: true,
                    reason: format!(
                        "'{}' already exists in the target folder.",
                        filename.trim()
                    ),
                    existing_task_id: None,
                    existing_path: Some(candidate_path.to_string_lossy().to_string()),
                    suggested_filename: suggested,
                });
            }
        }
    }

    Ok(DuplicateCheckResult {
        is_duplicate: false,
        reason: String::new(),
        existing_task_id: None,
        existing_path: None,
        suggested_filename: suggested,
    })
}

#[command]
async fn export_download_list(
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    file_path: String,
) -> Result<usize, String> {
    let mut merged: HashMap<String, PersistedTask> = HashMap::new();
    {
        let http_tasks = state.tasks.read().await;
        for (id, t) in http_tasks.iter() {
            merged.insert(
                id.clone(),
                PersistedTask {
                    task: t.clone(),
                    options: None,
                    is_media: false,
                },
            );
        }
    }
    {
        let mtasks = download_state.tasks.lock().unwrap();
        let mopts = download_state.task_options.lock().unwrap();
        for (id, t) in mtasks.iter() {
            merged.insert(
                id.clone(),
                PersistedTask {
                    task: t.clone(),
                    options: mopts.get(id).cloned(),
                    is_media: true,
                },
            );
        }
    }
    let mut entries: Vec<PersistedTask> = merged.into_values().collect();
    entries.sort_by(|a, b| a.task.queue_order.cmp(&b.task.queue_order));
    let count = entries.len();
    let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, json).map_err(|e| e.to_string())?;
    Ok(count)
}

#[command]
async fn import_download_list(
    app: AppHandle,
    state: State<'_, EngineState>,
    download_state: State<'_, DownloadState>,
    file_path: String,
) -> Result<usize, String> {
    let raw = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
    let mut imported_count = 0usize;
    if let Ok(entries) = serde_json::from_str::<Vec<PersistedTask>>(&raw) {
        for mut entry in entries {
            if matches!(
                entry.task.status.as_str(),
                "downloading" | "connecting" | "queued" | "assembling"
            ) {
                entry.task.status = "paused".to_string();
                entry.task.speed_bps = 0;
                entry.task.speed_formatted = "0 B/s".to_string();
                entry.task.eta_formatted = "--".to_string();
            }
            if entry.is_media || entry.options.is_some() {
                download_state
                    .tasks
                    .lock()
                    .unwrap()
                    .insert(entry.task.id.clone(), entry.task.clone());
                if let Some(opts) = entry.options {
                    download_state
                        .task_options
                        .lock()
                        .unwrap()
                        .insert(entry.task.id.clone(), opts);
                }
            } else {
                state
                    .tasks
                    .write()
                    .await
                    .insert(entry.task.id.clone(), entry.task.clone());
            }
            imported_count += 1;
        }
    } else if let Ok(tasks_only) = serde_json::from_str::<Vec<DownloadTask>>(&raw) {
        let mut map = state.tasks.write().await;
        for mut t in tasks_only {
            if matches!(
                t.status.as_str(),
                "downloading" | "connecting" | "queued" | "assembling"
            ) {
                t.status = "paused".to_string();
            }
            map.insert(t.id.clone(), t);
            imported_count += 1;
        }
    } else {
        return Err("Invalid download list JSON file".to_string());
    }
    save_all_tasks(&app).await;
    Ok(imported_count)
}

#[command]
async fn read_text_file_content(file_path: String) -> Result<String, String> {
    std::fs::read_to_string(&file_path).map_err(|e| e.to_string())
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

fn extract_header_value(req: &str, header_name: &str) -> Option<String> {
    for line in req.lines() {
        if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case(header_name) {
                return Some(val.trim().to_string());
            }
        }
    }
    None
}

fn extract_bridge_token(req: &str) -> Option<String> {
    // 1. Check X-ZR-Token header
    if let Some(token) = extract_header_value(req, "x-zr-token") {
        let t = token.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    // 2. Check Authorization: Bearer <token>
    if let Some(auth) = extract_header_value(req, "authorization") {
        if let Some(token) = auth.strip_prefix("Bearer ").or_else(|| auth.strip_prefix("bearer ")) {
            let t = token.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    // 3. Check query string token=... in the first line
    let first_line = req.lines().next().unwrap_or("");
    if let Some(pos) = first_line.find('?') {
        let end_idx = first_line.find(" HTTP/").unwrap_or(first_line.len());
        let query = &first_line[pos + 1..end_idx];
        for part in query.split('&') {
            if let Some(tok) = part.strip_prefix("token=") {
                let decoded = decode_percent_encoded(tok);
                let t = decoded.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    // 4. Check JSON body if present
    if let Some(body_start) = req.find("\r\n\r\n") {
        let body = &req[body_start + 4..];
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(tok) = v.get("token").and_then(|t| t.as_str()) {
                let t = tok.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

fn load_existing_bridge_auth() -> (u16, String) {
    let mut path = None;
    if let Ok(appdata) = env::var("APPDATA") {
        let p = PathBuf::from(appdata).join("com.golammowla.zrdownloader").join("settings.json");
        if p.exists() {
            path = Some(p);
        }
    }
    if path.is_none() {
        let p = env::temp_dir().join("zr-downloader").join("settings.json");
        if p.exists() {
            path = Some(p);
        }
    }
    if let Some(p) = path {
        if let Ok(content) = std::fs::read_to_string(p) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let port = val.get("bridgePort").and_then(|v| v.as_u64()).map(|u| u as u16).unwrap_or(9666);
                let token = val.get("bridgePairToken").and_then(|v| v.as_str()).unwrap_or("").to_string();
                return (if port == 0 { 9666 } else { port }, token);
            }
        }
    }
    (9666, String::new())
}

async fn start_local_bridge(app_handle: AppHandle) {
    let settings_state = app_handle.state::<SettingsState>();
    let (configured_port, auto_port) = {
        let s = settings_state.get_blocking();
        (s.bridge_port, s.bridge_auto_port)
    };

    let base_port = if configured_port == 0 { 9666 } else { configured_port };
    let mut listener = None;
    let mut actual_port = base_port;

    // 1. Try binding base_port with short retries in case previous process is shutting down
    for _ in 0..5 {
        match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", base_port)).await {
            Ok(l) => {
                actual_port = base_port;
                listener = Some(l);
                break;
            }
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
        }
    }

    // 2. If configured port is busy and auto_port is enabled, fall back to free port
    if listener.is_none() && auto_port {
        for p in (base_port + 1)..=(base_port + 20) {
            if let Ok(l) = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", p)).await {
                actual_port = p;
                listener = Some(l);
                break;
            }
        }
        if listener.is_none() {
            if let Ok(l) = tokio::net::TcpListener::bind("127.0.0.1:0").await {
                if let Ok(addr) = l.local_addr() {
                    actual_port = addr.port();
                }
                listener = Some(l);
            }
        }
    }

    let listener = match listener {
        Some(l) => {
            ACTIVE_BRIDGE_PORT.store(actual_port, Ordering::Relaxed);
            eprintln!("ZR Downloader: Local bridge listening on 127.0.0.1:{}", actual_port);
            l
        }
        None => {
            eprintln!("ZR Downloader: Could not bind local bridge port {} (auto_port={})", base_port, auto_port);
            return;
        }
    };

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let handle = app_handle.clone();
            let port = actual_port;
            tauri::async_runtime::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 16384];
                let n = match stream.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };
                let req_str = String::from_utf8_lossy(&buf[..n]);

                // Origin allow-list: ONLY browser extensions (chrome-extension:// and moz-extension://)
                let origin = extract_header_value(&req_str, "origin");
                let is_allowed_ext = origin.as_deref().map_or(false, |o| {
                    let ol = o.to_ascii_lowercase();
                    ol.starts_with("chrome-extension://") || ol.starts_with("moz-extension://")
                });

                let first_line = req_str.lines().next().unwrap_or("").to_string();

                // Handle OPTIONS preflight
                if first_line.starts_with("OPTIONS") {
                    if is_allowed_ext {
                        let resp = format!(
                            "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: X-ZR-Token, Content-Type, Authorization\r\nAccess-Control-Max-Age: 86400\r\nConnection: close\r\n\r\n",
                            origin.as_deref().unwrap_or("")
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                    } else {
                        let resp = "HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes()).await;
                    }
                    return;
                }

                // Prepare CORS header: Only extensions receive Access-Control-Allow-Origin;
                // normal web origins receive NO CORS headers at all.
                let cors_header = if is_allowed_ext {
                    format!(
                        "Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Headers: X-ZR-Token, Content-Type, Authorization\r\nVary: Origin\r\n",
                        origin.as_deref().unwrap_or("")
                    )
                } else {
                    String::new()
                };

                // Health check GET /ping is the ONLY unauthenticated endpoint
                if first_line.starts_with("GET /ping") || (req_str.contains("/ping") && !first_line.contains("/open?")) {
                    let body = serde_json::json!({
                        "status": "alive",
                        "app": "ZR Downloader",
                        "version": env!("CARGO_PKG_VERSION"),
                        "port": port,
                    }).to_string();
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\n{}Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        cors_header,
                        body.as_bytes().len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    return;
                }

                // Authentication verification: every other request must carry the bridge pair token
                let settings_state = handle.state::<SettingsState>();
                let expected_token = settings_state.get_blocking().bridge_pair_token;
                let provided_token = extract_bridge_token(&req_str);

                let is_authenticated = !expected_token.is_empty()
                    && provided_token.as_deref() == Some(&expected_token);

                if !is_authenticated {
                    let body = "{\"error\":\"unauthorized\",\"message\":\"Valid X-ZR-Token required\"}";
                    let resp = format!(
                        "HTTP/1.1 401 Unauthorized\r\n{}Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        cors_header,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    return;
                }

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
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(60),
                            media::fetch_media_info_with_app(
                                Some(handle.clone()),
                                target.clone(),
                                None,
                                None,
                                None,
                                None,
                            ),
                        )
                        .await
                        {
                            Ok(Ok(info)) => {
                                let body = serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string());
                                ("200 OK", body)
                            }
                            Ok(Err(e)) => ("502 Bad Gateway", serde_json::json!({ "error": e }).to_string()),
                            Err(_) => ("504 Gateway Timeout", "{\"error\":\"timeout\"}".to_string()),
                        }
                    };
                    let resp = format!(
                        "HTTP/1.1 {}\r\n{}Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status_line,
                        cors_header,
                        body.as_bytes().len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    return;
                }

                if first_line.starts_with("GET /focus") {
                    if let Some(win) = handle.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.unminimize();
                        let _ = win.set_focus();
                    }
                    let body = "{\"status\":\"focused\",\"app\":\"ZR Downloader\"}";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\n{}Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        cors_header,
                        body.len(),
                        body
                    );
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
                        "HTTP/1.1 200 OK\r\n{}Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        cors_header,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                } else {
                    let body = "{\"error\":\"missing_url\"}";
                    let resp = format!(
                        "HTTP/1.1 400 Bad Request\r\n{}Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        cors_header,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
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

pub fn append_failure_log(
    app: &AppHandle,
    task_id: &str,
    filename: &str,
    url: &str,
    summary: &str,
    details: &[String],
) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("zr_error_logs.log");
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > 262_144 {
                if let Ok(existing) = std::fs::read_to_string(&path) {
                    let start = existing.len() / 2;
                    let slice = existing[start..]
                        .find('\n')
                        .map(|i| &existing[start + i + 1..])
                        .unwrap_or("");
                    let _ = std::fs::write(&path, slice);
                }
            }
        }
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let mut entry = format!(
            "[{}] [{}] {} ({})\nReason: {}\n",
            ts, task_id, filename, url, summary
        );
        for line in details.iter().take(30) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                entry.push_str(&format!("  | {}\n", trimmed));
            }
        }
        entry.push_str("---\n");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            use std::io::Write;
            let _ = f.write_all(entry.as_bytes());
        }
    }
}

#[command]
async fn get_error_logs(app: AppHandle) -> Result<String, String> {
    if let Ok(dir) = app.path().app_data_dir() {
        let path = dir.join("zr_error_logs.log");
        if path.exists() {
            return tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to read error log: {}", e));
        }
    }
    Ok(String::new())
}

#[command]
async fn clear_error_logs(app: AppHandle) -> Result<(), String> {
    if let Ok(dir) = app.path().app_data_dir() {
        let path = dir.join("zr_error_logs.log");
        if path.exists() {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
    Ok(())
}

#[command]
async fn ytdlp_get_info(app: AppHandle, url: String) -> Result<media::MediaInfo, String> {
    media::fetch_media_info_with_app(Some(app), url, None, None, None, None).await
}

#[command]
async fn read_clipboard_link() -> Result<Option<String>, String> {
    let txt_opt = tokio::task::spawn_blocking(|| {
        arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
    })
    .await
    .ok()
    .flatten();

    if let Some(txt) = txt_opt {
        let trimmed = txt.trim();
        let low = trimmed.to_lowercase();
        let valid_scheme =
            low.starts_with("http://") || low.starts_with("https://") || low.starts_with("magnet:?");
        if valid_scheme && trimmed.len() >= 10 && !trimmed.chars().any(|c| c.is_whitespace()) {
            return Ok(Some(trimmed.to_string()));
        }
    }
    Ok(None)
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
async fn show_completion_notice(app: AppHandle, name: String, folder: String) -> Result<(), String> {
    static NOTICE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = NOTICE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let label = format!("ntf_{}_{}", stamp, seq);
    let url_str = format!(
        "index.html?notify=true&name={}&folder={}",
        url_encode(&name),
        url_encode(&folder)
    );
    let (w, h) = (380.0_f64, 112.0_f64);
    let mut builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url_str.into()))
        .title("Download complete")
        .inner_size(w, h)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .shadow(true);
    if let Ok(Some(mon)) = app.primary_monitor() {
        let scale = mon.scale_factor();
        let size = mon.size();
        let pos = mon.position();
        let x = (pos.x as f64 + size.width as f64) / scale - w - 16.0;
        let y = (pos.y as f64 + size.height as f64) / scale - h - 64.0;
        builder = builder.position(x.max(0.0), y.max(0.0));
    }
    builder
        .build()
        .map_err(|e| format!("Notice window error: {}", e))?;
    Ok(())
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
                        ..Default::default()
                    };
                    tasks_lock.insert(task_id.clone(), t);
                    spawn_save_all_tasks(&app);
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
                    ..Default::default()
                };
                engine_tasks.insert(task_id.clone(), t);
                drop(engine_tasks);
                spawn_save_all_tasks(&app);
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

    let (ipc_port, ipc_token) = load_existing_bridge_auth();
    if let Some(ref req_path) = pending_arg_req {
        if let Ok(addr) = format!("127.0.0.1:{}", ipc_port).parse::<std::net::SocketAddr>() {
            if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
                &addr,
                std::time::Duration::from_millis(350),
            ) {
                use std::io::Write;
                let auth_hdr = if !ipc_token.is_empty() {
                    format!("X-ZR-Token: {}\r\n", ipc_token)
                } else {
                    String::new()
                };
                let req = format!(
                    "GET /{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{}Connection: close\r\n\r\n",
                    req_path, ipc_port, auth_hdr
                );
                let _ = stream.write_all(req.as_bytes());
                std::process::exit(0);
            }
        }
    }

    // Plain launch while ZR Downloader is already running (e.g. hidden in the tray):
    // ask the running one to show its window, then exit, so no second tray icon appears.
    if pending_arg_req.is_none() {
        if let Ok(addr) = format!("127.0.0.1:{}", ipc_port).parse::<std::net::SocketAddr>() {
            if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
                &addr,
                std::time::Duration::from_millis(350),
            ) {
                use std::io::{Read, Write};
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(800)));
                let auth_hdr = if !ipc_token.is_empty() {
                    format!("X-ZR-Token: {}\r\n", ipc_token)
                } else {
                    String::new()
                };
                let req = format!(
                    "GET /focus HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{}Connection: close\r\n\r\n",
                    ipc_port, auth_hdr
                );
                let _ = stream.write_all(req.as_bytes());
                let mut buf = [0u8; 512];
                if let Ok(n) = stream.read(&mut buf) {
                    if String::from_utf8_lossy(&buf[..n]).contains("ZR Downloader") {
                        std::process::exit(0);
                    }
                }
            }
        }
    }

    tauri::Builder::default()
        .manage(DownloadState {
            active_pids: Arc::new(Mutex::new(HashMap::new())),
            cancelled_tasks: Arc::new(Mutex::new(HashMap::new())),
            task_options: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            running_workers: Arc::new(Mutex::new(std::collections::HashSet::new())),
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
                save_all_tasks_sync(window.app_handle());
                // Main window: the X button always hides to the system tray (downloads keep running;
                // use the tray menu "Exit ZR Downloader" to quit). Minimize is a normal taskbar
                // minimize and never goes to the tray. Popup windows (dl_*, ntf_*) close normally.
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            // Initialize SettingsState & SchedulerState from app_data_dir
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("zr-downloader"));
            let settings_state =
                SettingsState::load_or_default(Some(app_data_dir.join("settings.json")));
            let initial_settings = settings_state.get_blocking();
            let scheduler_state = SchedulerState::load_or_default(app_data_dir);

            {
                let engine_state = app.state::<EngineState>();
                let es = engine_state.inner().clone();
                let init_s = initial_settings.clone();
                tauri::async_runtime::block_on(async move {
                    es.apply_settings(&init_s).await;
                });
            }

            app.manage(settings_state);
            app.manage(scheduler_state);

            // Load persisted download tasks from tasks.json BEFORE the UI loads
            {
                let mut persisted = read_persisted_tasks_file(app.handle());
                // Migrate legacy queue_order == 0 so drag reordering & priority queue stay consistent
                let needs_order_seed = persisted.iter().all(|p| p.task.queue_order == 0);
                if needs_order_seed {
                    persisted.sort_by(|a, b| b.task.created_at.cmp(&a.task.created_at));
                    for (idx, p) in persisted.iter_mut().enumerate() {
                        p.task.queue_order = (idx as i64) + 1;
                    }
                }

                let engine_state = app.state::<EngineState>();
                let download_state = app.state::<DownloadState>();
                let mut http_to_insert = Vec::new();
                for entry in &mut persisted {
                    if matches!(
                        entry.task.status.as_str(),
                        "downloading" | "connecting" | "queued" | "assembling"
                    ) {
                        entry.task.status = "paused".to_string();
                        entry.task.speed_bps = 0;
                        entry.task.speed_formatted = "0 B/s".to_string();
                        entry.task.eta_formatted = "--".to_string();
                    }
                    if entry.is_media || entry.options.is_some() {
                        download_state
                            .tasks
                            .lock()
                            .unwrap()
                            .insert(entry.task.id.clone(), entry.task.clone());
                        if let Some(ref opts) = entry.options {
                            download_state
                                .task_options
                                .lock()
                                .unwrap()
                                .insert(entry.task.id.clone(), opts.clone());
                        }
                    } else {
                        http_to_insert.push(entry.task.clone());
                    }
                }
                if !http_to_insert.is_empty() {
                    let es = engine_state.inner().clone();
                    tauri::async_runtime::block_on(async move {
                        let mut map = es.tasks.write().await;
                        for mut t in http_to_insert {
                            downloader::hydrate_task_from_sparse_meta(&mut t).await;
                            map.insert(t.id.clone(), t);
                        }
                    });
                }
                if let Ok(initial_json) = serde_json::to_string_pretty(&persisted) {
                    let cache = LAST_SAVED_TASKS_JSON.get_or_init(|| Mutex::new(String::new()));
                    if let Ok(mut prev) = cache.lock() {
                        *prev = initial_json;
                    }
                }
            }

            // Periodic 2-second persistence loop (only writes if serialized JSON changed)
            let save_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
                loop {
                    interval.tick().await;
                    save_all_tasks(&save_handle).await;
                }
            });

            // Start Scheduler Loop (window schedule + when finished actions)
            scheduler::start_scheduler_loop(app.handle().clone());

            // Background tool bootstrap (ensures Deno runtime is ready on fresh Windows 10/11 machines)
            media::bootstrap_deno_in_background(app.handle().clone());

            // Start Clipboard Monitor Loop
            let clip_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut last_seen = String::new();
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(1200));
                loop {
                    interval.tick().await;
                    let enabled = clip_handle
                        .try_state::<SettingsState>()
                        .map(|s| s.get_blocking().clipboard_monitor)
                        .unwrap_or(false);
                    if !enabled {
                        continue;
                    }
                    let txt_opt = tokio::task::spawn_blocking(|| {
                        arboard::Clipboard::new()
                            .ok()
                            .and_then(|mut cb| cb.get_text().ok())
                    })
                    .await
                    .ok()
                    .flatten();
                    if let Some(raw_text) = txt_opt {
                        let trimmed = raw_text.trim().to_string();
                        if !trimmed.is_empty()
                            && trimmed != last_seen
                            && !trimmed.contains(char::is_whitespace)
                            && (trimmed.starts_with("http://")
                                || trimmed.starts_with("https://")
                                || trimmed.starts_with("magnet:?"))
                        {
                            last_seen = trimmed.clone();
                            let _ = clip_handle.emit(
                                "clipboard-link-detected",
                                serde_json::json!({ "url": trimmed }),
                            );
                        }
                    }
                }
            });

            #[cfg(target_os = "windows")]
            register_windows_protocols();

            // Setup System Tray
            if let (Ok(show_i), Ok(pause_all_i), Ok(resume_all_i), Ok(quit_i)) = (
                MenuItem::with_id(app, "show", "Open ZR Downloader", true, None::<&str>),
                MenuItem::with_id(app, "pause_all", "Pause All Downloads", true, None::<&str>),
                MenuItem::with_id(app, "resume_all", "Resume All Downloads", true, None::<&str>),
                MenuItem::with_id(app, "quit", "Exit ZR Downloader", true, None::<&str>),
            ) {
                if let Ok(tray_menu) =
                    Menu::with_items(app, &[&show_i, &pause_all_i, &resume_all_i, &quit_i])
                {
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
                                    "pause_all" => {
                                        let app_c = app.clone();
                                        tauri::async_runtime::spawn(async move {
                                            queue::pause_all_downloads(&app_c).await;
                                        });
                                    }
                                    "resume_all" => {
                                        let app_c = app.clone();
                                        tauri::async_runtime::spawn(async move {
                                            queue::resume_scheduled_queue(&app_c).await;
                                        });
                                    }
                                    "quit" => {
                                        save_all_tasks_sync(app);
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

            let handle_rpc = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                aria2_rpc::start_aria2_rpc_server(handle_rpc).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Settings, Scheduler, Queue & Utility Commands
            get_app_settings,
            save_app_settings,
            get_active_bridge_port,
            regenerate_bridge_token,
            get_active_aria2_rpc_status,
            regenerate_aria2_token,
            toggle_global_speed_limit,
            get_scheduler_config,
            save_scheduler_config,
            cancel_scheduled_shutdown,
            pause_all_downloads_cmd,
            resume_all_downloads_cmd,
            set_task_priority,
            reorder_tasks,
            update_task_connections,
            set_task_speed_limit,
            check_duplicate_download,
            export_download_list,
            import_download_list,
            read_text_file_content,
            read_clipboard_link,
            get_error_logs,
            clear_error_logs,
            // HTTP Downloader Commands
            probe_http_url,
            start_http_download,
            pause_http_download,
            resume_http_download,
            relink_http_download,
            cancel_http_download,
            remove_http_download,
            get_all_downloads,
            get_default_download_dir,
            // Media Downloader Commands
            media::fetch_media_info,
            media::start_download,
            media::resume_media_download,
            media::cancel_download,
            media::stop_and_save_live_recording,
            media::get_installed_tools,
            media::check_tool_updates,
            media::update_tool,
            media::open_folder,
            media::open_file,
            media::check_app_update,
            media::open_release_page,
            media::install_app_update,
            get_pending_url,
            ytdlp_get_info,
            ytdlp_download,
            open_download_window,
            show_completion_notice,
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
        .build(tauri::generate_context!())
        .expect("error while building ZR Downloader")
        .run(|app_handle, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                save_all_tasks_sync(app_handle);
            }
        });
}

#[cfg(test)]
mod bridge_tests {
    use super::{extract_bridge_token, extract_header_value};

    #[test]
    fn test_extract_bridge_token_header_and_query() {
        let req1 = "GET /open?url=https%3A%2F%2Fexample.com HTTP/1.1\r\nHost: 127.0.0.1:9666\r\nX-ZR-Token: secret123\r\n\r\n";
        assert_eq!(extract_bridge_token(req1).as_deref(), Some("secret123"));

        let req2 = "GET /open?url=https%3A%2F%2Fexample.com&token=querytoken456 HTTP/1.1\r\nHost: 127.0.0.1:9666\r\n\r\n";
        assert_eq!(extract_bridge_token(req2).as_deref(), Some("querytoken456"));

        let req3 = "GET /formats?url=https%3A%2F%2Fexample.com HTTP/1.1\r\nHost: 127.0.0.1:9666\r\nAuthorization: Bearer bearertoken789\r\n\r\n";
        assert_eq!(extract_bridge_token(req3).as_deref(), Some("bearertoken789"));

        let req4 = "GET /open?url=https%3A%2F%2Fexample.com HTTP/1.1\r\nHost: 127.0.0.1:9666\r\n\r\n";
        assert_eq!(extract_bridge_token(req4), None);
    }

    #[test]
    fn test_origin_allowlist_validation() {
        let req_header = "Host: 127.0.0.1:9666\r\nOrigin: chrome-extension://abcdefghijklmnop\r\n";
        assert_eq!(
            extract_header_value(req_header, "origin").as_deref(),
            Some("chrome-extension://abcdefghijklmnop")
        );

        let chrome_origin = "chrome-extension://abcdefghijklmnop";
        let moz_origin = "moz-extension://12345678-1234-1234-1234-123456789abc";
        let web_origin = "https://malicious-website.com";

        let is_allowed = |o: &str| {
            let ol = o.to_ascii_lowercase();
            ol.starts_with("chrome-extension://") || ol.starts_with("moz-extension://")
        };

        assert!(is_allowed(chrome_origin));
        assert!(is_allowed(moz_origin));
        assert!(!is_allowed(web_origin));
        assert!(!is_allowed("http://localhost:3000"));
    }
}
