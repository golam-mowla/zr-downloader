
pub fn dirs_or_default_downloads() -> String {
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
    "Downloads".to_string()
}
use crate::downloader::DownloadTask;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{command, AppHandle, Emitter, Manager, State};
use tokio::io::AsyncWriteExt;

static ENGINE_LOCK: Mutex<()> = Mutex::new(());
static TOOL_UPDATES_CACHE: Mutex<Option<(Instant, Vec<ToolStatus>)>> = Mutex::new(None);
static FORMATS_CACHE: Mutex<Option<HashMap<String, (Instant, MediaInfo)>>> = Mutex::new(None);

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct DownloadState {
    pub active_pids: Arc<Mutex<HashMap<String, u32>>>,
    pub cancelled_tasks: Arc<Mutex<HashMap<String, bool>>>,
    pub task_options: Arc<Mutex<HashMap<String, DownloadOptions>>>,
    pub tasks: Arc<Mutex<HashMap<String, DownloadTask>>>,
    pub running_workers: Arc<Mutex<std::collections::HashSet<String>>>,
}

pub struct AppUrlState {
    pub pending_url: Arc<Mutex<Option<serde_json::Value>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaStream {
    pub id: String,
    pub label: String,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub ext: Option<String>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub filesize: Option<u64>,
    #[serde(rename = "hasAudio")]
    pub has_audio: Option<bool>,
    #[serde(rename = "formatNote")]
    pub format_note: Option<String>,
    #[serde(rename = "isPremium")]
    pub is_premium: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrack {
    pub code: String,
    pub name: String,
    #[serde(rename = "isAuto")]
    pub is_auto: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub id: Option<String>,
    pub title: String,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    #[serde(rename = "videoStreams")]
    pub video_streams: Vec<MediaStream>,
    #[serde(rename = "audioStreams")]
    pub audio_streams: Vec<MediaStream>,
    pub subtitles: Vec<SubtitleTrack>,
}

#[allow(dead_code)]
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadOptions {
    #[serde(alias = "task_id", alias = "taskId")]
    pub task_id: Option<String>,
    pub url: String,
    #[serde(alias = "save_folder", alias = "saveFolder", alias = "save_path", alias = "savePath")]
    pub save_folder: String,
    #[serde(alias = "custom_filename", alias = "customFilename", alias = "filename", alias = "title")]
    pub custom_filename: Option<String>,
    #[serde(alias = "download_mode", alias = "downloadMode")]
    pub download_mode: String,
    pub container: String,
    #[serde(alias = "video_stream_id", alias = "videoStreamId")]
    pub video_stream_id: String,
    #[serde(alias = "audio_stream_id", alias = "audioStreamId")]
    pub audio_stream_id: String,
    #[serde(alias = "audio_bitrate", alias = "audioBitrate")]
    pub audio_bitrate: String,

    // Subtitle Section
    #[serde(alias = "enable_subtitles", alias = "enableSubtitles")]
    pub enable_subtitles: bool,
    #[serde(alias = "subtitle_lang", alias = "subtitleLang")]
    pub subtitle_lang: String,
    #[serde(alias = "subtitle_format", alias = "subtitleFormat")]
    pub subtitle_format: String,
    #[serde(alias = "subtitle_save_mode", alias = "subtitleSaveMode")]
    pub subtitle_save_mode: String,

    // Clip Section
    #[serde(alias = "enable_clip", alias = "enableClip")]
    pub enable_clip: bool,
    #[serde(alias = "start_time", alias = "startTime")]
    pub start_time: String,
    #[serde(alias = "end_time", alias = "endTime")]
    pub end_time: String,
    #[serde(alias = "precise_cut", alias = "preciseCut")]
    pub precise_cut: bool,

    // Parabolic Options - UI & Downloads
    #[serde(alias = "max_active_downloads", alias = "maxActiveDownloads")]
    pub max_active_downloads: u32,
    #[serde(alias = "overwrite_files", alias = "overwriteFiles")]
    pub overwrite_files: bool,
    #[serde(alias = "include_media_id", alias = "includeMediaId")]
    pub include_media_id: bool,
    #[serde(alias = "include_auto_subtitles", alias = "includeAutoSubtitles")]
    pub include_auto_subtitles: bool,
    #[serde(alias = "include_super_res", alias = "includeSuperRes")]
    pub include_super_res: bool,
    #[serde(alias = "preferred_video_codec", alias = "preferredVideoCodec")]
    pub preferred_video_codec: String,
    #[serde(alias = "preferred_audio_codec", alias = "preferredAudioCodec")]
    pub preferred_audio_codec: String,
    #[serde(alias = "preferred_subtitle_format", alias = "preferredSubtitleFormat")]
    pub preferred_subtitle_format: String,
    #[serde(alias = "preferred_fps", alias = "preferredFps")]
    pub preferred_fps: String,

    // Parabolic Options - Downloader
    #[serde(alias = "use_part_files", alias = "usePartFiles")]
    pub use_part_files: bool,
    #[serde(alias = "sponsor_block", alias = "sponsorBlock")]
    pub sponsor_block: bool,
    #[serde(alias = "limit_speed", alias = "limitSpeed")]
    pub limit_speed: bool,
    #[serde(alias = "speed_limit_kb", alias = "speedLimitKb")]
    pub speed_limit_kb: u32,
    pub proxy: String,
    #[serde(alias = "cookies_file", alias = "cookiesFile")]
    pub cookies_file: String,
    #[serde(alias = "cookies_browser", alias = "cookiesBrowser")]
    pub cookies_browser: String,
    #[serde(alias = "use_aria2c", alias = "useAria2c")]
    pub use_aria2c: bool,
    #[serde(alias = "aria2_connections", alias = "aria2Connections")]
    pub aria2_connections: u32,
    #[serde(alias = "ytdlp_discovery_args", alias = "ytdlpDiscoveryArgs")]
    pub ytdlp_discovery_args: String,
    #[serde(alias = "ytdlp_download_args", alias = "ytdlpDownloadArgs")]
    pub ytdlp_download_args: String,

    // Parabolic Options - Converter
    #[serde(alias = "translate_metadata", alias = "translateMetadata")]
    pub translate_metadata: bool,
    #[serde(alias = "embed_metadata", alias = "embedMetadata")]
    pub embed_metadata: bool,
    #[serde(alias = "embed_thumbnails", alias = "embedThumbnails")]
    pub embed_thumbnails: bool,
    #[serde(alias = "crop_audio_thumbnails", alias = "cropAudioThumbnails")]
    pub crop_audio_thumbnails: bool,
    #[serde(alias = "embed_chapters", alias = "embedChapters")]
    pub embed_chapters: bool,
    #[serde(alias = "split_chapters", alias = "splitChapters")]
    pub split_chapters: bool,
    #[serde(alias = "embed_subtitles_global", alias = "embedSubtitlesGlobal")]
    pub embed_subtitles_global: bool,
    #[serde(alias = "ffmpeg_threads", alias = "ffmpegThreads")]
    pub ffmpeg_threads: u32,
    #[serde(alias = "post_processor_args", alias = "postProcessorArgs")]
    pub post_processor_args: String,
    // Low Disk Space Zero-Temp Mode
    #[serde(alias = "low_disk_space", alias = "lowDiskSpace", alias = "low_disk")]
    pub low_disk_space: Option<bool>,
    #[serde(default, alias = "referrer", alias = "referer")]
    pub referer: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub is_resume: bool,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub queue_order: i64,
    #[serde(default)]
    pub expected_checksum: String,
    #[serde(default)]
    pub post_download_action: String,
    #[serde(default)]
    pub post_download_program: String,
    #[serde(default)]
    pub auth_username: String,
    #[serde(default)]
    pub auth_password: String,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            task_id: None,
            url: String::new(),
            save_folder: String::new(),
            custom_filename: None,
            download_mode: "video".to_string(),
            container: "MP4 (Video)".to_string(),
            video_stream_id: "best".to_string(),
            audio_stream_id: "best".to_string(),
            audio_bitrate: "320".to_string(),
            enable_subtitles: false,
            subtitle_lang: "en".to_string(),
            subtitle_format: "srt".to_string(),
            subtitle_save_mode: "separate".to_string(),
            enable_clip: false,
            start_time: "00:00:00".to_string(),
            end_time: "00:00:10".to_string(),
            precise_cut: false,
            max_active_downloads: 3,
            overwrite_files: false,
            include_media_id: false,
            include_auto_subtitles: false,
            include_super_res: false,
            preferred_video_codec: "h264".to_string(),
            preferred_audio_codec: "aac".to_string(),
            preferred_subtitle_format: "srt".to_string(),
            preferred_fps: "60".to_string(),
            use_part_files: true,
            sponsor_block: false,
            limit_speed: false,
            speed_limit_kb: 0,
            proxy: String::new(),
            cookies_file: String::new(),
            cookies_browser: String::new(),
            use_aria2c: true,
            aria2_connections: 8,
            ytdlp_discovery_args: String::new(),
            ytdlp_download_args: String::new(),
            translate_metadata: false,
            embed_metadata: true,
            embed_thumbnails: true,
            crop_audio_thumbnails: true,
            embed_chapters: false,
            split_chapters: false,
            embed_subtitles_global: false,
            ffmpeg_threads: 4,
            post_processor_args: String::new(),
            low_disk_space: Some(false),
            referer: String::new(),
            duration: None,
            is_resume: false,
            priority: "normal".to_string(),
            queue_order: 0,
            expected_checksum: String::new(),
            post_download_action: String::new(),
            post_download_program: String::new(),
            auth_username: String::new(),
            auth_password: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub task_id: String,
    pub percent: f64,
    pub text: String,
    pub speed: String,
    pub eta: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub name: String,
    pub installed_version: String,
    pub latest_version: String,
    pub has_update: bool,
}

// ---------------------------------------------------------------------------
// Cross-Platform Binary & PATH Discovery (Fixes Windows GUI PATH issues)
// ---------------------------------------------------------------------------

/// Keeps the backend task list in sync when a download fails, so windows that fetch the task
/// (e.g. the progress popup) see "error" instead of a task that is "downloading" forever.
fn mark_task_error(tasks: &Arc<Mutex<HashMap<String, DownloadTask>>>, tid: &str, msg: &str) {
    mark_task_error_with_log(tasks, tid, msg, Vec::new());
}

fn mark_task_error_with_log(
    tasks: &Arc<Mutex<HashMap<String, DownloadTask>>>,
    tid: &str,
    msg: &str,
    log_lines: Vec<String>,
) {
    if let Ok(mut lock) = tasks.lock() {
        if let Some(t) = lock.get_mut(tid) {
            t.status = "error".to_string();
            t.error_message = Some(msg.to_string());
            if !log_lines.is_empty() {
                t.error_log = log_lines;
            } else if t.error_log.is_empty() {
                t.error_log = vec![msg.to_string()];
            }
            if t.percent >= 100.0 {
                t.percent = 99.0;
            }
            t.speed_bps = 0;
            t.speed_formatted = "0 B/s".to_string();
            t.eta_formatted = "--".to_string();
        }
    }
}

/// Marks a media task as completed with a non-fatal post-processing warning so the kept file stays accessible.
fn mark_task_completed_with_warning(
    tasks: &Arc<Mutex<HashMap<String, DownloadTask>>>,
    tid: &str,
    msg: &str,
    filename_override: Option<String>,
    file_size: Option<u64>,
) {
    if let Ok(mut lock) = tasks.lock() {
        if let Some(t) = lock.get_mut(tid) {
            t.status = "completed_with_warning".to_string();
            t.percent = 100.0;
            t.error_message = Some(msg.to_string());
            t.speed_bps = 0;
            t.speed_formatted = "0 B/s".to_string();
            t.eta_formatted = "Done".to_string();
            if let Some(name) = filename_override {
                if !name.is_empty() {
                    t.filename = name;
                }
            }
            if let Some(sz) = file_size {
                if sz > 0 {
                    t.total_bytes = sz;
                    t.downloaded_bytes = sz;
                }
            }
        }
    }
}

fn get_augmented_path() -> String {
    // Computed once: scanning many directories on every command made the app feel slow/hung.
    static AUG_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    AUG_PATH.get_or_init(compute_augmented_path).clone()
}

fn compute_augmented_path() -> String {
    let current_path = env::var("PATH").unwrap_or_default();
    let mut extra_dirs: Vec<PathBuf> = Vec::new();

    // 1. Current Executable Directory & subdirectories (Portable folder)
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            extra_dirs.push(exe_dir.to_path_buf());
            extra_dirs.push(exe_dir.join("bin"));
            extra_dirs.push(exe_dir.join("tools"));
            extra_dirs.push(exe_dir.join("resources"));
            extra_dirs.push(exe_dir.join("resources").join("bin"));
        }
    }

    // 2. Current Working Directory
    if let Ok(cwd) = env::current_dir() {
        extra_dirs.push(cwd.clone());
        extra_dirs.push(cwd.join("bin"));
        extra_dirs.push(cwd.join("tools"));
    }

    // 3. User Dedicated Tool Paths
    extra_dirs.push(PathBuf::from(r"D:\00VideoEditing Tools\yt-dlp"));
    extra_dirs.push(PathBuf::from(r"D:\00VideoEditing Tools\yt-dlp\bin"));
    extra_dirs.push(PathBuf::from(r"D:\Antigravity\yt-dlp"));

    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        let local = PathBuf::from(&local_app_data);
        // App's own local managed bin directory
        extra_dirs.push(local.clone());
        extra_dirs.push(local.join("ZR Downloader").join("bin"));
        extra_dirs.push(local.join("ZR Downloader"));
        extra_dirs.push(local.join("ZR Downloader").join("bin"));
        extra_dirs.push(local.join("GM-Downloader").join("bin"));
        extra_dirs.push(local.join("Programs").join("yt-dlp"));

        // WinGet Links
        let winget_links = local.join("Microsoft").join("WinGet").join("Links");
        if winget_links.exists() {
            extra_dirs.push(winget_links);
        }

        // WinGet Packages (FFmpeg, Deno, aria2, yt-dlp)
        let winget_pkgs = local.join("Microsoft").join("WinGet").join("Packages");
        if let Ok(entries) = fs::read_dir(&winget_pkgs) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    extra_dirs.push(p.clone());
                    if let Ok(sub_entries) = fs::read_dir(&p) {
                        for sub in sub_entries.flatten() {
                            let sp = sub.path();
                            if sp.is_dir() {
                                extra_dirs.push(sp.clone());
                                let bin_dir = sp.join("bin");
                                if bin_dir.exists() {
                                    extra_dirs.push(bin_dir);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Python Scripts directories
        let py_root = local.join("Programs").join("Python");
        if let Ok(entries) = fs::read_dir(&py_root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let scripts = p.join("Scripts");
                    if scripts.exists() {
                        extra_dirs.push(scripts);
                    }
                    extra_dirs.push(p);
                }
            }
        }
    }

    if let Ok(app_data) = env::var("APPDATA") {
        let appdata_p = PathBuf::from(&app_data);
        extra_dirs.push(appdata_p.join("Python").join("Scripts"));
    }

    if let Ok(user_profile) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
        let home = PathBuf::from(&user_profile);
        extra_dirs.push(home.join(".deno").join("bin"));
        extra_dirs.push(home.join(".local").join("bin"));
        extra_dirs.push(home.join("scoop").join("shims"));
        extra_dirs.push(home.join("scoop").join("apps").join("yt-dlp").join("current"));
    }

    extra_dirs.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
    extra_dirs.push(PathBuf::from(r"C:\scoop\shims"));
    extra_dirs.push(PathBuf::from(r"C:\Program Files\yt-dlp"));
    extra_dirs.push(PathBuf::from(r"C:\Program Files (x86)\yt-dlp"));

    #[cfg(not(target_os = "windows"))]
    {
        extra_dirs.push(PathBuf::from("/opt/homebrew/bin"));
        extra_dirs.push(PathBuf::from("/usr/local/bin"));
        extra_dirs.push(PathBuf::from("/usr/bin"));
    }

    let sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    let mut parts: Vec<String> = extra_dirs
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    parts.push(current_path);
    parts.join(sep)
}

fn find_exe_in_augmented_path(bin_name: &str) -> Option<PathBuf> {
    static EXE_CACHE: std::sync::OnceLock<Mutex<HashMap<String, PathBuf>>> = std::sync::OnceLock::new();
    let cache = EXE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock() {
        if let Some(p) = map.get(bin_name) {
            if p.is_file() {
                return Some(p.clone());
            }
        }
    }
    let found = scan_exe_in_augmented_path(bin_name);
    if let Some(ref p) = found {
        if let Ok(mut map) = cache.lock() {
            map.insert(bin_name.to_string(), p.clone());
        }
    }
    found
}

fn scan_exe_in_augmented_path(bin_name: &str) -> Option<PathBuf> {
    let aug_path = get_augmented_path();
    let sep = if cfg!(target_os = "windows") { ';' } else { ':' };
    let exts = if cfg!(target_os = "windows") {
        vec![".exe", ".cmd", ".bat", ""]
    } else {
        vec![""]
    };

    for dir in aug_path.split(sep) {
        if dir.trim().is_empty() {
            continue;
        }
        let base = PathBuf::from(dir);
        for ext in &exts {
            let candidate = base.join(format!("{}{}", bin_name, ext));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn ensure_ffmpeg_available() -> Option<PathBuf> {
    // 1. Check well-known locations first
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let candidates = [
                PathBuf::from(&local).join("ZR Downloader").join("bin").join("ffmpeg.exe"),
                PathBuf::from(&local).join("ZR Downloader").join("ffmpeg.exe"),
                PathBuf::from(&local).join("GM-Downloader").join("bin").join("ffmpeg.exe"),
                PathBuf::from(&local).join("ffmpeg.exe"),
                PathBuf::from(r"D:\Subtitle Edit\ffmpeg\ffmpeg.exe"),
                PathBuf::from(r"D:\Subtitle Edit\ffmpeg\bin\ffmpeg.exe"),
                PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"),
                PathBuf::from(r"C:\Program Files\ffmpeg\bin\ffmpeg.exe"),
            ];
            for p in &candidates {
                if p.exists() {
                    return Some(p.clone());
                }
            }
        }
    }

    // 2. Scan augmented PATH
    if let Some(existing) = find_exe_in_augmented_path("ffmpeg") {
        return Some(existing);
    }

    None
}

fn replace_from_part(part: &Path, dest: &Path) -> Result<(), String> {
    if !part.exists() {
        return Err(format!("Downloaded part file missing: {}", part.display()));
    }
    let mut last_err = None;
    for attempt in 0..6 {
        match fs::rename(part, dest) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let code = e.raw_os_error();
                last_err = Some(e);
                if matches!(code, Some(32) | Some(5)) && attempt < 5 {
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
                break;
            }
        }
    }
    let _ = fs::remove_file(part);
    Err(format!(
        "Failed to finalize {}: {}",
        dest.display(),
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
}

fn find_local_ytdlp() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let p_zr = PathBuf::from(&local).join("ZR Downloader").join("bin").join("yt-dlp.exe");
            if p_zr.exists() { return Some(p_zr); }
            let p_zr_root = PathBuf::from(&local).join("ZR Downloader").join("yt-dlp.exe");
            if p_zr_root.exists() { return Some(p_zr_root); }
            let p_gm = PathBuf::from(&local).join("GM-Downloader").join("bin").join("yt-dlp.exe");
            if p_gm.exists() { return Some(p_gm); }
            let p_direct = PathBuf::from(&local).join("yt-dlp.exe");
            if p_direct.exists() { return Some(p_direct); }
        }
    }

    find_exe_in_augmented_path("yt-dlp")
}

fn find_local_deno() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let p_zr = PathBuf::from(&local).join("ZR Downloader").join("bin").join("deno.exe");
            if p_zr.exists() {
                return Some(p_zr);
            }
        }
    }

    find_exe_in_augmented_path("deno")
}

fn find_local_aria2c() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let p_zr = PathBuf::from(&local).join("ZR Downloader").join("bin").join("aria2c.exe");
            if p_zr.exists() {
                return Some(p_zr);
            }
        }
    }

    find_exe_in_augmented_path("aria2c")
}

fn ensure_ytdlp_available() -> Option<PathBuf> {
    let _engine_guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(existing) = find_local_ytdlp() {
        return Some(existing);
    }

    // Do not hammer the network (and block the worker) if a recent download attempt failed.
    static LAST_FAIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last_fail = LAST_FAIL.load(std::sync::atomic::Ordering::Relaxed);
    if last_fail > 0 && now_secs.saturating_sub(last_fail) < 600 {
        return None;
    }
    let downloaded = download_ytdlp_with_timeouts();
    if downloaded.is_none() {
        LAST_FAIL.store(now_secs, std::sync::atomic::Ordering::Relaxed);
    }
    downloaded
}

fn download_ytdlp_with_timeouts() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            let target_dir = PathBuf::from(&local_app_data).join("ZR Downloader").join("bin");
            let _ = fs::create_dir_all(&target_dir);
            let target_exe = target_dir.join("yt-dlp.exe");
            let part_exe = target_dir.join("yt-dlp.exe.part");
            let _ = fs::remove_file(&part_exe);

            // 1. Try curl.exe into .part then atomic rename
            let mut curl_cmd = Command::new("curl.exe");
            curl_cmd.args([
                "-L",
                "--fail",
                "--connect-timeout",
                "8",
                "--max-time",
                "30",
                "-o",
                part_exe.to_string_lossy().as_ref(),
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
            ]);
            curl_cmd.creation_flags(CREATE_NO_WINDOW);
            if let Ok(status) = curl_cmd.status() {
                if status.success() && part_exe.exists() {
                    if replace_from_part(&part_exe, &target_exe).is_ok() && target_exe.exists() {
                        return Some(target_exe);
                    }
                }
            }
            let _ = fs::remove_file(&part_exe);

            // 2. Try PowerShell Invoke-WebRequest fallback into .part with -UseBasicParsing (critical for Windows 10)
            let ps_script = format!(
                "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -UseBasicParsing -TimeoutSec 35 -Uri 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' -OutFile '{}'",
                part_exe.display().to_string().replace('\'', "''")
            );
            let mut ps_cmd = Command::new("powershell.exe");
            ps_cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
            ps_cmd.creation_flags(CREATE_NO_WINDOW);
            if let Ok(status) = ps_cmd.status() {
                if status.success() && part_exe.exists() {
                    if replace_from_part(&part_exe, &target_exe).is_ok() && target_exe.exists() {
                        return Some(target_exe);
                    }
                }
            }
            let _ = fs::remove_file(&part_exe);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(user_profile) = env::var("HOME") {
            let target_dir = PathBuf::from(&user_profile).join(".local").join("bin");
            let _ = fs::create_dir_all(&target_dir);
            let target_bin = target_dir.join("yt-dlp");
            let part_bin = target_dir.join("yt-dlp.part");
            let _ = fs::remove_file(&part_bin);

            let mut curl_cmd = Command::new("curl");
            curl_cmd.args([
                "-L",
                "--fail",
                "--connect-timeout",
                "10",
                "--max-time",
                "180",
                "-o",
                part_bin.to_string_lossy().as_ref(),
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp",
            ]);
            if let Ok(status) = curl_cmd.status() {
                if status.success() && part_bin.exists() {
                    let _ = Command::new("chmod").args(["+x", part_bin.to_string_lossy().as_ref()]).status();
                    if fs::rename(&part_bin, &target_bin).is_ok() && target_bin.exists() {
                        return Some(target_bin);
                    }
                }
            }
            let _ = fs::remove_file(&part_bin);
        }
    }

    None
}

fn ensure_deno_available() -> Option<PathBuf> {
    find_local_deno()
}

pub fn bootstrap_deno_in_background(app: AppHandle) {
    #[cfg(target_os = "windows")]
    {
        if find_local_deno().is_none() {
            tauri::async_runtime::spawn(async move {
                let _ = install_engine(app, "deno").await;
            });
        }
    }
}

fn make_command_from_path(resolved: &str) -> Command {
    let mut cmd = Command::new(resolved);
    cmd.env("PATH", get_augmented_path());
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUNBUFFERED", "1");

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd
}

fn make_command(bin_name: &str) -> Command {
    let resolved = if bin_name == "yt-dlp" {
        ensure_ytdlp_available()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| bin_name.to_string())
    } else if bin_name == "ffmpeg" {
        ensure_ffmpeg_available()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| bin_name.to_string())
    } else if bin_name == "deno" {
        ensure_deno_available()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| bin_name.to_string())
    } else if bin_name == "aria2c" {
        find_local_aria2c()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| bin_name.to_string())
    } else {
        find_exe_in_augmented_path(bin_name)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| bin_name.to_string())
    };

    make_command_from_path(&resolved)
}

fn short_codec(c: &str) -> String {
    let cl = c.to_lowercase();
    if cl.starts_with("avc1") || cl.starts_with("h264") {
        "H.264".to_string()
    } else if cl.starts_with("vp09") || cl.starts_with("vp9") {
        "VP9".to_string()
    } else if cl.starts_with("av01") {
        "AV1".to_string()
    } else if cl.starts_with("hev1") || cl.starts_with("hvc1") {
        "H.265".to_string()
    } else if cl.starts_with("mp4a") {
        "AAC".to_string()
    } else if cl.starts_with("opus") {
        "OPUS".to_string()
    } else if cl.starts_with("vorbis") {
        "OGG".to_string()
    } else {
        c.split('.').next().unwrap_or("?").to_uppercase()
    }
}

fn extract_json_size(v: &serde_json::Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        if n > 0 {
            return Some(n);
        }
    }
    if let Some(f) = v.as_f64() {
        if f > 0.0 {
            return Some(f as u64);
        }
    }
    if let Some(s) = v.as_str() {
        if let Ok(n) = s.parse::<u64>() {
            if n > 0 {
                return Some(n);
            }
        }
        if let Ok(f) = s.parse::<f64>() {
            if f > 0.0 {
                return Some(f as u64);
            }
        }
    }
    None
}

fn parse_human_size_to_bytes(s: &str) -> u64 {
    let clean = s.trim().trim_start_matches('~').trim();
    let num_part: String = clean.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let unit_part: String = clean.chars().skip_while(|c| c.is_ascii_digit() || *c == '.').collect();

    let val = match num_part.parse::<f64>() {
        Ok(v) => v,
        Err(_) => return 0,
    };

    let unit = unit_part.trim().to_lowercase();
    if unit.starts_with("gi") || unit.starts_with("gb") {
        (val * 1024.0 * 1024.0 * 1024.0) as u64
    } else if unit.starts_with("mi") || unit.starts_with("mb") {
        (val * 1024.0 * 1024.0) as u64
    } else if unit.starts_with("ki") || unit.starts_with("kb") {
        (val * 1024.0) as u64
    } else if unit.starts_with('b') {
        val as u64
    } else {
        0
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "Live / Dynamic".to_string();
    }
    let mib = bytes as f64 / (1024.0 * 1024.0);
    if mib >= 1024.0 {
        format!("{:.2} GiB", mib / 1024.0)
    } else {
        format!("{:.1} MiB", mib)
    }
}

fn parse_time_to_seconds(t: &str) -> Option<f64> {
    let clean = t.trim().to_lowercase();
    if clean.is_empty() || clean == "inf" {
        return None;
    }
    let parts: Vec<&str> = clean.split(':').collect();
    match parts.len() {
        1 => parts[0].parse::<f64>().ok(),
        2 => {
            let m = parts[0].parse::<f64>().ok()?;
            let s = parts[1].parse::<f64>().ok()?;
            Some(m * 60.0 + s)
        }
        3 => {
            let h = parts[0].parse::<f64>().ok()?;
            let m = parts[1].parse::<f64>().ok()?;
            let s = parts[2].parse::<f64>().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        }
        _ => None,
    }
}

fn format_seconds_to_timestamp(sec: f64) -> String {
    let total_secs = sec.max(0.0).floor() as u64;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn origin_of(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if let Some(scheme_end) = trimmed.find("://") {
        let after_scheme = &trimmed[scheme_end + 3..];
        let host_end = after_scheme
            .find(|c| c == '/' || c == '?' || c == '#')
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..host_end];
        if !authority.is_empty() {
            return format!("{}://{}", &trimmed[..scheme_end], authority);
        }
    }
    trimmed.to_string()
}

fn parse_ffmpeg_duration(line: &str) -> Option<f64> {
    if let Some(idx) = line.find("Duration:") {
        let after = line[idx + 9..].trim_start();
        let token = after.split(|c: char| c == ',' || c.is_whitespace()).next()?;
        if token.eq_ignore_ascii_case("N/A") {
            return None;
        }
        return parse_time_to_seconds(token);
    }
    None
}

fn parse_ffmpeg_progress(line: &str) -> Option<(f64, u64, String)> {
    if let Some(t_idx) = line.find("time=") {
        let after_time = &line[t_idx + 5..];
        let time_token = after_time.split_whitespace().next()?;
        if time_token.eq_ignore_ascii_case("N/A") {
            return None;
        }
        let sec = parse_time_to_seconds(time_token)?;

        let size_bytes = if let Some(sz_idx) = line.find("size=") {
            let after_sz = line[sz_idx + 5..].trim_start();
            let sz_token = after_sz.split_whitespace().next().unwrap_or("");
            if sz_token.eq_ignore_ascii_case("N/A") || sz_token.is_empty() {
                0
            } else {
                parse_human_size_to_bytes(sz_token)
            }
        } else {
            0
        };

        let speed = if let Some(s_idx) = line.find("speed=") {
            let after_speed = line[s_idx + 6..].trim_start();
            let tok = after_speed.split_whitespace().next().unwrap_or("");
            if tok.eq_ignore_ascii_case("N/A") {
                String::new()
            } else {
                tok.to_string()
            }
        } else {
            String::new()
        };

        return Some((sec, size_bytes, speed));
    }
    None
}

fn strip_ansi_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if !ch.is_control() || ch == '\t' {
            out.push(ch);
        }
    }
    out
}

/// Reads child stdout/stderr immediately splitting on both `\n` and `\r` so tools using `\r`
/// carriage-return console updates (aria2c, ffmpeg, yt-dlp) emit real-time progress updates.
fn read_lines_cr_or_lf<R: std::io::Read, F: FnMut(&str) -> bool>(mut reader: R, mut on_line: F) {
    let mut chunk = [0u8; 4096];
    let mut pending: Vec<u8> = Vec::with_capacity(512);
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                for &b in &chunk[..n] {
                    if b == b'\n' || b == b'\r' {
                        if !pending.is_empty() {
                            let raw = String::from_utf8_lossy(&pending);
                            let cleaned = strip_ansi_codes(&raw);
                            let trimmed = cleaned.trim();
                            pending.clear();
                            if !trimmed.is_empty() && !on_line(trimmed) {
                                return;
                            }
                        }
                    } else {
                        pending.push(b);
                    }
                }
            }
            Err(_) => break,
        }
    }
    if !pending.is_empty() {
        let raw = String::from_utf8_lossy(&pending);
        let cleaned = strip_ansi_codes(&raw);
        let trimmed = cleaned.trim();
        if !trimmed.is_empty() {
            let _ = on_line(trimmed);
        }
    }
}

/// Parses aria2c progress lines like `[#2089b0 45.2MiB/168.5MiB(26%) CN:8 DL:5.4MiB ETA:22s]`
/// or `[#2089b0 45.2Mi/168.5Mi(26%) CN:8 DL:5.4Mi ETA:22s]`.
fn parse_aria2c_progress(line: &str) -> Option<(f64, u64, u64, String, String)> {
    if !line.contains('%') || (!line.contains("DL:") && !line.contains("CN:") && !line.starts_with("[#")) {
        return None;
    }

    let mut pct_val: Option<f64> = None;
    let mut down_b: u64 = 0;
    let mut tot_b: u64 = 0;

    for raw_tok in line.split_whitespace() {
        let tok = raw_tok.trim_matches(|c| c == '[' || c == ']');
        if tok.contains('/') && tok.contains('%') {
            if let Some(slash_idx) = tok.find('/') {
                down_b = parse_human_size_to_bytes(&tok[..slash_idx]);
                let after_slash = &tok[slash_idx + 1..];
                if let Some(paren_idx) = after_slash.find('(') {
                    tot_b = parse_human_size_to_bytes(&after_slash[..paren_idx]);
                    let inside = &after_slash[paren_idx + 1..];
                    if let Some(pct_end) = inside.find('%') {
                        if let Ok(p) = inside[..pct_end].trim().parse::<f64>() {
                            pct_val = Some(p);
                        }
                    }
                }
            }
        }
    }

    let pct = pct_val.or_else(|| {
        if tot_b > 0 && down_b > 0 {
            Some(((down_b as f64 / tot_b as f64) * 100.0).clamp(0.0, 100.0))
        } else {
            None
        }
    })?;

    let speed_raw = line
        .split_whitespace()
        .find_map(|tok| {
            let t = tok.trim_matches(|c| c == '[' || c == ']');
            t.strip_prefix("DL:").map(|v| v.to_string())
        })
        .unwrap_or_default();

    let speed_str = if speed_raw.is_empty() {
        String::new()
    } else if speed_raw.ends_with("/s") {
        speed_raw
    } else {
        format!("{}/s", speed_raw)
    };

    let eta_str = line
        .split_whitespace()
        .find_map(|tok| {
            let t = tok.trim_matches(|c| c == '[' || c == ']');
            t.strip_prefix("ETA:").map(|v| v.to_string())
        })
        .unwrap_or_else(|| "--".to_string());

    Some((pct, down_b, tot_b, speed_str, eta_str))
}

#[allow(clippy::too_many_arguments)]
fn emit_unified_media_progress(
    app: &AppHandle,
    media_tasks: &Arc<Mutex<HashMap<String, DownloadTask>>>,
    tid: &str,
    pct: f64,
    downloaded_bytes: u64,
    total_bytes: u64,
    speed_str: &str,
    eta_str: &str,
    last_emit: &mut std::time::Instant,
    force_emit: bool,
) {
    let safe_pct = pct.clamp(0.0, 99.0);
    let speed_bps = parse_human_size_to_bytes(speed_str);
    let clean_speed = if !speed_str.is_empty() {
        speed_str.to_string()
    } else if speed_bps > 0 {
        format!("{}/s", format_bytes(speed_bps))
    } else {
        "Connecting...".to_string()
    };
    let clean_eta = if eta_str.is_empty() || eta_str == "Unknown" || eta_str == "NA" {
        "--".to_string()
    } else {
        eta_str.to_string()
    };

    let effective_total = total_bytes.max(downloaded_bytes);

    if let Ok(mut tasks_lock) = media_tasks.lock() {
        if let Some(t) = tasks_lock.get_mut(tid) {
            t.percent = safe_pct;
            if downloaded_bytes > 0 {
                t.downloaded_bytes = downloaded_bytes;
            }
            if effective_total > 0 {
                t.total_bytes = effective_total;
            }
            if speed_bps > 0 || !speed_str.is_empty() {
                t.speed_bps = speed_bps;
                t.speed_formatted = clean_speed.clone();
            }
            t.eta_formatted = clean_eta.clone();
            t.status = "downloading".to_string();
        }
    }

    let now = std::time::Instant::now();
    if force_emit || now.duration_since(*last_emit).as_millis() >= 200 || safe_pct >= 99.0 {
        *last_emit = now;
        let payload = serde_json::json!({
            "id": tid.to_string(),
            "taskId": tid.to_string(),
            "percent": safe_pct,
            "text": format!("{:.1}% of {} at {}", safe_pct, format_bytes(effective_total), clean_speed),
            "speed": clean_speed.clone(),
            "speed_formatted": clean_speed.clone(),
            "speedFormatted": clean_speed.clone(),
            "speed_bps": speed_bps,
            "speedBps": speed_bps,
            "eta": clean_eta.clone(),
            "eta_formatted": clean_eta.clone(),
            "etaFormatted": clean_eta.clone(),
            "total_bytes": effective_total,
            "totalBytes": effective_total,
            "downloaded_bytes": downloaded_bytes,
            "downloadedBytes": downloaded_bytes,
            "status": "downloading".to_string(),
        });
        let _ = app.emit("download-progress", payload);
    }
}

/// Per-task throughput tracker for ffmpeg-based (clip / HLS-via-ffmpeg) progress lines:
/// (time of last sample, output size at last sample, smoothed bytes/sec).
static FFMPEG_RATE_TRACK: std::sync::OnceLock<Mutex<HashMap<String, (Instant, u64, u64)>>> =
    std::sync::OnceLock::new();

/// Real transfer speed (bytes/sec) from successive ffmpeg `size=` values. ffmpeg itself only
/// reports a media-time multiplier ("0.92x"), which is not a download speed.
fn ffmpeg_real_speed_bps(tid: &str, size_bytes: u64) -> u64 {
    if size_bytes == 0 {
        return 0;
    }
    let now = Instant::now();
    let map = FFMPEG_RATE_TRACK.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = match map.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let entry = guard
        .entry(tid.to_string())
        .or_insert((now, size_bytes, 0u64));
    if size_bytes < entry.1 {
        // Output restarted from scratch (new ffmpeg run): reset the sample window.
        *entry = (now, size_bytes, 0u64);
        return 0;
    }
    let dt = now.duration_since(entry.0).as_secs_f64();
    if dt >= 1.0 {
        let inst = ((size_bytes - entry.1) as f64 / dt) as u64;
        entry.2 = if entry.2 == 0 {
            inst
        } else {
            (entry.2 as f64 * 0.6 + inst as f64 * 0.4) as u64
        };
        entry.0 = now;
        entry.1 = size_bytes;
    }
    entry.2
}

#[allow(clippy::too_many_arguments)]
fn emit_ffmpeg_progress_update(
    app: &AppHandle,
    media_tasks: &Arc<Mutex<HashMap<String, DownloadTask>>>,
    tid: &str,
    sec: f64,
    size_bytes: u64,
    speed: &str,
    target_duration: Option<f64>,
    last_percent: &Arc<Mutex<f64>>,
    is_clipping: bool,
) {
    let pct = {
        let mut lp = last_percent.lock().unwrap();
        if let Some(dur) = target_duration.filter(|&d| d > 0.0) {
            let computed = ((sec / dur) * 100.0).clamp(2.0, 99.5);
            *lp = computed;
            computed
        } else {
            if *lp < 2.0 {
                *lp = 2.0;
            }
            *lp
        }
    };

    let elapsed_str = format_seconds_to_timestamp(sec);
    let speed_disp = if speed.is_empty() { "1x" } else { speed };
    let text = if let Some(dur) = target_duration.filter(|&d| d > 0.0) {
        if is_clipping {
            format!(
                "Cutting & Encoding: {:.1}% ({}/{} at {})",
                pct,
                elapsed_str,
                format_seconds_to_timestamp(dur),
                speed_disp
            )
        } else {
            format!(
                "Downloading HLS: {:.1}% ({}/{} at {})",
                pct,
                elapsed_str,
                format_seconds_to_timestamp(dur),
                speed_disp
            )
        }
    } else if is_clipping {
        format!(
            "Cutting media: {} ({} at {})",
            elapsed_str,
            format_bytes(size_bytes),
            speed_disp
        )
    } else {
        format!(
            "Downloading HLS: {} ({} at {})",
            elapsed_str,
            format_bytes(size_bytes),
            speed_disp
        )
    };

    // Real MB/s from output growth; falls back to ffmpeg's own multiplier only when no size is known.
    let speed_bps = ffmpeg_real_speed_bps(tid, size_bytes);
    let speed_out: String = if speed_bps > 0 {
        crate::downloader::format_speed(speed_bps)
    } else if size_bytes > 0 {
        "Calculating...".to_string()
    } else {
        speed.to_string()
    };

    // Estimated final size for a time-bounded clip: bytes so far / fraction of the clip done.
    let mut est_total: u64 = 0;
    let mut eta_out = String::from("--");
    if let Some(dur) = target_duration.filter(|&d| d > 0.0) {
        let frac = (sec / dur).clamp(0.0, 1.0);
        if size_bytes > 0 && frac >= 0.02 {
            est_total = ((size_bytes as f64) / frac) as u64;
        }
        let mult = speed
            .trim()
            .trim_end_matches('x')
            .parse::<f64>()
            .ok()
            .filter(|m| *m > 0.01);
        if let Some(m) = mult {
            let remaining_secs = (dur - sec).max(0.0) / m;
            if remaining_secs.is_finite() {
                eta_out = crate::downloader::format_eta(remaining_secs as u64);
            }
        }
    }

    let safe_pct = pct.clamp(0.0, 99.0);
    if let Ok(mut tasks_lock) = media_tasks.lock() {
        if let Some(t) = tasks_lock.get_mut(tid) {
            t.percent = safe_pct;
            if size_bytes > 0 {
                t.downloaded_bytes = size_bytes;
            }
            if est_total > 0 {
                t.total_bytes = est_total.max(size_bytes);
            }
            if !speed_out.is_empty() {
                t.speed_formatted = speed_out.clone();
            }
            t.speed_bps = speed_bps;
            t.eta_formatted = eta_out.clone();
            t.status = "downloading".to_string();
        }
    }

    // null (not 0) while the size is still unknown, so the UI keeps any previous estimate.
    let total_json = if est_total > 0 {
        serde_json::json!(est_total)
    } else {
        serde_json::Value::Null
    };
    let payload = serde_json::json!({
        "id": tid,
        "taskId": tid,
        "percent": safe_pct,
        "text": text,
        "speed": speed_out,
        "speed_formatted": speed_out,
        "speedFormatted": speed_out,
        "speed_bps": speed_bps,
        "speedBps": speed_bps,
        "downloaded_bytes": size_bytes,
        "downloadedBytes": size_bytes,
        "total_bytes": total_json,
        "totalBytes": total_json,
        "eta": eta_out,
        "eta_formatted": eta_out,
        "etaFormatted": eta_out,
        "status": "downloading",
    });
    let _ = app.emit("download-progress", payload.clone());
    let _ = app.emit("ytdlp-progress", payload);
}

// ---------------------------------------------------------------------------
// Command 1: Fetch Media Info (Parallel Attempts + 10-Min Cache)
// ---------------------------------------------------------------------------

static INFLIGHT_INSPECTS: std::sync::OnceLock<
    std::sync::Mutex<HashMap<String, tokio::sync::watch::Receiver<bool>>>,
> = std::sync::OnceLock::new();

struct InflightInspectGuard {
    key: String,
    tx: tokio::sync::watch::Sender<bool>,
}

impl Drop for InflightInspectGuard {
    fn drop(&mut self) {
        if let Some(lock) = INFLIGHT_INSPECTS.get() {
            if let Ok(mut guard) = lock.lock() {
                guard.remove(&self.key);
            }
        }
        let _ = self.tx.send(true);
    }
}

/// Normalize YouTube watch/short links to a canonical `watch?v=<id>` URL so mix/playlist/timestamp
/// params don't bypass the format cache or force extra playlist extraction overhead.
pub fn canonicalize_inspect_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();
    if let Some(idx) = lower.find("youtu.be/") {
        let rest = &trimmed[idx + 9..];
        let vid = rest.split(['?', '#', '&', '/']).next().unwrap_or("").trim();
        if !vid.is_empty() {
            return format!("https://www.youtube.com/watch?v={}", vid);
        }
    }
    if lower.contains("youtube.com/watch") {
        if let Some(q_idx) = trimmed.find('?') {
            let query = &trimmed[q_idx + 1..];
            for pair in query.split('#').next().unwrap_or(query).split('&') {
                if let Some(vid) = pair.strip_prefix("v=") {
                    let clean_vid = vid.trim();
                    if !clean_vid.is_empty() {
                        return format!("https://www.youtube.com/watch?v={}", clean_vid);
                    }
                }
            }
        }
    }
    // Normalize raw Kick Amazon IVS stream directory URL to its master playlist so yt-dlp
    // treats it as an HLS stream instead of trying to fetch an HTML webpage
    if lower.contains("stream.kick.com") && lower.contains("/ivs/v1/") {
        if trimmed.ends_with('/') {
            return format!("{}master.m3u8", trimmed);
        }
    }
    trimmed.to_string()
}

pub fn is_raw_stream_cdn(url: &str) -> bool {
    let low = url.to_lowercase();
    low.contains("stream.kick.com")
        || low.contains("/ivs/")
        || low.contains(".m3u8")
        || low.contains(".mpd")
        || low.contains("googlevideo.com")
        || low.contains("cdninstagram.com")
        || low.contains("fbcdn.net")
        || low.contains("tiktokcdn.com")
        || low.contains("byteoversea.com")
        || low.contains("ibyteimg.com")
}

fn is_cookie_db_error(msg: &str) -> bool {
    let low = msg.to_lowercase();
    low.contains("could not copy")
        || low.contains("cookie")
        || low.contains("dpapi")
        || low.contains("sqlite")
        || low.contains("decrypt")
}

async fn run_ytdlp_inspect_once(
    target_url: String,
    extractor_arg: Option<&'static str>,
    cookies_browser: Option<String>,
    cookies_file: Option<String>,
    proxy: Option<String>,
    discovery_args: Option<String>,
) -> Result<std::process::Output, String> {
    let is_raw_cdn = is_raw_stream_cdn(&target_url);
    let has_browser_cookie = !is_raw_cdn
        && cookies_browser
            .as_deref()
            .map(|b| b != "None" && !b.trim().is_empty())
            .unwrap_or(false);

    for attempt_without_browser in [false, true] {
        if attempt_without_browser && !has_browser_cookie {
            break;
        }
        let u = target_url.clone();
        let cb = if attempt_without_browser || is_raw_cdn {
            None
        } else {
            cookies_browser.clone()
        };
        let cf = cookies_file.clone();
        let pr = proxy.clone();
        let da = discovery_args.clone();

        let output = tauri::async_runtime::spawn_blocking(move || {
            // Same lean invocation as GM Downloader: plain --dump-single-json, no hard kill timeout.
            // yt-dlp's own socket timeout stops dead connections; slow-but-working inspects are allowed to finish.
            let mut cmd = make_command("yt-dlp");
            cmd.args([
                "--dump-single-json",
                "--no-playlist",
                "--no-check-certificates",
                "--socket-timeout",
                "20",
            ]);

            let u_low = u.to_lowercase();
            let is_yt = u_low.contains("youtube.com") || u_low.contains("youtu.be");
            if is_yt {
                cmd.args(["--no-warnings", "--no-write-comments"]);
                if find_exe_in_augmented_path("deno").is_some() || ensure_deno_available().is_some() {
                    cmd.args(["--js-runtimes", "deno"]);
                } else if find_exe_in_augmented_path("node").is_some() {
                    cmd.args(["--js-runtimes", "node"]);
                }
            }

            if let Some(arg) = extractor_arg {
                cmd.args(["--extractor-args", arg]);
            }

            if let Some(ref browser) = cb {
                if browser != "None" && !browser.trim().is_empty() {
                    cmd.args(["--cookies-from-browser", &browser.to_lowercase()]);
                }
            }
            if let Some(ref file) = cf {
                if !file.trim().is_empty() && PathBuf::from(file).exists() {
                    cmd.args(["--cookies", file]);
                }
            }
            if let Some(ref proxy_url) = pr {
                if !proxy_url.trim().is_empty() {
                    cmd.args(["--proxy", proxy_url.trim()]);
                }
            }
            if let Some(ref extra) = da {
                for arg in extra.split_whitespace() {
                    if !arg.is_empty() {
                        cmd.arg(arg);
                    }
                }
            }

            cmd.arg(&u);
            cmd.stdin(Stdio::null());

            // output() drains stdout/stderr while waiting, so large JSON can no longer deadlock the pipe
            cmd.output().map_err(|e| format!("Could not launch yt-dlp: {}", e))
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))??;

        if output.status.success() {
            return Ok(output);
        }

        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}\n{}", err.trim(), out.trim());
        if !attempt_without_browser && has_browser_cookie && is_cookie_db_error(&combined) {
            continue;
        }

        let combined_low = combined.to_lowercase();
        let friendly_err = if combined_low.contains("the channel is not currently live")
            || combined_low.contains("stream is offline")
            || combined_low.contains("offline")
            || (combined_low.contains("403") && (target_url.contains("kick.com") || target_url.contains("ivs")))
        {
            "This live stream is currently offline or has ended (HTTP 403 Forbidden). Please try again when the stream is live.".to_string()
        } else if combined_low.contains("403") && combined_low.contains("forbidden") {
            "Access forbidden (HTTP 403). The stream might be offline, restricted, or expired.".to_string()
        } else if combined_low.contains("404") && combined_low.contains("not found") {
            "Media stream not found (HTTP 404). The stream or video may have ended or been deleted.".to_string()
        } else if combined_low.contains("timed out") || combined_low.contains("timeout") {
            "Stream inspection timed out (server did not respond in time).".to_string()
        } else if combined.trim().is_empty() {
            "Failed to fetch metadata. Please verify the link or network connection.".to_string()
        } else {
            let mut best_line = "";
            for line in combined.lines().rev() {
                let trimmed = line.trim();
                if trimmed.starts_with("ERROR:") {
                    best_line = trimmed.strip_prefix("ERROR:").unwrap_or(trimmed).trim();
                    break;
                }
            }
            if !best_line.is_empty() {
                best_line.to_string()
            } else {
                combined.trim().chars().take(250).collect()
            }
        };

        return Err(friendly_err);
    }

    Err("Failed to inspect stream.".to_string())
}

#[command]
pub async fn fetch_media_info(
    app: AppHandle,
    url: String,
    cookies_browser: Option<String>,
    cookies_file: Option<String>,
    proxy: Option<String>,
    discovery_args: Option<String>,
) -> Result<MediaInfo, String> {
    fetch_media_info_with_app(
        Some(app),
        url,
        cookies_browser,
        cookies_file,
        proxy,
        discovery_args,
    )
    .await
}

pub async fn fetch_media_info_with_app(
    app: Option<AppHandle>,
    url: String,
    cookies_browser: Option<String>,
    cookies_file: Option<String>,
    proxy: Option<String>,
    discovery_args: Option<String>,
) -> Result<MediaInfo, String> {
    let raw_trimmed = url.trim();
    if raw_trimmed.is_empty() {
        return Err("URL cannot be empty".to_string());
    }
    let clean_url = canonicalize_inspect_url(raw_trimmed);

    // Return cached result if fresh (< 10 minutes)
    if let Ok(mut guard) = FORMATS_CACHE.lock() {
        if let Some(map) = guard.as_mut() {
            map.retain(|_, (ts, _)| ts.elapsed() < Duration::from_secs(600));
            if let Some((ts, cached)) = map.get(&clean_url) {
                if ts.elapsed() < Duration::from_secs(600) {
                    return Ok(cached.clone());
                }
            }
        }
    }

    // Deduplicate concurrent inspections for the same canonical URL so multiple callers
    // (prefetch + click + modal) await one yt-dlp process instead of spawning extras.
    let existing_rx = {
        let lock = INFLIGHT_INSPECTS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
        lock.lock()
            .ok()
            .and_then(|map| map.get(&clean_url).cloned())
    };
    if let Some(mut rx) = existing_rx {
        if !*rx.borrow() {
            let _ = tokio::time::timeout(Duration::from_secs(600), rx.changed()).await;
        }
        if let Ok(guard) = FORMATS_CACHE.lock() {
            if let Some(map) = guard.as_ref() {
                if let Some((ts, cached)) = map.get(&clean_url) {
                    if ts.elapsed() < Duration::from_secs(600) {
                        return Ok(cached.clone());
                    }
                }
            }
        }
    }

    let _inflight_guard = {
        let (tx, rx) = tokio::sync::watch::channel(false);
        let lock = INFLIGHT_INSPECTS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
        if let Ok(mut map) = lock.lock() {
            map.insert(clean_url.clone(), rx);
        }
        InflightInspectGuard {
            key: clean_url.clone(),
            tx,
        }
    };

    // Inherit saved cookies & proxy from SettingsState when callers pass None/"None"
    let saved_settings = app
        .as_ref()
        .and_then(|a| a.try_state::<crate::settings::SettingsState>())
        .map(|s| s.get_blocking());

    let eff_cookies_browser = cookies_browser
        .filter(|b| !b.trim().is_empty() && b != "None")
        .or_else(|| {
            saved_settings
                .as_ref()
                .map(|s| s.cookies_browser.clone())
                .filter(|b| !b.trim().is_empty() && b != "None")
        });

    let eff_cookies_file = cookies_file
        .filter(|f| !f.trim().is_empty())
        .or_else(|| {
            saved_settings
                .as_ref()
                .map(|s| s.cookies_file.clone())
                .filter(|f| !f.trim().is_empty())
        });

    let eff_proxy = proxy
        .filter(|p| !p.trim().is_empty())
        .or_else(|| saved_settings.as_ref().and_then(|s| s.effective_proxy_url()));

    // If Deno/Node is available, default extractor returns ALL qualities (including 1080p Premium) in ~2s.
    // When Deno is missing (e.g. fresh Windows 10 laptop), default web client stalls for ~60s;
    // so we use android,web first (6s) while triggering Deno download in the background.
    let url_low = clean_url.to_lowercase();
    let is_youtube = url_low.contains("youtube.com") || url_low.contains("youtu.be");
    let has_js_runtime = find_exe_in_augmented_path("deno").is_some()
        || ensure_deno_available().is_some()
        || find_exe_in_augmented_path("node").is_some();

    if is_youtube && !has_js_runtime {
        if let Some(ref a) = app {
            bootstrap_deno_in_background(a.clone());
        }
    }

    let attempts: Vec<Option<&'static str>> = if is_youtube {
        if has_js_runtime {
            vec![
                None,
                Some("youtube:player_client=tv,web"),
                Some("youtube:player_client=android,web"),
            ]
        } else {
            vec![
                Some("youtube:player_client=android,web"),
                Some("youtube:player_client=tv,web"),
                None,
            ]
        }
    } else {
        vec![None]
    };

    let mut output_opt: Option<std::process::Output> = None;
    let mut last_err: Option<String> = None;
    for extractor_arg in attempts {
        match run_ytdlp_inspect_once(
            clean_url.clone(),
            extractor_arg,
            eff_cookies_browser.clone(),
            eff_cookies_file.clone(),
            eff_proxy.clone(),
            discovery_args.clone(),
        )
        .await
        {
            Ok(out) => {
                output_opt = Some(out);
                break;
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
    }

    let output = output_opt.ok_or_else(|| {
        last_err.unwrap_or_else(|| {
            "Failed to fetch metadata. Please verify the link or network connection.".to_string()
        })
    })?;

    let info = tauri::async_runtime::spawn_blocking(move || -> Result<MediaInfo, String> {

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stdout_trimmed = stdout_str.trim();

        // Robust JSON slice extraction between first '{' and last '}'
        let json_slice = if let (Some(start), Some(end)) = (stdout_trimmed.find('{'), stdout_trimmed.rfind('}')) {
            if start < end {
                &stdout_trimmed[start..=end]
            } else {
                stdout_trimmed
            }
        } else {
            stdout_trimmed
        };

        let v: serde_json::Value = serde_json::from_str(json_slice)
            .map_err(|e| format!("Failed to parse media JSON: {} (Output was: {})", e, stdout_trimmed.chars().take(200).collect::<String>()))?;

        let id = v["id"].as_str().map(|s| s.to_string());
        let title = v["title"].as_str().unwrap_or("Untitled Media").to_string();
        let uploader = v["uploader"]
            .as_str()
            .or_else(|| v["channel"].as_str())
            .map(|s| s.to_string());
        let duration = v["duration"].as_f64();
        let thumbnail = v["thumbnail"].as_str().map(|s| s.to_string());

        let mut video_raw: Vec<(u32, u32, u64, MediaStream)> = Vec::new();
        let mut audio_raw: Vec<(u32, u64, MediaStream)> = Vec::new();

        if let Some(formats) = v["formats"].as_array() {
            for f in formats {
                let fid = f["format_id"].as_str().unwrap_or("").to_string();
                let ext = f["ext"].as_str().unwrap_or("").to_string();
                let format_note = f["format_note"].as_str().unwrap_or("");
                if format_note == "storyboard" || ext == "mhtml" {
                    continue;
                }

                let vcodec = f["vcodec"].as_str().unwrap_or("none");
                let acodec = f["acodec"].as_str().unwrap_or("none");
                let mut size = extract_json_size(&f["filesize"])
                    .or_else(|| extract_json_size(&f["filesize_approx"]))
                    .unwrap_or(0);

                if size == 0 {
                    if let Some(dur) = duration {
                        if dur > 0.0 {
                            let br = f["tbr"]
                                .as_f64()
                                .or_else(|| {
                                    let v = f["vbr"].as_f64().unwrap_or(0.0);
                                    let a = f["acodec"].as_str().map(|_| f["abr"].as_f64().unwrap_or(128.0)).unwrap_or(0.0);
                                    if v + a > 0.0 { Some(v + a) } else { None }
                                });
                            if let Some(bitrate_kbps) = br {
                                if bitrate_kbps > 0.0 {
                                    size = ((bitrate_kbps * 1000.0 / 8.0) * dur) as u64;
                                }
                            }
                        }
                    }
                }

                if size == 0 {
                    size = extract_json_size(&v["filesize"])
                        .or_else(|| extract_json_size(&v["filesize_approx"]))
                        .unwrap_or(0);
                }

                if vcodec != "none" {
                    let h = f["height"].as_u64().unwrap_or(0) as u32;
                    let w = f["width"].as_u64().unwrap_or(0) as u32;
                    let fps = f["fps"].as_f64().unwrap_or(0.0).round() as u32;
                    let tbr = f["tbr"].as_f64().unwrap_or(0.0).round() as u64;
                    let has_audio = acodec != "none";
                    let note_low = format_note.to_lowercase();
                    let is_premium = note_low.contains("premium") || fid == "616" || fid == "356";
                    let res_tag = if h > 0 && is_premium {
                        format!("{}p (Premium)", h)
                    } else if h >= 2160 {
                        format!("{}p (4K UHD)", h)
                    } else if h >= 1440 {
                        format!("{}p (2K QHD)", h)
                    } else if h >= 1080 {
                        format!("{}p (Full HD)", h)
                    } else if h >= 720 {
                        format!("{}p (HD)", h)
                    } else if h > 0 {
                        format!("{}p", h)
                    } else if w > 0 {
                        format!("{}x{}", w, h)
                    } else {
                        format_note.to_string()
                    };

                    let est_size = if size > 0 {
                        size
                    } else if let Some(dur) = duration.filter(|d| *d > 0.0) {
                        if tbr > 0 {
                            (((tbr * 1000) / 8) as f64 * dur).round() as u64
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    let size_badge = if size > 0 {
                        format_bytes(size)
                    } else if est_size > 0 {
                        format!("~{}", format_bytes(est_size))
                    } else {
                        format_bytes(0)
                    };
                    let audio_badge = if has_audio { " [+Audio]" } else { "" };
                    let label = format!(
                        "{} • {} • {}fps • {} • {}{} (ID: {})",
                        res_tag,
                        short_codec(vcodec),
                        fps,
                        ext.to_uppercase(),
                        size_badge,
                        audio_badge,
                        fid
                    );

                    video_raw.push((
                        h,
                        fps,
                        tbr,
                        MediaStream {
                            id: fid,
                            label,
                            height: Some(h),
                            fps: Some(fps),
                            ext: Some(ext),
                            vcodec: Some(short_codec(vcodec)),
                            acodec: Some(short_codec(acodec)),
                            filesize: if est_size > 0 { Some(est_size) } else { None },
                            has_audio: Some(has_audio),
                            format_note: if format_note.trim().is_empty() {
                                None
                            } else {
                                Some(format_note.to_string())
                            },
                            is_premium: Some(is_premium),
                        },
                    ));
                } else if acodec != "none" {
                    let abr = f["abr"]
                        .as_f64()
                        .or_else(|| f["tbr"].as_f64())
                        .unwrap_or(0.0)
                        .round() as u32;
                    let lang = f["language"].as_str().unwrap_or("und");
                    let label = format!(
                        "{} kbps • {} • {} • {} • {} (ID: {})",
                        abr,
                        short_codec(acodec),
                        lang.to_uppercase(),
                        ext.to_uppercase(),
                        format_bytes(size),
                        fid
                    );

                    audio_raw.push((
                        abr,
                        size,
                        MediaStream {
                            id: fid,
                            label,
                            height: None,
                            fps: None,
                            ext: Some(ext),
                            vcodec: None,
                            acodec: Some(short_codec(acodec)),
                            filesize: if size > 0 { Some(size) } else { None },
                            has_audio: Some(true),
                            format_note: if format_note.trim().is_empty() {
                                None
                            } else {
                                Some(format_note.to_string())
                            },
                            is_premium: Some(false),
                        },
                    ));
                }
            }
        }

        // Sort highest quality first, ranking Premium above standard at the same height
        video_raw.sort_by(|a, b| {
            let ap = if a.3.is_premium.unwrap_or(false) { 1u8 } else { 0u8 };
            let bp = if b.3.is_premium.unwrap_or(false) { 1u8 } else { 0u8 };
            (b.0, bp, b.1, b.2).cmp(&(a.0, ap, a.1, a.2))
        });
        audio_raw.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));

        let video_streams = video_raw.into_iter().map(|x| x.3).collect();
        let audio_streams = audio_raw.into_iter().map(|x| x.2).collect();

        // Parse subtitles
        let mut subtitles: Vec<SubtitleTrack> = Vec::new();
        if let Some(subs_obj) = v["subtitles"].as_object() {
            for (lang_code, tracks) in subs_obj {
                if lang_code == "live_chat" {
                    continue;
                }
                let name = tracks
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|t| t["name"].as_str())
                    .unwrap_or(lang_code)
                    .to_string();
                subtitles.push(SubtitleTrack {
                    code: lang_code.clone(),
                    name: format!("{} ({})", name, lang_code),
                    is_auto: false,
                });
            }
        }
        if let Some(auto_obj) = v["automatic_captions"].as_object() {
            // Pick popular auto-caption languages first so dropdown stays clean
            let priority_langs = ["en", "bn", "hi", "ar", "es", "fr", "de", "ja", "ko", "pt", "ru", "zh-Hans"];
            for code in priority_langs {
                if let Some(tracks) = auto_obj.get(code) {
                    if !subtitles.iter().any(|s| s.code == code) {
                        let name = tracks
                            .as_array()
                            .and_then(|arr| arr.first())
                            .and_then(|t| t["name"].as_str())
                            .unwrap_or(code);
                        subtitles.push(SubtitleTrack {
                            code: code.to_string(),
                            name: format!("{} [Auto-Generated] ({})", name, code),
                            is_auto: true,
                        });
                    }
                }
            }
        }

        Ok(MediaInfo {
            id,
            title,
            uploader,
            duration,
            thumbnail,
            video_streams,
            audio_streams,
            subtitles,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    if let Ok(mut guard) = FORMATS_CACHE.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(clean_url, (Instant::now(), info.clone()));
    }

    Ok(info)
}

// ---------------------------------------------------------------------------
// Command 2: Start Download (Video, Audio, Subtitles Only, Clipping, Parabolic)
// ---------------------------------------------------------------------------

fn strip_media_extension_for_stem(raw: &str) -> String {
    let trimmed = Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(raw)
        .trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let known_exts = [
        ".mp4", ".mkv", ".webm", ".ts", ".flv", ".mov", ".avi", ".m4a", ".mp3", ".aac", ".opus",
        ".ogg", ".flac", ".wav", ".srt", ".vtt", ".ass", ".m3u8", ".part", ".ytdl",
    ];
    let mut current = trimmed.to_string();
    loop {
        let lower = current.to_lowercase();
        if let Some(ext) = known_exts.iter().find(|e| lower.ends_with(**e)) {
            current.truncate(current.len() - ext.len());
        } else {
            break;
        }
    }
    current.trim().to_string()
}

pub use crate::clipper::{is_audio_format_file, multi_phase_percent};

/// Compatibility wrapper for HLS phase scaling.
#[allow(dead_code)]
fn hls_phase_percent(raw: f64, idx: usize, total: usize) -> f64 {
    multi_phase_percent(raw, idx, total, false)
}

/// Number of formats yt-dlp announces, e.g. `[info] abc: Downloading 1 format(s): 1080p+audio`.
fn parse_format_count(line: &str) -> Option<usize> {
    if !line.starts_with("[info]") {
        return None;
    }
    let idx = line.find("format(s):")?;
    let list = line[idx + "format(s):".len()..].trim();
    if list.is_empty() {
        return None;
    }
    Some(list.matches('+').count() + 1)
}

/// Path yt-dlp reports when it starts (or skips, because it is already complete) one format file:
/// `[download] Destination: X.f137.mp4` / `[download] X.f137.mp4 has already been downloaded`.
fn format_file_from_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("[download]")?.trim();
    let path = if let Some(p) = rest.strip_prefix("Destination:") {
        p.trim()
    } else if let Some(p) = rest.strip_suffix("has already been downloaded") {
        p.trim()
    } else {
        return None;
    };
    if path.is_empty() {
        return None;
    }
    Some(path.trim_matches('"').to_string())
}

/// (bytes of all downloaded format files, bytes of the biggest `*.temp.*` merge output) for a stem.
fn merge_progress_bytes(save_folder: &str, stem: &str) -> (u64, u64) {
    let mut src: u64 = 0;
    let mut out: u64 = 0;
    let Ok(entries) = fs::read_dir(save_folder) else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let low = name.to_lowercase();
        if !name_matches_stem(&low, stem) {
            continue;
        }
        let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if low.contains(".temp.") {
            out = out.max(len);
        } else if is_ytdlp_format_fragment_name(&low) {
            src = src.saturating_add(len);
        }
    }
    (src, out)
}

/// yt-dlp prints nothing while ffmpeg merges / fixes up a multi-GB file, so the UI looked frozen
/// ("stuck") at 99%. This heartbeat reports how much of the merged file is written until `stop`.
fn spawn_merge_heartbeat(
    app: AppHandle,
    tid: String,
    save_folder: String,
    stem: String,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            let (src, out) = merge_progress_bytes(&save_folder, &stem);
            let secs = started.elapsed().as_secs();
            let (pct, text) = if src > 0 && out > 0 {
                let ratio = (out as f64 / src as f64).clamp(0.0, 1.0);
                (
                    99.0 + ratio * 0.9,
                    format!(
                        "Merging video + audio: {} of about {} written",
                        format_bytes(out),
                        format_bytes(src)
                    ),
                )
            } else {
                (99.0, format!("Merging video + audio... {}s", secs))
            };
            let payload = serde_json::json!({
                "id": tid.clone(),
                "taskId": tid.clone(),
                "percent": pct,
                "text": text,
                "speed": "",
                "speed_formatted": "Merging...",
                "speedFormatted": "Merging...",
                "eta": "--",
                "eta_formatted": "--",
                "etaFormatted": "--",
                "status": "assembling",
            });
            let _ = app.emit("download-progress", payload);
            for _ in 0..8 {
                if stop.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(125));
            }
        }
    });
}

fn is_ytdlp_format_fragment_name(name_lower: &str) -> bool {
    for (idx, _) in name_lower.match_indices(".f") {
        let after = &name_lower[idx + 2..];
        if let Some(dot_pos) = after.find('.') {
            let fmt_id = &after[..dot_pos];
            let has_trailing_ext = dot_pos + 1 < after.len();
            if has_trailing_ext && !fmt_id.is_empty() && fmt_id.len() <= 24 {
                let is_numeric_id = fmt_id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b'-' || b == b'_')
                    && fmt_id.bytes().any(|b| b.is_ascii_digit());
                let is_stream_id = (fmt_id.starts_with("hls") || fmt_id.starts_with("dash"))
                    && fmt_id.bytes().any(|b| b.is_ascii_digit());
                if is_numeric_id || is_stream_id {
                    return true;
                }
            }
        }
    }
    false
}

fn is_partial_media_filename(name_lower: &str) -> bool {
    name_lower.ends_with(".part")
        || name_lower.ends_with(".ytdl")
        || name_lower.contains(".part-frag")
        || name_lower.contains(".temp.")
        || is_ytdlp_format_fragment_name(name_lower)
}

/// True when `name_lower` starts with the alphanumeric characters of `hint` AND the very next
/// character is a real boundary (end of name, a `.` such as `.mp4.part` / `.f137.mp4` / `.temp.mp4`,
/// or, optionally, yt-dlp's ` [id]` suffix). This stops "Highlights" from matching the unrelated
/// "Highlights Day 2 [abc].mp4.part" of another task in the same folder.
fn stem_boundary_match(name_lower: &str, hint: &str, allow_id_suffix: bool) -> bool {
    let hint_chars: Vec<char> = hint.chars().collect();
    let hint_len = hint_chars.len();
    if hint_len == 0 {
        return false;
    }
    let mut matched = 0usize;
    let mut end_idx: Option<usize> = None;
    for (i, c) in name_lower.char_indices() {
        if matched == hint_len {
            end_idx = Some(i);
            break;
        }
        if c.is_alphanumeric() {
            if hint_chars[matched] == c {
                matched += 1;
            } else {
                return false;
            }
        }
    }
    let rest = match end_idx {
        Some(i) => &name_lower[i..],
        None => {
            if matched == hint_len {
                ""
            } else {
                return false;
            }
        }
    };
    // closing brackets/quotes can be the tail of the stem itself, e.g. stem "Live [abc]"
    let rest = rest.trim_start_matches(|c: char| matches!(c, ']' | ')' | '}' | '\'' | '"' | '!' | '?'));
    rest.is_empty() || rest.starts_with('.') || (allow_id_suffix && rest.starts_with(" ["))
}

fn name_matches_stem(name_lower: &str, stem: &str) -> bool {
    let base_stem = strip_media_extension_for_stem(stem);
    if base_stem.is_empty() {
        return false;
    }
    let stem_lower = base_stem.to_lowercase();
    let clean_hint: String = stem_lower.chars().filter(|c| c.is_alphanumeric()).collect();
    if clean_hint.len() < 3 {
        return false;
    }
    let stem_has_id = base_stem.contains(" [");
    let base_no_id = if let Some(pos) = base_stem.rfind(" [") {
        base_stem[..pos].trim().to_string()
    } else {
        base_stem.clone()
    };
    let clean_base_no_id: String = base_no_id
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    // A stem without an id may match files that carry ` [id]`; a stem WITH an id must not match
    // another task's ` [otherid]` file.
    stem_boundary_match(name_lower, &clean_hint, !stem_has_id)
        || (clean_base_no_id.len() >= 3 && stem_boundary_match(name_lower, &clean_base_no_id, false))
}

fn is_readable_nonempty_file(path: &Path) -> bool {
    use std::io::Read;
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 {
        return false;
    }
    let Ok(mut f) = fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 1];
    matches!(f.read(&mut buf), Ok(n) if n > 0)
}

/// Removes partial artifacts (`*.part`, `*.ytdl`, `*.part-Frag*`, `*.temp.*`, `*.f<id>.*`)
/// as well as 0-byte or unreadable final files matching `stem` inside `save_folder`.
/// If `stem` is empty/too short to identify a task, NOTHING is deleted (it used to wipe every
/// partial file in the folder, including other running downloads).
pub fn clean_partial_files(save_folder: &str, stem: &str) {
    if strip_media_extension_for_stem(stem).len() < 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(save_folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name_lower = name.to_lowercase();
        if !name_matches_stem(&name_lower, stem) {
            continue;
        }
        if is_partial_media_filename(&name_lower) {
            let _ = fs::remove_file(&path);
        } else if !name_lower.starts_with(".zr_sparse_meta_") && !is_readable_nonempty_file(&path) {
            // Delete 0-byte or unreadable final file for this output stem
            let _ = fs::remove_file(&path);
        }
    }
}

/// Removes ONLY the leftovers of a failed post-processing step (ffmpeg `*.temp.*` output and
/// 0-byte/unreadable finals). The downloaded data (`*.part`, `*.ytdl`, `*.part-Frag*`,
/// `*.f<id>.*`) is deliberately kept so a retry can continue instead of starting at 0%.
pub fn clean_postprocess_leftovers(save_folder: &str, stem: &str) {
    if strip_media_extension_for_stem(stem).len() < 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(save_folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name_lower = name.to_lowercase();
        if !name_matches_stem(&name_lower, stem) {
            continue;
        }
        if name_lower.contains(".temp.") {
            let _ = fs::remove_file(&path);
        } else if !is_partial_media_filename(&name_lower)
            && !name_lower.starts_with(".zr_sparse_meta_")
            && !is_readable_nonempty_file(&path)
        {
            let _ = fs::remove_file(&path);
        }
    }
}

/// True if any non-empty partial/fragment file for this stem exists (i.e. a retry can resume).
pub fn has_partial_data(save_folder: &str, stem: &str) -> bool {
    if strip_media_extension_for_stem(stem).len() < 3 {
        return false;
    }
    let Ok(entries) = fs::read_dir(save_folder) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name_lower = name.to_lowercase();
        if is_partial_media_filename(&name_lower)
            && !name_lower.contains(".temp.")
            && name_matches_stem(&name_lower, stem)
        {
            if let Ok(meta) = entry.metadata() {
                if meta.len() > 0 {
                    return true;
                }
            }
        }
    }
    false
}

/// Free bytes available to the current user on the volume that holds `dir` (None = unknown).
#[cfg(windows)]
pub fn free_space_bytes(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lp_directory_name: *const u16,
            lp_free_bytes_available_to_caller: *mut u64,
            lp_total_number_of_bytes: *mut u64,
            lp_total_number_of_free_bytes: *mut u64,
        ) -> i32;
    }
    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
    if ok != 0 {
        Some(avail)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn free_space_bytes(dir: &Path) -> Option<u64> {
    let out = std::process::Command::new("df")
        .arg("-Pk")
        .arg(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let avail_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(avail_kb.saturating_mul(1024))
}

pub fn is_disk_full_error(diags: &[String]) -> bool {
    diags.iter().any(|l| {
        let low = l.to_lowercase();
        low.contains("no space left")
            || low.contains("os error 28")
            || low.contains("os error 112")
            || low.contains("errno 28")
            || low.contains("not enough space")
            || low.contains("disk full")
            || low.contains("there is not enough space on the disk")
    })
}

/// HLS / DASH / Kick / Twitch / IVS streams: Low Disk Space mode can never apply (yt-dlp needs
/// fragment files plus merge space). Mirrors `isHlsLikeUrl` in src/lib/utils.ts.
pub fn is_hls_like_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    let host_lower = origin_of(&url_lower);
    url_lower.contains("m3u8")
        || url_lower.contains("/hls/")
        || url_lower.contains("playlist.m3u8")
        || url_lower.contains(".mpd")
        || host_lower.contains("kick.com")
        || host_lower.contains("twitch.tv")
        || host_lower.contains("ivs")
}

fn fmt_gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
}

fn find_fresh_kept_output_file(
    save_folder: &str,
    stem: &str,
    run_started_at: std::time::SystemTime,
    detected_path: Option<&str>,
) -> Option<std::path::PathBuf> {
    if let Some(p_str) = detected_path {
        let p = Path::new(p_str);
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            let low = name.to_lowercase();
            if !is_partial_media_filename(&low) && is_readable_nonempty_file(p) {
                return Some(p.to_path_buf());
            }
        }
    }

    let entries = fs::read_dir(save_folder).ok()?;
    let mut best: Option<(std::time::SystemTime, u64, std::path::PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let low = name.to_lowercase();
        if low.ends_with(".part")
            || low.ends_with(".ytdl")
            || low.contains(".part-frag")
            || low.contains(".temp.")
            || low.ends_with(".tmp")
            || low.starts_with(".zr_sparse_meta_")
        {
            continue;
        }
        // Only consider format fragments as a last resort? Prefer non-fragment final media files
        if is_ytdlp_format_fragment_name(&low) {
            continue;
        }
        if !is_readable_nonempty_file(&path) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(mod_time) = meta.modified() else {
            continue;
        };
        let fresh = mod_time + Duration::from_secs(2) >= run_started_at;
        let matches = name_matches_stem(&low, stem);
        if fresh || matches {
            let take = match &best {
                None => true,
                Some((best_t, best_len, _)) => mod_time > *best_t || (mod_time == *best_t && meta.len() > *best_len),
            };
            if take {
                best = Some((mod_time, meta.len(), path));
            }
        }
    }
    best.map(|(_, _, p)| p)
}

/// Removes a forgotten media task from all tracking maps so persistence never re-adds it.
pub fn forget_task(state: &DownloadState, task_id: &str) {
    if let Ok(mut tasks) = state.tasks.lock() {
        tasks.remove(task_id);
    }
    if let Ok(mut opts) = state.task_options.lock() {
        opts.remove(task_id);
    }
    if let Ok(mut cancelled) = state.cancelled_tasks.lock() {
        cancelled.remove(task_id);
    }
    if let Ok(mut pids) = state.active_pids.lock() {
        pids.remove(task_id);
    }
}

/// Deletes final file(s) and stale partial artifacts for a deleted media task.
pub fn delete_task_files_and_partials(save_path: &str, filename: &str) {
    let trimmed_save = save_path.trim();
    let trimmed_file = filename.trim();
    let p = Path::new(trimmed_save);

    if p.exists() && p.is_file() {
        let _ = fs::remove_file(p);
        if let Some(parent) = p.parent() {
            let parent_str = parent.to_string_lossy().to_string();
            let stem = if !trimmed_file.is_empty() {
                strip_media_extension_for_stem(trimmed_file)
            } else {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            };
            clean_partial_files(&parent_str, &stem);
        }
        return;
    }

    if !trimmed_save.is_empty() && !trimmed_file.is_empty() {
        let joined = p.join(trimmed_file);
        if joined.exists() && joined.is_file() {
            let _ = fs::remove_file(&joined);
        }
        let stem = strip_media_extension_for_stem(trimmed_file);
        clean_partial_files(trimmed_save, &stem);
        if !stem.is_empty() {
            if let Ok(entries) = fs::read_dir(trimmed_save) {
                for entry in entries.flatten() {
                    let ep = entry.path();
                    if !ep.is_file() {
                        continue;
                    }
                    if let Some(name) = ep.file_name().and_then(|n| n.to_str()) {
                        let low = name.to_lowercase();
                        if !low.starts_with(".zr_sparse_meta_") && name_matches_stem(&low, &stem) {
                            let _ = fs::remove_file(&ep);
                        }
                    }
                }
            }
        }
    } else if !trimmed_save.is_empty() {
        clean_partial_files(trimmed_save, "");
    }
}

pub fn cleanup_temp_and_part_files(save_folder: &str, title_hint: &str) {
    if strip_media_extension_for_stem(title_hint).len() >= 3 {
        clean_partial_files(save_folder, title_hint);
    }
}

fn strip_format_id_suffix_for_stem(stem: &str) -> String {
    let low = stem.to_lowercase();
    if let Some(idx) = low.rfind(".f") {
        let rest = &low[idx + 2..];
        if !rest.is_empty()
            && rest.len() <= 24
            && ((rest.bytes().all(|b| b.is_ascii_digit() || b == b'-' || b == b'_')
                && rest.bytes().any(|b| b.is_ascii_digit()))
                || ((rest.starts_with("hls") || rest.starts_with("dash"))
                    && rest.bytes().any(|b| b.is_ascii_digit())))
        {
            return stem[..idx].to_string();
        }
    }
    stem.to_string()
}

fn parse_ytdlp_output_path(line: &str) -> Option<String> {
    let prefixes = [
        "[download] Destination:",
        "[ExtractAudio] Destination:",
        "[Merger] Merging formats into ",
    ];
    for pfx in prefixes {
        if let Some(idx) = line.find(pfx) {
            let candidate = line[idx + pfx.len()..]
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }
    if line.starts_with("[Fixup") {
        if let Some(idx) = line.rfind(" of ") {
            let candidate = line[idx + 4..]
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

pub fn resolve_fresh_direct_url_sync(
    app_settings: &crate::settings::AppSettings,
    page_url: &str,
) -> Result<String, String> {
    let page = page_url.trim();
    if page.is_empty() {
        return Err("Empty origin page URL".to_string());
    }
    ensure_ytdlp_available();
    let mut cmd = make_command("yt-dlp");
    cmd.args(["--no-playlist", "--no-check-certificates", "-g", page]);
    if let Some(proxy_url) = app_settings.effective_proxy_url() {
        cmd.args(["--proxy", &proxy_url]);
    }
    if app_settings.cookies_browser != "None" && !app_settings.cookies_browser.trim().is_empty() {
        cmd.args([
            "--cookies-from-browser",
            &app_settings.cookies_browser.to_lowercase(),
        ]);
    } else if !app_settings.cookies_file.trim().is_empty()
        && PathBuf::from(&app_settings.cookies_file).exists()
    {
        cmd.args(["--cookies", app_settings.cookies_file.trim()]);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Failed to invoke yt-dlp: {}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            "yt-dlp failed to refresh expired URL".to_string()
        } else {
            err
        });
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .map(|l| l.trim())
        .find(|l| l.starts_with("http://") || l.starts_with("https://"))
        .map(|s| s.to_string())
        .ok_or_else(|| "yt-dlp returned no fresh URL".to_string())
}

/// Re-extract a fresh direct media stream URL from the original webpage via yt-dlp
/// when a CDN link returns HTTP 403/410 or expires mid-download.
pub async fn resolve_fresh_direct_url(app: &AppHandle, page_url: &str) -> Result<String, String> {
    let page = page_url.trim().to_string();
    let app_settings = app
        .try_state::<crate::settings::SettingsState>()
        .map(|s| s.get_blocking())
        .unwrap_or_default();

    tokio::task::spawn_blocking(move || resolve_fresh_direct_url_sync(&app_settings, &page))
        .await
        .map_err(|e| e.to_string())?
}

fn is_transient_ytdlp_error(diags: &[String]) -> bool {
    diags.iter().any(|l| {
        let low = l.to_lowercase();
        low.contains("timed out")
            || low.contains("timeout")
            || low.contains("connection reset")
            || low.contains("connection aborted")
            || low.contains("temporary failure")
            || low.contains("unable to download webpage")
            || low.contains("http error 5")
            || low.contains("http error 429")
            || low.contains("eof")
            || low.contains("transport")
    })
}

fn is_expired_link_error(diags: &[String]) -> bool {
    diags.iter().any(|l| {
        let low = l.to_lowercase();
        low.contains("http error 403")
            || low.contains("http error 410")
            || low.contains("403 forbidden")
            || low.contains("410 gone")
            || low.contains("expired")
    })
}

pub use crate::clipper::*;

pub fn resolve_media_container_ext(container: &str, is_audio: bool) -> &'static str {
    let upper = container.to_uppercase();
    if is_audio || upper.contains("(AUDIO)") {
        if upper.starts_with("MP3") {
            "mp3"
        } else if upper.starts_with("M4A") {
            "m4a"
        } else if upper.starts_with("OPUS") {
            "opus"
        } else if upper.starts_with("FLAC") {
            "flac"
        } else if upper.starts_with("WAV") {
            "wav"
        } else if upper.starts_with("OGG") {
            "ogg"
        } else {
            "mp3"
        }
    } else if upper.starts_with("WEBM") {
        "webm"
    } else if upper.starts_with("MKV") {
        "mkv"
    } else if upper.starts_with("MOV") {
        "mov"
    } else if upper.starts_with("AVI") {
        "avi"
    } else if upper.starts_with("TS") {
        "ts"
    } else {
        "mp4"
    }
}

#[allow(dead_code)]
enum PassOutcome {
    Success(Option<PathBuf>),
    Cancelled,
    Error(String),
}

#[command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, DownloadState>,
    mut options: DownloadOptions,
) -> Result<String, String> {
    let app_settings = app
        .try_state::<crate::settings::SettingsState>()
        .map(|s| s.get_blocking())
        .unwrap_or_default();

    let resolved_folder = if options.save_folder.trim().is_empty()
        || options.save_folder.trim() == "Downloads"
        || !std::path::Path::new(&options.save_folder).is_absolute()
    {
        if !app_settings.default_download_dir.trim().is_empty() {
            app_settings.default_download_dir.clone()
        } else {
            dirs_or_default_downloads()
        }
    } else {
        options.save_folder.clone()
    };
    options.save_folder = resolved_folder.clone();

    let task_id = options.task_id.clone().unwrap_or_else(|| {
        format!(
            "task_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        )
    });
    let existing_task_snapshot = state.tasks.lock().unwrap().get(&task_id).cloned();
    let is_resume_run = options.is_resume
        || existing_task_snapshot
            .as_ref()
            .map(|t| matches!(t.status.as_str(), "paused" | "error" | "cancelled"))
            .unwrap_or(false);
    options.is_resume = is_resume_run;
    let existing_real_filename = existing_task_snapshot
        .as_ref()
        .map(|t| t.filename.clone())
        .filter(|n| !n.trim().is_empty() && n != "Online Media.mp4" && n != "Downloading...");
    let preserved_created_at = existing_task_snapshot
        .as_ref()
        .map(|t| t.created_at.clone())
        .filter(|s| !s.trim().is_empty());

    state.cancelled_tasks.lock().unwrap().insert(task_id.clone(), false);
    state.task_options.lock().unwrap().insert(task_id.clone(), options.clone());

    let resolved_display_filename = options
        .custom_filename
        .clone()
        .filter(|n| !n.trim().is_empty())
        .or_else(|| existing_real_filename.clone())
        .unwrap_or_else(|| "Online Media.mp4".to_string());

    let preserved_percent = if is_resume_run {
        existing_task_snapshot.as_ref().map(|t| t.percent).unwrap_or(0.0)
    } else {
        0.0
    };
    let preserved_downloaded = if is_resume_run {
        existing_task_snapshot.as_ref().map(|t| t.downloaded_bytes).unwrap_or(0)
    } else {
        0
    };
    let preserved_total = if is_resume_run {
        existing_task_snapshot.as_ref().map(|t| t.total_bytes).unwrap_or(0)
    } else {
        0
    };
    let effective_priority = if !options.priority.trim().is_empty() {
        options.priority.clone()
    } else if let Some(ref prev) = existing_task_snapshot {
        prev.priority.clone()
    } else {
        "normal".to_string()
    };
    let effective_queue_order = if options.queue_order != 0 {
        options.queue_order
    } else if let Some(ref prev) = existing_task_snapshot {
        prev.queue_order
    } else {
        chrono::Utc::now().timestamp_millis()
    };

    let initial_task = DownloadTask {
        id: task_id.clone(),
        url: options.url.clone(),
        filename: resolved_display_filename.clone(),
        save_path: resolved_folder.clone(),
        total_bytes: preserved_total,
        downloaded_bytes: preserved_downloaded,
        percent: preserved_percent,
        speed_bps: 0,
        speed_formatted: "Queued".to_string(),
        eta_formatted: "--".to_string(),
        status: "queued".to_string(),
        category: if options.download_mode == "audio" {
            "audio".to_string()
        } else {
            "video".to_string()
        },
        chunk_count: (options.aria2_connections as usize).clamp(1, 32),
        is_resumable: true,
        low_disk_space: options.low_disk_space.unwrap_or(false) && !is_hls_like_url(&options.url),
        priority: effective_priority,
        queue_order: effective_queue_order,
        expected_checksum: options.expected_checksum.clone(),
        post_download_action: options.post_download_action.clone(),
        post_download_program: options.post_download_program.clone(),
        auth_username: options.auth_username.clone(),
        auth_password: options.auth_password.clone(),
        created_at: preserved_created_at
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        ..Default::default()
    };
    state.tasks.lock().unwrap().insert(task_id.clone(), initial_task.clone());
    let _ = app.emit("download-progress", initial_task);
    crate::spawn_save_all_tasks(&app);

    // Trigger unified queue manager (HTTP + media share max_active_downloads)
    crate::queue::process_queue(&app).await;

    Ok(task_id)
}

pub fn launch_media_worker(app: AppHandle, task_id: String) {
    let Some(state) = app.try_state::<DownloadState>() else {
        return;
    };

    // Ensure only one media worker executes for this task at any time
    {
        let mut workers = state.running_workers.lock().unwrap();
        if !workers.insert(task_id.clone()) {
            eprintln!("Worker for task {} is already running; skipping duplicate launch.", task_id);
            return;
        }
    }

    let Some(options) = state.task_options.lock().unwrap().get(&task_id).cloned() else {
        state.running_workers.lock().unwrap().remove(&task_id);
        return;
    };
    let existing_task_snapshot = state.tasks.lock().unwrap().get(&task_id).cloned();
    let is_resume_run = options.is_resume
        || existing_task_snapshot
            .as_ref()
            .map(|t| matches!(t.status.as_str(), "paused" | "error" | "cancelled") || t.downloaded_bytes > 0)
            .unwrap_or(false);
    let existing_real_filename = existing_task_snapshot
        .as_ref()
        .map(|t| t.filename.clone())
        .filter(|n| !n.trim().is_empty() && n != "Online Media.mp4" && n != "Downloading...");
    let preserved_percent = existing_task_snapshot
        .as_ref()
        .map(|t| t.percent)
        .unwrap_or(0.0);
    let preserved_downloaded = existing_task_snapshot
        .as_ref()
        .map(|t| t.downloaded_bytes)
        .unwrap_or(0);
    let preserved_total = existing_task_snapshot
        .as_ref()
        .map(|t| t.total_bytes)
        .unwrap_or(0);

    let active_pids = state.active_pids.clone();
    let cancelled_tasks = state.cancelled_tasks.clone();
    cancelled_tasks.lock().unwrap().insert(task_id.clone(), false);

    let tid = task_id.clone();
    let media_tasks = state.tasks.clone();
    let running_workers_clone = state.running_workers.clone();
    tauri::async_runtime::spawn_blocking(move || {
        struct WorkerGuard {
            task_id: String,
            running_workers: Arc<Mutex<std::collections::HashSet<String>>>,
        }
        impl Drop for WorkerGuard {
            fn drop(&mut self) {
                if let Ok(mut w) = self.running_workers.lock() {
                    w.remove(&self.task_id);
                }
            }
        }
        let _guard = WorkerGuard {
            task_id: tid.clone(),
            running_workers: running_workers_clone,
        };

        let app_settings = app
            .try_state::<crate::settings::SettingsState>()
            .map(|s| s.get_blocking())
            .unwrap_or_default();
        if let Err(e) = fs::create_dir_all(&options.save_folder) {
            let err_msg = format!("Failed to create output folder: {}", e);
            mark_task_error(&media_tasks, &tid, &err_msg);
            crate::spawn_save_all_tasks(&app);
            let _ = app.emit("download-error", serde_json::json!({ "id": tid.clone(), "error": err_msg }));
            let _ = app.emit(
                "download-progress",
                serde_json::json!({
                    "id": tid,
                    "status": "error",
                    "error_message": err_msg,
                    "speed_formatted": "0 B/s",
                    "eta_formatted": "--",
                }),
            );
            return;
        }

        let is_subtitles_only = options.download_mode == "subtitles";

        // Timeframe / Clipping Setup & Filename Suffix
        let mut is_clipping_active = false;
        let mut clip_target_duration: Option<f64> = None;
        let mut suffix_tag = String::new();
        let mut clip_start_str = String::new();
        let mut clip_end_str = String::new();
        let mut clip_start_sec: f64 = 0.0;
        let mut clip_end_sec: f64 = 0.0;

        if options.enable_clip && !is_subtitles_only {
            let s_raw = options.start_time.trim();
            let e_raw = options.end_time.trim();

            let s_sec = parse_time_to_seconds(s_raw).unwrap_or(0.0);
            let e_sec_opt = parse_time_to_seconds(e_raw);

            if let Some(e_sec) = e_sec_opt {
                if e_sec > s_sec {
                    is_clipping_active = true;
                    clip_target_duration = Some(e_sec - s_sec);
                    clip_start_sec = s_sec;
                    clip_end_sec = e_sec;
                    clip_start_str = format_seconds_to_timestamp(s_sec);
                    clip_end_str = format_seconds_to_timestamp(e_sec);
                    suffix_tag = format!(" [Clip {} to {}]", clip_start_str.replace(':', "-"), clip_end_str.replace(':', "-"));
                } else if s_sec > 0.0 {
                    is_clipping_active = true;
                    clip_start_sec = s_sec;
                    clip_start_str = format_seconds_to_timestamp(s_sec);
                    clip_end_str = "inf".to_string();
                    suffix_tag = format!(" [Clip {} to End]", clip_start_str.replace(':', "-"));
                }
            } else if s_sec > 0.0 || e_raw.eq_ignore_ascii_case("inf") {
                is_clipping_active = true;
                clip_start_sec = s_sec;
                clip_start_str = format_seconds_to_timestamp(s_sec);
                clip_end_str = "inf".to_string();
                suffix_tag = format!(" [Clip {} to End]", clip_start_str.replace(':', "-"));
            }
        }

        let is_chunked_clip = is_clipping_active
            && clip_end_str != "inf"
            && clip_target_duration.map(|d| d > CLIP_PIECE_SECS).unwrap_or(false);

        let container_upper = options.container.to_uppercase();
        let is_audio_container = options.download_mode == "audio"
            || container_upper.contains("(AUDIO)");
        let media_ext = resolve_media_container_ext(&options.container, is_audio_container);

        let out_tmpl = if let Some(ref custom_name) = options.custom_filename {
            let trimmed = custom_name.trim();
            if !trimmed.is_empty() {
                let name_without_ext = if let Some(dot_idx) = trimmed.rfind('.') {
                    let ext = &trimmed[dot_idx + 1..];
                    if ext.len() <= 4 && ext.chars().all(|c| c.is_alphanumeric()) {
                        &trimmed[..dot_idx]
                    } else {
                        trimmed
                    }
                } else {
                    trimmed
                };
                let sanitized: String = name_without_ext.chars().map(|c| match c {
                    '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                    _ => c,
                }).collect();
                if options.include_media_id {
                    format!(
                        "{}/{}{} [%(id)s].%(ext)s",
                        options.save_folder.trim_end_matches(&['/', '\\'][..]),
                        sanitized,
                        suffix_tag
                    )
                } else {
                    format!(
                        "{}/{}{}.%(ext)s",
                        options.save_folder.trim_end_matches(&['/', '\\'][..]),
                        sanitized,
                        suffix_tag
                    )
                }
            } else if options.include_media_id {
                format!(
                    "{}/%(title).140s{} [%(id)s].%(ext)s",
                    options.save_folder.trim_end_matches(&['/', '\\'][..]),
                    suffix_tag
                )
            } else {
                format!(
                    "{}/%(title).150s{}.%(ext)s",
                    options.save_folder.trim_end_matches(&['/', '\\'][..]),
                    suffix_tag
                )
            }
        } else if options.include_media_id {
            format!(
                "{}/%(title).140s{} [%(id)s].%(ext)s",
                options.save_folder.trim_end_matches(&['/', '\\'][..]),
                suffix_tag
            )
        } else {
            format!(
                "{}/%(title).150s{}.%(ext)s",
                options.save_folder.trim_end_matches(&['/', '\\'][..]),
                suffix_tag
            )
        };
        let url_lower = options.url.to_lowercase();
        let is_youtube = url_lower.contains("youtube.com") || url_lower.contains("youtu.be");
        let is_low_disk = options.low_disk_space.unwrap_or(false) || !options.use_part_files;
        let is_hls = is_hls_like_url(&options.url);
        // Never write directly into the final file in low-disk mode for HLS; keep --part enabled.
        let part_flag = if is_low_disk && !is_hls { "--no-part" } else { "--part" };

        let task_stem = {
            let from_custom = options
                .custom_filename
                .as_deref()
                .map(strip_media_extension_for_stem)
                .unwrap_or_default();
            if !from_custom.is_empty() && !from_custom.eq_ignore_ascii_case("Online Media") {
                format!("{}{}", from_custom, suffix_tag)
            } else if let Some(ref real_name) = existing_real_filename {
                strip_media_extension_for_stem(real_name)
            } else {
                options
                    .url
                    .split('?')
                    .next()
                    .and_then(|u| u.trim_end_matches('/').rsplit('/').next())
                    .map(strip_media_extension_for_stem)
                    .filter(|s| s.len() >= 3 && !s.eq_ignore_ascii_case("playlist") && !s.eq_ignore_ascii_case("master"))
                    .unwrap_or_default()
            }
        };

        // When resuming, preserve partial files so yt-dlp continues instead of restarting at 0%.
        let build_cmd = |attempt_stage: u8,
                         active_url: &str,
                         effective_out_tmpl: &str,
                         section_range: Option<(&str, &str)>,
                         capture_title: bool|
         -> Command {
            let mut cmd = make_command("yt-dlp");
            cmd.args([
                "--newline",
                "--progress",
                "--no-playlist",
                "--no-wait-for-video",
                "--no-live-from-start",
                "--socket-timeout",
                "20",
                "--extractor-retries",
                "3",
                "--progress-template",
                "download:[ZR_PROG] %(progress._percent_str)s | %(progress.downloaded_bytes)s | %(progress.total_bytes)s | %(progress.total_bytes_estimate)s | %(progress._speed_str)s | %(progress._eta_str)s",
            ]);

            let effective_referer = if !options.referer.trim().is_empty() {
                options.referer.trim().to_string()
            } else {
                origin_of(active_url)
            };
            if !effective_referer.is_empty() {
                let eff_origin = origin_of(&effective_referer);
                cmd.args(["--referer", &effective_referer]);
                if !eff_origin.is_empty() {
                    cmd.args(["--add-header", &format!("Origin:{}", eff_origin)]);
                }
                cmd.args([
                    "--add-header",
                    "User-Agent:Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36",
                ]);
            }

            // Prefer Deno (fast bundled binary) then Node JS runtime
            if find_exe_in_augmented_path("deno").is_some() || ensure_deno_available().is_some() {
                cmd.args(["--js-runtimes", "deno"]);
            } else if find_exe_in_augmented_path("node").is_some() {
                cmd.args(["--js-runtimes", "node"]);
            }

            // Ensure FFmpeg location is explicitly passed if found
            if let Some(ffmpeg_path) = ensure_ffmpeg_available() {
                cmd.arg("--ffmpeg-location").arg(ffmpeg_path.to_string_lossy().to_string());
            }

            // On resume: NEVER --force-overwrites; always --no-overwrites --continue --part
            // ALWAYS --continue so already-downloaded fragments / format files are reused.
            // Only the ffmpeg fresh-start fallback (attempt_stage 2, which is only entered when no
            // progress had been made yet) uses --no-continue.
            let continue_flag = if is_resume_run || options.is_resume || attempt_stage != 2 {
                "--continue"
            } else {
                "--no-continue"
            };
            let stage_part_flag = if is_resume_run || options.is_resume {
                "--part"
            } else if attempt_stage == 2 {
                "--no-part"
            } else {
                part_flag
            };

            if is_hls || is_youtube {
                cmd.args([
                    "-o", effective_out_tmpl,
                    "--windows-filenames",
                    "--no-playlist",
                    "--no-check-certificates",
                    "--retries", "10",
                    "--fragment-retries", "15",
                    "--file-access-retries", "5",
                    continue_flag,
                    stage_part_flag,
                ]);
                if !is_hls && is_youtube {
                    cmd.args([
                        "--concurrent-fragments", "8",
                        "--http-chunk-size", "10M",
                        "--buffer-size", "64K",
                    ]);
                }
                if is_hls {
                    cmd.args([
                        "--concurrent-fragments", "8",
                        "--fixup", "warn",
                    ]);
                    if attempt_stage == 2 {
                        cmd.args([
                            "--downloader", "m3u8:ffmpeg",
                            "--downloader-args", "ffmpeg:-c copy",
                        ]);
                    }
                } else if attempt_stage >= 1 {
                    cmd.args(["--fixup", "warn"]);
                }
            } else {
                cmd.args([
                    "-o", effective_out_tmpl,
                    "--windows-filenames",
                    "--no-playlist",
                    "--no-check-certificates",
                    "--concurrent-fragments", "8",
                    "--retries", "10",
                    "--fragment-retries", "15",
                    "--file-access-retries", "5",
                    continue_flag,
                    stage_part_flag,
                ]);
                if attempt_stage >= 1 {
                    cmd.args(["--fixup", "warn"]);
                }
            }
        if section_range.is_some() {
            cmd.arg("--force-overwrites");
        } else if is_resume_run || options.is_resume {
            cmd.arg("--no-overwrites");
        } else if (options.overwrite_files && !options.is_resume) || is_clipping_active || attempt_stage == 2 {
            cmd.arg("--force-overwrites");
        } else if attempt_stage == 1 {
            // retry after a failed post-process: keep downloaded data, no overwrite flag at all
        } else {
            cmd.arg("--no-overwrites");
        }

        // Mode & Container Handling
        let container_upper = options.container.to_uppercase();
        let is_audio_container = options.download_mode == "audio"
            || container_upper.contains("(AUDIO)");

        if is_subtitles_only {
            cmd.arg("--skip-download");
        } else if is_audio_container {
            let aud_fmt = if options.audio_stream_id.is_empty() || options.audio_stream_id == "ba" {
                "ba/b".to_string()
            } else {
                options.audio_stream_id.clone()
            };
            cmd.args(["-f", &aud_fmt, "-x"]);

            // Determine audio format from container option:
            // Audio (Generic), MP3 (Audio), M4A (Audio), OPUS (Audio), FLAC (Audio), WAV (Audio), OGG (Audio)
            if !container_upper.starts_with("AUDIO (GENERIC)") {
                let target_codec = if container_upper.starts_with("MP3") {
                    "mp3"
                } else if container_upper.starts_with("M4A") {
                    "m4a"
                } else if container_upper.starts_with("OPUS") {
                    "opus"
                } else if container_upper.starts_with("FLAC") {
                    "flac"
                } else if container_upper.starts_with("WAV") {
                    "wav"
                } else if container_upper.starts_with("OGG") {
                    "vorbis"
                } else {
                    "mp3"
                };
                cmd.args(["--audio-format", target_codec]);
            }

            let aq = if options.audio_bitrate.is_empty() || options.audio_bitrate == "Best" {
                "0"
            } else {
                options.audio_bitrate.as_str()
            };
            cmd.args(["--audio-quality", aq]);
        } else {
            // Video Mode
            let vid_sel = if options.video_stream_id.is_empty() || options.video_stream_id == "best" {
                "bv*".to_string()
            } else {
                options.video_stream_id.clone()
            };
            let aud_sel = if options.audio_stream_id.is_empty() || options.audio_stream_id == "ba" {
                "ba".to_string()
            } else {
                options.audio_stream_id.clone()
            };

            let fmt_expr = if options.url.contains("instagram.com") {
                if vid_sel.contains("height") {
                    format!("{}+ba/b/best", vid_sel)
                } else {
                    "bv*+ba/b/best".to_string()
                }
            } else {
                format!("{}+{}/{}/b", vid_sel, aud_sel, vid_sel)
            };
            cmd.args(["-f", &fmt_expr]);

            // Video Containers: Video (Generic), MP4 (Video), WEBM (Video), MKV (Video), MOV (Video), AVI (Video), TS (Video)
            if !container_upper.starts_with("VIDEO (GENERIC)") && !container_upper.is_empty() {
                if container_upper.starts_with("MP4") {
                    if attempt_stage != 2 {
                        cmd.args(["--merge-output-format", "mp4"]);
                    }
                } else if container_upper.starts_with("WEBM") {
                    cmd.args(["--merge-output-format", "webm"]);
                } else if container_upper.starts_with("MKV") {
                    cmd.args(["--merge-output-format", "mkv"]);
                } else if container_upper.starts_with("MOV") {
                    cmd.args(["--remux-video", "mov"]);
                } else if container_upper.starts_with("AVI") {
                    cmd.args(["--recode-video", "avi"]);
                } else if container_upper.starts_with("TS") {
                    cmd.args(["--merge-output-format", "ts"]);
                }
            } else if !is_hls {
                cmd.args(["--merge-output-format", "mp4"]);
            }
        }

        // Subtitles Handling
        if is_subtitles_only || options.enable_subtitles {
            cmd.arg("--write-subs");
            if options.include_auto_subtitles || options.subtitle_lang.contains("auto") {
                cmd.arg("--write-auto-subs");
            }
            let clean_lang = options.subtitle_lang.replace("-auto", "");
            let lang_arg = if clean_lang.trim().is_empty() {
                "en,bn"
            } else {
                clean_lang.trim()
            };
            cmd.args(["--sub-langs", lang_arg]);

            if !options.subtitle_format.is_empty() && options.subtitle_format != "any" {
                cmd.args(["--convert-subs", &options.subtitle_format.to_lowercase()]);
            }

            if !is_subtitles_only
                && !is_audio_container
                && (options.subtitle_save_mode == "embed"
                    || options.subtitle_save_mode == "both"
                    || options.embed_subtitles_global)
            {
                cmd.arg("--embed-subs");
            }
        }

        // Timeframe / Clipping Section (Download only requested timeframe & frame-accurate cut)
        if let Some((s_str, e_str)) = section_range {
            let section_arg = format!("*{}-{}", s_str, e_str);
            cmd.args(["--download-sections", &section_arg]);

            if options.precise_cut {
                cmd.arg("--force-keyframes-at-cuts");
                // Accelerate FFmpeg keyframe generation & encoding using all CPU threads and veryfast preset
                cmd.args(["--downloader-args", "ffmpeg:-preset veryfast -threads 0"]);
            } else {
                cmd.arg("--no-force-keyframes-at-cuts");
            }
        } else if is_clipping_active {
            let section_arg = format!("*{}-{}", clip_start_str, clip_end_str);
            cmd.args(["--download-sections", &section_arg]);

            if options.precise_cut {
                cmd.arg("--force-keyframes-at-cuts");
                // Accelerate FFmpeg keyframe generation & encoding using all CPU threads and veryfast preset
                cmd.args(["--downloader-args", "ffmpeg:-preset veryfast -threads 0"]);
            } else {
                cmd.arg("--no-force-keyframes-at-cuts");
            }
        }

        // Metadata, Thumbnails, Chapters, SponsorBlock
        if !is_subtitles_only {
            if options.embed_metadata {
                cmd.arg("--embed-metadata");
            }
            if options.embed_thumbnails {
                cmd.arg("--embed-thumbnail");
                if is_audio_container && options.crop_audio_thumbnails {
                    cmd.args([
                        "--ppa",
                        "ThumbnailsConvertor:-vf crop=ih:ih",
                    ]);
                }
            }
            if options.embed_chapters {
                cmd.arg("--embed-chapters");
            }
            if options.split_chapters {
                cmd.arg("--split-chapters");
            }
            if options.sponsor_block {
                cmd.args(["--sponsorblock-remove", "sponsor,selfpromo,interaction,intro,outro"]);
            }
        }

        // Speed Limit, Proxy, Cookies, Auth Credentials, & Aria2c
        let live_settings = app
            .try_state::<crate::settings::SettingsState>()
            .map(|s| s.get_blocking())
            .unwrap_or_else(|| app_settings.clone());
        let effective_speed_kb: u64 = if options.speed_limit_kb > 0 && (options.limit_speed || options.speed_limit_kb > 0) {
            options.speed_limit_kb as u64
        } else {
            live_settings.global_speed_limit_kbps()
        };
        if effective_speed_kb > 0 {
            cmd.args(["--limit-rate", &format!("{}K", effective_speed_kb)]);
        }

        let effective_proxy = if !options.proxy.trim().is_empty() {
            options.proxy.trim().to_string()
        } else {
            live_settings.effective_proxy_url().unwrap_or_default()
        };
        if !effective_proxy.is_empty() {
            cmd.args(["--proxy", &effective_proxy]);
        }

        let eff_browser = if options.cookies_browser != "None" && !options.cookies_browser.trim().is_empty() {
            options.cookies_browser.clone()
        } else {
            live_settings.cookies_browser.clone()
        };
        let eff_cookies_file = if !options.cookies_file.trim().is_empty() {
            options.cookies_file.clone()
        } else {
            live_settings.cookies_file.clone()
        };
        let is_raw_cdn = is_raw_stream_cdn(active_url);
        if !is_raw_cdn && eff_browser != "None" && !eff_browser.trim().is_empty() {
            cmd.args(["--cookies-from-browser", &eff_browser.to_lowercase()]);
        } else if !eff_cookies_file.trim().is_empty() && PathBuf::from(&eff_cookies_file).exists() {
            cmd.args(["--cookies", eff_cookies_file.trim()]);
        }

        if !options.auth_username.trim().is_empty() {
            cmd.args(["--username", options.auth_username.trim(), "--password", &options.auth_password]);
        } else if let Some((u, pw)) = crate::settings::find_credentials_for_url(&live_settings, active_url) {
            cmd.args(["--username", u, "--password", pw]);
        }

        // Aria2c Downloader (capped at 16 connections per server; includes speed limit & proxy)
        let aria2c_found =
            find_local_aria2c().is_some() || find_exe_in_augmented_path("aria2c").is_some();
        if (options.use_aria2c || aria2c_found)
            && aria2c_found
            && !options.enable_clip
            && !is_hls
            && !is_youtube
            && attempt_stage != 2
        {
            let aria_conns = if options.aria2_connections > 0 {
                options.aria2_connections.clamp(1, 16)
            } else {
                (live_settings.default_connections as u32).clamp(1, 16)
            };
            let mut aria_args = format!(
                "-x {} -s {} -k 1M --summary-interval=1 --show-console-readout=true --truncate-console-readout=false --enable-color=false --console-log-level=notice",
                aria_conns, aria_conns
            );
            if effective_speed_kb > 0 {
                aria_args.push_str(&format!(" --max-overall-download-limit={}K", effective_speed_kb));
            }
            if !effective_proxy.is_empty() {
                aria_args.push_str(&format!(" --all-proxy={}", effective_proxy));
            }
            cmd.args([
                "--downloader",
                "aria2c",
                "--downloader-args",
                &format!("aria2c:{}", aria_args),
            ]);
        }

        // FFmpeg Threads & Post-Processor Args
        if options.ffmpeg_threads > 0 {
            cmd.args([
                "--ppa",
                &format!("ffmpeg:-threads {}", options.ffmpeg_threads),
            ]);
            if attempt_stage != 2 {
                cmd.args([
                    "--downloader-args",
                    &format!("ffmpeg:-threads {}", options.ffmpeg_threads),
                ]);
            }
        }
        if !options.post_processor_args.trim().is_empty() {
            cmd.args([
                "--ppa",
                &format!("ffmpeg:{}", options.post_processor_args.trim()),
            ]);
        }

        // Extra yt-dlp download args
        if !options.ytdlp_download_args.trim().is_empty() {
            for arg in options.ytdlp_download_args.split_whitespace() {
                if !arg.is_empty() {
                    cmd.arg(arg);
                }
            }
        }

        if capture_title {
            cmd.args(["--print", "before_dl:TITLE:%(title)s"]);
        }

            cmd.arg(active_url.trim());
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd
        };

        let initial_pct = if is_resume_run && preserved_percent > 0.0 {
            preserved_percent.clamp(0.0, 98.0)
        } else {
            2.0
        };
        let init_p = serde_json::json!({
            "id": tid.clone(),
            "taskId": tid.clone(),
            "percent": initial_pct,
            "downloaded_bytes": preserved_downloaded,
            "total_bytes": preserved_total,
            "text": "Initializing download stream...".to_string(),
            "speed": "".to_string(),
            "speed_formatted": "Connecting...".to_string(),
            "speedFormatted": "Connecting...".to_string(),
            "eta": "".to_string(),
            "eta_formatted": "--".to_string(),
            "etaFormatted": "--".to_string(),
            "status": "downloading".to_string(),
        });
        let _ = app.emit("download-progress", init_p.clone());
        let _ = app.emit("ytdlp-progress", init_p);

        let known_duration = Arc::new(Mutex::new(clip_target_duration.or(options.duration)));
        let last_percent = Arc::new(Mutex::new(initial_pct));
        let run_ytdlp_pass = |
            target_out_tmpl: &str,
            target_stem: &str,
            target_save_folder: &str,
            section_range: Option<(&str, &str)>,
            prog_offset_sec: f64,
            prog_offset_bytes: u64,
            target_clip_duration: Option<f64>,
            capture_title: bool,
            captured_title_out: Arc<Mutex<Option<String>>>,
            last_percent: &Arc<Mutex<f64>>,
            known_duration: &Arc<Mutex<Option<f64>>>,
        | -> PassOutcome {
            let run_started_at = std::time::SystemTime::now();
            let mut attempt_stage: u8 = 0;
            let mut network_retries: usize = 0;
            let mut expired_refreshed = false;
            let mut current_media_url = options.url.clone();
            let last_total_bytes = Arc::new(Mutex::new(0u64));
            let last_downloaded_bytes = Arc::new(Mutex::new(prog_offset_bytes));
            let current_stem = Arc::new(Mutex::new(target_stem.to_string()));
            let detected_output_path: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

            let is_clip_section = is_clipping_active || section_range.is_some();
            let effective_piece_dur = section_range.map(|(s, e)| {
                let s_num = parse_time_to_seconds(s).unwrap_or(0.0);
                let e_num = parse_time_to_seconds(e).unwrap_or(0.0);
                (e_num - s_num).max(0.0)
            }).unwrap_or_else(|| target_clip_duration.unwrap_or(0.0));

            let has_fresh_output_file = |folder: &str| -> bool {
                let Ok(entries) = fs::read_dir(folder) else {
                    return false;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    let low = name.to_lowercase();
                    if is_partial_media_filename(&low)
                        || low.ends_with(".tmp")
                        || low.starts_with(".zr_sparse_meta_")
                    {
                        continue;
                    }
                    if let Ok(meta) = entry.metadata() {
                        if meta.len() > 0 {
                            if let Ok(mod_time) = meta.modified() {
                                if mod_time >= run_started_at {
                                    return true;
                                }
                            }
                        }
                    }
                }
                false
            };

            loop {
                let mut spawned_child = None;
                let mut last_spawn_err = None;
                for attempt in 0..6 {
                    let mut cmd = build_cmd(attempt_stage, &current_media_url, target_out_tmpl, section_range, capture_title);
                    match cmd.spawn() {
                        Ok(c) => {
                            spawned_child = Some(c);
                            break;
                        }
                        Err(e) => {
                            let is_locked = matches!(e.raw_os_error(), Some(32) | Some(5));
                            last_spawn_err = Some(e);
                            if is_locked && attempt < 5 {
                                std::thread::sleep(Duration::from_millis(500));
                                continue;
                            }
                            break;
                        }
                    }
                }

                let mut child = match spawned_child {
                    Some(c) => c,
                    None => {
                        let e_str = last_spawn_err
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "Failed to spawn yt-dlp".to_string());
                        mark_task_error(&media_tasks, &tid, &e_str);
                        return PassOutcome::Error(e_str);
                    }
                };

                let child_pid = child.id();
                active_pids.lock().unwrap_or_else(|e| e.into_inner()).insert(tid.clone(), child_pid);

                let app_err = app.clone();
                let media_tasks_err = media_tasks.clone();
                let tid_err = tid.clone();
                let last_pct_err = last_percent.clone();
                let last_down_bytes_err = last_downloaded_bytes.clone();
                let err_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
                let err_lines_clone = err_lines.clone();
                let dur_opt = target_clip_duration.or_else(|| *known_duration.lock().unwrap_or_else(|e| e.into_inner()));

                let stderr_handle = child.stderr.take().map(|stderr| {
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stderr);
                        read_lines_cr_or_lf(reader, |line| {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                return true;
                            }
                            if trimmed.contains("time=")
                                && (trimmed.contains("frame=")
                                    || trimmed.contains("size=")
                                    || trimmed.contains("speed="))
                            {
                                if let Some((sec, size_bytes, speed)) = parse_ffmpeg_progress(trimmed) {
                                    let effective_sec = prog_offset_sec + sec;
                                    let effective_size = prog_offset_bytes + size_bytes;
                                    if effective_size > 0 {
                                        *last_down_bytes_err.lock().unwrap_or_else(|e| e.into_inner()) = effective_size;
                                    }
                                    emit_ffmpeg_progress_update(
                                        &app_err,
                                        &media_tasks_err,
                                        &tid_err,
                                        effective_sec,
                                        effective_size,
                                        &speed,
                                        dur_opt,
                                        &last_pct_err,
                                        is_clip_section,
                                    );
                                    return true;
                                }
                            }

                            let mut lock = err_lines_clone.lock().unwrap_or_else(|e| e.into_inner());
                            if lock.len() >= 250 {
                                lock.remove(0);
                            }
                            lock.push(trimmed.to_string());
                            true
                        });
                    })
                });

                let stdout_diag_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
                let stdout_diag_clone = stdout_diag_lines.clone();
                let app_std = app.clone();
                let media_tasks_std = media_tasks.clone();
                let tid_std = tid.clone();
                let captured_title_clone = captured_title_out.clone();
                let detected_path_clone = detected_output_path.clone();
                let current_stem_clone = current_stem.clone();
                let known_dur_std = known_duration.clone();
                let last_pct_std = last_percent.clone();
                let last_tot_std = last_total_bytes.clone();
                let last_down_std = last_downloaded_bytes.clone();
                let completed_format_bytes = Arc::new(Mutex::new(0u64));
                let completed_fmt_std = completed_format_bytes.clone();

                let merge_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let merge_stop_std = merge_stop.clone();
                let pass_save_folder = target_save_folder.to_string();

                let stdout_handle = child.stdout.take().map(|stdout| {
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stdout);
                        let mut phase_seen: Vec<String> = Vec::new();
                        let mut phase_total: usize = 1;
                        let mut last_emit_instant = Instant::now();
                        let mut merge_hb_started = false;
                        read_lines_cr_or_lf(reader, |line| {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                return true;
                            }

                            if !merge_hb_started
                                && (trimmed.starts_with("[Merger]")
                                    || trimmed.starts_with("[Fixup")
                                    || trimmed.starts_with("[VideoRemuxer]")
                                    || trimmed.starts_with("[VideoConvertor]"))
                            {
                                merge_hb_started = true;
                                let stem_val = current_stem_clone.lock().unwrap_or_else(|e| e.into_inner()).clone();
                                spawn_merge_heartbeat(
                                    app_std.clone(),
                                    tid_std.clone(),
                                    pass_save_folder.clone(),
                                    stem_val,
                                    merge_stop_std.clone(),
                                );
                            }

                            if let Some(t_line) = trimmed.strip_prefix("TITLE:") {
                                let raw_t = t_line.trim();
                                if !raw_t.is_empty() {
                                    let mut lock = captured_title_clone.lock().unwrap_or_else(|e| e.into_inner());
                                    if lock.is_none() {
                                        *lock = Some(raw_t.to_string());
                                    }
                                }
                            }

                            if let Some(fc) = parse_format_count(trimmed) {
                                phase_total = fc;
                            }
                            if let Some(fpath) = format_file_from_line(trimmed) {
                                if !phase_seen.contains(&fpath) {
                                    if !phase_seen.is_empty() {
                                        let prev_d = *last_down_std.lock().unwrap_or_else(|e| e.into_inner());
                                        let prev_t = *last_tot_std.lock().unwrap_or_else(|e| e.into_inner());
                                        let prev_fmt_size = if prev_t > 0 { prev_t } else { prev_d };
                                        *completed_fmt_std.lock().unwrap_or_else(|e| e.into_inner()) += prev_fmt_size;
                                        *last_down_std.lock().unwrap_or_else(|e| e.into_inner()) = 0;
                                        *last_tot_std.lock().unwrap_or_else(|e| e.into_inner()) = 0;
                                    }
                                    phase_seen.push(fpath);
                                    let msg = format!(
                                        "Downloading stream part {}/{}",
                                        phase_seen.len(),
                                        phase_total.max(phase_seen.len())
                                    );
                                    let _ = app_std.emit(
                                        "download-log",
                                        serde_json::json!({ "id": tid_std.clone(), "message": msg, "level": "info" }),
                                    );
                                }
                            }

                            if let Some(dur) = parse_ffmpeg_duration(trimmed) {
                                let mut kd = known_dur_std.lock().unwrap_or_else(|e| e.into_inner());
                                if kd.is_none() {
                                    *kd = Some(dur);
                                }
                            }

                            if let Some((pct_raw, down_b, tot_b, speed_str, eta_str)) = parse_aria2c_progress(trimmed) {
                                let effective_down = prog_offset_bytes + down_b;
                                if effective_down > 0 {
                                    *last_down_std.lock().unwrap_or_else(|e| e.into_inner()) = effective_down;
                                }
                                let prev_tot = *last_tot_std.lock().unwrap_or_else(|e| e.into_inner());
                                if tot_b > 0 {
                                    *last_tot_std.lock().unwrap_or_else(|e| e.into_inner()) = tot_b;
                                }
                                let prev_pct = *last_pct_std.lock().unwrap_or_else(|e| e.into_inner());
                                let pct = if prev_pct >= 98.0 && tot_b > 0 && prev_tot > 0 && tot_b < prev_tot / 2 {
                                    99.0
                                } else {
                                    pct_raw
                                };
                                if pct > 0.0 {
                                    *last_pct_std.lock().unwrap_or_else(|e| e.into_inner()) = pct;
                                }
                                let cur_d = *last_down_std.lock().unwrap_or_else(|e| e.into_inner());
                                let cur_t = *last_tot_std.lock().unwrap_or_else(|e| e.into_inner());
                                emit_unified_media_progress(
                                    &app_std,
                                    &media_tasks_std,
                                    &tid_std,
                                    pct,
                                    cur_d,
                                    cur_t,
                                    &speed_str,
                                    &eta_str,
                                    &mut last_emit_instant,
                                    false,
                                );
                                return true;
                            }

                            if !trimmed.contains("[ZR_PROG]") {
                                let low = trimmed.to_lowercase();
                                if low.contains("error:")
                                    || low.contains("warning:")
                                    || low.contains("postprocessing")
                                    || low.contains("invalid data found")
                                    || low.contains("error opening input")
                                    || low.contains("[aria2c]")
                                    || low.contains("exception")
                                    || low.contains("failed")
                                    || trimmed.starts_with("[Fixup")
                                    || trimmed.starts_with("[Merger]")
                                {
                                    let mut diag_lock = stdout_diag_clone.lock().unwrap_or_else(|e| e.into_inner());
                                    if diag_lock.len() >= 150 {
                                        diag_lock.remove(0);
                                    }
                                    diag_lock.push(trimmed.to_string());
                                }
                                if let Some(out_path) = parse_ytdlp_output_path(trimmed) {
                                    *detected_path_clone.lock().unwrap_or_else(|e| e.into_inner()) = Some(out_path.clone());
                                    if let Some(fname) = Path::new(&out_path).file_name().and_then(|n| n.to_str()) {
                                        let base_stem = strip_format_id_suffix_for_stem(&strip_media_extension_for_stem(fname));
                                        if base_stem.len() >= 3 {
                                            *current_stem_clone.lock().unwrap_or_else(|e| e.into_inner()) = base_stem;
                                        }
                                        let flow = fname.to_lowercase();
                                        if !is_partial_media_filename(&flow) {
                                            if !is_chunked_clip {
                                                if let Ok(mut lock) = media_tasks_std.lock() {
                                                    if let Some(t) = lock.get_mut(&tid_std) {
                                                        if t.filename == "Online Media.mp4"
                                                            || t.filename.trim().is_empty()
                                                            || !trimmed.contains(".f")
                                                        {
                                                            t.filename = fname.to_string();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let is_zr_prog = trimmed.contains("[ZR_PROG]");
                            if is_zr_prog {
                                let prog_idx = trimmed.find("[ZR_PROG]").unwrap_or(0);
                                let payload = trimmed[prog_idx + 9..].trim();
                                let parts: Vec<&str> = payload.split('|').map(|s| s.trim()).collect();
                                if parts.len() >= 6 {
                                    let pct_str = parts[0].trim_end_matches('%').trim();
                                    let mut pct = pct_str.parse::<f64>().unwrap_or(0.0);

                                    let down_b = parts[1].parse::<u64>().unwrap_or(0);
                                    let effective_down = prog_offset_bytes + down_b;
                                    if effective_down > 0 {
                                        *last_down_std.lock().unwrap_or_else(|e| e.into_inner()) = effective_down;
                                    }

                                    let mut tot_b = parts[2].parse::<u64>().unwrap_or(0);
                                    if tot_b == 0 {
                                        if let Ok(f) = parts[3].parse::<f64>() {
                                            if f > 0.0 {
                                                tot_b = f as u64;
                                            }
                                        }
                                    }
                                    let prev_tot = *last_tot_std.lock().unwrap_or_else(|e| e.into_inner());
                                    if tot_b > 0 {
                                        *last_tot_std.lock().unwrap_or_else(|e| e.into_inner()) = tot_b;
                                    }

                                    let done_prior = *completed_fmt_std.lock().unwrap_or_else(|e| e.into_inner());
                                    let cur_d = done_prior + *last_down_std.lock().unwrap_or_else(|e| e.into_inner());
                                    let stream_t = *last_tot_std.lock().unwrap_or_else(|e| e.into_inner());
                                    let cur_t = if stream_t > 0 { done_prior + stream_t } else { 0 };
                                    if pct <= 0.0 && cur_t > 0 && cur_d > 0 {
                                        pct = ((cur_d as f64 / cur_t as f64) * 100.0).clamp(0.0, 99.0);
                                    }

                                    if let Some(total_dur) = target_clip_duration {
                                        if total_dur > 0.0 {
                                            let sec_done = prog_offset_sec + (pct / 100.0) * effective_piece_dur;
                                            pct = ((sec_done / total_dur) * 100.0).clamp(0.0, 99.0);
                                        }
                                    } else {
                                        let is_multi_stream = phase_total > 1 || phase_seen.len() > 1 || is_hls;
                                        if is_multi_stream {
                                            let first_is_audio = phase_seen.first().map(|f| is_audio_format_file(f)).unwrap_or(false);
                                            pct = multi_phase_percent(pct, phase_seen.len().saturating_sub(1), phase_total.max(phase_seen.len()), first_is_audio);
                                        } else {
                                            pct = pct.min(99.0);
                                        }
                                    }

                                    let prev_pct = *last_pct_std.lock().unwrap_or_else(|e| e.into_inner());
                                    if prev_pct >= 98.0 && tot_b > 0 && prev_tot > 0 && tot_b < prev_tot / 2 {
                                        pct = 99.0;
                                    }
                                    if pct <= 0.0 {
                                        pct = prev_pct;
                                    }
                                    let smoothed_pct = pct.max(prev_pct).min(99.0);
                                    *last_pct_std.lock().unwrap_or_else(|e| e.into_inner()) = smoothed_pct;

                                    let speed_str = parts[4].to_string();
                                    let eta_str = parts[5].to_string();

                                    emit_unified_media_progress(
                                        &app_std,
                                        &media_tasks_std,
                                        &tid_std,
                                        smoothed_pct,
                                        cur_d,
                                        cur_t,
                                        &speed_str,
                                        &eta_str,
                                        &mut last_emit_instant,
                                        false,
                                    );
                                    return true;
                                }
                            }

                            if trimmed.starts_with("[download]") && trimmed.contains('%') {
                                let after_tag = &trimmed[10..].trim();
                                let pct_end = after_tag.find('%').unwrap_or(0);
                                let mut pct = after_tag[..pct_end].trim().parse::<f64>().unwrap_or(0.0);
                                if let Some(total_dur) = target_clip_duration {
                                    if total_dur > 0.0 {
                                        let sec_done = prog_offset_sec + (pct / 100.0) * effective_piece_dur;
                                        pct = ((sec_done / total_dur) * 100.0).clamp(0.0, 99.0);
                                    }
                                } else {
                                    let is_multi_stream = phase_total > 1 || phase_seen.len() > 1 || is_hls;
                                    if is_multi_stream {
                                        let first_is_audio = phase_seen.first().map(|f| is_audio_format_file(f)).unwrap_or(false);
                                        pct = multi_phase_percent(pct, phase_seen.len().saturating_sub(1), phase_total.max(phase_seen.len()), first_is_audio);
                                    } else {
                                        pct = pct.min(99.0);
                                    }
                                }
                                let prev_pct = *last_pct_std.lock().unwrap_or_else(|e| e.into_inner());
                                if pct <= 0.0 {
                                    pct = prev_pct;
                                }
                                let smoothed = pct.max(prev_pct).min(99.0);
                                *last_pct_std.lock().unwrap_or_else(|e| e.into_inner()) = smoothed;

                                let cur_t_before = *last_tot_std.lock().unwrap_or_else(|e| e.into_inner());
                                let downloaded_bytes = if let Some(of_idx) = after_tag.find("of ") {
                                    let total_part = after_tag[of_idx + 3..]
                                        .split_whitespace()
                                        .next()
                                        .unwrap_or("");
                                    let total_bytes = parse_human_size_to_bytes(total_part);
                                    if total_bytes > 0 {
                                        *last_tot_std.lock().unwrap_or_else(|e| e.into_inner()) = total_bytes;
                                    }
                                    prog_offset_bytes + ((total_bytes as f64) * (pct / 100.0)) as u64
                                } else if cur_t_before > 0 {
                                    prog_offset_bytes + ((cur_t_before as f64) * (pct / 100.0)) as u64
                                } else {
                                    *last_down_std.lock().unwrap_or_else(|e| e.into_inner())
                                };
                                if downloaded_bytes > 0 {
                                    *last_down_std.lock().unwrap_or_else(|e| e.into_inner()) = downloaded_bytes;
                                }

                                let speed = if let Some(idx) = after_tag.find("at ") {
                                    after_tag[idx + 3..]
                                        .split_whitespace()
                                        .next()
                                        .unwrap_or("")
                                        .to_string()
                                } else {
                                    String::new()
                                };

                                let eta = if let Some(idx) = after_tag.find("ETA ") {
                                    after_tag[idx + 4..]
                                        .split_whitespace()
                                        .next()
                                        .unwrap_or("--")
                                        .to_string()
                                } else {
                                    "--".to_string()
                                };

                                let done_prior = *completed_fmt_std.lock().unwrap_or_else(|e| e.into_inner());
                                let cur_d = done_prior + *last_down_std.lock().unwrap_or_else(|e| e.into_inner());
                                let stream_t = *last_tot_std.lock().unwrap_or_else(|e| e.into_inner());
                                let cur_t = if stream_t > 0 { done_prior + stream_t } else { 0 };
                                emit_unified_media_progress(
                                    &app_std,
                                    &media_tasks_std,
                                    &tid_std,
                                    smoothed,
                                    cur_d,
                                    cur_t,
                                    &speed,
                                    &eta,
                                    &mut last_emit_instant,
                                    false,
                                );
                            } else if trimmed.contains("time=")
                                && (trimmed.contains("frame=")
                                    || trimmed.contains("size=")
                                    || trimmed.contains("speed="))
                            {
                                if let Some((sec, size_bytes, speed)) = parse_ffmpeg_progress(trimmed) {
                                    let effective_sec = prog_offset_sec + sec;
                                    let effective_size = prog_offset_bytes + size_bytes;
                                    let dur_opt = target_clip_duration.or_else(|| *known_dur_std.lock().unwrap_or_else(|e| e.into_inner()));
                                    if effective_size > 0 {
                                        *last_down_std.lock().unwrap_or_else(|e| e.into_inner()) = effective_size;
                                    }
                                    emit_ffmpeg_progress_update(
                                        &app_std,
                                        &media_tasks_std,
                                        &tid_std,
                                        effective_sec,
                                        effective_size,
                                        &speed,
                                        dur_opt,
                                        &last_pct_std,
                                        is_clip_section,
                                    );
                                }
                            }
                            true
                        });
                    })
                });

                let mut status = None;
                loop {
                    match child.try_wait() {
                        Ok(Some(s)) => {
                            status = Some(s);
                            break;
                        }
                        Ok(None) => {
                            let was_cancelled = cancelled_tasks.lock().unwrap_or_else(|e| e.into_inner()).get(&tid).copied().unwrap_or(false);
                            if was_cancelled {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }

                if let Some(h) = stderr_handle {
                    let _ = h.join();
                }
                if let Some(h) = stdout_handle {
                    let _ = h.join();
                }
                active_pids.lock().unwrap_or_else(|e| e.into_inner()).remove(&tid);
                merge_stop.store(true, std::sync::atomic::Ordering::Relaxed);

                let was_cancelled = cancelled_tasks.lock().unwrap_or_else(|e| e.into_inner()).get(&tid).copied().unwrap_or(false);
                if was_cancelled {
                    if let Ok(mut tasks_lock) = media_tasks.lock() {
                        if let Some(t) = tasks_lock.get_mut(&tid) {
                            t.status = "paused".to_string();
                            t.speed_bps = 0;
                            t.speed_formatted = "0 B/s".to_string();
                            t.eta_formatted = "--".to_string();
                        }
                    }
                    crate::spawn_save_all_tasks(&app);
                    let _ = app.emit(
                        "download-progress",
                        serde_json::json!({
                            "id": tid.clone(),
                            "status": "paused",
                            "speed": "0 B/s",
                            "speed_formatted": "0 B/s",
                            "eta": "--",
                            "eta_formatted": "--"
                        }),
                    );
                    let app_q = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::queue::process_queue(&app_q).await;
                    });
                    return PassOutcome::Cancelled;
                }

                let exit_ok = status.as_ref().map(|s| s.success()).unwrap_or(false);
                let combined_diags: Vec<String> = {
                    let errs = err_lines.lock().unwrap_or_else(|e| e.into_inner());
                    let std_diags = stdout_diag_lines.lock().unwrap_or_else(|e| e.into_inner());
                    errs.iter().chain(std_diags.iter()).cloned().collect()
                };
                let has_postprocess_marker = combined_diags.iter().any(|l| {
                    let low = l.to_lowercase();
                    low.contains("postprocessing")
                        || low.contains("invalid data found")
                        || low.contains("error opening input")
                });
                let postprocess_failed = has_postprocess_marker && !exit_ok;

                let active_stem = current_stem.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let cur_tot = *last_total_bytes.lock().unwrap_or_else(|e| e.into_inner());
                let cur_down = *last_downloaded_bytes.lock().unwrap_or_else(|e| e.into_inner());

                if !exit_ok || postprocess_failed {
                    let disk_full = is_disk_full_error(&combined_diags);
                    if disk_full || postprocess_failed {
                        clean_postprocess_leftovers(target_save_folder, &active_stem);
                    }
                    let need = cur_tot.max(cur_down);
                    let free = free_space_bytes(Path::new(target_save_folder));
                    let has_kept_final = {
                        let det = detected_output_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        find_fresh_kept_output_file(
                            target_save_folder,
                            &active_stem,
                            run_started_at,
                            det.as_deref(),
                        )
                        .is_some()
                    };
                    let too_small = postprocess_failed
                        && !has_kept_final
                        && need > 0
                        && free.map(|f| f < need.saturating_add(need / 20)).unwrap_or(false);
                    if disk_full || too_small {
                        let msg = match free {
                            Some(f) => format!(
                                "Not enough free disk space to finish (free {}, need about {}). Your downloaded data was kept - free some space and press Resume.",
                                fmt_gb(f),
                                fmt_gb(need)
                            ),
                            None => "Not enough free disk space to finish. Your downloaded data was kept - free some space and press Resume.".to_string(),
                        };
                        let tail_log: Vec<String> = combined_diags
                            .iter()
                            .filter(|l| !l.trim().is_empty())
                            .rev()
                            .take(40)
                            .cloned()
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        mark_task_error_with_log(&media_tasks, &tid, &msg, tail_log.clone());
                        crate::append_failure_log(
                            &app,
                            &tid,
                            &active_stem,
                            &current_media_url,
                            &msg,
                            &tail_log,
                        );
                        let error_pct = (*last_percent.lock().unwrap_or_else(|e| e.into_inner())).min(99.0);
                        crate::spawn_save_all_tasks(&app);
                        let _ = app.emit(
                            "download-error",
                            serde_json::json!({ "id": tid.clone(), "error": msg, "errorLog": tail_log }),
                        );
                        let _ = app.emit(
                            "download-progress",
                            serde_json::json!({
                                "id": tid.clone(),
                                "percent": error_pct,
                                "status": "error",
                                "error_message": msg,
                                "errorLog": tail_log,
                                "speed": "0 B/s",
                                "speed_formatted": "0 B/s",
                                "eta": "--",
                                "eta_formatted": "--"
                            }),
                        );
                        let app_q = app.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::queue::process_queue(&app_q).await;
                        });
                        return PassOutcome::Error(msg);
                    }
                }

                if attempt_stage == 0 && postprocess_failed {
                    attempt_stage = 1;
                    clean_postprocess_leftovers(target_save_folder, &active_stem);
                    let msg = "Post-processing failed - retrying the merge/fix-up only (downloaded data kept, --fixup warn)...".to_string();
                    eprintln!("[media] [{}] {}", tid, msg);
                    let _ = app.emit("download-log", serde_json::json!({ "id": tid.clone(), "message": msg.clone(), "level": "warn" }));
                    let fb_payload = serde_json::json!({
                        "id": tid.clone(),
                        "taskId": tid.clone(),
                        "percent": *last_percent.lock().unwrap_or_else(|e| e.into_inner()),
                        "text": msg,
                        "speed": "".to_string(),
                        "speed_formatted": "Retrying...".to_string(),
                        "speedFormatted": "Retrying...".to_string(),
                        "eta": "--".to_string(),
                        "eta_formatted": "--".to_string(),
                        "status": "connecting".to_string(),
                    });
                    let _ = app.emit("download-progress", fb_payload.clone());
                    let _ = app.emit("ytdlp-progress", fb_payload);
                    continue;
                }

                let hls_made_progress = cur_down > 0
                    || has_partial_data(target_save_folder, &active_stem);
                if is_hls
                    && attempt_stage == 0
                    && !exit_ok
                    && !hls_made_progress
                    && !has_fresh_output_file(target_save_folder)
                {
                    attempt_stage = 2;
                    let keep_pct = (*last_percent.lock().unwrap_or_else(|e| e.into_inner())).min(95.0).max(2.0);
                    let fb_payload = serde_json::json!({
                        "id": tid.clone(),
                        "taskId": tid.clone(),
                        "percent": keep_pct,
                        "text": "Retrying HLS stream via FFmpeg fallback...".to_string(),
                        "speed": "".to_string(),
                        "speed_formatted": "Connecting...".to_string(),
                        "speedFormatted": "Connecting...".to_string(),
                        "eta": "--".to_string(),
                        "eta_formatted": "--".to_string(),
                        "status": "downloading".to_string(),
                    });
                    let _ = app.emit("download-progress", fb_payload.clone());
                    let _ = app.emit("ytdlp-progress", fb_payload);
                    continue;
                }

                let detected_ref = detected_output_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let kept_file = find_fresh_kept_output_file(
                    target_save_folder,
                    &active_stem,
                    run_started_at,
                    detected_ref.as_deref(),
                );

                let kept_file_for_warn = kept_file.clone().filter(|p| {
                    if let Some(d) = detected_ref.as_deref() {
                        if Path::new(d) == p.as_path() {
                            return true;
                        }
                    }
                    !active_stem.is_empty()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| name_matches_stem(&n.to_lowercase(), &active_stem))
                            .unwrap_or(false)
                });

                if postprocess_failed {
                    if let Some(ref kept_path) = kept_file_for_warn {
                        let kept_name = kept_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| options.custom_filename.clone().unwrap_or_else(|| "Online Media.mp4".to_string()));
                        let kept_size = fs::metadata(kept_path).map(|m| m.len()).unwrap_or(0);
                        let final_sz = if kept_size > 0 {
                            kept_size
                        } else if cur_tot > 0 {
                            cur_tot
                        } else {
                            cur_down
                        };
                        let warn_msg = "Downloaded, but conversion failed — file kept";
                        mark_task_completed_with_warning(
                            &media_tasks,
                            &tid,
                            warn_msg,
                            Some(kept_name.clone()),
                            Some(final_sz),
                        );
                        crate::spawn_save_all_tasks(&app);
                        if !active_stem.is_empty() {
                            clean_partial_files(target_save_folder, &active_stem);
                        }
                        let warn_payload = serde_json::json!({
                            "id": tid.clone(),
                            "taskId": tid.clone(),
                            "filename": kept_name.clone(),
                            "percent": 100.0,
                            "text": warn_msg,
                            "error_message": warn_msg,
                            "speed": "0 B/s",
                            "speed_formatted": "0 B/s",
                            "speedFormatted": "0 B/s",
                            "eta": "Done",
                            "eta_formatted": "Done",
                            "etaFormatted": "Done",
                            "total_bytes": final_sz,
                            "totalBytes": final_sz,
                            "downloaded_bytes": final_sz,
                            "downloadedBytes": final_sz,
                            "status": "completed_with_warning",
                        });
                        let _ = app.emit("download-progress", warn_payload.clone());
                        let _ = app.emit("ytdlp-progress", warn_payload);
                        let _ = app.emit(
                            "download-complete",
                            serde_json::json!({
                                "id": tid.clone(),
                                "filename": kept_name,
                                "save_path": target_save_folder,
                                "percent": 100.0,
                                "total_bytes": final_sz,
                                "downloaded_bytes": final_sz,
                                "status": "completed_with_warning",
                                "error_message": warn_msg,
                            }),
                        );
                        let app_q = app.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::queue::process_queue(&app_q).await;
                        });
                        return PassOutcome::Success(kept_file_for_warn);
                    }
                }

                if exit_ok && kept_file.is_some() {
                    return PassOutcome::Success(kept_file);
                }
                if exit_ok && !has_postprocess_marker {
                    return PassOutcome::Success(kept_file);
                }

                if !expired_refreshed && is_expired_link_error(&combined_diags) {
                    expired_refreshed = true;
                    let msg = "HTTP 403/410 expired link detected — refreshing URL and resuming...".to_string();
                    let _ = app.emit(
                        "download-log",
                        serde_json::json!({ "id": tid.clone(), "message": msg.clone(), "level": "warn" }),
                    );
                    let _ = app.emit(
                        "download-progress",
                        serde_json::json!({
                            "id": tid.clone(),
                            "taskId": tid.clone(),
                            "percent": *last_percent.lock().unwrap_or_else(|e| e.into_inner()),
                            "text": msg,
                            "speed_formatted": "Refreshing link...".to_string(),
                            "status": "downloading".to_string(),
                        }),
                    );
                    let cur_settings = app
                        .try_state::<crate::settings::SettingsState>()
                        .map(|s| s.get_blocking())
                        .unwrap_or_else(|| app_settings.clone());
                    let extract_res = resolve_fresh_direct_url_sync(&cur_settings, &options.url);
                    if let Ok(fresh_url) = extract_res {
                        if options.url.to_lowercase().contains(".m3u8") || current_media_url != fresh_url {
                            current_media_url = fresh_url;
                        }
                    }
                    continue;
                }

                if is_transient_ytdlp_error(&combined_diags)
                    && network_retries < crate::downloader::NETWORK_RETRY_BACKOFF_SECS.len()
                {
                    let wait_s = crate::downloader::NETWORK_RETRY_BACKOFF_SECS[network_retries];
                    network_retries += 1;
                    let msg = format!(
                        "Network error — auto-retrying in {}s (attempt {}/5)...",
                        wait_s, network_retries
                    );
                    let _ = app.emit(
                        "download-log",
                        serde_json::json!({ "id": tid.clone(), "message": msg.clone(), "level": "warn" }),
                    );
                    let _ = app.emit(
                        "download-progress",
                        serde_json::json!({
                            "id": tid.clone(),
                            "taskId": tid.clone(),
                            "percent": *last_percent.lock().unwrap_or_else(|e| e.into_inner()),
                            "text": msg,
                            "speed_formatted": format!("Retry in {}s", wait_s),
                            "status": "downloading".to_string(),
                        }),
                    );
                    let mut cancelled_during_wait = false;
                    for _ in 0..(wait_s * 4) {
                        std::thread::sleep(Duration::from_millis(250));
                        if cancelled_tasks.lock().unwrap_or_else(|e| e.into_inner()).get(&tid).copied().unwrap_or(false) {
                            cancelled_during_wait = true;
                            break;
                        }
                    }
                    if !cancelled_during_wait {
                        continue;
                    }
                }

                let meaningful_err = combined_diags
                    .iter()
                    .rev()
                    .find(|l| l.starts_with("ERROR:") || l.starts_with("error:"))
                    .or_else(|| combined_diags.iter().rev().find(|l| {
                        let low = l.to_lowercase();
                        (low.contains("error") || low.contains("failed") || low.contains("not found"))
                            && !low.contains("[download]")
                            && !low.contains("warning")
                    }))
                    .or_else(|| combined_diags.iter().rev().find(|l| !l.trim().is_empty()))
                    .cloned()
                    .unwrap_or_else(|| "Unknown yt-dlp error".to_string());

                let tail_log: Vec<String> = combined_diags
                    .iter()
                    .filter(|l| !l.trim().is_empty())
                    .rev()
                    .take(40)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();

                mark_task_error_with_log(&media_tasks, &tid, &meaningful_err, tail_log.clone());
                crate::append_failure_log(
                    &app,
                    &tid,
                    &active_stem,
                    &current_media_url,
                    &meaningful_err,
                    &tail_log,
                );
                let error_pct = (*last_percent.lock().unwrap_or_else(|e| e.into_inner())).min(99.0);
                crate::spawn_save_all_tasks(&app);
                let _ = app.emit(
                    "download-error",
                    serde_json::json!({
                        "id": tid.clone(),
                        "error": meaningful_err,
                        "errorLog": tail_log
                    }),
                );
                let _ = app.emit(
                    "download-progress",
                    serde_json::json!({
                        "id": tid.clone(),
                        "percent": error_pct,
                        "status": "error",
                        "error_message": meaningful_err,
                        "errorLog": tail_log,
                        "speed": "0 B/s",
                        "speed_formatted": "0 B/s",
                        "eta": "--",
                        "eta_formatted": "--"
                    }),
                );
                let app_q = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::queue::process_queue(&app_q).await;
                });
                return PassOutcome::Error(meaningful_err);
            }
        };

        let final_kept_file: Option<PathBuf>;

        if is_chunked_clip {
            let working_dir = clip_working_dir(&options.save_folder, &tid);
            let mut clip_state = match load_clip_state(&working_dir) {
                Some(st) => st,
                None => {
                    if working_dir.exists() {
                        let _ = fs::remove_dir_all(&working_dir);
                    }
                    if let Err(e) = fs::create_dir_all(&working_dir) {
                        let err_msg = format!("Failed to create clip temporary folder: {}", e);
                        mark_task_error(&media_tasks, &tid, &err_msg);
                        return;
                    }
                    let pieces = plan_clip_pieces(clip_start_sec, clip_end_sec, CLIP_PIECE_SECS, media_ext);
                    let st = ClipState {
                        task_id: tid.clone(),
                        total_clip_duration: clip_target_duration.unwrap_or(clip_end_sec - clip_start_sec),
                        target_filename: String::new(),
                        pieces,
                    };
                    let _ = save_clip_state(&working_dir, &st);
                    st
                }
            };

            if clip_state.target_filename.is_empty() {
                let trimmed = options.custom_filename.as_deref().unwrap_or("").trim();
                if !trimmed.is_empty() {
                    let name_without_ext = if let Some(dot_idx) = trimmed.rfind('.') {
                        let ext = &trimmed[dot_idx + 1..];
                        if ext.len() <= 4 && ext.chars().all(|c| c.is_alphanumeric()) {
                            &trimmed[..dot_idx]
                        } else {
                            trimmed
                        }
                    } else {
                        trimmed
                    };
                    let sanitized = crate::downloader::sanitize_filename(name_without_ext);
                    clip_state.target_filename = if options.include_media_id {
                        format!("{}{} [%(id)s].{}", sanitized, suffix_tag, media_ext)
                    } else {
                        format!("{}{}.{}", sanitized, suffix_tag, media_ext)
                    };
                } else if let Some(ref real_name) = existing_real_filename {
                    let base = strip_media_extension_for_stem(real_name);
                    if !base.is_empty() && !base.eq_ignore_ascii_case("Online Media") {
                        let sanitized = crate::downloader::sanitize_filename(&base);
                        clip_state.target_filename = if sanitized.contains("[Clip ") {
                            format!("{}.{}", sanitized, media_ext)
                        } else {
                            format!("{}{}.{}", sanitized, suffix_tag, media_ext)
                        };
                    }
                }
                if !clip_state.target_filename.is_empty() {
                    let _ = save_clip_state(&working_dir, &clip_state);
                }
            }

            for piece in &mut clip_state.pieces {
                let p_path = working_dir.join(&piece.filename);
                if piece.completed {
                    if !p_path.exists() || fs::metadata(&p_path).map(|m| m.len()).unwrap_or(0) == 0 {
                        piece.completed = false;
                    }
                } else {
                    if p_path.exists() {
                        let _ = fs::remove_file(&p_path);
                    }
                    let p_stem = format!("piece_{:04}", piece.index);
                    if let Ok(entries) = fs::read_dir(&working_dir) {
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.starts_with(&p_stem) && name != piece.filename {
                                    let _ = fs::remove_file(entry.path());
                                }
                            }
                        }
                    }
                }
            }
            let _ = save_clip_state(&working_dir, &clip_state);

            let mut completed_dur = 0.0;
            let mut completed_bytes = 0u64;
            for piece in &clip_state.pieces {
                if piece.completed {
                    completed_dur += piece.end_sec - piece.start_sec;
                    let p_path = working_dir.join(&piece.filename);
                    completed_bytes += fs::metadata(&p_path).map(|m| m.len()).unwrap_or(0);
                }
            }

            let total_clip_dur = clip_state.total_clip_duration.max(0.001);
            let base_pct = ((completed_dur / total_clip_dur) * 100.0).clamp(0.0, 99.0);
            let initial_pct = base_pct.max(preserved_percent);
            *last_percent.lock().unwrap_or_else(|e| e.into_inner()) = initial_pct;

            let captured_title: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

            for piece_idx in 0..clip_state.pieces.len() {
                if clip_state.pieces[piece_idx].completed {
                    continue;
                }

                let piece = clip_state.pieces[piece_idx].clone();
                let piece_stem = format!("piece_{:04}", piece.index);
                let piece_out_tmpl = format!(
                    "{}/{}.%(ext)s",
                    working_dir.to_string_lossy().replace('\\', "/"),
                    piece_stem
                );
                let piece_start_str = format_seconds_to_timestamp(piece.start_sec);
                let piece_end_str = format_seconds_to_timestamp(piece.end_sec);

                let pass_res = run_ytdlp_pass(
                    &piece_out_tmpl,
                    &piece_stem,
                    working_dir.to_str().unwrap_or(&options.save_folder),
                    Some((&piece_start_str, &piece_end_str)),
                    completed_dur,
                    completed_bytes,
                    Some(total_clip_dur),
                    clip_state.target_filename.is_empty(),
                    captured_title.clone(),
                    &last_percent,
                    &known_duration,
                );

                if clip_state.target_filename.is_empty() {
                    if let Some(ref title) = *captured_title.lock().unwrap_or_else(|e| e.into_inner()) {
                        let clean_title = crate::downloader::sanitize_filename(title);
                        let shortened = if clean_title.chars().count() > 140 {
                            clean_title.chars().take(140).collect::<String>()
                        } else {
                            clean_title
                        };
                        let constructed = format!("{}{}.{}", shortened, suffix_tag, media_ext);
                        clip_state.target_filename = constructed.clone();
                        let _ = save_clip_state(&working_dir, &clip_state);
                        if let Ok(mut lock) = media_tasks.lock() {
                            if let Some(t) = lock.get_mut(&tid) {
                                t.filename = constructed;
                            }
                        }
                    }
                }

                match pass_res {
                    PassOutcome::Success(_) => {
                        let p_path = working_dir.join(&piece.filename);
                        let piece_size = fs::metadata(&p_path).map(|m| m.len()).unwrap_or(0);
                        if piece_size > 0 {
                            clip_state.pieces[piece_idx].completed = true;
                            completed_dur += piece.end_sec - piece.start_sec;
                            completed_bytes += piece_size;
                            let _ = save_clip_state(&working_dir, &clip_state);
                        } else {
                            let err_msg = format!("Piece {} output file missing or empty", piece.index);
                            mark_task_error(&media_tasks, &tid, &err_msg);
                            return;
                        }
                    }
                    PassOutcome::Cancelled => {
                        return;
                    }
                    PassOutcome::Error(_) => {
                        return;
                    }
                }
            }

            let list_txt_path = working_dir.join("list.txt");
            let piece_filenames: Vec<String> = clip_state.pieces.iter().map(|p| p.filename.clone()).collect();
            let list_content = generate_concat_list_content(&piece_filenames);
            if let Err(e) = fs::write(&list_txt_path, list_content) {
                let err_msg = format!("Failed to create FFmpeg concat list: {}", e);
                mark_task_error(&media_tasks, &tid, &err_msg);
                return;
            }

            let ffmpeg_bin = ensure_ffmpeg_available().unwrap_or_else(|| PathBuf::from("ffmpeg"));
            let final_target_name = if !clip_state.target_filename.is_empty() {
                clip_state.target_filename.clone()
            } else if let Some(ref rf) = existing_real_filename {
                rf.clone()
            } else {
                format!("Online Media{}.{}", suffix_tag, media_ext)
            };
            let final_target_path = Path::new(&options.save_folder).join(&final_target_name);
            let temp_joined_path = working_dir.join(format!("joined_output.{}", media_ext));

            let _ = app.emit("download-progress", serde_json::json!({
                "id": tid.clone(),
                "taskId": tid.clone(),
                "percent": 99.0,
                "text": "Joining clip pieces with FFmpeg...".to_string(),
                "speed_formatted": "Joining...".to_string(),
                "speedFormatted": "Joining...".to_string(),
                "eta_formatted": "--".to_string(),
                "status": "downloading".to_string(),
            }));

            let mut ffmpeg_cmd = Command::new(ffmpeg_bin);
            ffmpeg_cmd.arg("-y");
            ffmpeg_cmd.args(["-f", "concat", "-safe", "0", "-i"]);
            ffmpeg_cmd.arg(&list_txt_path);
            ffmpeg_cmd.args(["-c", "copy"]);
            if media_ext == "mp4" || media_ext == "m4a" || media_ext == "mov" {
                ffmpeg_cmd.args(["-movflags", "+faststart"]);
            }
            ffmpeg_cmd.arg(&temp_joined_path);
            #[cfg(target_os = "windows")]
            ffmpeg_cmd.creation_flags(CREATE_NO_WINDOW);

            let concat_res = ffmpeg_cmd.output();
            let concat_ok = match concat_res {
                Ok(ref out) => out.status.success() && temp_joined_path.exists() && fs::metadata(&temp_joined_path).map(|m| m.len()).unwrap_or(0) > 0,
                Err(_) => false,
            };

            if concat_ok {
                if final_target_path.exists() {
                    let _ = fs::remove_file(&final_target_path);
                }
                if let Err(e) = fs::rename(&temp_joined_path, &final_target_path) {
                    let err_msg = format!("Failed to move joined file to target path: {}", e);
                    mark_task_error(&media_tasks, &tid, &err_msg);
                    return;
                }
                let _ = fs::remove_dir_all(&working_dir);
                final_kept_file = Some(final_target_path);
            } else {
                let err_msg = "Clip join failed — downloaded pieces kept. Press Resume to retry join.".to_string();
                let stderr_tail: Vec<String> = concat_res.ok().map(|o| {
                    String::from_utf8_lossy(&o.stderr)
                        .lines()
                        .rev()
                        .take(20)
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect()
                }).unwrap_or_default();
                mark_task_error_with_log(&media_tasks, &tid, &err_msg, stderr_tail.clone());
                crate::append_failure_log(&app, &tid, &task_stem, &options.url, &err_msg, &stderr_tail);
                crate::spawn_save_all_tasks(&app);
                let _ = app.emit("download-error", serde_json::json!({ "id": tid.clone(), "error": err_msg, "errorLog": stderr_tail }));
                return;
            }
        } else {
            let pass_res = run_ytdlp_pass(
                &out_tmpl,
                &task_stem,
                &options.save_folder,
                if is_clipping_active { Some((&clip_start_str, &clip_end_str)) } else { None },
                0.0,
                0,
                clip_target_duration.or(options.duration),
                false,
                Arc::new(Mutex::new(None)),
                &last_percent,
                &known_duration,
            );
            match pass_res {
                PassOutcome::Success(kf) => {
                    final_kept_file = kf;
                }
                PassOutcome::Cancelled => {
                    return;
                }
                PassOutcome::Error(_) => {
                    return;
                }
            }
        }

        let live_settings = app
            .try_state::<crate::settings::SettingsState>()
            .map(|s| s.get_blocking())
            .unwrap_or_else(|| app_settings.clone());

        let final_kept_file = final_kept_file.map(|p| {
            crate::settings::maybe_move_to_auto_category(&live_settings, &p)
        });

        let kept_filename = final_kept_file
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        let final_folder = final_kept_file
            .as_ref()
            .and_then(|p| p.parent())
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| options.save_folder.clone());
        let disk_size = final_kept_file
            .as_ref()
            .and_then(|p| fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);

        let final_total = if disk_size > 0 {
            disk_size
        } else {
            media_tasks
                .lock()
                .ok()
                .and_then(|m| m.get(&tid).map(|t| t.downloaded_bytes))
                .unwrap_or(0)
        };

        if !media_tasks.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&tid) {
            return;
        }

        let mut checksum_status = String::new();
        let mut checksum_actual = String::new();
        if !options.expected_checksum.trim().is_empty() {
            if let Some(ref fpath) = final_kept_file {
                if let Ok((matched, _algo, actual)) =
                    crate::settings::verify_file_checksum(fpath, &options.expected_checksum)
                {
                    checksum_actual = actual;
                    checksum_status = if matched {
                        "verified".to_string()
                    } else {
                        "mismatch".to_string()
                    };
                } else {
                    checksum_status = "error".to_string();
                }
            }
        }

        if let Ok(mut tasks_lock) = media_tasks.lock() {
            if let Some(t) = tasks_lock.get_mut(&tid) {
                t.percent = 100.0;
                t.downloaded_bytes = final_total;
                t.total_bytes = final_total;
                t.speed_bps = 0;
                t.status = "completed".to_string();
                t.speed_formatted = "Finished".to_string();
                t.eta_formatted = "00:00:00".to_string();
                t.save_path = final_folder.clone();
                t.checksum_status = checksum_status.clone();
                t.checksum_actual = checksum_actual.clone();
                if let Some(ref kf) = kept_filename {
                    if !kf.is_empty() {
                        t.filename = kf.clone();
                    }
                }
            }
        }
        crate::spawn_save_all_tasks(&app);

        let hint_name = kept_filename
            .as_deref()
            .or(options.custom_filename.as_deref())
            .unwrap_or("");
        cleanup_temp_and_part_files(&options.save_folder, hint_name);
        let complete_payload = serde_json::json!({
            "id": tid.clone(),
            "taskId": tid.clone(),
            "filename": kept_filename.clone(),
            "save_path": final_folder.clone(),
            "percent": 100.0,
            "checksumStatus": checksum_status.clone(),
            "checksumActual": checksum_actual.clone(),
            "text": "Download Completed Successfully! ✔".to_string(),
            "speed": "Finished".to_string(),
            "speed_formatted": "Finished".to_string(),
            "speedFormatted": "Finished".to_string(),
            "eta": "00:00:00".to_string(),
            "eta_formatted": "00:00:00".to_string(),
            "etaFormatted": "00:00:00".to_string(),
            "total_bytes": final_total,
            "totalBytes": final_total,
            "downloaded_bytes": final_total,
            "downloadedBytes": final_total,
            "status": "completed".to_string(),
        });
        let _ = app.emit("download-progress", complete_payload.clone());
        let _ = app.emit("ytdlp-progress", complete_payload);
        let _ = app.emit(
            "download-complete",
            serde_json::json!({
                "id": tid.clone(),
                "filename": kept_filename,
                "save_path": final_folder,
                "percent": 100.0,
                "total_bytes": final_total,
                "downloaded_bytes": final_total,
                "status": "completed"
            }),
        );

        let effective_action = if !options.post_download_action.trim().is_empty() {
            options.post_download_action.clone()
        } else {
            live_settings.post_download_action.clone()
        };
        let effective_program = if !options.post_download_program.trim().is_empty() {
            options.post_download_program.clone()
        } else {
            live_settings.post_download_program.clone()
        };
        if let Some(ref fpath) = final_kept_file {
            let app_post = app.clone();
            let tid_post = tid.clone();
            let fpath_post = fpath.clone();
            tauri::async_runtime::spawn(async move {
                crate::settings::execute_post_download_action(
                    &app_post,
                    &tid_post,
                    &effective_action,
                    &effective_program,
                    &fpath_post,
                );
            });
        }

        let app_q = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::queue::process_queue(&app_q).await;
        });
    });
}

#[command]
pub async fn resume_media_download(
    app: AppHandle,
    state: State<'_, DownloadState>,
    task_id: String,
) -> Result<String, String> {
    let mut opts = {
        let guard = state.task_options.lock().unwrap();
        guard.get(&task_id).cloned()
    }
    .ok_or_else(|| format!("No saved media options found for task {}", task_id))?;
    opts.task_id = Some(task_id);
    opts.is_resume = true;
    if !opts.enable_clip {
        opts.overwrite_files = false;
    }
    start_download(app, state, opts).await
}

// ---------------------------------------------------------------------------
// Command 3: Cancel / Pause Active Download
// ---------------------------------------------------------------------------

#[command]
pub async fn cancel_download(
    app: AppHandle,
    state: State<'_, DownloadState>,
    task_id: Option<String>,
) -> Result<(), String> {
    let mut pids_to_kill = Vec::new();
    let mut affected_ids = Vec::new();
    {
        let mut pids = state.active_pids.lock().unwrap();
        let mut cancels = state.cancelled_tasks.lock().unwrap();
        if let Some(ref tid) = task_id {
            cancels.insert(tid.clone(), true);
            affected_ids.push(tid.clone());
            if let Some(pid) = pids.remove(tid) {
                pids_to_kill.push(pid);
            }
        } else {
            for (tid, pid) in pids.drain() {
                cancels.insert(tid.clone(), true);
                affected_ids.push(tid);
                pids_to_kill.push(pid);
            }
        }
    }
    if let Ok(mut tasks_lock) = state.tasks.lock() {
        for tid in &affected_ids {
            if let Some(t) = tasks_lock.get_mut(tid) {
                if t.status != "completed"
                    && t.status != "completed_with_warning"
                    && t.status != "error"
                {
                    t.status = "paused".to_string();
                    t.speed_bps = 0;
                    t.speed_formatted = "0 B/s".to_string();
                    t.eta_formatted = "--".to_string();
                }
            }
        }
    }
    for pid in pids_to_kill {
        #[cfg(target_os = "windows")]
        {
            let mut kill_cmd = Command::new("taskkill");
            kill_cmd.args(["/F", "/T", "/PID", &pid.to_string()]);
            kill_cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = kill_cmd.output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
        }
    }
    crate::spawn_save_all_tasks(&app);
    Ok(())
}

// ---------------------------------------------------------------------------
// Command 4: Tool Version Check & One-Click Updater (yt-dlp, Deno, FFmpeg)
// ---------------------------------------------------------------------------

fn curl_fetch_json_field(url: &str, json_path: &[&str]) -> Option<String> {
    #[cfg(target_os = "windows")]
    let mut cmd = Command::new("curl.exe");
    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new("curl");

    cmd.args([
        "-s",
        "--max-time",
        "4",
        "-L",
        "-H",
        "User-Agent: ZR-Downloader",
        url,
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output().ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let val: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let mut cur = &val;
    for key in json_path {
        cur = cur.get(*key)?;
    }
    cur.as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn clear_tool_updates_cache() {
    if let Ok(mut guard) = TOOL_UPDATES_CACHE.lock() {
        *guard = None;
    }
}

fn detect_local_ytdlp_version() -> String {
    if let Some(exe) = find_local_ytdlp() {
        make_command_from_path(&exe.to_string_lossy())
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Not Found".to_string())
    } else {
        "Not Found".to_string()
    }
}

fn detect_local_deno_version() -> String {
    if let Some(exe) = find_local_deno() {
        make_command_from_path(&exe.to_string_lossy())
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    s.lines()
                        .next()
                        .and_then(|l| l.split_whitespace().nth(1))
                        .map(|v| format!("v{}", v))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Not Found".to_string())
    } else {
        "Not Found".to_string()
    }
}

fn detect_local_ffmpeg_version() -> String {
    if let Some(exe) = ensure_ffmpeg_available() {
        make_command_from_path(&exe.to_string_lossy())
            .arg("-version")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    s.lines()
                        .next()
                        .and_then(|l| l.split_whitespace().nth(2))
                        .map(|v| v.chars().take(18).collect::<String>())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Not Found".to_string())
    } else {
        "Not Found".to_string()
    }
}

fn detect_local_aria2c_version() -> String {
    if let Some(exe) = find_local_aria2c() {
        make_command_from_path(&exe.to_string_lossy())
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    s.lines().next().map(|l| {
                        l.split_whitespace()
                            .nth(2)
                            .unwrap_or_else(|| l.trim())
                            .to_string()
                    })
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Not Found".to_string())
    } else {
        "Not Found".to_string()
    }
}

fn collect_installed_tools_local() -> Vec<ToolStatus> {
    let _ = get_augmented_path();
    std::thread::scope(|s| {
        let h_yt = s.spawn(detect_local_ytdlp_version);
        let h_deno = s.spawn(detect_local_deno_version);
        let h_ff = s.spawn(detect_local_ffmpeg_version);
        let h_a2 = s.spawn(detect_local_aria2c_version);

        vec![
            ToolStatus {
                name: "yt-dlp".to_string(),
                installed_version: h_yt.join().unwrap_or_else(|_| "Not Found".to_string()),
                latest_version: String::new(),
                has_update: false,
            },
            ToolStatus {
                name: "Deno".to_string(),
                installed_version: h_deno.join().unwrap_or_else(|_| "Not Found".to_string()),
                latest_version: String::new(),
                has_update: false,
            },
            ToolStatus {
                name: "FFmpeg".to_string(),
                installed_version: h_ff.join().unwrap_or_else(|_| "Not Found".to_string()),
                latest_version: String::new(),
                has_update: false,
            },
            ToolStatus {
                name: "aria2c".to_string(),
                installed_version: h_a2.join().unwrap_or_else(|_| "Not Found".to_string()),
                latest_version: String::new(),
                has_update: false,
            },
        ]
    })
}

fn emit_engine_progress(
    app: &AppHandle,
    tool: &str,
    stage: &str,
    percent: f64,
    downloaded_bytes: u64,
    total_bytes: u64,
    speed_bps: f64,
    message: &str,
) {
    let _ = app.emit(
        "engine-progress",
        serde_json::json!({
            "tool": tool,
            "stage": stage,
            "percent": percent,
            "downloadedBytes": downloaded_bytes,
            "totalBytes": total_bytes,
            "speedBps": speed_bps,
            "message": message,
        }),
    );
}

async fn stream_download_to_part(
    app: &AppHandle,
    tool_label: &str,
    url: &str,
    part_path: &Path,
) -> Result<u64, String> {
    let _ = fs::remove_file(part_path);
    let client = reqwest::Client::builder()
        .user_agent("ZR-Downloader")
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(1200))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error downloading {}: {}", tool_label, e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Failed to download {}: HTTP {}",
            tool_label,
            resp.status()
        ));
    }

    let total_bytes = resp.content_length().unwrap_or(0);
    emit_engine_progress(
        app,
        tool_label,
        "downloading",
        0.0,
        0,
        total_bytes,
        0.0,
        &format!("Downloading {}...", tool_label),
    );

    let mut file = tokio::fs::File::create(part_path)
        .await
        .map_err(|e| format!("Cannot create {}: {}", part_path.display(), e))?;

    let started = Instant::now();
    let mut last_emit = Instant::now();
    let mut downloaded_bytes: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk_res) = stream.next().await {
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(e) => {
                drop(file);
                let _ = fs::remove_file(part_path);
                return Err(format!("Download failed for {}: {}", tool_label, e));
            }
        };
        if let Err(e) = file.write_all(&chunk).await {
            drop(file);
            let _ = fs::remove_file(part_path);
            return Err(format!("Disk write error for {}: {}", tool_label, e));
        }
        downloaded_bytes += chunk.len() as u64;

        if last_emit.elapsed() >= Duration::from_millis(150) {
            let secs = started.elapsed().as_secs_f64().max(0.001);
            let speed_bps = downloaded_bytes as f64 / secs;
            let percent = if total_bytes > 0 {
                (downloaded_bytes as f64 / total_bytes as f64 * 100.0).min(100.0)
            } else {
                0.0
            };
            emit_engine_progress(
                app,
                tool_label,
                "downloading",
                percent,
                downloaded_bytes,
                total_bytes,
                speed_bps,
                &format!("Downloading {}...", tool_label),
            );
            last_emit = Instant::now();
        }
    }

    if let Err(e) = file.flush().await {
        drop(file);
        let _ = fs::remove_file(part_path);
        return Err(format!("Failed to flush {}: {}", part_path.display(), e));
    }
    drop(file);

    if downloaded_bytes == 0 {
        let _ = fs::remove_file(part_path);
        return Err(format!("{} download returned 0 bytes", tool_label));
    }

    let final_secs = started.elapsed().as_secs_f64().max(0.001);
    let final_speed = downloaded_bytes as f64 / final_secs;
    let final_total = total_bytes.max(downloaded_bytes);
    emit_engine_progress(
        app,
        tool_label,
        "downloading",
        100.0,
        downloaded_bytes,
        final_total,
        final_speed,
        &format!("Downloaded {}", tool_label),
    );

    Ok(downloaded_bytes)
}

#[cfg(target_os = "windows")]
fn extract_wanted_exes_from_zip(
    zip_path: &Path,
    bin_dir: &Path,
    wanted: &[&str],
) -> Result<(), String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open downloaded archive {}: {}", zip_path.display(), e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip archive: {}", e))?;

    let mut extracted: HashSet<String> = HashSet::new();
    for idx in 0..archive.len() {
        let mut entry = archive
            .by_index(idx)
            .map_err(|e| format!("Failed reading zip entry: {}", e))?;
        if entry.is_dir() {
            continue;
        }
        let matched_opt = {
            let base_name = entry
                .name()
                .rsplit(&['/', '\\'][..])
                .next()
                .unwrap_or("");
            wanted
                .iter()
                .copied()
                .find(|w| w.eq_ignore_ascii_case(base_name))
        };
        let Some(target_name) = matched_opt else {
            continue;
        };
        if extracted.contains(target_name) {
            continue;
        }

        let final_exe = bin_dir.join(target_name);
        let part_exe = bin_dir.join(format!("{}.part", target_name));
        let _ = fs::remove_file(&part_exe);

        {
            let mut out_file = fs::File::create(&part_exe)
                .map_err(|e| format!("Cannot create {}: {}", part_exe.display(), e))?;
            if let Err(e) = std::io::copy(&mut entry, &mut out_file) {
                drop(out_file);
                let _ = fs::remove_file(&part_exe);
                return Err(format!("Failed extracting {}: {}", target_name, e));
            }
            if let Err(e) = out_file.flush() {
                drop(out_file);
                let _ = fs::remove_file(&part_exe);
                return Err(format!("Failed flushing {}: {}", target_name, e));
            }
        }

        replace_from_part(&part_exe, &final_exe)?;
        extracted.insert(target_name.to_string());
    }

    for &w in wanted {
        if !extracted.contains(w) {
            return Err(format!("{} not found in the downloaded package", w));
        }
    }

    Ok(())
}

/// Downloads the latest official build of Deno / FFmpeg / aria2c into %LOCALAPPDATA%\ZR Downloader\bin
/// (a folder ZR already searches), so it works even when nothing is installed on the PC.
#[cfg(target_os = "windows")]
async fn install_engine(app: AppHandle, tool: &str) -> Result<String, String> {
    let lower = tool.to_lowercase();
    let (label, url, files): (&str, &str, Vec<&'static str>) = if lower.contains("deno") {
        (
            "Deno",
            "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip",
            vec!["deno.exe"],
        )
    } else if lower.contains("aria2") {
        (
            "aria2c",
            "https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip",
            vec!["aria2c.exe"],
        )
    } else {
        (
            "FFmpeg",
            "https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip",
            vec!["ffmpeg.exe", "ffprobe.exe"],
        )
    };

    let local = env::var("LOCALAPPDATA").map_err(|_| "LOCALAPPDATA is not set".to_string())?;
    let bin_dir = PathBuf::from(&local).join("ZR Downloader").join("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Cannot create {}: {}", bin_dir.display(), e))?;
    let zip_part = bin_dir.join(format!("{}.zip.part", lower));

    let rt = tokio::runtime::Handle::current();
    let app_for_task = app.clone();
    let zip_part_for_task = zip_part.clone();
    let bin_dir_for_task = bin_dir.clone();

    let res = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let _engine_guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let downloaded = rt.block_on(stream_download_to_part(
            &app_for_task,
            label,
            url,
            &zip_part_for_task,
        ))?;

        emit_engine_progress(
            &app_for_task,
            label,
            "extracting",
            100.0,
            downloaded,
            downloaded,
            0.0,
            &format!("Extracting {}...", label),
        );

        let extract_res =
            extract_wanted_exes_from_zip(&zip_part_for_task, &bin_dir_for_task, &files);
        let _ = fs::remove_file(&zip_part_for_task);
        extract_res?;

        if !bin_dir_for_task.join(files[0]).is_file() {
            return Err(format!(
                "{} download finished but {} was not found afterwards",
                label, files[0]
            ));
        }

        clear_tool_updates_cache();
        Ok(format!(
            "{} installed/updated to the latest version successfully!",
            label
        ))
    })
    .await
    .map_err(|e| format!("{} task error: {}", label, e))?;

    match &res {
        Ok(msg) => emit_engine_progress(&app, label, "done", 100.0, 0, 0, 0.0, msg),
        Err(err) => emit_engine_progress(&app, label, "error", 0.0, 0, 0, 0.0, err),
    }

    res
}

#[cfg(not(target_os = "windows"))]
async fn install_engine(app: AppHandle, tool: &str) -> Result<String, String> {
    let msg = format!(
        "Automatic {} install is only supported on Windows. Please install it with your package manager.",
        tool
    );
    emit_engine_progress(&app, tool, "error", 0.0, 0, 0, 0.0, &msg);
    Err(msg)
}

#[command]
pub async fn get_installed_tools() -> Result<Vec<ToolStatus>, String> {
    tauri::async_runtime::spawn_blocking(collect_installed_tools_local)
        .await
        .map_err(|e| format!("Check error: {}", e))
}

#[command]
pub async fn check_tool_updates() -> Result<Vec<ToolStatus>, String> {
    if let Ok(cache) = TOOL_UPDATES_CACHE.lock() {
        if let Some((checked_at, list)) = cache.as_ref() {
            if checked_at.elapsed() < Duration::from_secs(600) {
                return Ok(list.clone());
            }
        }
    }

    let list = tauri::async_runtime::spawn_blocking(move || {
        let _ = get_augmented_path();
        std::thread::scope(|s| {
            let h_yt_local = s.spawn(detect_local_ytdlp_version);
            let h_deno_local = s.spawn(detect_local_deno_version);
            let h_ff_local = s.spawn(detect_local_ffmpeg_version);
            let h_a2_local = s.spawn(detect_local_aria2c_version);

            let h_yt_remote = s.spawn(|| {
                curl_fetch_json_field(
                    "https://pypi.org/pypi/yt-dlp/json",
                    &["info", "version"],
                )
            });
            let h_deno_remote = s.spawn(|| {
                curl_fetch_json_field(
                    "https://api.github.com/repos/denoland/deno/releases/latest",
                    &["tag_name"],
                )
            });
            let h_ff_remote = s.spawn(|| {
                curl_fetch_json_field(
                    "https://api.github.com/repos/yt-dlp/FFmpeg-Builds/releases/latest",
                    &["tag_name"],
                )
            });
            let h_a2_remote = s.spawn(|| {
                curl_fetch_json_field(
                    "https://api.github.com/repos/aria2/aria2/releases/latest",
                    &["tag_name"],
                )
            });

            let ytdlp_ver = h_yt_local.join().unwrap_or_else(|_| "Not Found".to_string());
            let deno_ver = h_deno_local.join().unwrap_or_else(|_| "Not Found".to_string());
            let ffmpeg_ver = h_ff_local.join().unwrap_or_else(|_| "Not Found".to_string());
            let aria2_ver = h_a2_local.join().unwrap_or_else(|_| "Not Found".to_string());

            let latest_ytdlp = h_yt_remote
                .join()
                .ok()
                .flatten()
                .unwrap_or_else(|| ytdlp_ver.clone());
            let has_ytdlp_update = ytdlp_ver == "Not Found"
                || (!latest_ytdlp.is_empty()
                    && latest_ytdlp != "Not Found"
                    && ytdlp_ver != latest_ytdlp);

            let latest_deno_raw = h_deno_remote.join().ok().flatten().unwrap_or_default();
            let deno_missing = deno_ver == "Not Found";
            let deno_has_update = deno_missing
                || (!latest_deno_raw.is_empty()
                    && latest_deno_raw.trim_start_matches('v')
                        != deno_ver.trim_start_matches('v'));
            let deno_latest_display = if latest_deno_raw.is_empty() {
                deno_ver.clone()
            } else {
                latest_deno_raw
            };

            let latest_ff_raw = h_ff_remote.join().ok().flatten().unwrap_or_default();
            let ffmpeg_missing = ffmpeg_ver == "Not Found";
            let ffmpeg_latest_display = if latest_ff_raw.is_empty()
                || latest_ff_raw.eq_ignore_ascii_case("latest")
            {
                "Latest build".to_string()
            } else {
                latest_ff_raw
            };

            let latest_a2_raw = h_a2_remote.join().ok().flatten().unwrap_or_default();
            let latest_a2_clean = latest_a2_raw
                .trim_start_matches("release-")
                .trim_start_matches('v')
                .to_string();
            let aria2_missing = aria2_ver == "Not Found";
            let aria2_has_update = aria2_missing
                || (!latest_a2_clean.is_empty()
                    && latest_a2_clean != aria2_ver.trim_start_matches('v'));
            let aria2_latest_display = if latest_a2_clean.is_empty() {
                aria2_ver.clone()
            } else {
                latest_a2_clean
            };

            vec![
                ToolStatus {
                    name: "yt-dlp".to_string(),
                    installed_version: ytdlp_ver,
                    latest_version: latest_ytdlp,
                    has_update: has_ytdlp_update,
                },
                ToolStatus {
                    name: "Deno".to_string(),
                    installed_version: deno_ver,
                    latest_version: deno_latest_display,
                    has_update: deno_has_update,
                },
                ToolStatus {
                    name: "FFmpeg".to_string(),
                    installed_version: ffmpeg_ver,
                    latest_version: ffmpeg_latest_display,
                    has_update: ffmpeg_missing,
                },
                ToolStatus {
                    name: "aria2c".to_string(),
                    installed_version: aria2_ver,
                    latest_version: aria2_latest_display,
                    has_update: aria2_has_update,
                },
            ]
        })
    })
    .await
    .map_err(|e| format!("Check error: {}", e))?;

    if let Ok(mut cache) = TOOL_UPDATES_CACHE.lock() {
        *cache = Some((Instant::now(), list.clone()));
    }

    Ok(list)
}

#[command]
pub async fn update_tool(
    app: AppHandle,
    tool_name: Option<String>,
    name: Option<String>,
) -> Result<String, String> {
    let resolved = tool_name.or(name).unwrap_or_else(|| "yt-dlp".to_string());
    let lower = resolved.to_lowercase();

    if lower.contains("yt-dlp") {
        #[cfg(target_os = "windows")]
        let (target_dir, target_exe, url) = {
            let local_app_data = env::var("LOCALAPPDATA")
                .map_err(|_| "LOCALAPPDATA is not set".to_string())?;
            let dir = PathBuf::from(&local_app_data).join("ZR Downloader").join("bin");
            let exe = dir.join("yt-dlp.exe");
            (
                dir,
                exe,
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
            )
        };

        #[cfg(not(target_os = "windows"))]
        let (target_dir, target_exe, url) = {
            let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
            let dir = PathBuf::from(&home).join(".local").join("bin");
            let exe = dir.join("yt-dlp");
            (
                dir,
                exe,
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp",
            )
        };

        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Cannot create {}: {}", target_dir.display(), e))?;
        let part_exe = PathBuf::from(format!("{}.part", target_exe.display()));

        let rt = tokio::runtime::Handle::current();
        let app_for_task = app.clone();
        let part_for_task = part_exe.clone();
        let final_for_task = target_exe.clone();

        let res = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
            let _engine_guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());

            rt.block_on(stream_download_to_part(
                &app_for_task,
                "yt-dlp",
                url,
                &part_for_task,
            ))?;

            #[cfg(not(target_os = "windows"))]
            {
                let _ = Command::new("chmod")
                    .args(["+x", part_for_task.to_string_lossy().as_ref()])
                    .status();
            }

            replace_from_part(&part_for_task, &final_for_task)?;
            clear_tool_updates_cache();
            Ok("yt-dlp updated to the latest version successfully!".to_string())
        })
        .await
        .map_err(|e| format!("yt-dlp update task failed: {}", e))?;

        match &res {
            Ok(msg) => emit_engine_progress(&app, "yt-dlp", "done", 100.0, 0, 0, 0.0, msg),
            Err(err) => emit_engine_progress(&app, "yt-dlp", "error", 0.0, 0, 0, 0.0, err),
        }

        res
    } else if lower.contains("deno") {
        install_engine(app, "deno").await
    } else if lower.contains("aria2") {
        install_engine(app, "aria2c").await
    } else {
        install_engine(app, "ffmpeg").await
    }
}

// ---------------------------------------------------------------------------
// Command 5: Open Folder or File in Native File Manager / OS Associated App
// ---------------------------------------------------------------------------

#[command]
pub async fn open_file(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if p.exists() && p.is_file() {
        return launch_file(&path);
    }

    // Try appending common media/document extensions if missing
    for ext in &["mp4", "mkv", "webm", "mp3", "m4a", "ts", "zip", "exe", "pdf"] {
        let candidate = PathBuf::from(format!("{}.{}", path, ext));
        if candidate.exists() && candidate.is_file() {
            return launch_file(&candidate.to_string_lossy());
        }
    }

    // Check parent directory for matching file stem
    if let Some(parent) = p.parent() {
        if let Some(file_stem) = p.file_stem().and_then(|s| s.to_str()) {
            if let Ok(entries) = fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let ep = entry.path();
                    if ep.is_file() {
                        if let Some(stem) = ep.file_stem().and_then(|s| s.to_str()) {
                            if stem == file_stem || stem.starts_with(file_stem) {
                                return launch_file(&ep.to_string_lossy());
                            }
                        }
                    }
                }
            }
        }
    }
    Err(format!("File does not exist: {}", path))
}

fn launch_file(file_path: &str) -> Result<(), String> {
    open::that(file_path).map_err(|e| format!("Failed to open file: {}", e))
}

#[command]
pub async fn open_folder(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    #[cfg(target_os = "windows")]
    {
        if p.is_file() {
            Command::new("explorer")
                .args(["/select,", &path])
                .spawn()
                .map_err(|e| format!("Failed to open folder: {}", e))?;
            return Ok(());
        }
        let folder = if p.is_dir() { path.clone() } else {
            p.parent().and_then(|parent| parent.to_str()).unwrap_or(&path).to_string()
        };
        let _ = fs::create_dir_all(&folder);
        Command::new("explorer")
            .arg(&folder)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        let folder = if p.is_dir() { path.clone() } else {
            p.parent().and_then(|parent| parent.to_str()).unwrap_or(&path).to_string()
        };
        let _ = fs::create_dir_all(&folder);
        Command::new("open")
            .arg(&folder)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let folder = if p.is_dir() { path.clone() } else {
            p.parent().and_then(|parent| parent.to_str()).unwrap_or(&path).to_string()
        };
        let _ = fs::create_dir_all(&folder);
        Command::new("xdg-open")
            .arg(&folder)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// App update check (GitHub Releases) - the 1-click installer below uses its result
// ---------------------------------------------------------------------------

/// GitHub "owner/repo" whose Releases are checked for new app versions.
const APP_UPDATE_REPO: &str = "golam-mowla/zr-downloader";

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_name: String,
    pub release_notes: String,
    pub release_url: String,
    pub published_at: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
}

fn parse_version_parts(v: &str) -> Vec<u64> {
    let v = v.trim().trim_start_matches(|c: char| c == 'v' || c == 'V');
    let core = v.split(|c: char| c == '-' || c == '+').next().unwrap_or("");
    core.split('.')
        .map(|p| p.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

/// True when `latest` is a strictly higher version than `current` ("v1.2.39" vs "1.2.38").
fn is_newer_version(latest: &str, current: &str) -> bool {
    let a = parse_version_parts(latest);
    let b = parse_version_parts(current);
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Picks the installer asset that fits this OS: (name, url, size).
fn pick_update_asset(assets: &[serde_json::Value]) -> Option<(String, String, u64)> {
    let mut best: Option<(i32, String, String, u64)> = None;
    for a in assets {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let url = a
            .get("browser_download_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() || url.is_empty() {
            continue;
        }
        let low = name.to_lowercase();
        let size = a.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        let score: i32 = if cfg!(target_os = "windows") {
            if low.ends_with(".exe") && (low.contains("setup") || low.contains("install")) {
                100
            } else if low.ends_with(".exe") {
                80
            } else {
                0
            }
        } else if cfg!(target_os = "macos") {
            if low.ends_with(".dmg") {
                100
            } else if low.ends_with(".zip")
                && (low.contains("mac") || low.contains("darwin") || low.contains("osx"))
            {
                70
            } else {
                0
            }
        } else if low.ends_with(".appimage") {
            100
        } else {
            0
        };
        if score == 0 {
            continue;
        }
        let better = match &best {
            None => true,
            Some(b) => score > b.0,
        };
        if better {
            best = Some((score, name.to_string(), url.to_string(), size));
        }
    }
    best.map(|(_, n, u, sz)| (n, u, sz))
}

/// Checks GitHub Releases for a newer app version. Never installs anything by itself.
#[command]
pub async fn check_app_update(app: AppHandle) -> Result<AppUpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let proxy = app
        .try_state::<crate::settings::SettingsState>()
        .map(|s| s.get_blocking())
        .and_then(|s| s.effective_proxy_url());

    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("ZR-Downloader/{}", current));
    if let Some(p) = proxy {
        if !p.trim().is_empty() {
            if let Ok(px) = reqwest::Proxy::all(p.trim()) {
                builder = builder.proxy(px);
            }
        }
    }
    let client = builder.build().map_err(|e| e.to_string())?;

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        APP_UPDATE_REPO
    );
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {}", e))?;

    let code = resp.status().as_u16();
    if code == 404 {
        // no published (non-draft, non-prerelease) release yet
        return Ok(AppUpdateInfo {
            current_version: current,
            ..Default::default()
        });
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", code));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let assets: Vec<serde_json::Value> = json
        .get("assets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let (asset_name, asset_url, asset_size) = pick_update_asset(&assets).unwrap_or_default();
    let mut notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if notes.chars().count() > 4000 {
        notes = notes.chars().take(4000).collect::<String>() + "...";
    }

    Ok(AppUpdateInfo {
        update_available: !tag.is_empty() && is_newer_version(&tag, &current),
        current_version: current,
        latest_version: tag.trim_start_matches(|c: char| c == 'v' || c == 'V').to_string(),
        release_name: json
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        release_notes: notes,
        release_url: json
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        published_at: json
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        asset_name,
        asset_url,
        asset_size,
    })
}

/// Opens this project's GitHub release page in the default browser (nothing else is allowed).
#[command]
pub async fn open_release_page(url: String) -> Result<(), String> {
    let allowed = format!("https://github.com/{}/", APP_UPDATE_REPO);
    if !url.starts_with(&allowed) {
        return Err("Only this project's GitHub pages can be opened.".to_string());
    }
    open::that(&url).map_err(|e| format!("Failed to open browser: {}", e))
}

/// File extension of the download URL's last path segment (query stripped), or `default`.
#[allow(dead_code)]
fn update_file_ext(url: &str, default: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let last = path.rsplit('/').next().unwrap_or("");
    match last.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() && ext.len() <= 8 && ext.chars().all(|c| c.is_ascii_alphanumeric()) => {
            ext.to_lowercase()
        }
        _ => default.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Command 6: 1-Click App Updater (Downloads & Launches Installer from GitHub)
// ---------------------------------------------------------------------------

#[command]
pub async fn install_app_update(_app: AppHandle, download_url: String) -> Result<String, String> {
    // This command downloads AND RUNS the file, so it must never accept an arbitrary URL.
    let allowed_prefix = format!("https://github.com/{}/releases/download/", APP_UPDATE_REPO);
    if !download_url.starts_with(&allowed_prefix) {
        return Err("Refusing to install: the update must come from this app's GitHub Releases.".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let temp_dir = env::temp_dir();

        #[cfg(target_os = "windows")]
        {
            let installer_path = temp_dir.join("GM-Downloader-Setup.exe");
            let installer_str = installer_path.to_string_lossy().to_string();

            // Try curl.exe first
            let curl_res = Command::new("curl.exe")
                .args(["-L", "-f", "-s", "-S", "-o", &installer_str, &download_url])
                .status();

            let success = match curl_res {
                Ok(s) if s.success() => true,
                _ => {
                    let ps_script = format!(
                        "$ProgressPreference = 'SilentlyContinue'; [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (New-Object System.Net.WebClient).DownloadFile('{}', '{}')",
                        download_url,
                        installer_str.replace('\\', "\\\\")
                    );
                    let mut cmd = Command::new("powershell");
                    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
                    cmd.creation_flags(CREATE_NO_WINDOW);
                    cmd.status().map(|s| s.success()).unwrap_or(false)
                }
            };

            if !success || !installer_path.exists() {
                return Err("Failed to download update installer from GitHub.".to_string());
            }

            // Launch installer in background
            let _ = Command::new(&installer_path).spawn();

            // Exit current application after slight delay to prevent file locks
            std::thread::sleep(std::time::Duration::from_millis(1000));
            std::process::exit(0);
        }

        #[cfg(target_os = "macos")]
        {
            let dest = temp_dir.join(format!("GM-Downloader-Update.{}", update_file_ext(&download_url, "dmg")));
            let dest_str = dest.to_string_lossy().to_string();
            let status = Command::new("curl")
                .args(["-L", "-f", "-o", &dest_str, &download_url])
                .status()
                .map_err(|e| format!("Failed to download update: {}", e))?;
            if status.success() {
                let _ = Command::new("open").arg(&dest_str).spawn();
                std::thread::sleep(std::time::Duration::from_millis(500));
                std::process::exit(0);
            }
            return Err("Failed to download macOS update.".to_string());
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            let dest = temp_dir.join("GM-Downloader-Linux.AppImage");
            let dest_str = dest.to_string_lossy().to_string();
            let status = Command::new("curl")
                .args(["-L", "-f", "-o", &dest_str, &download_url])
                .status()
                .map_err(|e| format!("Failed to download update: {}", e))?;
            if status.success() {
                let _ = Command::new("chmod").args(["+x", &dest_str]).status();
                let _ = Command::new(&dest_str).spawn();
                std::thread::sleep(std::time::Duration::from_millis(500));
                std::process::exit(0);
            }
            return Err("Failed to download Linux update.".to_string());
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[allow(dead_code)]
fn decode_percent_encoded(input: &str) -> String {
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

#[cfg(test)]
mod resume_safety_tests {
    use super::*;

    fn touch(dir: &Path, name: &str, size: usize) {
        fs::write(dir.join(name), vec![1u8; size]).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn phase_percent_never_goes_back_to_zero_for_audio() {
        // 2 formats: video fills 0..90, audio 90..99
        assert_eq!(multi_phase_percent(100.0, 0, 2, false), 90.0);
        assert_eq!(multi_phase_percent(0.0, 1, 2, false), 90.0);
        assert!(multi_phase_percent(50.0, 1, 2, false) > 90.0 && multi_phase_percent(100.0, 1, 2, false) == 99.0);
        // Audio first: audio fills 0..15, video fills 15..99
        assert_eq!(multi_phase_percent(100.0, 0, 2, true), 15.0);
        assert_eq!(multi_phase_percent(0.0, 1, 2, true), 15.0);
        assert_eq!(multi_phase_percent(100.0, 1, 2, true), 99.0);
        // single format is capped at 99.0 during downloading, 3 formats are split evenly
        assert_eq!(multi_phase_percent(42.0, 0, 1, false), 42.0);
        assert_eq!(multi_phase_percent(100.0, 0, 1, false), 99.0);
        assert!((multi_phase_percent(100.0, 2, 3, false) - 99.0).abs() < 1e-9);

        // Audio format file detection
        assert!(is_audio_format_file("Live.f140.m4a"));
        assert!(is_audio_format_file("track.f251.webm"));
        assert!(is_audio_format_file("song.opus"));
        assert!(!is_audio_format_file("Live.f137.mp4"));
        assert!(!is_audio_format_file("video.webm"));
    }

    #[test]
    fn parses_format_count_and_format_files() {
        assert_eq!(parse_format_count("[info] abc: Downloading 1 format(s): hls-1080+hls-audio"), Some(2));
        assert_eq!(parse_format_count("[info] abc: Downloading 1 format(s): best"), Some(1));
        assert_eq!(parse_format_count("[download] 10%"), None);
        assert_eq!(
            format_file_from_line("[download] Destination: C:\\v\\Live.f137.mp4"),
            Some("C:\\v\\Live.f137.mp4".to_string())
        );
        assert_eq!(
            format_file_from_line("[download] Live.f140.m4a has already been downloaded"),
            Some("Live.f140.m4a".to_string())
        );
        assert_eq!(format_file_from_line("[download]  45.0% of 10MiB"), None);
    }

    #[test]
    fn merge_progress_counts_parts_and_temp_output() {
        let d = fresh_dir("mergehb");
        touch(&d, "Kick Stream.f137.mp4", 1000);
        touch(&d, "Kick Stream.f140.m4a", 200);
        touch(&d, "Kick Stream.temp.mp4", 600);
        touch(&d, "Other Show.f137.mp4", 5000);
        let (src, out) = merge_progress_bytes(d.to_str().unwrap(), "Kick Stream");
        assert_eq!((src, out), (1200, 600));
    }

    #[test]
    fn version_compare_handles_v_prefix_and_lengths() {
        assert!(is_newer_version("v1.2.39", "1.2.38"));
        assert!(is_newer_version("1.3.0", "1.2.99"));
        assert!(is_newer_version("2.0", "1.9.9"));
        assert!(!is_newer_version("v1.2.38", "1.2.38"));
        assert!(!is_newer_version("1.2.37", "1.2.38"));
        assert!(!is_newer_version("1.2", "1.2.0"));
    }

    #[test]
    fn picks_installer_asset_and_ext() {
        let assets = vec![
            serde_json::json!({"name":"notes.txt","browser_download_url":"https://x/notes.txt","size":1}),
            serde_json::json!({"name":"ZR-Downloader_1.2.39_x64-setup.exe","browser_download_url":"https://x/s.exe","size":99}),
            serde_json::json!({"name":"ZR-Downloader_1.2.39_amd64.AppImage","browser_download_url":"https://x/a.AppImage","size":98}),
            serde_json::json!({"name":"ZR-Downloader_1.2.39.dmg","browser_download_url":"https://x/m.dmg","size":97}),
        ];
        let picked = pick_update_asset(&assets).expect("an installer for this OS");
        if cfg!(target_os = "windows") {
            assert!(picked.0.ends_with("setup.exe"));
        } else if cfg!(target_os = "macos") {
            assert!(picked.0.ends_with(".dmg"));
        } else {
            assert!(picked.0.ends_with(".AppImage"));
        }
        assert_eq!(update_file_ext("https://github.com/o/r/releases/download/v1/App.dmg?x=1", "zip"), "dmg");
        assert_eq!(update_file_ext("https://github.com/o/r/releases/download/v1/noext", "zip"), "zip");
    }

    fn fresh_dir(tag: &str) -> PathBuf {
        let d = env::temp_dir().join(format!("zr_media_test_{}_{}", tag, uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn postprocess_cleanup_keeps_downloaded_data() {
        let d = fresh_dir("pp");
        touch(&d, "Kick Stream.fhls-1080p60.mp4", 100);
        touch(&d, "Kick Stream.fhls-audio-160.mp4", 100);
        touch(&d, "Kick Stream.temp.mp4", 100);
        clean_postprocess_leftovers(d.to_str().unwrap(), "Kick Stream");
        assert_eq!(
            names(&d),
            vec!["Kick Stream.fhls-1080p60.mp4", "Kick Stream.fhls-audio-160.mp4"]
        );
        assert!(has_partial_data(d.to_str().unwrap(), "Kick Stream"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn stem_matching_does_not_touch_other_tasks() {
        let d = fresh_dir("stem");
        touch(&d, "Highlights Day 2 [abc].mp4.part", 10);
        touch(&d, "Highlights.mp4.part", 10);
        clean_partial_files(d.to_str().unwrap(), "Highlights");
        assert_eq!(names(&d), vec!["Highlights Day 2 [abc].mp4.part"]);
        let _ = fs::remove_dir_all(&d);

        let d = fresh_dir("id");
        touch(&d, "Live [aaa].mp4.part", 10);
        touch(&d, "Live [bbb].mp4.part", 10);
        clean_partial_files(d.to_str().unwrap(), "Live [aaa]");
        assert_eq!(names(&d), vec!["Live [bbb].mp4.part"]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn empty_stem_never_wipes_folder() {
        let d = fresh_dir("empty");
        touch(&d, "Movie A.mp4.part", 10);
        touch(&d, "Other Show.mp4.part", 10);
        clean_partial_files(d.to_str().unwrap(), "");
        assert_eq!(names(&d).len(), 2);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn disk_full_detection() {
        assert!(is_disk_full_error(&[
            "OSError: [Errno 28] No space left on device".to_string()
        ]));
        assert!(!is_disk_full_error(&["HTTP Error 403: Forbidden".to_string()]));
    }
}


#[cfg(test)]
mod clip_chunk_tests {
    use super::*;

    #[test]
    fn piece_planning_exact_multiple() {
        let pieces = plan_clip_pieces(0.0, 600.0, CLIP_PIECE_SECS, "mp4");
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].index, 1);
        assert_eq!(pieces[0].start_sec, 0.0);
        assert_eq!(pieces[0].end_sec, 300.0);
        assert_eq!(pieces[0].filename, "piece_0001.mp4");
        assert!(!pieces[0].completed);

        assert_eq!(pieces[1].index, 2);
        assert_eq!(pieces[1].start_sec, 300.0);
        assert_eq!(pieces[1].end_sec, 600.0);
        assert_eq!(pieces[1].filename, "piece_0002.mp4");
        assert!(!pieces[1].completed);
    }

    #[test]
    fn piece_planning_remainder() {
        let pieces = plan_clip_pieces(100.0, 750.0, CLIP_PIECE_SECS, "mp4");
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].start_sec, 100.0);
        assert_eq!(pieces[0].end_sec, 400.0);
        assert_eq!(pieces[0].filename, "piece_0001.mp4");

        assert_eq!(pieces[1].start_sec, 400.0);
        assert_eq!(pieces[1].end_sec, 700.0);
        assert_eq!(pieces[1].filename, "piece_0002.mp4");

        assert_eq!(pieces[2].start_sec, 700.0);
        assert_eq!(pieces[2].end_sec, 750.0);
        assert_eq!(pieces[2].filename, "piece_0003.mp4");
    }

    #[test]
    fn piece_planning_short_clip() {
        let pieces = plan_clip_pieces(0.0, 150.0, CLIP_PIECE_SECS, "mp4");
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].start_sec, 0.0);
        assert_eq!(pieces[0].end_sec, 150.0);

        let clip_duration = 150.0;
        let is_chunked = clip_duration > CLIP_PIECE_SECS;
        assert!(!is_chunked);
    }

    #[test]
    fn piece_planning_inf_end() {
        let clip_end_str = "inf";
        let is_chunked = clip_end_str != "inf";
        assert!(!is_chunked);
    }

    #[test]
    fn state_json_save_load_round_trip() {
        let tmp = env::temp_dir().join(format!("zr_clip_state_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();

        let state = ClipState {
            task_id: "test_task_abc".to_string(),
            total_clip_duration: 650.0,
            target_filename: "Nature Video [Clip 00-01-40 to 00-12-30].mp4".to_string(),
            pieces: plan_clip_pieces(100.0, 750.0, CLIP_PIECE_SECS, "mp4"),
        };

        let save_res = save_clip_state(&tmp, &state);
        assert!(save_res.is_ok());

        let loaded = load_clip_state(&tmp);
        assert_eq!(loaded, Some(state));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn state_json_corrupt_file_fallback() {
        let tmp = env::temp_dir().join(format!("zr_clip_corrupt_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();

        fs::write(tmp.join("state.json"), "{ invalid json: true, not complete }").unwrap();

        let loaded = load_clip_state(&tmp);
        assert_eq!(loaded, None);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn concat_list_file_generation_paths_with_spaces_and_apostrophes() {
        let paths = vec![
            "piece 1.mp4".to_string(),
            "C:\\Videos\\Rock 'n' Roll\\piece '2'.mp4".to_string(),
        ];
        let content = generate_concat_list_content(&paths);
        assert_eq!(
            content,
            "file 'piece 1.mp4'\nfile 'C:/Videos/Rock '\\''n'\\'' Roll/piece '\\''2'\\''.mp4'\n"
        );
    }
}
