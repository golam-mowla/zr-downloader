use reqwest::header::{ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, RANGE, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::task::JoinSet;

use crate::settings::{self, AppSettings, SettingsState, TokenBucketLimiter};

pub const NETWORK_RETRY_BACKOFF_SECS: [u64; 5] = [2, 5, 15, 30, 60];
pub const MAX_CONNECTIONS: usize = 32;

fn default_priority() -> String {
    "normal".to_string()
}

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
    pub status: String, // "queued" | "downloading" | "paused" | "assembling" | "completed" | "completed_with_warning" | "error" | "cancelled"
    pub category: String, // "all" | "video" | "audio" | "docs" | "compressed" | "programs"
    pub chunk_count: usize,
    pub is_resumable: bool,
    #[serde(default)]
    pub low_disk_space: bool,
    pub chunks: Vec<ChunkProgress>,
    pub speed_history: Vec<u64>,
    pub error_message: Option<String>,
    pub created_at: String,

    // --- Queue Priority & Ordering (C) ---
    #[serde(default = "default_priority")]
    pub priority: String, // "high" | "normal" | "low"
    #[serde(default)]
    pub queue_order: i64,

    // --- Per-Task Connection & Speed Overrides (B, D) ---
    #[serde(default)]
    pub auto_connections: bool,
    #[serde(default)]
    pub speed_limit_kb: u64, // 0 = inherit global limit

    // --- File Handling & Verification (F, G) ---
    #[serde(default)]
    pub expected_checksum: String,
    #[serde(default)]
    pub checksum_status: String, // "" | "verified" | "mismatch" | "error"
    #[serde(default)]
    pub checksum_actual: String,
    #[serde(default)]
    pub origin_page_url: String,
    #[serde(default)]
    pub post_download_action: String, // "" = inherit global | "none" | "open_file" | "open_folder" | "run_program" | "defender_scan"
    #[serde(default)]
    pub post_download_program: String,
    #[serde(default)]
    pub scan_status: String,

    // --- HTTP Basic Auth (H) ---
    #[serde(default)]
    pub auth_username: String,
    #[serde(default)]
    pub auth_password: String,
}

impl Default for DownloadTask {
    fn default() -> Self {
        Self {
            id: String::new(),
            url: String::new(),
            filename: String::new(),
            save_path: String::new(),
            total_bytes: 0,
            downloaded_bytes: 0,
            percent: 0.0,
            speed_bps: 0,
            speed_formatted: "0 B/s".to_string(),
            eta_formatted: "--".to_string(),
            status: "queued".to_string(),
            category: "docs".to_string(),
            chunk_count: 8,
            is_resumable: true,
            low_disk_space: false,
            chunks: Vec::new(),
            speed_history: Vec::new(),
            error_message: None,
            created_at: String::new(),
            priority: default_priority(),
            queue_order: 0,
            auto_connections: false,
            speed_limit_kb: 0,
            expected_checksum: String::new(),
            checksum_status: String::new(),
            checksum_actual: String::new(),
            origin_page_url: String::new(),
            post_download_action: String::new(),
            post_download_program: String::new(),
            scan_status: String::new(),
            auth_username: String::new(),
            auth_password: String::new(),
        }
    }
}

pub struct ActiveDownloadControl {
    pub cancel_sender: broadcast::Sender<()>,
    pub is_paused: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct EngineState {
    pub tasks: Arc<RwLock<HashMap<String, DownloadTask>>>,
    pub controls: Arc<RwLock<HashMap<String, ActiveDownloadControl>>>,
    pub client: Arc<RwLock<reqwest::Client>>,
    pub global_limiter: TokenBucketLimiter,
    pub task_limiters: Arc<RwLock<HashMap<String, TokenBucketLimiter>>>,
}

impl EngineState {
    pub fn new() -> Self {
        let client = settings::build_reqwest_client(&AppSettings::default());
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            controls: Arc::new(RwLock::new(HashMap::new())),
            client: Arc::new(RwLock::new(client)),
            global_limiter: TokenBucketLimiter::new(0),
            task_limiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn current_client(&self) -> reqwest::Client {
        self.client.read().await.clone()
    }

    pub async fn apply_settings(&self, app_settings: &AppSettings) {
        let rebuilt = settings::build_reqwest_client(app_settings);
        *self.client.write().await = rebuilt;
        self.global_limiter
            .set_rate_bps(app_settings.global_speed_limit_bps());
    }

    pub async fn set_task_speed_limit(&self, task_id: &str, kb_per_sec: u64) {
        let bps = kb_per_sec.saturating_mul(1024);
        let mut map = self.task_limiters.write().await;
        if let Some(limiter) = map.get(task_id) {
            limiter.set_rate_bps(bps);
        } else {
            map.insert(task_id.to_string(), TokenBucketLimiter::new(bps));
        }
    }

    pub async fn get_task_limiter(&self, task_id: &str, kb_per_sec: u64) -> TokenBucketLimiter {
        let bps = kb_per_sec.saturating_mul(1024);
        let mut map = self.task_limiters.write().await;
        if let Some(existing) = map.get(task_id) {
            existing.set_rate_bps(bps);
            existing.clone()
        } else {
            let limiter = TokenBucketLimiter::new(bps);
            map.insert(task_id.to_string(), limiter.clone());
            limiter
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
            "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" => {
                return "video".to_string()
            }
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "opus" | "wma" => {
                return "audio".to_string()
            }
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "epub" | "csv" => {
                return "docs".to_string()
            }
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" => {
                return "compressed".to_string()
            }
            "exe" | "msi" | "apk" | "dmg" | "deb" | "rpm" | "appimage" => {
                return "programs".to_string()
            }
            _ => {}
        }
    }

    if let Some(ct) = content_type {
        let ct_lower = ct.to_lowercase();
        if ct_lower.starts_with("video/") {
            return "video".to_string();
        } else if ct_lower.starts_with("audio/") {
            return "audio".to_string();
        } else if ct_lower.contains("zip")
            || ct_lower.contains("compressed")
            || ct_lower.contains("tar")
            || ct_lower.contains("archive")
        {
            return "compressed".to_string();
        } else if ct_lower.contains("pdf")
            || ct_lower.contains("text")
            || ct_lower.contains("document")
        {
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
        format!(
            "{:.2} GB/s",
            bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0)
        )
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

fn apply_auth_header(
    req: reqwest::RequestBuilder,
    task_user: &str,
    task_pass: &str,
    site_cred: Option<(&str, &str)>,
) -> reqwest::RequestBuilder {
    if !task_user.trim().is_empty() {
        req.basic_auth(task_user.trim(), Some(task_pass))
    } else if let Some((u, p)) = site_cred {
        req.basic_auth(u, Some(p))
    } else {
        req
    }
}

pub async fn probe_url(client: &reqwest::Client, url: &str) -> Result<UrlMetadata, String> {
    probe_url_with_auth(client, url, None).await
}

pub async fn probe_url_with_auth(
    client: &reqwest::Client,
    url: &str,
    basic_auth: Option<(&str, &str)>,
) -> Result<UrlMetadata, String> {
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
    let mut range_req = client
        .get(url)
        .header(USER_AGENT, BROWSER_UA)
        .header(RANGE, "bytes=0-0")
        .header("Accept", "*/*")
        .header("Connection", "keep-alive")
        .timeout(Duration::from_secs(6));
    if !referer.is_empty() {
        range_req = range_req.header("Referer", &referer);
    }
    if let Some((u, p)) = basic_auth {
        range_req = range_req.basic_auth(u, Some(p));
    }

    let resp = match range_req.send().await {
        Ok(r) if r.status().is_success() || r.status() == reqwest::StatusCode::PARTIAL_CONTENT => {
            r
        }
        _ => {
            let mut head_req = client
                .head(url)
                .header(USER_AGENT, BROWSER_UA)
                .header("Accept", "*/*")
                .timeout(Duration::from_secs(4));
            if !referer.is_empty() {
                head_req = head_req.header("Referer", &referer);
            }
            if let Some((u, p)) = basic_auth {
                head_req = head_req.basic_auth(u, Some(p));
            }

            match head_req.send().await {
                Ok(r) if r.status().is_success() => r,
                _ => {
                    let mut get_req = client
                        .get(url)
                        .header(USER_AGENT, BROWSER_UA)
                        .header("Accept", "*/*")
                        .timeout(Duration::from_secs(6));
                    if !referer.is_empty() {
                        get_req = get_req.header("Referer", &referer);
                    }
                    if let Some((u, p)) = basic_auth {
                        get_req = get_req.basic_auth(u, Some(p));
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

    let supports_ranges = if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        true
    } else if let Some(val) = headers.get(ACCEPT_RANGES) {
        val.to_str().unwrap_or("").to_lowercase().contains("bytes")
    } else {
        false
    };

    let mut total_size = None;
    if let Some(cl) = headers.get(CONTENT_LENGTH) {
        if let Ok(str_val) = cl.to_str() {
            if let Ok(size) = str_val.parse::<u64>() {
                total_size = Some(size);
            }
        }
    }

    if total_size == Some(1) || total_size.is_none() {
        if let Some(cr) = headers.get("content-range") {
            if let Ok(cr_str) = cr.to_str() {
                if let Some(pos) = cr_str.rfind('/') {
                    if let Ok(size) = cr_str[pos + 1..].trim().parse::<u64>() {
                        total_size = Some(size);
                    }
                }
            }
        }
    }

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut filename = String::new();

    if let Some(cd) = headers.get(CONTENT_DISPOSITION) {
        if let Ok(cd_str) = cd.to_str() {
            if let Some(idx) = cd_str.find("filename*=") {
                let sub = &cd_str[idx + 10..];
                let end = sub.find(';').unwrap_or(sub.len());
                let raw = sub[..end].trim().trim_matches('"');
                let enc = if let Some(p) = raw.find("''") {
                    &raw[p + 2..]
                } else {
                    raw
                };
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
                let path_segments: Vec<&str> =
                    parsed.path_segments().map(|c| c.collect()).unwrap_or_default();
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseSegment {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseResumeMeta {
    #[serde(default)]
    pub chunk_count: usize,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub supports_ranges: bool,
    #[serde(default)]
    pub chunks: HashMap<usize, u64>,
    #[serde(default)]
    pub segments: Vec<SparseSegment>,
}

pub fn sparse_meta_path_for(save_path: &str, task_id: &str) -> PathBuf {
    let target_dest = PathBuf::from(save_path);
    let target_parent = target_dest
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    target_parent.join(format!(".zr_sparse_meta_{}.json", task_id))
}

pub fn read_sparse_meta(content: &str) -> Option<SparseResumeMeta> {
    if let Ok(meta) = serde_json::from_str::<SparseResumeMeta>(content) {
        if !meta.segments.is_empty()
            || !meta.chunks.is_empty()
            || meta.chunk_count > 0
            || meta.total_bytes > 0
        {
            return Some(meta);
        }
    }
    if let Ok(map) = serde_json::from_str::<HashMap<usize, u64>>(content) {
        return Some(SparseResumeMeta {
            chunk_count: map.len().max(1),
            total_bytes: 0,
            supports_ranges: true,
            chunks: map,
            segments: Vec::new(),
        });
    }
    None
}

/// Enrich a persisted HTTP `DownloadTask` from its adjacent `.zr_sparse_meta_<id>.json`
/// before resuming.
pub async fn hydrate_task_from_sparse_meta(task: &mut DownloadTask) {
    let meta_path = sparse_meta_path_for(&task.save_path, &task.id);
    if let Ok(content) = fs::read_to_string(&meta_path).await {
        if let Some(meta) = read_sparse_meta(&content) {
            if task.total_bytes == 0 && meta.total_bytes > 0 {
                task.total_bytes = meta.total_bytes;
            }
            if meta.chunk_count > 0 {
                task.chunk_count = meta.chunk_count;
            }
            if meta.supports_ranges {
                task.is_resumable = true;
            }
            let sum: u64 = if !meta.segments.is_empty() {
                meta.segments.iter().map(|s| s.downloaded).sum()
            } else {
                meta.chunks.values().copied().sum()
            };
            if sum > 0 {
                task.downloaded_bytes = sum.max(task.downloaded_bytes);
                if task.total_bytes > 0 {
                    task.percent =
                        ((task.downloaded_bytes as f64 / task.total_bytes as f64) * 100.0).min(100.0);
                }
            }
        }
    }
}

/// Pure helper to split `total_bytes` into `chunk_count` contiguous byte ranges `(index, start, end)`.
pub fn split_fixed_ranges(total_bytes: u64, chunk_count: usize) -> Vec<(usize, u64, u64)> {
    let count = chunk_count.clamp(1, MAX_CONNECTIONS);
    if total_bytes == 0 || count <= 1 {
        return vec![(0, 0, total_bytes.saturating_sub(1))];
    }
    let chunk_size = (total_bytes / count as u64).max(1);
    let mut ranges = Vec::with_capacity(count);
    for i in 0..count {
        let start = i as u64 * chunk_size;
        if start >= total_bytes {
            break;
        }
        let end = if i == count - 1 {
            total_bytes.saturating_sub(1)
        } else {
            ((i as u64 + 1) * chunk_size - 1).min(total_bytes.saturating_sub(1))
        };
        ranges.push((i, start, end));
    }
    ranges
}

/// Auto-connection scaling condition: add 4 connections every 5 s while total speed grew >10% up to 32.
pub fn should_scale_connections(prev_bps: u64, curr_bps: u64, current_conns: usize) -> bool {
    if current_conns >= MAX_CONNECTIONS || prev_bps == 0 {
        return false;
    }
    (curr_bps as f64) > (prev_bps as f64) * 1.10
}

#[derive(Clone)]
struct DynamicSegment {
    index: usize,
    start: u64,
    bounds: Arc<std::sync::Mutex<(u64, u64)>>, // (current_pos, current_end_inclusive)
    tracker: Arc<AtomicU64>,
}

async fn write_dynamic_sparse_meta(
    path: &PathBuf,
    total_bytes: u64,
    supports_ranges: bool,
    segments: &[DynamicSegment],
) {
    let mut chunks_map = HashMap::new();
    let mut saved_segments = Vec::with_capacity(segments.len());
    for seg in segments {
        let down = seg.tracker.load(Ordering::Relaxed);
        let (_, end) = *seg.bounds.lock().unwrap();
        chunks_map.insert(seg.index, down);
        saved_segments.push(SparseSegment {
            index: seg.index,
            start: seg.start,
            end,
            downloaded: down,
        });
    }
    let meta = SparseResumeMeta {
        chunk_count: saved_segments.len(),
        total_bytes,
        supports_ranges,
        chunks: chunks_map,
        segments: saved_segments,
    };
    if let Ok(json_str) = serde_json::to_string(&meta) {
        let _ = fs::write(path, json_str).await;
    }
}

struct ChunkWorkerParams {
    seg: DynamicSegment,
    current_url: Arc<RwLock<String>>,
    origin_page_url: String,
    auth_user: String,
    auth_pass: String,
    site_cred: Option<(String, String)>,
    temp_dir: PathBuf,
    dest_file_path: PathBuf,
    low_disk_space: bool,
    multi_or_resumed: bool,
    shared_downloaded: Arc<AtomicU64>,
    cancel_rx: broadcast::Receiver<()>,
    app: AppHandle,
    state: EngineState,
    task_id: String,
    task_limiter: TokenBucketLimiter,
    refresh_guard: Arc<Mutex<bool>>,
}

async fn run_chunk_worker(mut p: ChunkWorkerParams) -> Result<(), String> {
    let index = p.seg.index;
    let part_file_path = p.temp_dir.join(format!("chunk_{}.part", index));

    if index > 0 && index < 32 {
        tokio::time::sleep(Duration::from_millis((index as u64) * 20)).await;
    }

    let mut retries = 0usize;

    loop {
        let (chunk_start, chunk_end) = *p.seg.bounds.lock().unwrap();
        if p.seg.start > 0 || chunk_end > 0 {
            if chunk_start > chunk_end {
                return Ok(());
            }
        }

        let mut file = if p.low_disk_space {
            let mut f = OpenOptions::new()
                .write(true)
                .open(&p.dest_file_path)
                .await
                .map_err(|e| e.to_string())?;
            f.seek(SeekFrom::Start(chunk_start))
                .await
                .map_err(|e| e.to_string())?;
            f
        } else {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .open(&part_file_path)
                .await
                .map_err(|e| e.to_string())?;
            let target_local_offset = chunk_start.saturating_sub(p.seg.start);
            f.set_len(target_local_offset)
                .await
                .map_err(|e| e.to_string())?;
            f.seek(SeekFrom::Start(target_local_offset))
                .await
                .map_err(|e| e.to_string())?;
            f
        };

        let url = p.current_url.read().await.clone();
        let client = p.state.current_client().await;
        let mut req = client
            .get(&url)
            .header(
                USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "identity")
            .header("Connection", "keep-alive");

        if url.contains("safefilehosting.com") {
            req = req.header("Referer", "https://igetintopc.com/");
        } else if let Ok(parsed) = reqwest::Url::parse(&url) {
            if let Some(host) = parsed.host_str() {
                req = req.header("Referer", format!("{}://{}/", parsed.scheme(), host));
            }
        }

        req = apply_auth_header(
            req,
            &p.auth_user,
            &p.auth_pass,
            p.site_cred
                .as_ref()
                .map(|(u, pw)| (u.as_str(), pw.as_str())),
        );

        let mut sent_range = false;
        if chunk_end > 0 {
            req = req.header(RANGE, format!("bytes={}-{}", chunk_start, chunk_end));
            sent_range = true;
        } else if chunk_start > 0 {
            req = req.header(RANGE, format!("bytes={}-", chunk_start));
            sent_range = true;
        }

        let send_res = req.send().await;
        let mut resp = match send_res {
            Ok(r) => {
                if r.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                    let _ = file.flush().await;
                    return Ok(());
                }
                if sent_range
                    && (chunk_start > 0 || p.multi_or_resumed)
                    && r.status() == reqwest::StatusCode::OK
                {
                    return Err("RANGE_NOT_SUPPORTED".to_string());
                }
                if (r.status() == reqwest::StatusCode::FORBIDDEN
                    || r.status() == reqwest::StatusCode::GONE)
                    && !p.origin_page_url.trim().is_empty()
                {
                    // Expired link recovery via yt-dlp on original page URL
                    let mut refreshed_guard = p.refresh_guard.lock().await;
                    if !*refreshed_guard {
                        *refreshed_guard = true;
                        let before_url = p.current_url.read().await.clone();
                        if before_url == url {
                            if let Ok(fresh_url) =
                                crate::media::resolve_fresh_direct_url(&p.app, &p.origin_page_url)
                                    .await
                            {
                                *p.current_url.write().await = fresh_url.clone();
                                let mut tasks = p.state.tasks.write().await;
                                if let Some(t) = tasks.get_mut(&p.task_id) {
                                    t.url = fresh_url;
                                }
                            }
                        }
                    }
                    let after_url = p.current_url.read().await.clone();
                    if after_url != url {
                        retries = 0;
                        continue;
                    }
                }
                if !r.status().is_success() && r.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    if retries < NETWORK_RETRY_BACKOFF_SECS.len() {
                        let wait_s = NETWORK_RETRY_BACKOFF_SECS[retries];
                        retries += 1;
                        tokio::select! {
                            _ = p.cancel_rx.recv() => return Err("Cancelled".to_string()),
                            _ = tokio::time::sleep(Duration::from_secs(wait_s)) => continue,
                        }
                    }
                    return Err(format!("Chunk {} HTTP status {}", index, r.status()));
                }
                r
            }
            Err(e) => {
                if retries < NETWORK_RETRY_BACKOFF_SECS.len() {
                    let wait_s = NETWORK_RETRY_BACKOFF_SECS[retries];
                    retries += 1;
                    tokio::select! {
                        _ = p.cancel_rx.recv() => return Err("Cancelled".to_string()),
                        _ = tokio::time::sleep(Duration::from_secs(wait_s)) => continue,
                    }
                }
                return Err(format!("Chunk {} connect failed: {}", index, e));
            }
        };

        if let Some(cl) = resp.content_length() {
            if cl > 0 {
                let mut tasks = p.state.tasks.write().await;
                if let Some(t) = tasks.get_mut(&p.task_id) {
                    if t.total_bytes == 0 {
                        t.total_bytes = cl;
                        if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT
                            || resp.headers().contains_key(ACCEPT_RANGES)
                        {
                            t.is_resumable = true;
                        }
                        let _ = p.app.emit("download-progress", t.clone());
                        let _ = p.app.emit("zr://progress", t.clone());
                    }
                }
            }
        }

        let mut writer = tokio::io::BufWriter::with_capacity(512 * 1024, file);
        let mut stream_interrupted = false;

        loop {
            tokio::select! {
                _ = p.cancel_rx.recv() => {
                    let _ = writer.flush().await;
                    return Err("Cancelled".to_string());
                }
                chunk_res = resp.chunk() => {
                    match chunk_res {
                        Ok(Some(bytes)) => {
                            if bytes.is_empty() {
                                continue;
                            }
                            let (slice_len, reached_end) = {
                                let mut b = p.seg.bounds.lock().unwrap();
                                let cur_pos = b.0;
                                let cur_end = b.1;
                                if cur_end > 0 && cur_pos > cur_end {
                                    (0usize, true)
                                } else if cur_end > 0 {
                                    let avail = (cur_end - cur_pos + 1) as usize;
                                    let take = bytes.len().min(avail);
                                    b.0 = cur_pos + take as u64;
                                    (take, b.0 > cur_end)
                                } else {
                                    b.0 = cur_pos + bytes.len() as u64;
                                    (bytes.len(), false)
                                }
                            };

                            if slice_len == 0 {
                                break;
                            }

                            // Shared global and per-task token-bucket speed limiters
                            let limit_bytes = slice_len as u64;
                            tokio::select! {
                                _ = p.cancel_rx.recv() => {
                                    let _ = writer.flush().await;
                                    return Err("Cancelled".to_string());
                                }
                                _ = p.state.global_limiter.consume(limit_bytes) => {}
                            }
                            tokio::select! {
                                _ = p.cancel_rx.recv() => {
                                    let _ = writer.flush().await;
                                    return Err("Cancelled".to_string());
                                }
                                _ = p.task_limiter.consume(limit_bytes) => {}
                            }

                            if let Err(e) = writer.write_all(&bytes[..slice_len]).await {
                                return Err(e.to_string());
                            }
                            p.shared_downloaded.fetch_add(limit_bytes, Ordering::Relaxed);
                            p.seg.tracker.fetch_add(limit_bytes, Ordering::Relaxed);
                            retries = 0;

                            if reached_end {
                                break;
                            }
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
            return Ok(());
        }

        if retries < NETWORK_RETRY_BACKOFF_SECS.len() {
            let wait_s = NETWORK_RETRY_BACKOFF_SECS[retries];
            retries += 1;
            tokio::select! {
                _ = p.cancel_rx.recv() => return Err("Cancelled".to_string()),
                _ = tokio::time::sleep(Duration::from_secs(wait_s)) => continue,
            }
        } else {
            return Err(format!(
                "Chunk {} connection dropped after {} retries",
                index,
                NETWORK_RETRY_BACKOFF_SECS.len()
            ));
        }
    }
}

pub async fn execute_multithreaded_download(
    app: AppHandle,
    state: EngineState,
    task_id: String,
) -> Result<(), String> {
    let task_snapshot = {
        let tasks = state.tasks.read().await;
        tasks.get(&task_id).cloned().ok_or("Task not found")?
    };

    let url = task_snapshot.url.clone();
    let save_path = task_snapshot.save_path.clone();
    let mut total_bytes = task_snapshot.total_bytes;
    let mut chunk_count = task_snapshot.chunk_count.clamp(1, MAX_CONNECTIONS);
    let is_resumable = task_snapshot.is_resumable;
    let low_disk_space = task_snapshot.low_disk_space;
    let auto_connections = task_snapshot.auto_connections;
    if auto_connections && chunk_count < 8 {
        chunk_count = 8;
    }

    let app_settings = app
        .try_state::<SettingsState>()
        .map(|s| s.get_blocking())
        .unwrap_or_default();

    let site_cred = settings::find_credentials_for_url(&app_settings, &url)
        .map(|(u, p)| (u.to_string(), p.to_string()));
    let task_limiter = state
        .get_task_limiter(&task_id, task_snapshot.speed_limit_kb)
        .await;

    let target_dest = PathBuf::from(&save_path);
    let target_parent = target_dest
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    let _ = fs::create_dir_all(&target_parent).await;

    let temp_dir = target_parent.join(format!(".zr_tmp_{}", task_id));
    let state_file_path = target_parent.join(format!(".zr_sparse_meta_{}.json", task_id));

    let mut had_saved_meta = false;
    let mut sparse_meta_opt: Option<SparseResumeMeta> = None;
    if is_resumable && state_file_path.exists() {
        if let Ok(content) = fs::read_to_string(&state_file_path).await {
            if let Some(meta) = read_sparse_meta(&content) {
                had_saved_meta = true;
                if total_bytes == 0 && meta.total_bytes > 0 {
                    total_bytes = meta.total_bytes;
                }
                if meta.chunk_count > 1 && chunk_count <= 1 {
                    chunk_count = meta.chunk_count.clamp(1, MAX_CONNECTIONS);
                }
                sparse_meta_opt = Some(meta);
            }
        }
    }

    if total_bytes == 0 || !is_resumable || chunk_count <= 1 {
        chunk_count = 1;
        if !is_resumable {
            let _ = fs::remove_file(&state_file_path).await;
            let _ = fs::remove_dir_all(&temp_dir).await;
        }
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.chunk_count = 1;
            if total_bytes > 0 && t.total_bytes == 0 {
                t.total_bytes = total_bytes;
            }
            if !is_resumable {
                t.downloaded_bytes = 0;
                t.percent = 0.0;
            }
            let _ = app.emit("download-progress", t.clone());
            let _ = app.emit("zr://progress", t.clone());
        }
    }

    if !low_disk_space {
        fs::create_dir_all(&temp_dir)
            .await
            .map_err(|e| e.to_string())?;
    } else {
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

    let (cancel_tx, _cancel_rx) = broadcast::channel::<()>(16);
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

    // Build initial segments from saved SparseSegment list if resuming, otherwise split fresh
    let mut initial_segments: Vec<DynamicSegment> = Vec::new();
    let mut initial_downloaded = 0u64;

    if let Some(ref meta) = sparse_meta_opt {
        if !meta.segments.is_empty() && is_resumable {
            for s in &meta.segments {
                let c_down = if low_disk_space {
                    s.downloaded
                } else {
                    let part_path = temp_dir.join(format!("chunk_{}.part", s.index));
                    let disk_len = fs::metadata(&part_path).await.map(|m| m.len()).unwrap_or(0);
                    if disk_len > 0 {
                        disk_len.min(s.end.saturating_sub(s.start) + 1)
                    } else {
                        s.downloaded
                    }
                };
                initial_downloaded += c_down;
                initial_segments.push(DynamicSegment {
                    index: s.index,
                    start: s.start,
                    bounds: Arc::new(std::sync::Mutex::new((s.start + c_down, s.end))),
                    tracker: Arc::new(AtomicU64::new(c_down)),
                });
            }
        }
    }

    if initial_segments.is_empty() {
        let ranges = if total_bytes == 0 || !is_resumable || chunk_count <= 1 {
            vec![(
                0,
                0,
                if total_bytes > 0 && is_resumable {
                    total_bytes.saturating_sub(1)
                } else {
                    0
                },
            )]
        } else {
            split_fixed_ranges(total_bytes, chunk_count)
        };

        for (index, start, end) in ranges {
            let c_down = if !is_resumable {
                0
            } else if low_disk_space {
                sparse_meta_opt
                    .as_ref()
                    .and_then(|m| m.chunks.get(&index).copied())
                    .unwrap_or(0)
            } else {
                let part_path = temp_dir.join(format!("chunk_{}.part", index));
                let disk_len = fs::metadata(&part_path).await.map(|m| m.len()).unwrap_or(0);
                if disk_len > 0 {
                    disk_len
                } else {
                    sparse_meta_opt
                        .as_ref()
                        .and_then(|m| m.chunks.get(&index).copied())
                        .unwrap_or(0)
                }
            };
            initial_downloaded += c_down;
            initial_segments.push(DynamicSegment {
                index,
                start,
                bounds: Arc::new(std::sync::Mutex::new((start + c_down, end))),
                tracker: Arc::new(AtomicU64::new(c_down)),
            });
        }
    }

    let is_resuming = had_saved_meta || initial_downloaded > 0 || task_snapshot.downloaded_bytes > 0;
    let shared_downloaded = Arc::new(AtomicU64::new(initial_downloaded));
    let active_segments = Arc::new(Mutex::new(initial_segments.clone()));
    let current_url = Arc::new(RwLock::new(url.clone()));
    let refresh_guard = Arc::new(Mutex::new(false));

    let (ramp_tx, mut ramp_rx) = mpsc::unbounded_channel::<DynamicSegment>();
    let mut join_set: JoinSet<Result<(), String>> = JoinSet::new();

    let multi_or_resumed = initial_segments.len() > 1 || is_resuming;
    let spawn_params = |seg: DynamicSegment| ChunkWorkerParams {
        seg,
        current_url: current_url.clone(),
        origin_page_url: task_snapshot.origin_page_url.clone(),
        auth_user: task_snapshot.auth_username.clone(),
        auth_pass: task_snapshot.auth_password.clone(),
        site_cred: site_cred.clone(),
        temp_dir: temp_dir.clone(),
        dest_file_path: target_dest.clone(),
        low_disk_space,
        multi_or_resumed,
        shared_downloaded: shared_downloaded.clone(),
        cancel_rx: cancel_tx.subscribe(),
        app: app.clone(),
        state: state.clone(),
        task_id: task_id.clone(),
        task_limiter: task_limiter.clone(),
        refresh_guard: refresh_guard.clone(),
    };

    for seg in initial_segments {
        let p = spawn_params(seg);
        join_set.spawn(async move { run_chunk_worker(p).await });
    }

    // Monitor progress + Auto-connection scaling (start 8, +4 every 5s while >10% speed growth, cap 32)
    let progress_task_id = task_id.clone();
    let monitor_app = app.clone();
    let monitor_state = state.clone();
    let monitor_downloaded = shared_downloaded.clone();
    let mut monitor_cancel = cancel_tx.subscribe();
    let monitor_segments = active_segments.clone();
    let monitor_state_path = state_file_path.clone();

    tauri::async_runtime::spawn(async move {
        let mut last_downloaded = monitor_downloaded.load(Ordering::Relaxed);
        let mut last_instant = Instant::now();
        let mut speed_window: Vec<u64> = Vec::new();
        let mut ticks = 0u32;

        let mut ramp_window_start = Instant::now();
        let mut ramp_window_bytes = last_downloaded;
        let mut prev_5s_speed: u64 = 0;
        let mut baseline_speed: u64 = 0;
        let mut ramping_stopped = false;

        loop {
            tokio::select! {
                _ = monitor_cancel.recv() => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(300)) => {
                    ticks += 1;
                    if ticks % 4 == 0 {
                        let segs_snapshot = monitor_segments.lock().await.clone();
                        write_dynamic_sparse_meta(
                            &monitor_state_path,
                            total_bytes,
                            is_resumable,
                            &segs_snapshot,
                        ).await;
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

                    if ticks == 5 && baseline_speed == 0 && speed > 0 {
                        baseline_speed = speed;
                    }

                    // Dynamic Auto connection ramp check every 5s
                    if auto_connections && !ramping_stopped && is_resumable && total_bytes > 0 {
                        let win_elapsed = now.duration_since(ramp_window_start).as_secs_f64();
                        if win_elapsed >= 5.0 {
                            let avg_5s_speed = ((current_downloaded.saturating_sub(ramp_window_bytes)) as f64 / win_elapsed) as u64;
                            ramp_window_start = now;
                            ramp_window_bytes = current_downloaded;

                            let compare_base = if prev_5s_speed > 0 { prev_5s_speed } else { baseline_speed };
                            let mut segs = monitor_segments.lock().await;
                            let cur_count = segs.len();
                            if cur_count < MAX_CONNECTIONS && should_scale_connections(compare_base, avg_5s_speed, cur_count) {
                                let to_add = 4.min(MAX_CONNECTIONS - cur_count);
                                let mut added = 0;
                                for _ in 0..to_add {
                                    // Find segment with largest remaining span >= 1 MB
                                    let mut best_idx = None;
                                    let mut best_rem = 1024 * 1024u64;
                                    for (idx, s) in segs.iter().enumerate() {
                                        let (pos, end) = *s.bounds.lock().unwrap();
                                        if end > pos {
                                            let rem = end - pos + 1;
                                            if rem >= best_rem {
                                                best_rem = rem;
                                                best_idx = Some(idx);
                                            }
                                        }
                                    }
                                    if let Some(target_idx) = best_idx {
                                        let new_seg = {
                                            let mut b = segs[target_idx].bounds.lock().unwrap();
                                            let (pos, end) = *b;
                                            let mid = pos + (end - pos) / 2;
                                            if mid <= pos || mid >= end {
                                                None
                                            } else {
                                                b.1 = mid;
                                                let next_index = segs.iter().map(|s| s.index).max().unwrap_or(0) + 1;
                                                Some(DynamicSegment {
                                                    index: next_index,
                                                    start: mid + 1,
                                                    bounds: Arc::new(std::sync::Mutex::new((mid + 1, end))),
                                                    tracker: Arc::new(AtomicU64::new(0)),
                                                })
                                            }
                                        };
                                        if let Some(ns) = new_seg {
                                            segs.push(ns.clone());
                                            let _ = ramp_tx.send(ns);
                                            added += 1;
                                        }
                                    } else {
                                        break;
                                    }
                                }
                                if added == 0 {
                                    ramping_stopped = true;
                                }
                            } else if compare_base > 0 {
                                ramping_stopped = true;
                            }
                            prev_5s_speed = avg_5s_speed;
                            let seg_count_now = segs.len();
                            drop(segs);
                            let mut tasks = monitor_state.tasks.write().await;
                            if let Some(t) = tasks.get_mut(&progress_task_id) {
                                t.chunk_count = seg_count_now;
                            }
                        }
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

    // Await all initial + dynamically spawned chunks
    let mut chunk_errors = Vec::new();
    loop {
        if join_set.is_empty() {
            break;
        }
        tokio::select! {
            Some(join_res) = join_set.join_next() => {
                match join_res {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => chunk_errors.push(e),
                    Err(e) => chunk_errors.push(e.to_string()),
                }
            }
            Some(new_seg) = ramp_rx.recv() => {
                let p = spawn_params(new_seg);
                join_set.spawn(async move { run_chunk_worker(p).await });
            }
        }
    }

    let _ = cancel_tx.send(());

    let final_segments = {
        let mut segs = active_segments.lock().await.clone();
        segs.sort_by_key(|s| s.start);
        segs
    };

    if !chunk_errors.is_empty() {
        if chunk_errors.iter().any(|e| e == "RANGE_NOT_SUPPORTED") {
            // Requirement A: On resume, NEVER silently restart from 0% — report clear error.
            if is_resuming {
                write_dynamic_sparse_meta(&state_file_path, total_bytes, false, &final_segments)
                    .await;
                let msg = "Server does not support resume".to_string();
                let mut tasks = state.tasks.write().await;
                if let Some(t) = tasks.get_mut(&task_id) {
                    t.status = "error".to_string();
                    t.is_resumable = false;
                    t.speed_bps = 0;
                    t.speed_formatted = "0 B/s".to_string();
                    t.eta_formatted = "--".to_string();
                    t.error_message = Some(msg.clone());
                    let _ = app.emit(
                        "download-error",
                        serde_json::json!({ "id": task_id, "error": msg }),
                    );
                    let _ = app.emit("download-progress", t.clone());
                    let _ = app.emit("zr://progress", t.clone());
                }
                crate::spawn_save_all_tasks(&app);
                crate::queue::kick_queue(app);
                return Err("Server does not support resume".to_string());
            }

            // Fresh start fallback: restart as single connection if probe had falsely advertised Range
            let _ = fs::remove_file(&state_file_path).await;
            let _ = fs::remove_dir_all(&temp_dir).await;
            if low_disk_space {
                let _ = fs::remove_file(&target_dest).await;
            }
            {
                let mut tasks = state.tasks.write().await;
                if let Some(t) = tasks.get_mut(&task_id) {
                    t.is_resumable = false;
                    t.chunk_count = 1;
                    t.auto_connections = false;
                    t.downloaded_bytes = 0;
                    t.percent = 0.0;
                    t.speed_bps = 0;
                    t.speed_formatted = "Restarting...".to_string();
                    t.error_message = None;
                    let _ = app.emit("download-progress", t.clone());
                    let _ = app.emit("zr://progress", t.clone());
                }
            }
            crate::spawn_save_all_tasks(&app);
            return Box::pin(execute_multithreaded_download(app, state, task_id)).await;
        }

        write_dynamic_sparse_meta(&state_file_path, total_bytes, is_resumable, &final_segments)
            .await;

        let is_user_paused = is_paused.load(Ordering::Relaxed)
            || chunk_errors
                .iter()
                .any(|e| e == "Cancelled" || e.contains("cancel"));

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
            crate::spawn_save_all_tasks(&app);
            crate::queue::kick_queue(app);
            return Ok(());
        }

        let err_msg = chunk_errors.join("; ");
        {
            let mut tasks = state.tasks.write().await;
            if let Some(t) = tasks.get_mut(&task_id) {
                t.status = "error".to_string();
                t.speed_bps = 0;
                t.speed_formatted = "0 B/s".to_string();
                t.eta_formatted = "--".to_string();
                t.error_message = Some(err_msg.clone());
                let _ = app.emit(
                    "download-error",
                    serde_json::json!({ "id": task_id, "error": err_msg }),
                );
                let _ = app.emit("download-progress", t.clone());
                let _ = app.emit("zr://progress", t.clone());
            }
        }
        crate::spawn_save_all_tasks(&app);
        crate::queue::kick_queue(app);
        return Err(err_msg);
    }

    let _ = fs::remove_file(&state_file_path).await;
    if !low_disk_space {
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

        if let Some(parent) = target_dest.parent() {
            let _ = fs::create_dir_all(parent).await;
        }

        if final_segments.len() <= 1 {
            let idx = final_segments.first().map(|s| s.index).unwrap_or(0);
            let single_part = temp_dir.join(format!("chunk_{}.part", idx));
            if single_part.exists() {
                let _ = fs::remove_file(&target_dest).await;
                if fs::rename(&single_part, &target_dest).await.is_err() {
                    let _ = fs::copy(&single_part, &target_dest).await;
                    let _ = fs::remove_file(&single_part).await;
                }
            }
        } else {
            let first_idx = final_segments[0].index;
            let first_part = temp_dir.join(format!("chunk_{}.part", first_idx));
            let _ = fs::remove_file(&target_dest).await;
            let mut start_pos = 0usize;
            if fs::rename(&first_part, &target_dest).await.is_ok() {
                start_pos = 1;
            }

            let mut dest_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target_dest)
                .await
                .map_err(|e| e.to_string())?;

            let mut merge_buf = vec![0u8; 8 * 1024 * 1024];
            let seg_total = final_segments.len();

            for (pos_idx, seg) in final_segments.iter().enumerate().skip(start_pos) {
                {
                    let mut tasks = state.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&task_id) {
                        t.status = "assembling".to_string();
                        t.speed_formatted = format!("Merging part {}/{}", pos_idx + 1, seg_total);
                        let _ = app.emit("download-progress", t.clone());
                        let _ = app.emit("zr://progress", t.clone());
                    }
                }

                let part_file_path = temp_dir.join(format!("chunk_{}.part", seg.index));
                if let Ok(mut part_file) = File::open(&part_file_path).await {
                    loop {
                        let n = part_file
                            .read(&mut merge_buf)
                            .await
                            .map_err(|e| e.to_string())?;
                        if n == 0 {
                            break;
                        }
                        dest_file
                            .write_all(&merge_buf[..n])
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    let _ = fs::remove_file(&part_file_path).await;
                }
            }
            dest_file.flush().await.map_err(|e| e.to_string())?;
            drop(dest_file);
        }

        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    // Post-completion pipeline: auto-categorize move, checksum verification, post-download action
    let current_settings = app
        .try_state::<SettingsState>()
        .map(|s| s.get_blocking())
        .unwrap_or_default();

    let final_path = settings::maybe_move_to_auto_category(&current_settings, &target_dest);
    let final_save_path = final_path.to_string_lossy().to_string();

    let mut checksum_status = String::new();
    let mut checksum_actual = String::new();
    if !task_snapshot.expected_checksum.trim().is_empty() && final_path.exists() {
        let p_clone = final_path.clone();
        let expected_clone = task_snapshot.expected_checksum.clone();
        match tokio::task::spawn_blocking(move || {
            settings::verify_file_checksum(&p_clone, &expected_clone)
        })
        .await
        {
            Ok(Ok((matched, _algo, actual))) => {
                checksum_actual = actual;
                checksum_status = if matched {
                    "verified".to_string()
                } else {
                    "mismatch".to_string()
                };
            }
            _ => {
                checksum_status = "error".to_string();
            }
        }
    }

    let final_size = tokio::fs::metadata(&final_path)
        .await
        .map(|m| m.len())
        .unwrap_or(total_bytes);

    {
        let mut tasks = state.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.status = "completed".to_string();
            t.percent = 100.0;
            t.speed_bps = 0;
            t.speed_formatted = "Done".to_string();
            t.eta_formatted = "0s".to_string();
            t.save_path = final_save_path;
            if final_size > 0 {
                t.downloaded_bytes = final_size;
                if t.total_bytes == 0 {
                    t.total_bytes = final_size;
                }
            }
            t.checksum_status = checksum_status;
            t.checksum_actual = checksum_actual;

            let _ = app.emit("download-complete", t.clone());
            let _ = app.emit("download-progress", t.clone());
            let _ = app.emit("zr://completed", t.clone());
            let _ = app.emit("zr://progress", t.clone());
        }
    }
    crate::spawn_save_all_tasks(&app);

    let effective_action = if !task_snapshot.post_download_action.trim().is_empty() {
        task_snapshot.post_download_action.clone()
    } else {
        current_settings.post_download_action.clone()
    };
    let effective_program = if !task_snapshot.post_download_program.trim().is_empty() {
        task_snapshot.post_download_program.clone()
    } else {
        current_settings.post_download_program.clone()
    };
    let app_post = app.clone();
    let tid_post = task_id.clone();
    let path_post = final_path.clone();
    tokio::spawn(async move {
        settings::execute_post_download_action(
            &app_post,
            &tid_post,
            &effective_action,
            &effective_program,
            &path_post,
        );
    });

    crate::queue::kick_queue(app);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_32_connections_cover_full_range_without_gaps_or_overlaps() {
        let total = 100 * 1024 * 1024 + 137; // non-divisible size
        let ranges = split_fixed_ranges(total, 32);
        assert_eq!(ranges.len(), 32);
        assert_eq!(ranges[0].1, 0);
        assert_eq!(ranges.last().unwrap().2, total - 1);
        for w in ranges.windows(2) {
            assert_eq!(w[0].2 + 1, w[1].1);
        }
    }

    #[test]
    fn auto_connection_scaling_threshold_enforces_10_percent_and_cap_32() {
        assert!(should_scale_connections(1_000_000, 1_150_000, 8));
        assert!(!should_scale_connections(1_000_000, 1_100_000, 8)); // strictly >10%
        assert!(!should_scale_connections(1_000_000, 900_000, 12));
        assert!(!should_scale_connections(1_000_000, 5_000_000, 32)); // capped at 32
    }

    #[tokio::test]
    async fn hydrates_sparse_meta_including_dynamic_segments() {
        let dir = std::env::temp_dir().join(format!("zr_test_dl_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let save_path = dir.join("ubuntu.iso");
        let meta_path = sparse_meta_path_for(save_path.to_str().unwrap(), "t1");
        let meta = SparseResumeMeta {
            chunk_count: 12,
            total_bytes: 1000,
            supports_ranges: true,
            chunks: HashMap::new(),
            segments: vec![
                SparseSegment {
                    index: 0,
                    start: 0,
                    end: 499,
                    downloaded: 250,
                },
                SparseSegment {
                    index: 1,
                    start: 500,
                    end: 999,
                    downloaded: 300,
                },
            ],
        };
        std::fs::write(&meta_path, serde_json::to_string(&meta).unwrap()).unwrap();

        let mut task = DownloadTask {
            id: "t1".to_string(),
            save_path: save_path.to_string_lossy().to_string(),
            total_bytes: 0,
            downloaded_bytes: 0,
            percent: 0.0,
            ..Default::default()
        };
        hydrate_task_from_sparse_meta(&mut task).await;
        assert_eq!(task.total_bytes, 1000);
        assert_eq!(task.chunk_count, 12);
        assert_eq!(task.downloaded_bytes, 550);
        assert!((task.percent - 55.0).abs() < 0.01);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
