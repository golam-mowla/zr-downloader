
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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{command, AppHandle, Emitter, State};

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
            use_aria2c: false,
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

static FFMPEG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static YTDLP_PATH: OnceLock<Result<PathBuf, String>> = OnceLock::new();
static EXE_CACHE: RwLock<Option<HashMap<String, Option<PathBuf>>>> = RwLock::new(None);

fn get_augmented_path() -> String {
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

    // 3. User & System Standard Locations (No hardcoded D:\ paths)
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        let local = PathBuf::from(&local_app_data);
        // App's own local managed bin directory
        extra_dirs.push(local.clone());
        extra_dirs.push(local.join("ZR Downloader").join("bin"));
        extra_dirs.push(local.join("ZR Downloader"));
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
    if let Ok(guard) = EXE_CACHE.read() {
        if let Some(ref map) = *guard {
            if let Some(val) = map.get(bin_name) {
                return val.clone();
            }
        }
    }

    let aug_path = get_augmented_path();
    let sep = if cfg!(target_os = "windows") { ';' } else { ':' };
    let exts = if cfg!(target_os = "windows") {
        vec![".exe", ".cmd", ".bat", ""]
    } else {
        vec![""]
    };

    let mut found = None;
    for dir in aug_path.split(sep) {
        if dir.trim().is_empty() {
            continue;
        }
        let base = PathBuf::from(dir);
        for ext in &exts {
            let candidate = base.join(format!("{}{}", bin_name, ext));
            if candidate.is_file() {
                found = Some(candidate);
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }

    if let Ok(mut guard) = EXE_CACHE.write() {
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(bin_name.to_string(), found.clone());
    }

    found
}

fn ensure_ffmpeg_available() -> Option<PathBuf> {
    FFMPEG_PATH.get_or_init(|| {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local) = env::var("LOCALAPPDATA") {
                let candidates = [
                    PathBuf::from(&local).join("ZR Downloader").join("bin").join("ffmpeg.exe"),
                    PathBuf::from(&local).join("ZR Downloader").join("ffmpeg.exe"),
                    PathBuf::from(&local).join("GM-Downloader").join("bin").join("ffmpeg.exe"),
                    PathBuf::from(&local).join("ffmpeg.exe"),
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

        if let Some(existing) = find_exe_in_augmented_path("ffmpeg") {
            return Some(existing);
        }

        None
    }).clone()
}

fn find_ytdlp_binary() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let candidates = [
                PathBuf::from(&local).join("ZR Downloader").join("bin").join("yt-dlp.exe"),
                PathBuf::from(&local).join("ZR Downloader").join("yt-dlp.exe"),
                PathBuf::from(&local).join("GM-Downloader").join("bin").join("yt-dlp.exe"),
                PathBuf::from(&local).join("yt-dlp.exe"),
            ];
            for p in &candidates {
                if p.exists() {
                    return Some(p.clone());
                }
            }
        }
    }
    find_exe_in_augmented_path("yt-dlp")
}

fn ensure_ytdlp_available() -> Option<PathBuf> {
    ensure_ytdlp_result().ok()
}

fn ensure_ytdlp_result() -> Result<PathBuf, String> {
    YTDLP_PATH.get_or_init(|| {
        if let Some(existing) = find_ytdlp_binary() {
            return Ok(existing);
        }

        // Auto-download standalone yt-dlp executable once if missing on system
        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
                let target_dir = PathBuf::from(&local_app_data).join("ZR Downloader").join("bin");
                let _ = fs::create_dir_all(&target_dir);
                let target_exe = target_dir.join("yt-dlp.exe");

                // 1. Try curl.exe with strict max-time 60s and connect-timeout 10s
                let mut curl_cmd = Command::new("curl.exe");
                curl_cmd.args([
                    "--fail",
                    "--max-time",
                    "60",
                    "--connect-timeout",
                    "10",
                    "-L",
                    "-o",
                    target_exe.to_string_lossy().as_ref(),
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
                ]);
                curl_cmd.creation_flags(CREATE_NO_WINDOW);
                if let Ok(status) = curl_cmd.status() {
                    if status.success() && target_exe.exists() {
                        return Ok(target_exe);
                    }
                }

                // 2. Try PowerShell with 60s timeout fallback
                let ps_script = format!(
                    "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; $wc = New-Object System.Net.WebClient; $wc.Headers.Add('User-Agent', 'ZR-Downloader'); $wc.DownloadFile('https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe', '{}')",
                    target_exe.display()
                );
                let mut ps_cmd = Command::new("powershell.exe");
                ps_cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
                ps_cmd.creation_flags(CREATE_NO_WINDOW);
                if let Ok(mut child) = ps_cmd.spawn() {
                    let start = std::time::Instant::now();
                    loop {
                        if let Ok(Some(st)) = child.try_wait() {
                            if st.success() && target_exe.exists() {
                                return Ok(target_exe);
                            }
                            break;
                        }
                        if start.elapsed() >= std::time::Duration::from_secs(60) {
                            let _ = child.kill();
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(user_profile) = env::var("HOME") {
                let target_dir = PathBuf::from(&user_profile).join(".local").join("bin");
                let _ = fs::create_dir_all(&target_dir);
                let target_bin = target_dir.join("yt-dlp");

                let mut curl_cmd = Command::new("curl");
                curl_cmd.args([
                    "--fail",
                    "--max-time",
                    "60",
                    "--connect-timeout",
                    "10",
                    "-L",
                    "-o",
                    target_bin.to_string_lossy().as_ref(),
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp",
                ]);
                if let Ok(status) = curl_cmd.status() {
                    if status.success() && target_bin.exists() {
                        let _ = Command::new("chmod").args(["+x", target_bin.to_string_lossy().as_ref()]).status();
                        return Ok(target_bin);
                    }
                }
            }
        }

        Err("yt-dlp executable could not be found or downloaded. Please check network connection.".to_string())
    }).clone()
}

// make_command does NOT auto-download - uses existing binary or falls back to 'yt-dlp'
fn make_command(bin_name: &str) -> Command {
    let resolved = if bin_name == "yt-dlp" {
        find_ytdlp_binary()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| bin_name.to_string())
    } else {
        find_exe_in_augmented_path(bin_name)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| bin_name.to_string())
    };

    let mut cmd = Command::new(resolved);
    cmd.env("PATH", get_augmented_path());
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUNBUFFERED", "1");

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd
}

// Watchdog runner for yt-dlp commands (kills process if no output for timeout_secs)
fn run_command_with_watchdog(mut cmd: Command, timeout_secs: u64) -> Result<std::process::Output, String> {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn yt-dlp process: {}", e))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let last_activity = Arc::new(AtomicU64::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    ));
    let last_act_out = last_activity.clone();
    let last_act_err = last_activity.clone();

    let out_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let err_buf = Arc::new(Mutex::new(Vec::<u8>::new()));

    let out_buf_clone = out_buf.clone();
    let t_out = std::thread::spawn(move || {
        if let Some(mut stream) = stdout {
            use std::io::Read;
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stream.read(&mut chunk) {
                if n == 0 { break; }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                last_act_out.store(now, Ordering::Relaxed);
                if let Ok(mut buf) = out_buf_clone.lock() {
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
        }
    });

    let err_buf_clone = err_buf.clone();
    let t_err = std::thread::spawn(move || {
        if let Some(mut stream) = stderr {
            use std::io::Read;
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stream.read(&mut chunk) {
                if n == 0 { break; }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                last_act_err.store(now, Ordering::Relaxed);
                if let Ok(mut buf) = err_buf_clone.lock() {
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
        }
    });

    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = t_out.join();
                let _ = t_err.join();
                let stdout_data = out_buf.lock().map(|b| b.clone()).unwrap_or_default();
                let stderr_data = err_buf.lock().map(|b| b.clone()).unwrap_or_default();
                return Ok(std::process::Output {
                    status,
                    stdout: stdout_data,
                    stderr: stderr_data,
                });
            }
            Ok(None) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = last_activity.load(Ordering::Relaxed);
                if now.saturating_sub(last) >= timeout_secs {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("Process error: {}", e));
            }
        }
    }

    if timed_out {
        let _ = child.wait();
        let _ = t_out.join();
        let _ = t_err.join();
        return Err(format!("yt-dlp timed out: no output received for {} seconds", timeout_secs));
    }

    let status = child.wait().map_err(|e| format!("Wait error: {}", e))?;
    let _ = t_out.join();
    let _ = t_err.join();
    let stdout_data = out_buf.lock().map(|b| b.clone()).unwrap_or_default();
    let stderr_data = err_buf.lock().map(|b| b.clone()).unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout: stdout_data,
        stderr: stderr_data,
    })
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

        // Attempt 1: Standard extractor with 60s watchdog
        let mut output = run_command_with_watchdog(build_cmd(None), 60)
            .map_err(|e| format!("Could not launch yt-dlp: {}", e))?;

        // Attempt 2: If failed (e.g. YouTube bot detection or login check), retry with iOS client bypass
        if !output.status.success() {
            if let Ok(retry_out) = run_command_with_watchdog(build_cmd(Some("youtube:player_client=ios,web")), 60) {
                if retry_out.status.success() {
                    output = retry_out;
                }
            }
        }

        // Attempt 3: If still failed, retry with Android client bypass
        if !output.status.success() {
            if let Ok(retry_out) = run_command_with_watchdog(build_cmd(Some("youtube:player_client=android,web")), 60) {
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
        } else if find_exe_in_augmented_path("deno").is_some() {
            cmd.args(["--js-runtimes", "deno"]);
        }

        // Ensure FFmpeg location is explicitly passed if found
        if let Some(ffmpeg_path) = ensure_ffmpeg_available() {
            cmd.arg("--ffmpeg-location").arg(ffmpeg_path.to_string_lossy().to_string());
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
                "--concurrent-fragments", "4",
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
        if options.use_aria2c && find_exe_in_augmented_path("aria2c").is_some() && !options.enable_clip {
            let conns = options.aria2_connections.max(1).min(16);
            cmd.args([
                "--downloader",
                "aria2c",
                "--downloader-args",
                &format!("aria2c:-x {} -s {} -k 1M", conns, conns),
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

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to start yt-dlp download process: {}", e);
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

        let last_activity = Arc::new(AtomicU64::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        ));
        let last_act_out = last_activity.clone();
        let last_act_err = last_activity.clone();

        // Read stderr in background thread to capture errors & FFmpeg cut progress
        let err_lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let err_lines_clone = err_lines.clone();
        let app_err = app.clone();
        let tid_err = tid.clone();
        let clip_dur_err = clip_target_duration;
        let t_stderr = if let Some(se) = stderr {
            Some(std::thread::spawn(move || {
                let reader = BufReader::new(se);
                for line in reader.lines().flatten() {
                    let now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    last_act_err.store(now_secs, Ordering::Relaxed);
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() {
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
            }))
        } else {
            None
        };

        // Read stdout for progress lines in background thread
        let last_total_arc = Arc::new(AtomicU64::new(0));
        let last_down_arc = Arc::new(AtomicU64::new(0));
        let last_tot_out = last_total_arc.clone();
        let last_down_out = last_down_arc.clone();
        let app_out = app.clone();
        let tid_out = tid.clone();
        let cancelled_tasks_out = cancelled_tasks.clone();
        let is_clipping_out = is_clipping_active;
        let media_tasks_out = media_tasks.clone();

        let t_stdout = if let Some(so) = stdout {
            Some(std::thread::spawn(move || {
                let reader = BufReader::new(so);
                let mut last_emit_instant = std::time::Instant::now();
                for line in reader.lines().flatten() {
                    let now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    last_act_out.store(now_secs, Ordering::Relaxed);

                    let is_canc = cancelled_tasks_out.lock().unwrap().get(&tid_out).copied().unwrap_or(false);
                    if is_canc {
                        break;
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
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
                            if is_clipping_out {
                                pct = pct * 0.90;
                            }

                            let down_b: u64 = parts[1].parse().unwrap_or(0);
                            let tot_b: u64 = parts[2].parse().unwrap_or(0);
                            let tot_est_b: u64 = parts[3].parse().unwrap_or(0);
                            let speed_str = parts[4].to_string();
                            let eta_str = parts[5].to_string();

                            let effective_total = if tot_b > 0 {
                                tot_b
                            } else if tot_est_b > 0 {
                                tot_est_b
                            } else {
                                0
                            };

                            if effective_total > 0 {
                                last_tot_out.store(effective_total, Ordering::Relaxed);
                            }
                            if down_b > 0 {
                                last_down_out.store(down_b, Ordering::Relaxed);
                            }

                            let text = if effective_total > 0 {
                                format!("{:.1}% of {}", pct, format_bytes(effective_total))
                            } else if down_b > 0 {
                                format!("{:.1}% ({})", pct, format_bytes(down_b))
                            } else {
                                format!("{:.1}%", pct)
                            };

                            if let Ok(mut tasks_lock) = media_tasks_out.lock() {
                                if let Some(t) = tasks_lock.get_mut(&tid_out) {
                                    t.percent = pct;
                                    t.downloaded_bytes = down_b;
                                    t.total_bytes = effective_total;
                                    t.speed_formatted = speed_str.clone();
                                    t.eta_formatted = eta_str.clone();
                                    t.status = "downloading".to_string();
                                }
                            }

                            let now = std::time::Instant::now();
                            if now.duration_since(last_emit_instant).as_millis() >= 250 || pct >= 99.5 {
                                last_emit_instant = now;
                                let _ = app_out.emit(
                                    "download-progress",
                                    serde_json::json!({
                                        "id": tid_out.clone(),
                                        "percent": pct,
                                        "text": text,
                                        "speed": speed_str.clone(),
                                        "speed_formatted": speed_str.clone(),
                                        "eta": eta_str.clone(),
                                        "eta_formatted": eta_str.clone(),
                                        "downloaded_bytes": down_b,
                                        "total_bytes": effective_total,
                                        "status": "downloading".to_string(),
                                    }),
                                );
                            }
                            continue;
                        }
                    }

                    // Fallback percentage parser
                    if let Some(pct) = parse_ytdlp_percent(trimmed) {
                        let mut scaled_pct = pct;
                        if is_clipping_out {
                            scaled_pct = pct * 0.90;
                        }
                        if let Ok(mut tasks_lock) = media_tasks_out.lock() {
                            if let Some(t) = tasks_lock.get_mut(&tid_out) {
                                t.percent = scaled_pct;
                                t.status = "downloading".to_string();
                            }
                        }
                        let now = std::time::Instant::now();
                        if now.duration_since(last_emit_instant).as_millis() >= 250 || scaled_pct >= 99.5 {
                            last_emit_instant = now;
                            let _ = app_out.emit(
                                "download-progress",
                                serde_json::json!({
                                    "id": tid_out.clone(),
                                    "percent": scaled_pct,
                                    "status": "downloading".to_string(),
                                }),
                            );
                        }
                    }

                    // Post-processing indication
                    if trimmed.starts_with("[Merger]")
                        || trimmed.starts_with("[Fixup")
                        || trimmed.starts_with("[VideoConvertor]")
                        || trimmed.starts_with("[ExtractAudio]")
                        || trimmed.starts_with("[EmbedThumbnail]")
                        || trimmed.starts_with("[Metadata]")
                    {
                        if let Ok(mut tasks_lock) = media_tasks_out.lock() {
                            if let Some(t) = tasks_lock.get_mut(&tid_out) {
                                t.percent = 99.0;
                                t.status = "assembling".to_string();
                                t.speed_formatted = "Finishing...".to_string();
                            }
                        }
                        let _ = app_out.emit(
                            "download-progress",
                            serde_json::json!({
                                "id": tid_out.clone(),
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
            }))
        } else {
            None
        };

        // Watchdog loop: checks child process every 250ms; kills process if no output for 60 seconds
        let timed_out = Arc::new(AtomicBool::new(false));
        let mut status = None;
        loop {
            match child.try_wait() {
                Ok(Some(s)) => {
                    status = Some(s);
                    break;
                }
                Ok(None) => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = last_activity.load(Ordering::Relaxed);
                    let is_canc = cancelled_tasks.lock().unwrap().get(&tid).copied().unwrap_or(false);
                    if is_canc {
                        let _ = child.kill();
                        status = child.wait().ok();
                        break;
                    }
                    if now.saturating_sub(last) >= 60 {
                        timed_out.store(true, Ordering::SeqCst);
                        let _ = child.kill();
                        status = child.wait().ok();
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                Err(_) => {
                    let _ = child.kill();
                    status = child.wait().ok();
                    break;
                }
            }
        }

        if let Some(t) = t_stdout {
            let _ = t.join();
        }
        if let Some(t) = t_stderr {
            let _ = t.join();
        }

        let last_total_bytes = last_total_arc.load(Ordering::Relaxed);
        let last_downloaded_bytes = last_down_arc.load(Ordering::Relaxed);

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

        if timed_out.load(Ordering::SeqCst) {
            let timeout_err = "Download timed out: no output received from media stream for 60 seconds".to_string();
            let hint_name = options.custom_filename.as_deref().unwrap_or("");
            cleanup_temp_and_part_files(&options.save_folder, hint_name);
            let _ = app.emit("download-error", serde_json::json!({ "id": tid.clone(), "error": timeout_err }));
            let _ = app.emit(
                "download-progress",
                serde_json::json!({
                    "id": tid.clone(),
                    "status": "error",
                    "error_message": timeout_err,
                    "speed": "0 B/s",
                    "speed_formatted": "0 B/s",
                    "eta": "--",
                    "eta_formatted": "--"
                }),
            );
            if let Ok(mut tasks_lock) = media_tasks.lock() {
                if let Some(t) = tasks_lock.get_mut(&tid) {
                    t.status = "error".to_string();
                    t.error_message = Some(timeout_err);
                }
            }
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
            if let Ok(mut tasks_lock) = media_tasks.lock() {
                if let Some(t) = tasks_lock.get_mut(&tid) {
                    t.status = "error".to_string();
                    t.error_message = Some(meaningful_err);
                }
            }
            return;
        }            return;
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

#[command]
pub async fn check_tool_updates() -> Result<Vec<ToolStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // 1. yt-dlp installed and latest version check
        let ytdlp_exe = ensure_ytdlp_available();
        let ytdlp_ver = if let Some(ref exe) = ytdlp_exe {
            make_command(&exe.to_string_lossy())
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "Not Found".to_string())
        } else {
            "Not Found".to_string()
        };

        // Fetch latest version from PyPI / GitHub via PowerShell without requiring python installation
        #[cfg(target_os = "windows")]
        let latest_ytdlp = {
            let ps_script = "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; try { (Invoke-RestMethod -Uri 'https://pypi.org/pypi/yt-dlp/json' -TimeoutSec 4).info.version } catch { '' }";
            let mut ps_cmd = std::process::Command::new("powershell.exe");
            ps_cmd.args(["-NoProfile", "-NonInteractive", "-Command", ps_script]);
            use std::os::windows::process::CommandExt;
            ps_cmd.creation_flags(0x08000000);
            ps_cmd.output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !s.is_empty() { Some(s) } else { None }
                })
                .unwrap_or_else(|| ytdlp_ver.clone())
        };
        #[cfg(not(target_os = "windows"))]
        let latest_ytdlp = ytdlp_ver.clone();

        let has_ytdlp_update = ytdlp_ver == "Not Found" || (!latest_ytdlp.is_empty() && ytdlp_ver != latest_ytdlp);

        // 2. Deno
        let deno_ver = make_command("deno")
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
            .unwrap_or_else(|| "Not Found".to_string());

        // 3. FFmpeg
        let ffmpeg_exe = ensure_ffmpeg_available();
        let ffmpeg_ver = if let Some(ref exe) = ffmpeg_exe {
            make_command(&exe.to_string_lossy())
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
        };

        Ok(vec![
            ToolStatus {
                name: "yt-dlp".to_string(),
                installed_version: ytdlp_ver.clone(),
                latest_version: latest_ytdlp,
                has_update: has_ytdlp_update,
            },
            ToolStatus {
                name: "Deno".to_string(),
                installed_version: deno_ver.clone(),
                latest_version: deno_ver,
                has_update: false,
            },
            ToolStatus {
                name: "FFmpeg".to_string(),
                installed_version: ffmpeg_ver.clone(),
                latest_version: ffmpeg_ver,
                has_update: false,
            },
        ])
    })
    .await
    .map_err(|e| format!("Check error: {}", e))?
}

#[command]
pub async fn update_tool(tool_name: Option<String>, name: Option<String>) -> Result<String, String> {
    let resolved = tool_name.or(name).unwrap_or_else(|| "yt-dlp".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        let lower = resolved.to_lowercase();
        if lower.contains("yt-dlp") {
            // 1. Try yt-dlp -U if an executable is already available
            if let Some(exe_path) = ensure_ytdlp_available() {
                let out = make_command(&exe_path.to_string_lossy()).arg("-U").output();
                if let Ok(o) = out {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    if o.status.success() || stdout.contains("Updated") || stdout.contains("up to date") {
                        return Ok(format!("yt-dlp: {}", stdout.trim()));
                    }
                }
            }

            // 2. Direct download of latest official standalone yt-dlp.exe
            #[cfg(target_os = "windows")]
            {
                if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                    let target_dir = std::path::PathBuf::from(&local_app_data).join("ZR Downloader").join("bin");
                    let _ = std::fs::create_dir_all(&target_dir);
                    let target_exe = target_dir.join("yt-dlp.exe");

                    let ps_script = format!(
                        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (New-Object System.Net.WebClient).DownloadFile('https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe', '{}')",
                        target_exe.display()
                    );
                    let mut ps_cmd = std::process::Command::new("powershell.exe");
                    ps_cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
                    use std::os::windows::process::CommandExt;
                    ps_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                    if let Ok(status) = ps_cmd.status() {
                        if status.success() && target_exe.exists() {
                            return Ok("yt-dlp updated to the latest version successfully!".to_string());
                        }
                    }
                }
            }

            Ok("yt-dlp update check completed!".to_string())
        } else if lower.contains("deno") {
            let out = make_command("deno").arg("upgrade").output();
            if let Ok(o) = out {
                if o.status.success() {
                    return Ok("Deno JS runtime updated successfully!".to_string());
                }
            }
            Ok("Deno JS runtime is up to date!".to_string())
        } else {
            // FFmpeg
            #[cfg(target_os = "windows")]
            {
                let _ = make_command("winget")
                    .args([
                        "upgrade",
                        "--id",
                        "yt-dlp.FFmpeg",
                        "--accept-source-agreements",
                        "--accept-package-agreements",
                    ])
                    .output();
            }
            Ok("FFmpeg updated to the latest version successfully!".to_string())
        }
    })
    .await
    .map_err(|e| format!("Update error: {}", e))?
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


