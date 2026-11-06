use reqwest::header::{ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, RANGE, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkProgress {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlMetadata {
    pub url: String,
    pub filename: String,
    pub total_size: Option<u64>,
    pub supports_ranges: bool,
    pub content_type: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub percent: f64,
    pub speed_bps: u64,
    pub speed_formatted: String,
    pub eta_formatted: String,
    pub status: String, // "queued" | "downloading" | "paused" | "assembling" | "completed" | "error" | "cancelled"
    pub category: String, // "all" | "video" | "audio" | "docs" | "compressed" | "programs"
    pub chunk_count: usize,
    pub is_resumable: bool,
    #[serde(default)]
    pub low_disk_space: bool,
    pub chunks: Vec<ChunkProgress>,
    pub speed_history: Vec<u64>,
    pub error_message: Option<String>,
    pub created_at: String,
}

pub struct ActiveDownloadControl {
    pub cancel_sender: broadcast::Sender<()>,
    pub is_paused: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct EngineState {
    pub tasks: Arc<RwLock<HashMap<String, DownloadTask>>>,
    pub controls: Arc<RwLock<HashMap<String, ActiveDownloadControl>>>,
    pub client: reqwest::Client,
}

impl EngineState {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(64)
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            controls: Arc::new(RwLock::new(HashMap::new())),
            client,
        }
    }
}

pub fn extension_from_mime(mime: &str) -> Option<&'static str> {
    let clean = mime.split(';').next().unwrap_or("").trim().to_lowercase();
    match clean.as_str() {
        "video/mp4" => Some("mp4"),
        "video/x-matroska" | "video/mkv" => Some("mkv"),
        "video/webm" => Some("webm"),
        "video/quicktime" => Some("mov"),
        "video/x-msvideo" => Some("avi"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/ogg" | "application/ogg" => Some("ogg"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "application/zip" => Some("zip"),
        "application/x-rar-compressed" | "application/vnd.rar" => Some("rar"),
        "application/x-7z-compressed" => Some("7z"),
        "application/x-tar" => Some("tar"),
        "application/gzip" => Some("tar.gz"),
        "application/pdf" => Some("pdf"),
        "application/x-msdownload" | "application/vnd.microsoft.portable-executable" => Some("exe"),
        "application/x-iso9660-image" => Some("iso"),
        _ => None,
    }
}

pub fn detect_category_with_mime(filename: &str, content_type: Option<&str>) -> String {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !ext.is_empty() {
        match ext.as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" => return "video".to_string(),
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "opus" | "wma" => return "audio".to_string(),
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "epub" | "csv" => return "docs".to_string(),
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" => return "compressed".to_string(),
            "exe" | "msi" | "apk" | "dmg" | "deb" | "rpm" | "appimage" => return "programs".to_string(),
            _ => {}
        }
    }

    if let Some(ct) = content_type {
        let ct_lower = ct.to_lowercase();
        if ct_lower.starts_with("video/") {
            return "video".to_string();
        } else if ct_lower.starts_with("audio/") {
            return "audio".to_string();
        } else if ct_lower.contains("zip") || ct_lower.contains("compressed") || ct_lower.contains("tar") || ct_lower.contains("archive") {
            return "compressed".to_string();
        } else if ct_lower.contains("pdf") || ct_lower.contains("text") || ct_lower.contains("document") {
            return "docs".to_string();
        } else if ct_lower.contains("executable") || ct_lower.contains("x-msdownload") {
            return "programs".to_string();
        }
    }

    "docs".to_string()
}

pub fn detect_category(filename: &str) -> String {
    detect_category_with_mime(filename, None)
}

pub fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec >= 1024 * 1024 * 1024 {
        format!("{:.2} GB/s", bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024 * 1024 {
        format!("{:.2} MB/s", bytes_per_sec as f64 / (1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024 {
        format!("{:.1} KB/s", bytes_per_sec as f64 / 1024.0)
    } else {
        format!("{} B/s", bytes_per_sec)
    }
}

pub fn format_eta(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

pub async fn probe_url(client: &reqwest::Client, url: &str) -> Result<UrlMetadata, String> {
    const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    let mut referer = String::new();
    if url.contains("safefilehosting.com") {
        referer = "https://igetintopc.com/".to_string();
    } else if let Ok(parsed) = reqwest::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            referer = format!("{}://{}/", parsed.scheme(), host);
        }
    }

    // Fast probe: send Range: bytes=0-0 first (IDM method).
    // In one single roundtrip (~50-100ms), 206 Partial Content returns Content-Range (exact total size),
    // Content-Disposition (filename), and verifies Range support simultaneously without downloading the file.
    let mut range_req = client.get(url)
        .header(USER_AGENT, BROWSER_UA)
        .header(RANGE, "bytes=0-0")
        .header("Accept", "*/*")
        .header("Connection", "keep-alive")
        .timeout(Duration::from_secs(6));
    if !referer.is_empty() {
        range_req = range_req.header("Referer", &referer);
    }

    let resp = match range_req.send().await {
        Ok(r) if r.status().is_success() || r.status() == reqwest::StatusCode::PARTIAL_CONTENT => r,
        _ => {
            // Fallback to fast HEAD request
            let mut head_req = client.head(url)
                .header(USER_AGENT, BROWSER_UA)
                .header("Accept", "*/*")
                .timeout(Duration::from_secs(4));
            if !referer.is_empty() {
                head_req = head_req.header("Referer", &referer);
            }

            match head_req.send().await {
                Ok(r) if r.status().is_success() => r,
                _ => {
                    // Final fallback to plain GET
                    let mut get_req = client.get(url)
                        .header(USER_AGENT, BROWSER_UA)
                        .header("Accept", "*/*")
                        .timeout(Duration::from_secs(6));
                    if !referer.is_empty() {
                        get_req = get_req.header("Referer", &referer);
                    }
                    get_req
                        .send()
                        .await
                        .map_err(|e| format!("Failed to probe URL: {}", e))?
                }
            }
        }
    };

    let headers = resp.headers();

    // Check Accept-Ranges or 206 Partial Content
    let supports_ranges = if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        true
    } else if let Some(val) = headers.get(ACCEPT_RANGES) {
        val.to_str().unwrap_or("").to_lowercase().contains("bytes")
    } else {
        false
    };

    // Total content length
    let mut total_size = None;
    if let Some(cl) = headers.get(CONTENT_LENGTH) {
        if let Ok(str_val) = cl.to_str() {
            if let Ok(size) = str_val.parse::<u64>() {
                total_size = Some(size);
            }
        }
    }

    // Try Content-Range if Content-Length was 1 (from bytes=0-0 probe)
    if total_size == Some(1) || total_size.is_none() {
        if let Some(cr) = headers.get("content-range") {
            if let Ok(cr_str) = cr.to_str() {
                // e.g., bytes 0-0/1234567
                if let Some(pos) = cr_str.rfind('/') {
                    if let Ok(size) = cr_str[pos + 1..].trim().parse::<u64>() {
                        total_size = Some(size);
                    }
                }
            }
        }
    }

    let content_type = headers.get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Detect filename from Content-Disposition, URL Query Params, or URL path
    let mut filename = String::new();

    if let Some(cd) = headers.get(CONTENT_DISPOSITION) {
        if let Ok(cd_str) = cd.to_str() {
            if let Some(idx) = cd_str.find("filename*=") {
                let sub = &cd_str[idx + 10..];
                let end = sub.find(';').unwrap_or(sub.len());
                let raw = sub[..end].trim().trim_matches('"');
                let enc = if let Some(p) = raw.find("''") { &raw[p + 2..] } else { raw };
                filename = crate::decode_percent_encoded(enc);
            } else if let Some(idx) = cd_str.find("filename=") {
                let mut name = cd_str[idx + 9..].trim().trim_matches('"').to_string();
                if let Some(semi) = name.find(';') {
                    name = name[..semi].trim().trim_matches('"').to_string();
                }
                if !name.is_empty() {
                    filename = name;
                }
            }
        }
    }

    // Inspect URL Query parameters for filename
    if filename.is_empty() {
        if let Ok(parsed) = reqwest::Url::parse(url) {
            for (k, v) in parsed.query_pairs() {
                if k == "filename" || k == "name" || k == "title" {
                    if !v.trim().is_empty() {
                        filename = v.trim().to_string();
                        break;
                    }
                } else if k == "response-content-disposition" {
                    if let Some(idx) = v.find("filename=") {
                        let sub = &v[idx + 9..];
                        let end = sub.find(';').unwrap_or(sub.len());
                        let name = sub[..end].trim().trim_matches('"').to_string();
                        if !name.is_empty() {
                            filename = name;
                            break;
                        }
                    }
                }
            }

            if filename.is_empty() {
                let path_segments: Vec<&str> = parsed.path_segments().map(|c| c.collect()).unwrap_or_default();
                if let Some(last) = path_segments.last() {
                    let clean_last = last.trim();
                    if !clean_last.is_empty() && clean_last != "/" {
                        filename = clean_last.to_string();
                    }
                }
            }
        }
    }

    if filename.is_empty() {
        filename = "download.bin".to_string();
    }

    // If filename has no extension (e.g. hash '06470d56ae4338382cd84ac7b6afc308'), deduce from Content-Type
    if !filename.contains('.') {
        if let Some(ref ct) = content_type {
            if let Some(ext) = extension_from_mime(ct) {
                filename = format!("{}.{}", filename, ext);
            }
        }
    }

    let category = detect_category_with_mime(&filename, content_type.as_deref());

    Ok(UrlMetadata {
        url: url.to_string(),
        filename,
        total_size,
        supports_ranges,
        content_type,
        category,
    })
}

pub async fn execute_multithreaded_download(
    app: AppHandle,
    state: EngineState,
    task_id: String,
) -> Result<(), String> {
    let (url, _filename, save_path, total_bytes, mut chunk_count, is_resumable, low_disk_space) = {
        let tasks = state.tasks.read().await;
        let task = tasks.get(&task_id).ok_or("Task not found")?;
        (
            task.url.clone(),
            task.filename.clone(),
            task.save_path.clone(),
            task.total_bytes,
            task.chunk_count,
            task.is_resumable,
            task.low_disk_space,
        )
    };

    // If total_bytes is 0 (unknown size) or ranges unsupported, force single stream
    if total_bytes == 0 || !is_resumable || chunk_count <= 1 {
        chunk_count = 1;
        // Update task's chunk_count so UI doesn't render 16 fake parallel streams
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.chunk_count = 1;
            let _ = app.emit("download-progress", t.clone());
            let _ = app.emit("zr://progress", t.clone());
        }
    }

    let target_dest = PathBuf::from(&save_path);
    let target_parent = target_dest
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir());
    let _ = fs::create_dir_all(&target_parent).await;

    let temp_dir = target_parent.join(format!(".zr_tmp_{}", task_id));
    let state_file_path = target_parent.join(format!(".zr_sparse_meta_{}.json", task_id));

    if !low_disk_space {
        fs::create_dir_all(&temp_dir).await.map_err(|e| e.to_string())?;
    } else {
        // Zero-Temp Low Disk Space Mode: Pre-allocate target destination file directly
        let dest_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&target_dest)
            .await
            .map_err(|e| e.to_string())?;
        if total_bytes > 0 {
            let _ = dest_file.set_len(total_bytes).await;
        }
        drop(dest_file);
    }

    // Create cancel broadcast channel
    let (cancel_tx, _cancel_rx) = broadcast::channel::<()>(4);
    let is_paused = Arc::new(AtomicBool::new(false));

    {
        let mut controls = state.controls.write().await;
        controls.insert(
            task_id.clone(),
            ActiveDownloadControl {
                cancel_sender: cancel_tx.clone(),
                is_paused: is_paused.clone(),
            },
        );
    }

    // Initialize chunks
    let chunk_size = if total_bytes > 0 && chunk_count > 1 {
        total_bytes / chunk_count as u64
    } else {
        0
    };

    let mut chunk_ranges: Vec<(usize, u64, u64)> = Vec::new();
    if total_bytes == 0 || chunk_count <= 1 {
        // Single unbounded stream or whole file (end = total_bytes or 0 for unknown)
        chunk_ranges.push((0, 0, if total_bytes > 0 { total_bytes.saturating_sub(1) } else { 0 }));
    } else {
        for i in 0..chunk_count {
            let start = i as u64 * chunk_size;
            let end = if i == chunk_count - 1 {
                total_bytes.saturating_sub(1)
            } else {
                (i as u64 + 1) * chunk_size - 1
            };
            chunk_ranges.push((i, start, end));
        }
    }

    let mut sparse_saved: HashMap<usize, u64> = HashMap::new();
    if low_disk_space && state_file_path.exists() {
        if let Ok(content) = fs::read_to_string(&state_file_path).await {
            if let Ok(map) = serde_json::from_str::<HashMap<usize, u64>>(&content) {
                sparse_saved = map;
            }
        }
    }

    let mut initial_downloaded = 0u64;
    let mut chunk_trackers: Vec<Arc<AtomicU64>> = Vec::new();
    for (index, _, _) in &chunk_ranges {
        let c_down = if low_disk_space {
            *sparse_saved.get(index).unwrap_or(&0u64)
        } else {
            let part_file_path = temp_dir.join(format!("chunk_{}.part", index));
            fs::metadata(&part_file_path).await.map(|m| m.len()).unwrap_or(0)
        };
        initial_downloaded += c_down;
        chunk_trackers.push(Arc::new(AtomicU64::new(c_down)));
    }

    let shared_downloaded = Arc::new(AtomicU64::new(initial_downloaded));
    let mut chunk_join_handles = Vec::new();

    let app_handle_for_chunks = app.clone();
    let state_for_chunks = state.clone();
    let task_id_for_chunks = task_id.clone();

    for (index, start, end) in chunk_ranges {
        let client = state.client.clone();
        let url = url.clone();
        let part_file_path = temp_dir.join(format!("chunk_{}.part", index));
        let dest_file_path = target_dest.clone();
        let mut chunk_cancel = cancel_tx.subscribe();
        let shared_downloaded = shared_downloaded.clone();
        let chunk_tracker = chunk_trackers[index].clone();
        let app_c = app_handle_for_chunks.clone();
        let state_c = state_for_chunks.clone();
        let tid_c = task_id_for_chunks.clone();

        let handle = tauri::async_runtime::spawn(async move {
            // Stagger parallel connections slightly to avoid server SYN flood / rate limiter kicks
            if index > 0 {
                tokio::time::sleep(Duration::from_millis((index as u64) * 25)).await;
            }

            let mut retries = 0;
            const MAX_RETRIES: usize = 12;

            loop {
                let current_len = if low_disk_space {
                    chunk_tracker.load(Ordering::Relaxed)
                } else {
                    let file = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .read(true)
                        .open(&part_file_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    file.metadata().await.map(|m| m.len()).unwrap_or(0)
                };

                // Note: if end is 0 (unknown size), target_chunk_len is 0 (read until EOF)
                let target_chunk_len = if end > 0 && end >= start { end - start + 1 } else { 0 };

                if target_chunk_len > 0 && current_len >= target_chunk_len {
                    return Ok::<(), String>(());
                }

                let chunk_start = start + current_len;
                if end > 0 && chunk_start > end {
                    return Ok::<(), String>(());
                }

                let mut file = if low_disk_space {
                    let mut f = OpenOptions::new()
                        .write(true)
                        .open(&dest_file_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    f.seek(SeekFrom::Start(chunk_start)).await.map_err(|e| e.to_string())?;
                    f
                } else {
                    let mut f = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .read(true)
                        .open(&part_file_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    f.seek(SeekFrom::End(0)).await.map_err(|e| e.to_string())?;
                    f
                };

                let mut req = client.get(&url)
                    .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                    .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
                    .header("Accept-Language", "en-US,en;q=0.9")
                    .header("Accept-Encoding", "identity")
                    .header("Connection", "keep-alive")
                    .header("Sec-Ch-Ua", "\"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\"")
                    .header("Sec-Ch-Ua-Mobile", "?0")
                    .header("Sec-Ch-Ua-Platform", "\"Windows\"")
                    .header("Sec-Fetch-Dest", "document")
                    .header("Sec-Fetch-Mode", "navigate")
                    .header("Sec-Fetch-Site", "cross-site")
                    .header("Sec-Fetch-User", "?1")
                    .header("Upgrade-Insecure-Requests", "1");

                // Parse origin domain for Referer header
                if url.contains("safefilehosting.com") {
                    req = req.header("Referer", "https://igetintopc.com/");
                } else if let Ok(parsed) = reqwest::Url::parse(&url) {
                    if let Some(host) = parsed.host_str() {
                        let proto = parsed.scheme();
                        req = req.header("Referer", format!("{}://{}/", proto, host));
                    }
                }

                if end > 0 {
                    req = req.header(RANGE, format!("bytes={}-{}", chunk_start, end));
                } else if chunk_start > 0 {
                    req = req.header(RANGE, format!("bytes={}-", chunk_start));
                }

                let send_res = req.send().await;
                let mut resp = match send_res {
                    Ok(r) => {
                        if r.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                            let _ = file.flush().await;
                            return Ok::<(), String>(());
                        }
                        if !r.status().is_success() && r.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                            if retries < MAX_RETRIES {
                                retries += 1;
                                tokio::time::sleep(Duration::from_millis(800 * retries as u64)).await;
                                continue;
                            }
                            return Err(format!("Chunk {} HTTP status {}", index, r.status()));
                        }
                        r
                    }
                    Err(e) => {
                        if retries < MAX_RETRIES {
                            retries += 1;
                            tokio::time::sleep(Duration::from_millis(800 * retries as u64)).await;
                            continue;
                        }
                        return Err(format!("Chunk {} connect failed: {}", index, e));
                    }
                };

                // Check for dynamic Content-Length if task total_bytes was unknown (0)
                if let Some(cl) = resp.content_length() {
                    if cl > 0 {
                        let mut tasks = state_c.tasks.write().await;
                        if let Some(t) = tasks.get_mut(&tid_c) {
                            if t.total_bytes == 0 {
                                t.total_bytes = cl;
                                if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT || resp.headers().contains_key(ACCEPT_RANGES) {
                                    t.is_resumable = true;
                                }
                                let _ = app_c.emit("download-progress", t.clone());
                                let _ = app_c.emit("zr://progress", t.clone());
                            }
                        }
                    }
                }

                let mut writer = tokio::io::BufWriter::with_capacity(512 * 1024, file);
                let mut stream_interrupted = false;

                loop {
                    tokio::select! {
                        _ = chunk_cancel.recv() => {
                            let _ = writer.flush().await;
                            return Err("Cancelled".to_string());
                        }
                        chunk_res = resp.chunk() => {
                            match chunk_res {
                                Ok(Some(bytes)) => {
                                    if let Err(e) = writer.write_all(&bytes).await {
                                        return Err(e.to_string());
                                    }
                                    let b_len = bytes.len() as u64;
                                    shared_downloaded.fetch_add(b_len, Ordering::Relaxed);
                                    chunk_tracker.fetch_add(b_len, Ordering::Relaxed);
                                    retries = 0;
                                }
                                Ok(None) => {
                                    break;
                                }
                                Err(e) => {
                                    eprintln!("Chunk {} stream interrupted: {}. Reconnecting...", index, e);
                                    stream_interrupted = true;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = writer.flush().await;

                if !stream_interrupted {
                    return Ok::<(), String>(());
                }

                if retries < MAX_RETRIES {
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(600 * retries as u64)).await;
                    continue;
                } else {
                    return Err(format!("Chunk {} connection dropped after {} retries", index, MAX_RETRIES));
                }
            }
        });

        chunk_join_handles.push(handle);
    }

    // Monitor progress loop
    let progress_task_id = task_id.clone();
    let monitor_app = app.clone();
    let monitor_state = state.clone();
    let monitor_downloaded = shared_downloaded.clone();
    let mut monitor_cancel = cancel_tx.subscribe();
    let monitor_trackers = chunk_trackers.clone();
    let monitor_state_path = state_file_path.clone();

    tauri::async_runtime::spawn(async move {
        let mut last_downloaded = monitor_downloaded.load(Ordering::Relaxed);
        let mut last_instant = Instant::now();
        let mut speed_window: Vec<u64> = Vec::new();
        let mut ticks = 0u32;

        loop {
            tokio::select! {
                _ = monitor_cancel.recv() => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(300)) => {
                    ticks += 1;
                    if low_disk_space && ticks % 4 == 0 {
                        let mut map: HashMap<usize, u64> = HashMap::new();
                        for (idx, trk) in monitor_trackers.iter().enumerate() {
                            map.insert(idx, trk.load(Ordering::Relaxed));
                        }
                        if let Ok(json_str) = serde_json::to_string(&map) {
                            let _ = fs::write(&monitor_state_path, json_str).await;
                        }
                    }

                    let current_downloaded = monitor_downloaded.load(Ordering::Relaxed);
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_instant).as_secs_f64();

                    let bytes_diff = current_downloaded.saturating_sub(last_downloaded);
                    let speed = if elapsed > 0.0 {
                        (bytes_diff as f64 / elapsed) as u64
                    } else {
                        0
                    };

                    last_downloaded = current_downloaded;
                    last_instant = now;

                    speed_window.push(speed);
                    if speed_window.len() > 20 {
                        speed_window.remove(0);
                    }

                    let mut tasks = monitor_state.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&progress_task_id) {
                        t.downloaded_bytes = current_downloaded;
                        t.percent = if t.total_bytes > 0 {
                            ((current_downloaded as f64 / t.total_bytes as f64) * 100.0).min(100.0)
                        } else {
                            0.0
                        };
                        t.speed_bps = speed;
                        t.speed_formatted = format_speed(speed);
                        t.speed_history = speed_window.clone();

                        let remaining = t.total_bytes.saturating_sub(current_downloaded);
                        t.eta_formatted = if speed > 0 && remaining > 0 {
                            format_eta(remaining / speed)
                        } else {
                            "--".to_string()
                        };

                        let _ = monitor_app.emit("download-progress", t.clone());
                        let _ = monitor_app.emit("zr://progress", t.clone());
                    } else {
                        break;
                    }
                }
            }
        }
    });

    // Await all chunks
    let mut chunk_errors = Vec::new();
    for handle in chunk_join_handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => chunk_errors.push(e),
            Err(e) => chunk_errors.push(e.to_string()),
        }
    }

    // Stop progress monitor
    let _ = cancel_tx.send(());

    if !chunk_errors.is_empty() {
        let is_user_paused = is_paused.load(Ordering::Relaxed)
            || chunk_errors.iter().any(|e| e == "Cancelled" || e.contains("cancel"));

        if is_user_paused {
            let mut tasks = state.tasks.write().await;
            if let Some(t) = tasks.get_mut(&task_id) {
                t.status = "paused".to_string();
                t.speed_bps = 0;
                t.speed_formatted = "0 B/s".to_string();
                t.eta_formatted = "--".to_string();
                let _ = app.emit("download-progress", t.clone());
                let _ = app.emit("zr://progress", t.clone());
            }
            return Ok(());
        }

        let err_msg = chunk_errors.join("; ");
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.status = "error".to_string();
            t.speed_bps = 0;
            t.speed_formatted = "0 B/s".to_string();
            t.eta_formatted = "--".to_string();
            t.error_message = Some(err_msg.clone());
            let _ = app.emit("download-error", serde_json::json!({ "id": task_id, "error": err_msg }));
            let _ = app.emit("download-progress", t.clone());
            let _ = app.emit("zr://progress", t.clone());
        }
        return Err(err_msg);
    }

    if low_disk_space {
        // --- Zero-Temp Direct In-Place: Destination file already complete! ---
        // 0 extra temporary disk space used! 20 GB file takes strictly 20 GB!
        let _ = fs::remove_file(&state_file_path).await;
    } else {
        // --- IDM-Style High-Performance Assembly Phase ---
        {
            let mut tasks = state.tasks.write().await;
            if let Some(t) = tasks.get_mut(&task_id) {
                t.status = "assembling".to_string();
                t.speed_formatted = "Assembling chunks...".to_string();
                t.eta_formatted = "Finishing...".to_string();
                let _ = app.emit("download-progress", t.clone());
                let _ = app.emit("zr://progress", t.clone());
            }
        }

        let target_dest = PathBuf::from(&save_path);
        if let Some(parent) = target_dest.parent() {
            let _ = fs::create_dir_all(parent).await;
        }

        // Optimization: If single chunk, rename/move immediately (instantaneous 0.001s!)
        if chunk_count <= 1 {
            let single_part = temp_dir.join("chunk_0.part");
            if single_part.exists() {
                let _ = fs::remove_file(&target_dest).await;
                if fs::rename(&single_part, &target_dest).await.is_err() {
                    let _ = fs::copy(&single_part, &target_dest).await;
                    let _ = fs::remove_file(&single_part).await;
                }
            }
        } else {
            // Multi-chunk: move chunk 0 as base file, then append chunk 1..N using 8 MB buffer
            let first_part = temp_dir.join("chunk_0.part");
            let _ = fs::remove_file(&target_dest).await;
            let mut start_idx = 0;
            if fs::rename(&first_part, &target_dest).await.is_ok() {
                start_idx = 1;
            }

            let mut dest_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target_dest)
                .await
                .map_err(|e| e.to_string())?;

            let mut merge_buf = vec![0u8; 8 * 1024 * 1024]; // 8 MB buffer for lightning-fast disk I/O

            for i in start_idx..chunk_count {
                // Live progress update for each chunk merged
                {
                    let mut tasks = state.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&task_id) {
                        t.status = "assembling".to_string();
                        t.speed_formatted = format!("Merging chunk {}/{}", i + 1, chunk_count);
                        let _ = app.emit("download-progress", t.clone());
                        let _ = app.emit("zr://progress", t.clone());
                    }
                }

                let part_file_path = temp_dir.join(format!("chunk_{}.part", i));
                if let Ok(mut part_file) = File::open(&part_file_path).await {
                    loop {
                        let n = part_file.read(&mut merge_buf).await.map_err(|e| e.to_string())?;
                        if n == 0 { break; }
                        dest_file.write_all(&merge_buf[..n]).await.map_err(|e| e.to_string())?;
                    }
                    // Free disk space immediately after merging each chunk
                    let _ = fs::remove_file(&part_file_path).await;
                }
            }
            dest_file.flush().await.map_err(|e| e.to_string())?;
            drop(dest_file);
        }

        // Clean up temporary segment directory
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    // Mark completed
    {
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.status = "completed".to_string();
            t.percent = 100.0;
            t.speed_bps = 0;
            t.speed_formatted = "Done".to_string();
            t.eta_formatted = "0s".to_string();
            let _ = app.emit("download-complete", t.clone());
            let _ = app.emit("download-progress", t.clone());
            let _ = app.emit("zr://completed", t.clone());
            let _ = app.emit("zr://progress", t.clone());
        }
    }

    Ok(())
}
