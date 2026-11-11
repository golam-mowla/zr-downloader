
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
use tauri::{command, AppHandle, Emitter, State};
use tokio::io::AsyncWriteExt;

static ENGINE_LOCK: Mutex<()> = Mutex::new(());
static TOOL_UPDATES_CACHE: Mutex<Option<(Instant, Vec<ToolStatus>)>> = Mutex::new(None);

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
            overwrite_files: true,
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
            t.speed_formatted = "0 B/s".to_string();
            t.eta_formatted = "--".to_string();
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

fn parse_ffmpeg_time_and_speed(line: &str) -> Option<(f64, String)> {
    if let Some(t_idx) = line.find("time=") {
        let after_time = &line[t_idx + 5..];
        let time_token = after_time.split_whitespace().next()?;
        let time_clean = time_token.split('.').next().unwrap_or(time_token);
        let sec = parse_time_to_seconds(time_clean)?;

        let speed = if let Some(s_idx) = line.find("speed=") {
            let after_speed = &line[s_idx + 6..];
            after_speed.split_whitespace().next().unwrap_or("").to_string()
        } else {
            "".to_string()
        };

        return Some((sec, speed));
    }
    None
}

// ---------------------------------------------------------------------------
// Command 1: Fetch Media Info (Dynamic Qualities, Formats, Subtitles)
// ---------------------------------------------------------------------------

#[command]
pub async fn fetch_media_info(
    url: String,
    cookies_browser: Option<String>,
    cookies_file: Option<String>,
    proxy: Option<String>,
    discovery_args: Option<String>,
) -> Result<MediaInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let clean_url = url.trim().to_string();
        if clean_url.is_empty() {
            return Err("URL cannot be empty".to_string());
        }

        let build_cmd = |extractor_arg: Option<&str>| {
            let mut cmd = make_command("yt-dlp");
            cmd.args([
                "--dump-single-json",
                "--no-playlist",
                "--no-check-certificates",
                "--no-warnings",
                "--socket-timeout",
                "10",
                "--extractor-retries",
                "1",
            ]);

            if find_exe_in_augmented_path("node").is_some() {
                cmd.args(["--js-runtimes", "node"]);
            } else if find_exe_in_augmented_path("deno").is_some() {
                cmd.args(["--js-runtimes", "deno"]);
            }

            if let Some(arg) = extractor_arg {
                cmd.args(["--extractor-args", arg]);
            }

            if let Some(ref cb) = cookies_browser {
                if cb != "None" && !cb.trim().is_empty() {
                    cmd.args(["--cookies-from-browser", &cb.to_lowercase()]);
                }
            }
            if let Some(ref cf) = cookies_file {
                if !cf.trim().is_empty() && PathBuf::from(cf).exists() {
                    cmd.args(["--cookies", cf]);
                }
            }
            if let Some(ref pr) = proxy {
                if !pr.trim().is_empty() {
                    cmd.args(["--proxy", pr.trim()]);
                }
            }
            if let Some(ref extra) = discovery_args {
                for arg in extra.split_whitespace() {
                    if !arg.is_empty() {
                        cmd.arg(arg);
                    }
                }
            }

            cmd.arg(&clean_url);
            cmd
        };

        // Attempt 1: Standard extractor (discovers full 4K, 2K, 1080p, 60fps, Opus, etc. without format restrictions)
        let mut output = build_cmd(None)
            .output()
            .map_err(|e| format!("Could not launch yt-dlp: {}", e))?;

        // Attempt 2: If failed (e.g. YouTube bot detection or login check), retry with iOS client bypass
        if !output.status.success() {
            if let Ok(retry_out) = build_cmd(Some("youtube:player_client=ios,web")).output() {
                if retry_out.status.success() {
                    output = retry_out;
                }
            }
        }

        // Attempt 3: If still failed, retry with Android client bypass
        if !output.status.success() {
            if let Ok(retry_out) = build_cmd(Some("youtube:player_client=android,web")).output() {
                if retry_out.status.success() {
                    output = retry_out;
                }
            }
        }

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let out = String::from_utf8_lossy(&output.stdout);
            let combined = format!("{}\n{}", err.trim(), out.trim());
            let final_err = if combined.trim().is_empty() {
                "Failed to fetch metadata. Please verify the link or network connection.".to_string()
            } else {
                combined.trim().to_string()
            };
            return Err(format!("yt-dlp fetch failed: {}", final_err));
        }

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
                    let res_tag = if h >= 2160 {
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

                    let audio_badge = if has_audio { " [+Audio]" } else { "" };
                    let label = format!(
                        "{} • {} • {}fps • {} • {}{} (ID: {})",
                        res_tag,
                        short_codec(vcodec),
                        fps,
                        ext.to_uppercase(),
                        format_bytes(size),
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
                            filesize: if size > 0 { Some(size) } else { None },
                            has_audio: Some(has_audio),
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
                        },
                    ));
                }
            }
        }

        // Sort highest quality first
        video_raw.sort_by(|a, b| (b.0, b.1, b.2).cmp(&(a.0, a.1, a.2)));
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
    .map_err(|e| format!("Task join error: {}", e))?
}

// ---------------------------------------------------------------------------
// Command 2: Start Download (Video, Audio, Subtitles Only, Clipping, Parabolic)
// ---------------------------------------------------------------------------

pub fn cleanup_temp_and_part_files(save_folder: &str, title_hint: &str) {
    if let Ok(entries) = fs::read_dir(save_folder) {
        let clean_hint = title_hint.replace(|c: char| !c.is_alphanumeric(), "").to_lowercase();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let name_lower = name.to_lowercase();
                if name_lower.contains(".temp.") || name_lower.ends_with(".part") || name_lower.ends_with(".ytdl") {
                    let clean_name = name_lower.replace(|c: char| !c.is_alphanumeric(), "");
                    if clean_hint.is_empty()
                        || clean_name.contains(&clean_hint)
                        || clean_hint.contains(&clean_name)
                        || clean_hint.len() < 3
                    {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }
}

#[command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, DownloadState>,
    mut options: DownloadOptions,
) -> Result<String, String> {
    let resolved_folder = if options.save_folder.trim().is_empty()
        || options.save_folder.trim() == "Downloads"
        || !std::path::Path::new(&options.save_folder).is_absolute()
    {
        dirs_or_default_downloads()
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
    let active_pids = state.active_pids.clone();
    let cancelled_tasks = state.cancelled_tasks.clone();
    cancelled_tasks.lock().unwrap().insert(task_id.clone(), false);
    state.task_options.lock().unwrap().insert(task_id.clone(), options.clone());

    let initial_task = DownloadTask {
        id: task_id.clone(),
        url: options.url.clone(),
        filename: options.custom_filename.clone().unwrap_or_else(|| "Online Media.mp4".to_string()),
        save_path: resolved_folder.clone(),
        total_bytes: 0,
        downloaded_bytes: 0,
        percent: 1.0,
        speed_bps: 0,
        speed_formatted: "Connecting...".to_string(),
        eta_formatted: "--".to_string(),
        status: "downloading".to_string(),
        category: if options.download_mode == "audio" { "audio".to_string() } else { "video".to_string() },
        chunk_count: 1,
        is_resumable: true,
        low_disk_space: options.low_disk_space.unwrap_or(false),
        chunks: Vec::new(),
        speed_history: Vec::new(),
        error_message: None,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    state.tasks.lock().unwrap().insert(task_id.clone(), initial_task);

    let tid = task_id.clone();
    let media_tasks = state.tasks.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = fs::create_dir_all(&options.save_folder) {
            let err_msg = format!("Failed to create output folder: {}", e);
            mark_task_error(&media_tasks, &tid, &err_msg);
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
        let is_youtube = options.url.contains("youtube.com") || options.url.contains("youtu.be");
        let is_low_disk = options.low_disk_space.unwrap_or(false) || !options.use_part_files;
        let part_flag = if is_low_disk { "--no-part" } else { "--part" };
        let is_hls = options.url.contains(".m3u8")
            || options.url.contains("kick.com")
            || options.url.contains("twitch.tv");

        let build_cmd = || -> Command {
            let mut cmd = make_command("yt-dlp");
            cmd.args([
                "--newline",
                "--progress",
                "--no-playlist",
                "--progress-template",
                "download:[ZR_PROG] %(progress._percent_str)s | %(progress.downloaded_bytes)s | %(progress.total_bytes)s | %(progress.total_bytes_estimate)s | %(progress._speed_str)s | %(progress._eta_str)s",
            ]);

            // Ensure Node or Deno JS runtime
            if find_exe_in_augmented_path("node").is_some() {
                cmd.args(["--js-runtimes", "node"]);
            } else if ensure_deno_available().is_some() {
                cmd.args(["--js-runtimes", "deno"]);
            }

            // Ensure FFmpeg location is explicitly passed if found
            if let Some(ffmpeg_path) = ensure_ffmpeg_available() {
                cmd.arg("--ffmpeg-location").arg(ffmpeg_path.to_string_lossy().to_string());
            }

            if is_hls || is_youtube {
                cmd.args([
                    "-o", &out_tmpl,
                    "--windows-filenames",
                    "--no-playlist",
                    "--no-check-certificates",
                    "--retries", "10",
                    "--fragment-retries", "15",
                    "--file-access-retries", "5",
                    "-c",
                    part_flag,
                ]);
                if is_youtube {
                    cmd.args([
                        "--concurrent-fragments", "8",
                        "--http-chunk-size", "10M",
                        "--buffer-size", "64K",
                    ]);
                }
                if is_hls {
                    cmd.arg("--skip-unavailable-fragments");
                    if is_low_disk {
                        // Zero-Temp In-Place for HLS: stream directly into destination file without duplicate .temp.mp4
                        cmd.args([
                            "--downloader", "m3u8:ffmpeg",
                            "--downloader-args", "ffmpeg:-c copy",
                            "--fixup", "never",
                        ]);
                    }
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
                    "-c",
                    part_flag,
                ]);
            }
        if options.overwrite_files || is_clipping_active {
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
                    if !is_low_disk || !is_hls {
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

        // Speed Limit, Proxy, Cookies
        if options.limit_speed && options.speed_limit_kb > 0 {
            cmd.args(["--limit-rate", &format!("{}K", options.speed_limit_kb)]);
        }
        if !options.proxy.trim().is_empty() {
            cmd.args(["--proxy", options.proxy.trim()]);
        }
        if options.cookies_browser != "None" && !options.cookies_browser.trim().is_empty() {
            cmd.args([
                "--cookies-from-browser",
                &options.cookies_browser.to_lowercase(),
            ]);
        } else if !options.cookies_file.trim().is_empty()
            && PathBuf::from(&options.cookies_file).exists()
        {
            cmd.args(["--cookies", options.cookies_file.trim()]);
        }

        // Aria2c Downloader
        let aria2c_found =
            find_local_aria2c().is_some() || find_exe_in_augmented_path("aria2c").is_some();
        if (options.use_aria2c || aria2c_found) && aria2c_found && !options.enable_clip {
            cmd.args([
                "--downloader",
                "aria2c",
                "--downloader-args",
                "aria2c:-x 16 -s 16 -k 1M",
            ]);
        }

        // FFmpeg Threads & Post-Processor Args
        if options.ffmpeg_threads > 0 {
            cmd.args([
                "--ppa",
                &format!("ffmpeg:-threads {}", options.ffmpeg_threads),
                "--downloader-args",
                &format!("ffmpeg:-threads {}", options.ffmpeg_threads),
            ]);
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

            cmd.arg(options.url.trim());
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd
        };

        let init_p = serde_json::json!({
            "id": tid.clone(),
            "taskId": tid.clone(),
            "percent": 2.0,
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

        let mut spawned_child = None;
        let mut last_spawn_err = None;
        for attempt in 0..6 {
            let mut cmd = build_cmd();
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
        };

        active_pids.lock().unwrap().insert(tid.clone(), child.id());

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Read stderr in background thread to capture errors & FFmpeg cut progress
        let err_lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let err_lines_clone = err_lines.clone();
        let app_err = app.clone();
        let tid_err = tid.clone();
        let clip_dur_err = clip_target_duration;
        if let Some(se) = stderr {
            std::thread::spawn(move || {
                let reader = BufReader::new(se);
                for line in reader.lines().flatten() {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() {
                        // err log throttled

                        // Live progress for FFmpeg cut & encoding from stderr
                        if let Some((sec, speed)) = parse_ffmpeg_time_and_speed(&trimmed) {
                            let pct = if let Some(target_dur) = clip_dur_err {
                                if target_dur > 0.0 {
                                    ((sec / target_dur) * 100.0).clamp(2.0, 99.5)
                                } else {
                                    50.0
                                }
                            } else {
                                50.0
                            };
                            let text = if let Some(target_dur) = clip_dur_err {
                                format!(
                                    "Cutting & Encoding: {:.1}% ({}/{} at {})",
                                    pct,
                                    format_seconds_to_timestamp(sec),
                                    format_seconds_to_timestamp(target_dur),
                                    if speed.is_empty() { "1x".to_string() } else { speed.clone() }
                                )
                            } else {
                                format!("Cutting media: {} (Speed: {})", format_seconds_to_timestamp(sec), speed)
                            };
                            let _ = app_err.emit(
                                "download-progress",
                                serde_json::json!({
                                    "id": tid_err.clone(),
                                    "percent": pct,
                                    "text": text,
                                    "speed": speed.clone(),
                                    "speed_formatted": speed.clone(),
                                    "eta": "".to_string(),
                                    "eta_formatted": "--".to_string(),
                                    "status": "downloading".to_string(),
                                }),
                            );
                        }

                        err_lines_clone.lock().unwrap().push(trimmed);
                    }
                }
            });
        }

        // Read stdout for progress lines
        let mut last_total_bytes: u64 = 0;
        let mut last_downloaded_bytes: u64 = 0;
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
                // log emit throttled

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

                    // Parse total size: "[download]  45.2% of 84.60MiB at  6.42MiB/s ETA 00:06"
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
                } else if trimmed.contains("time=") && (trimmed.contains("frame=") || trimmed.contains("size=")) {
                    if let Some((sec, speed)) = parse_ffmpeg_time_and_speed(trimmed) {
                        let pct = if let Some(target_dur) = clip_target_duration {
                            if target_dur > 0.0 {
                                ((sec / target_dur) * 100.0).clamp(2.0, 99.5)
                            } else {
                                50.0
                            }
                        } else {
                            50.0
                        };
                        let text = if let Some(target_dur) = clip_target_duration {
                            format!(
                                "Cutting & Encoding: {:.1}% ({}/{} at {})",
                                pct,
                                format_seconds_to_timestamp(sec),
                                format_seconds_to_timestamp(target_dur),
                                if speed.is_empty() { "1x".to_string() } else { speed.clone() }
                            )
                        } else {
                            format!("Cutting media: {} (Speed: {})", format_seconds_to_timestamp(sec), speed)
                        };
                        let _ = app.emit(
                            "download-progress",
                            serde_json::json!({
                                "id": tid.clone(),
                                "percent": pct,
                                "text": text,
                                "speed": speed.clone(),
                                "speed_formatted": speed.clone(),
                                "eta": "".to_string(),
                                "eta_formatted": "--".to_string(),
                                "status": "downloading".to_string(),
                            }),
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
        active_pids.lock().unwrap().remove(&tid);

        let was_cancelled = cancelled_tasks.lock().unwrap().remove(&tid).unwrap_or(false);
        if was_cancelled {
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
            return;
        }

        if !status.as_ref().map(|s| s.success()).unwrap_or(false) {
            let errs = err_lines.lock().unwrap();
            // Find the most meaningful error: prioritize lines starting with "ERROR:",
            // then lines containing "error" or "failed", then fall back to the last non-empty line.
            let meaningful_err = errs.iter().rev()
                .find(|l| l.starts_with("ERROR:") || l.starts_with("error:"))
                .or_else(|| errs.iter().rev().find(|l| {
                    let low = l.to_lowercase();
                    (low.contains("error") || low.contains("failed") || low.contains("not found"))
                        && !low.contains("[download]")
                        && !low.contains("warning")
                }))
                .or_else(|| errs.iter().rev().find(|l| !l.trim().is_empty()))
                .cloned()
                .unwrap_or_else(|| "Unknown yt-dlp error".to_string());
            let hint_name = options.custom_filename.as_deref().unwrap_or("");
            cleanup_temp_and_part_files(&options.save_folder, hint_name);
            mark_task_error(&media_tasks, &tid, &meaningful_err);
            let _ = app.emit("download-error", serde_json::json!({ "id": tid.clone(), "error": meaningful_err }));
            let _ = app.emit(
                "download-progress",
                serde_json::json!({
                    "id": tid.clone(),
                    "status": "error",
                    "error_message": meaningful_err,
                    "speed": "0 B/s",
                    "speed_formatted": "0 B/s",
                    "eta": "--",
                    "eta_formatted": "--"
                }),
            );
            return;
        }

        let final_total = if last_total_bytes > 0 {
            last_total_bytes
        } else {
            last_downloaded_bytes
        };

        if let Ok(mut tasks_lock) = media_tasks.lock() {
            if let Some(t) = tasks_lock.get_mut(&tid) {
                t.percent = 100.0;
                t.downloaded_bytes = final_total;
                t.total_bytes = final_total;
                t.status = "completed".to_string();
                t.speed_formatted = "Finished".to_string();
                t.eta_formatted = "00:00:00".to_string();
            }
        }

        let hint_name = options.custom_filename.as_deref().unwrap_or("");
        cleanup_temp_and_part_files(&options.save_folder, hint_name);
        let complete_payload = serde_json::json!({
            "id": tid.clone(),
            "taskId": tid.clone(),
            "percent": 100.0,
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
                "percent": 100.0,
                "total_bytes": final_total,
                "downloaded_bytes": final_total,
                "status": "completed"
            }),
        );
    });

    Ok(task_id)
}

// ---------------------------------------------------------------------------
// Command 3: Cancel / Pause Active Download
// ---------------------------------------------------------------------------

#[command]
pub async fn cancel_download(
    state: State<'_, DownloadState>,
    task_id: Option<String>,
) -> Result<(), String> {
    let mut pids_to_kill = Vec::new();
    {
        let mut pids = state.active_pids.lock().unwrap();
        let mut cancels = state.cancelled_tasks.lock().unwrap();
        if let Some(ref tid) = task_id {
            cancels.insert(tid.clone(), true);
            if let Some(pid) = pids.remove(tid) {
                pids_to_kill.push(pid);
            }
        } else {
            for (tid, pid) in pids.drain() {
                cancels.insert(tid, true);
                pids_to_kill.push(pid);
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


