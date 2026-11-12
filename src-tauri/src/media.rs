
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
use std::io::{BufRead, BufReader, Write};
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
    if let Ok(mut lock) = tasks.lock() {
        if let Some(t) = lock.get_mut(tid) {
            t.status = "error".to_string();
            t.error_message = Some(msg.to_string());
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
                "10",
                "--max-time",
                "180",
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

            // 2. Try PowerShell Invoke-WebRequest fallback into .part then rename
            let ps_script = format!(
                "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -TimeoutSec 120 -Uri 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' -OutFile '{}'",
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
    if unit.starts_with("gib") || unit.starts_with("gb") {
        (val * 1024.0 * 1024.0 * 1024.0) as u64
    } else if unit.starts_with("mib") || unit.starts_with("mb") {
        (val * 1024.0 * 1024.0) as u64
    } else if unit.starts_with("kib") || unit.starts_with("kb") {
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

    if let Ok(mut tasks_lock) = media_tasks.lock() {
        if let Some(t) = tasks_lock.get_mut(tid) {
            t.percent = pct;
            if size_bytes > 0 {
                t.downloaded_bytes = size_bytes;
            }
            if !speed.is_empty() {
                t.speed_formatted = speed.to_string();
            }
            t.status = "downloading".to_string();
        }
    }

    let payload = serde_json::json!({
        "id": tid,
        "taskId": tid,
        "percent": pct,
        "text": text,
        "speed": speed,
        "speed_formatted": speed,
        "speedFormatted": speed,
        "downloaded_bytes": size_bytes,
        "downloadedBytes": size_bytes,
        "eta": "--",
        "eta_formatted": "--",
        "etaFormatted": "--",
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
    trimmed.to_string()
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
    let has_browser_cookie = cookies_browser
        .as_deref()
        .map(|b| b != "None" && !b.trim().is_empty())
        .unwrap_or(false);

    for attempt_without_browser in [false, true] {
        if attempt_without_browser && !has_browser_cookie {
            break;
        }
        let u = target_url.clone();
        let cb = if attempt_without_browser {
            None
        } else {
            cookies_browser.clone()
        };
        let cf = cookies_file.clone();
        let pr = proxy.clone();
        let da = discovery_args.clone();

        let output = tauri::async_runtime::spawn_blocking(move || {
            let mut cmd = make_command("yt-dlp");
            cmd.args([
                "--dump-single-json",
                "--no-playlist",
                "--no-check-certificates",
                "--no-warnings",
                "--no-write-comments",
                "--skip-unavailable-fragments",
                "--socket-timeout",
                "15",
                "--extractor-retries",
                "1",
            ]);

            if find_exe_in_augmented_path("deno").is_some() || ensure_deno_available().is_some() {
                cmd.args(["--js-runtimes", "deno"]);
            } else if find_exe_in_augmented_path("node").is_some() {
                cmd.args(["--js-runtimes", "node"]);
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

        let final_err = if combined.trim().is_empty() {
            "Failed to fetch metadata. Please verify the link or network connection.".to_string()
        } else {
            combined.trim().to_string()
        };
        return Err(format!("yt-dlp fetch failed: {}", final_err));
    }

    Err("yt-dlp fetch failed".to_string())
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
            let _ = tokio::time::timeout(Duration::from_secs(45), rx.changed()).await;
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

    // Run default extractor first (yt-dlp + Deno returns ALL qualities including 1080p & 1080p Premium);
    // never race `android,web` in parallel because `android,web` finishes fast with ONLY 360p (format 18).
    let url_low = clean_url.to_lowercase();
    let is_youtube = url_low.contains("youtube.com") || url_low.contains("youtu.be");
    let attempts: Vec<Option<&'static str>> = if is_youtube {
        vec![
            None,
            Some("youtube:player_client=tv,web"),
            Some("youtube:player_client=android,web"),
        ]
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
    let clean_name: String = name_lower.chars().filter(|c| c.is_alphanumeric()).collect();

    name_lower.starts_with(&stem_lower)
        || clean_name.starts_with(&clean_hint)
        || clean_name.contains(&clean_hint)
        || (clean_base_no_id.len() >= 3
            && (clean_name.starts_with(&clean_base_no_id) || clean_name.contains(&clean_base_no_id)))
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
pub fn clean_partial_files(save_folder: &str, stem: &str) {
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
        let is_partial = is_partial_media_filename(&name_lower);
        let matches = name_matches_stem(&name_lower, stem);
        if is_partial {
            if matches || strip_media_extension_for_stem(stem).len() < 3 {
                let _ = fs::remove_file(&path);
            }
        } else if matches && !name_lower.starts_with(".zr_sparse_meta_") && !is_readable_nonempty_file(&path) {
            // Delete 0-byte or unreadable final file for this output stem
            let _ = fs::remove_file(&path);
        }
    }
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
        low_disk_space: options.low_disk_space.unwrap_or(false),
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
    let Some(options) = state.task_options.lock().unwrap().get(&task_id).cloned() else {
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
    tauri::async_runtime::spawn_blocking(move || {
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

        if options.enable_clip && !is_subtitles_only {
            let s_raw = options.start_time.trim();
            let e_raw = options.end_time.trim();

            let s_sec = parse_time_to_seconds(s_raw).unwrap_or(0.0);
            let e_sec_opt = parse_time_to_seconds(e_raw);

            if let Some(e_sec) = e_sec_opt {
                if e_sec > s_sec {
                    is_clipping_active = true;
                    clip_target_duration = Some(e_sec - s_sec);
                    clip_start_str = format_seconds_to_timestamp(s_sec);
                    clip_end_str = format_seconds_to_timestamp(e_sec);
                    suffix_tag = format!(" [Clip {} to {}]", clip_start_str.replace(':', "-"), clip_end_str.replace(':', "-"));
                } else if s_sec > 0.0 {
                    is_clipping_active = true;
                    clip_start_str = format_seconds_to_timestamp(s_sec);
                    clip_end_str = "inf".to_string();
                    suffix_tag = format!(" [Clip {} to End]", clip_start_str.replace(':', "-"));
                }
            } else if s_sec > 0.0 || e_raw.eq_ignore_ascii_case("inf") {
                is_clipping_active = true;
                clip_start_str = format_seconds_to_timestamp(s_sec);
                clip_end_str = "inf".to_string();
                suffix_tag = format!(" [Clip {} to End]", clip_start_str.replace(':', "-"));
            }
        }

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
        let host_lower = origin_of(&url_lower);
        let is_youtube = url_lower.contains("youtube.com") || url_lower.contains("youtu.be");
        let is_low_disk = options.low_disk_space.unwrap_or(false) || !options.use_part_files;
        let is_hls = url_lower.contains("m3u8")
            || url_lower.contains("/hls/")
            || url_lower.contains("playlist.m3u8")
            || url_lower.contains(".mpd")
            || host_lower.contains("kick.com")
            || host_lower.contains("twitch.tv")
            || host_lower.contains("ivs");
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
        let build_cmd = |attempt_stage: u8, active_url: &str| -> Command {
            let mut cmd = make_command("yt-dlp");
            cmd.args([
                "--newline",
                "--progress",
                "--no-playlist",
                "--socket-timeout",
                "20",
                "--extractor-retries",
                "3",
                "--throttled-rate",
                "100K",
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
            // For attempt_stage 0 (including HLS): use --continue to preserve partial data.
            // For attempt_stage >= 1 (retry/fallback): use --no-continue for a clean restart.
            let continue_flag = if is_resume_run || options.is_resume || attempt_stage == 0 {
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
                    "-o", &out_tmpl,
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
                        "--skip-unavailable-fragments",
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
                    "-o", &out_tmpl,
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
        if is_resume_run || options.is_resume {
            cmd.arg("--no-overwrites");
        } else if (options.overwrite_files && !options.is_resume) || is_clipping_active || attempt_stage >= 1 {
            cmd.arg("--force-overwrites");
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
        if is_clipping_active {
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
        if eff_browser != "None" && !eff_browser.trim().is_empty() {
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
            && attempt_stage != 2
        {
            let aria_conns = if options.aria2_connections > 0 {
                options.aria2_connections.clamp(1, 16)
            } else {
                (live_settings.default_connections as u32).clamp(1, 16)
            };
            let mut aria_args = format!("-x {} -s {} -k 1M", aria_conns, aria_conns);
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

            cmd.arg(active_url.trim());
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd
        };

        let initial_pct = if is_resume_run && preserved_percent > 0.0 {
            preserved_percent
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
        let run_started_at = std::time::SystemTime::now();
        let mut attempt_stage: u8 = 0;
        let mut network_retries: usize = 0;
        let mut expired_refreshed = false;
        let mut current_media_url = options.url.clone();
        let mut last_total_bytes: u64 = preserved_total;
        let mut last_downloaded_bytes: u64 = preserved_downloaded;
        let mut current_stem = task_stem.clone();
        let detected_output_path: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

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
                let mut cmd = build_cmd(attempt_stage, &current_media_url);
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
                    let err_detail = last_spawn_err
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "Unknown spawn error".to_string());
                    let err_msg = format!("Failed to start yt-dlp download process: {}", err_detail);
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
                    let app_q = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::queue::process_queue(&app_q).await;
                    });
                    return;
                }
            };

            active_pids.lock().unwrap().insert(tid.clone(), child.id());

            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // Read stderr in background thread to capture errors, Duration:, & FFmpeg progress
            let err_lines = Arc::new(Mutex::new(Vec::<String>::new()));
            let err_lines_clone = err_lines.clone();
            let app_err = app.clone();
            let tid_err = tid.clone();
            let media_tasks_err = media_tasks.clone();
            let known_dur_err = known_duration.clone();
            let last_pct_err = last_percent.clone();
            let stderr_handle = stderr.map(|se| {
                std::thread::spawn(move || {
                    let reader = BufReader::new(se);
                    for line in reader.lines().flatten() {
                        let trimmed = line.trim().to_string();
                        if !trimmed.is_empty() {
                            if let Some(dur) = parse_ffmpeg_duration(&trimmed) {
                                let mut kd = known_dur_err.lock().unwrap();
                                if kd.is_none() {
                                    *kd = Some(dur);
                                }
                            }

                            if let Some((sec, size_bytes, speed)) = parse_ffmpeg_progress(&trimmed) {
                                let dur_opt = *known_dur_err.lock().unwrap();
                                emit_ffmpeg_progress_update(
                                    &app_err,
                                    &media_tasks_err,
                                    &tid_err,
                                    sec,
                                    size_bytes,
                                    &speed,
                                    dur_opt,
                                    &last_pct_err,
                                    is_clipping_active,
                                );
                            }

                            err_lines_clone.lock().unwrap().push(trimmed);
                        }
                    }
                })
            });

            let stdout_diag_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

            // Read stdout for progress lines
            let mut last_emit_instant = std::time::Instant::now();
            if let Some(so) = stdout {
                let reader = BufReader::new(so);
                for line in reader.lines().flatten() {
                    let is_canc = cancelled_tasks.lock().unwrap().get(&tid).copied().unwrap_or(false);
                    if is_canc {
                        break;
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    if let Some(dur) = parse_ffmpeg_duration(trimmed) {
                        let mut kd = known_duration.lock().unwrap();
                        if kd.is_none() {
                            *kd = Some(dur);
                        }
                    }

                    if !trimmed.contains("[ZR_PROG]") {
                        let low = trimmed.to_lowercase();
                        if low.contains("error:")
                            || low.contains("warning:")
                            || low.contains("postprocessing")
                            || low.contains("invalid data found")
                            || low.contains("error opening input")
                            || trimmed.starts_with("[Fixup")
                            || trimmed.starts_with("[Merger]")
                        {
                            stdout_diag_lines.lock().unwrap().push(trimmed.to_string());
                        }
                        if let Some(out_path) = parse_ytdlp_output_path(trimmed) {
                            *detected_output_path.lock().unwrap() = Some(out_path.clone());
                            if let Some(fname) = Path::new(&out_path).file_name().and_then(|n| n.to_str()) {
                                let base_stem = strip_format_id_suffix_for_stem(&strip_media_extension_for_stem(fname));
                                if base_stem.len() >= 3 {
                                    current_stem = base_stem;
                                }
                                let flow = fname.to_lowercase();
                                if !is_partial_media_filename(&flow) {
                                    if let Ok(mut lock) = media_tasks.lock() {
                                        if let Some(t) = lock.get_mut(&tid) {
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

                    // Parse structured ZR progress template: [ZR_PROG] 45.2% | 1234567 | 2345678 | NA | 3.80MiB/s | 01:30:00
                    let is_zr_prog = trimmed.contains("[ZR_PROG]");
                    if is_zr_prog {
                        let prog_idx = trimmed.find("[ZR_PROG]").unwrap_or(0);
                        let payload = trimmed[prog_idx + 9..].trim();
                        let parts: Vec<&str> = payload.split('|').map(|s| s.trim()).collect();
                        if parts.len() >= 6 {
                            let pct_str = parts[0].trim_end_matches('%').trim();
                            let mut pct = pct_str.parse::<f64>().unwrap_or(0.0);

                            let down_b = parts[1].parse::<u64>().unwrap_or(0);
                            if down_b > 0 {
                                last_downloaded_bytes = down_b;
                            }

                            let mut tot_b = parts[2].parse::<u64>().unwrap_or(0);
                            if tot_b == 0 {
                                if let Ok(f) = parts[3].parse::<f64>() {
                                    if f > 0.0 {
                                        tot_b = f as u64;
                                    }
                                }
                            }
                            if tot_b > 0 {
                                last_total_bytes = tot_b;
                            }

                            if pct <= 0.0 && last_total_bytes > 0 && last_downloaded_bytes > 0 {
                                pct = ((last_downloaded_bytes as f64 / last_total_bytes as f64) * 100.0).clamp(0.0, 99.9);
                            }
                            if pct > 0.0 {
                                *last_percent.lock().unwrap() = pct;
                            }

                            let speed_str = if parts[4] == "NA" || parts[4].is_empty() {
                                "".to_string()
                            } else {
                                parts[4].to_string()
                            };

                            let eta_str = if parts[5] == "NA" || parts[5] == "Unknown" || parts[5].is_empty() {
                                "--".to_string()
                            } else {
                                parts[5].to_string()
                            };

                            let speed_bps = parse_human_size_to_bytes(&speed_str);

                            if let Ok(mut tasks_lock) = media_tasks.lock() {
                                if let Some(t) = tasks_lock.get_mut(&tid) {
                                    t.percent = pct;
                                    t.downloaded_bytes = last_downloaded_bytes;
                                    t.total_bytes = last_total_bytes;
                                    t.speed_bps = speed_bps;
                                    t.speed_formatted = speed_str.clone();
                                    t.eta_formatted = eta_str.clone();
                                    t.status = "downloading".to_string();
                                }
                            }

                            let now = std::time::Instant::now();
                            let elapsed_ms = now.duration_since(last_emit_instant).as_millis();
                            if elapsed_ms >= 250 || pct >= 99.5 {
                                last_emit_instant = now;
                                let zr_payload = serde_json::json!({
                                    "id": tid.clone(),
                                    "taskId": tid.clone(),
                                    "percent": pct,
                                    "text": format!("{:.1}% of {} at {}", pct, format_bytes(last_total_bytes), speed_str),
                                    "speed": speed_str.clone(),
                                    "speed_formatted": speed_str.clone(),
                                    "speedFormatted": speed_str.clone(),
                                    "speed_bps": speed_bps,
                                    "speedBps": speed_bps,
                                    "eta": eta_str.clone(),
                                    "eta_formatted": eta_str.clone(),
                                    "etaFormatted": eta_str.clone(),
                                    "total_bytes": last_total_bytes,
                                    "totalBytes": last_total_bytes,
                                    "downloaded_bytes": last_downloaded_bytes,
                                    "downloadedBytes": last_downloaded_bytes,
                                    "status": "downloading".to_string(),
                                });
                                let _ = app.emit("download-progress", zr_payload);
                            }
                            continue;
                        }
                    }

                    // Parse standard yt-dlp progress line fallback: [download]  45.2% of 84.60MiB at  6.42MiB/s ETA 00:06
                    if trimmed.starts_with("[download]") && trimmed.contains('%') {
                        let after_tag = trimmed.trim_start_matches("[download]").trim();
                        let pct_part = after_tag.split('%').next().unwrap_or("0").trim();
                        let pct = pct_part.parse::<f64>().unwrap_or(0.0);
                        if pct > 0.0 {
                            *last_percent.lock().unwrap() = pct;
                        }

                        if let Some(of_idx) = after_tag.find("of ") {
                            let after_of = &after_tag[of_idx + 3..];
                            let size_str = after_of.split_whitespace().next().unwrap_or("");
                            let b = parse_human_size_to_bytes(size_str);
                            if b > 0 {
                                last_total_bytes = b;
                            }
                        }

                        let downloaded_bytes = if last_total_bytes > 0 {
                            ((last_total_bytes as f64) * (pct / 100.0)) as u64
                        } else if last_downloaded_bytes > 0 {
                            last_downloaded_bytes
                        } else {
                            0
                        };
                        if downloaded_bytes > 0 {
                            last_downloaded_bytes = downloaded_bytes;
                        }

                        let speed = if let Some(idx) = after_tag.find("at ") {
                            after_tag[idx + 3..]
                                .split_whitespace()
                                .next()
                                .unwrap_or("")
                                .to_string()
                        } else {
                            "".to_string()
                        };

                        let eta = if let Some(idx) = after_tag.find("ETA ") {
                            after_tag[idx + 4..]
                                .split_whitespace()
                                .next()
                                .unwrap_or("")
                                .to_string()
                        } else {
                            "".to_string()
                        };

                        let _ = app.emit(
                            "download-progress",
                            serde_json::json!({
                                "id": tid.clone(),
                                "percent": pct,
                                "text": after_tag.to_string(),
                                "speed": speed.clone(),
                                "speed_formatted": speed.clone(),
                                "eta": eta.clone(),
                                "eta_formatted": eta.clone(),
                                "total_bytes": last_total_bytes,
                                "downloaded_bytes": last_downloaded_bytes,
                                "status": "downloading".to_string(),
                            }),
                        );
                    } else if trimmed.contains("time=")
                        && (trimmed.contains("frame=")
                            || trimmed.contains("size=")
                            || trimmed.contains("speed="))
                    {
                        if let Some((sec, size_bytes, speed)) = parse_ffmpeg_progress(trimmed) {
                            let dur_opt = *known_duration.lock().unwrap();
                            if size_bytes > 0 {
                                last_downloaded_bytes = size_bytes;
                            }
                            emit_ffmpeg_progress_update(
                                &app,
                                &media_tasks,
                                &tid,
                                sec,
                                size_bytes,
                                &speed,
                                dur_opt,
                                &last_percent,
                                is_clipping_active,
                            );
                        }
                    } else if trimmed.contains("Downloading 1 time ranges") {
                        let _ = app.emit(
                            "download-progress",
                            serde_json::json!({
                                "id": tid.clone(),
                                "percent": 5.0,
                                "text": format!("Extracting timeframe section: {}", trimmed),
                                "speed": "".to_string(),
                                "speed_formatted": "0 B/s".to_string(),
                                "eta": "".to_string(),
                                "eta_formatted": "--".to_string(),
                                "status": "downloading".to_string(),
                            }),
                        );
                    } else if trimmed.starts_with("[Merger]")
                        || trimmed.starts_with("[ExtractAudio]")
                        || trimmed.starts_with("[EmbedSubtitle]")
                        || trimmed.starts_with("[Metadata]")
                    {
                        let _ = app.emit(
                            "download-progress",
                            serde_json::json!({
                                "id": tid.clone(),
                                "percent": 99.0,
                                "text": format!("Post-processing: {}", trimmed),
                                "speed": "".to_string(),
                                "speed_formatted": "Finishing...".to_string(),
                                "eta": "00:00".to_string(),
                                "eta_formatted": "0s".to_string(),
                                "status": "assembling".to_string(),
                            }),
                        );
                    }
                }
            }

            let status = child.wait();
            if let Some(handle) = stderr_handle {
                let _ = handle.join();
            }
            active_pids.lock().unwrap().remove(&tid);

            let still_tracked = media_tasks.lock().unwrap().contains_key(&tid);
            if !still_tracked {
                return;
            }

            let was_cancelled = cancelled_tasks.lock().unwrap().remove(&tid).unwrap_or(false);
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
                return;
            }

            let exit_ok = status.as_ref().map(|s| s.success()).unwrap_or(false);
            let combined_diags: Vec<String> = {
                let errs = err_lines.lock().unwrap();
                let std_diags = stdout_diag_lines.lock().unwrap();
                errs.iter().chain(std_diags.iter()).cloned().collect()
            };
            let has_postprocess_marker = combined_diags.iter().any(|l| {
                let low = l.to_lowercase();
                low.contains("postprocessing")
                    || low.contains("invalid data found")
                    || low.contains("error opening input")
            });
            let postprocess_failed = has_postprocess_marker
                && (!exit_ok
                    || combined_diags.iter().any(|l| {
                        let low = l.to_lowercase();
                        low.contains("error:")
                            || low.contains("warning: postprocessing")
                            || low.contains("invalid data found")
                            || low.contains("error opening input")
                    }));

            if attempt_stage == 0 && postprocess_failed {
                attempt_stage = 1;
                clean_partial_files(&options.save_folder, &current_stem);
                *last_percent.lock().unwrap() = 0.0;
                last_downloaded_bytes = 0;
                last_total_bytes = 0;
                let msg = "Post-processing failed — retrying cleanly (--no-continue --fixup warn)...".to_string();
                eprintln!("[media] [{}] {}", tid, msg);
                let _ = app.emit("download-log", serde_json::json!({ "id": tid.clone(), "message": msg.clone(), "level": "warn" }));
                let fb_payload = serde_json::json!({
                    "id": tid.clone(),
                    "taskId": tid.clone(),
                    "percent": 0.0,
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

            if attempt_stage == 1 && (postprocess_failed || !exit_ok) {
                attempt_stage = 2;
                clean_partial_files(&options.save_folder, &current_stem);
                *last_percent.lock().unwrap() = 0.0;
                last_downloaded_bytes = 0;
                last_total_bytes = 0;
                let msg = "Retry post-processing failed — switching to ffmpeg copy (--no-part)...".to_string();
                eprintln!("[media] [{}] {}", tid, msg);
                let _ = app.emit("download-log", serde_json::json!({ "id": tid.clone(), "message": msg.clone(), "level": "warn" }));
                let fb_payload = serde_json::json!({
                    "id": tid.clone(),
                    "taskId": tid.clone(),
                    "percent": 0.0,
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

            if is_hls && attempt_stage == 0 && !exit_ok && !has_fresh_output_file(&options.save_folder) {
                attempt_stage = 2;
                let keep_pct = (*last_percent.lock().unwrap()).min(95.0).max(2.0);
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

            let detected_ref = detected_output_path.lock().unwrap().clone();
            let kept_file = find_fresh_kept_output_file(
                &options.save_folder,
                &current_stem,
                run_started_at,
                detected_ref.as_deref(),
            );

            if postprocess_failed {
                if let Some(ref kept_path) = kept_file {
                    let kept_name = kept_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| options.custom_filename.clone().unwrap_or_else(|| "Online Media.mp4".to_string()));
                    let kept_size = fs::metadata(kept_path).map(|m| m.len()).unwrap_or(0);
                    let final_sz = if kept_size > 0 {
                        kept_size
                    } else if last_total_bytes > 0 {
                        last_total_bytes
                    } else {
                        last_downloaded_bytes
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
                    if !current_stem.is_empty() {
                        clean_partial_files(&options.save_folder, &current_stem);
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
                            "save_path": options.save_folder,
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
                    return;
                }
            }

            if exit_ok && kept_file.is_some() {
                break;
            }
            if exit_ok && !has_postprocess_marker {
                break;
            }

            // Requirement F: Expired link refresh (403/410) via yt-dlp re-extraction
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
                        "percent": *last_percent.lock().unwrap(),
                        "text": msg,
                        "speed_formatted": "Refreshing link...".to_string(),
                        "status": "downloading".to_string(),
                    }),
                );
                // Re-extract via yt-dlp -g on webpage URL (keeps options.url updated if webpage returns a fresh direct CDN URL)
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

            // Requirement F: Auto retry up to 5 times with backoff [2, 5, 15, 30, 60] s on network errors
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
                        "percent": *last_percent.lock().unwrap(),
                        "text": msg,
                        "speed_formatted": format!("Retry in {}s", wait_s),
                        "status": "downloading".to_string(),
                    }),
                );
                let mut cancelled_during_wait = false;
                for _ in 0..(wait_s * 4) {
                    std::thread::sleep(Duration::from_millis(250));
                    if cancelled_tasks.lock().unwrap().get(&tid).copied().unwrap_or(false) {
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

            mark_task_error(&media_tasks, &tid, &meaningful_err);
            let error_pct = (*last_percent.lock().unwrap()).min(99.0);
            crate::spawn_save_all_tasks(&app);
            let _ = app.emit("download-error", serde_json::json!({ "id": tid.clone(), "error": meaningful_err }));
            let _ = app.emit(
                "download-progress",
                serde_json::json!({
                    "id": tid.clone(),
                    "percent": error_pct,
                    "status": "error",
                    "error_message": meaningful_err,
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
            return;
        }

        let detected_ref = detected_output_path.lock().unwrap().clone();
        let kept_file = find_fresh_kept_output_file(
            &options.save_folder,
            &current_stem,
            run_started_at,
            detected_ref.as_deref(),
        );

        let live_settings = app
            .try_state::<crate::settings::SettingsState>()
            .map(|s| s.get_blocking())
            .unwrap_or_else(|| app_settings.clone());

        let final_kept_file = kept_file.map(|p| {
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
        } else if last_total_bytes > 0 {
            last_total_bytes
        } else if last_downloaded_bytes > 0 {
            last_downloaded_bytes
        } else {
            media_tasks
                .lock()
                .ok()
                .and_then(|m| m.get(&tid).map(|t| t.downloaded_bytes))
                .unwrap_or(0)
        };

        if !media_tasks.lock().unwrap().contains_key(&tid) {
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
// Command 6: 1-Click App Updater (Downloads & Launches Installer from GitHub)
// ---------------------------------------------------------------------------

#[command]
pub async fn install_app_update(_app: AppHandle, download_url: String) -> Result<String, String> {
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
            let dest = temp_dir.join("GM-Downloader-macOS.zip");
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


