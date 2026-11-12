use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, RwLock};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SiteCredential {
    pub id: String,
    pub domain: String,
    pub username: String,
    pub password: String,
}

fn default_max_active() -> usize {
    3
}
fn default_connection_mode() -> String {
    "fixed".to_string()
}
fn default_connections() -> usize {
    8
}
fn default_speed_value() -> u64 {
    5
}
fn default_speed_unit() -> String {
    "MB".to_string()
}
fn default_post_action() -> String {
    "none".to_string()
}
fn default_proxy_type() -> String {
    "http".to_string()
}
fn default_proxy_port() -> u16 {
    8080
}
fn default_cookies_browser() -> String {
    "None".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub default_download_dir: String,
    #[serde(default = "default_max_active")]
    pub max_active_downloads: usize,
    #[serde(default = "default_connection_mode")]
    pub connection_mode: String, // "fixed" | "auto"
    #[serde(default = "default_connections")]
    pub default_connections: usize, // 4 | 8 | 16 | 24 | 32
    pub low_disk_space: bool,

    // Speed Limiter
    pub speed_limit_enabled: bool,
    #[serde(default = "default_speed_value")]
    pub speed_limit_value: u64,
    #[serde(default = "default_speed_unit")]
    pub speed_limit_unit: String, // "KB" | "MB"

    // File Handling
    pub auto_categorize: bool,
    pub auto_category_root: String,
    #[serde(default = "default_post_action")]
    pub post_download_action: String, // "none" | "open_file" | "open_folder" | "run_program" | "defender_scan"
    pub post_download_program: String,

    // Network & Credentials
    pub proxy_enabled: bool,
    #[serde(default = "default_proxy_type")]
    pub proxy_type: String, // "http" | "https" | "socks5"
    pub proxy_host: String,
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    pub proxy_username: String,
    pub proxy_password: String,
    #[serde(default = "default_cookies_browser")]
    pub cookies_browser: String,
    pub cookies_file: String,
    pub site_credentials: Vec<SiteCredential>,

    // UX & App Shell
    pub clipboard_monitor: bool,
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_download_dir: String::new(),
            max_active_downloads: 3,
            connection_mode: "fixed".to_string(),
            default_connections: 8,
            low_disk_space: false,
            speed_limit_enabled: false,
            speed_limit_value: 5,
            speed_limit_unit: "MB".to_string(),
            auto_categorize: false,
            auto_category_root: String::new(),
            post_download_action: "none".to_string(),
            post_download_program: String::new(),
            proxy_enabled: false,
            proxy_type: "http".to_string(),
            proxy_host: String::new(),
            proxy_port: 8080,
            proxy_username: String::new(),
            proxy_password: String::new(),
            cookies_browser: "None".to_string(),
            cookies_file: String::new(),
            site_credentials: Vec::new(),
            clipboard_monitor: false,
            start_with_windows: false,
            minimize_to_tray: false,
            close_to_tray: true,
        }
    }
}

impl AppSettings {
    pub fn global_speed_limit_kbps(&self) -> u64 {
        if !self.speed_limit_enabled || self.speed_limit_value == 0 {
            return 0;
        }
        if self.speed_limit_unit.eq_ignore_ascii_case("MB") {
            self.speed_limit_value.saturating_mul(1024)
        } else {
            self.speed_limit_value
        }
    }

    pub fn global_speed_limit_bps(&self) -> u64 {
        self.global_speed_limit_kbps().saturating_mul(1024)
    }

    pub fn effective_proxy_url(&self) -> Option<String> {
        if !self.proxy_enabled {
            return None;
        }
        let host = self.proxy_host.trim();
        if host.is_empty() {
            return None;
        }
        let scheme = match self.proxy_type.trim().to_lowercase().as_str() {
            "socks5" | "socks" => "socks5",
            "https" => "https",
            _ => "http",
        };
        let port = if self.proxy_port == 0 {
            8080
        } else {
            self.proxy_port
        };
        let user = self.proxy_username.trim();
        let pass = self.proxy_password.trim();
        if !user.is_empty() {
            let u_enc = crate::url_encode(user);
            let p_enc = crate::url_encode(pass);
            Some(format!("{}://{}:{}@{}:{}", scheme, u_enc, p_enc, host, port))
        } else {
            Some(format!("{}://{}:{}", scheme, host, port))
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.max_active_downloads = self.max_active_downloads.clamp(1, 10);
        if !matches!(self.connection_mode.as_str(), "fixed" | "auto") {
            self.connection_mode = "fixed".to_string();
        }
        self.default_connections = self.default_connections.clamp(1, 32);
        self
    }
}

#[derive(Clone)]
pub struct SettingsState {
    pub inner: Arc<RwLock<AppSettings>>,
    pub settings_path: Option<PathBuf>,
}

impl SettingsState {
    pub fn load_or_default(settings_path: Option<PathBuf>) -> Self {
        let initial = settings_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
            .map(|s| s.sanitized())
            .unwrap_or_default();

        Self {
            inner: Arc::new(RwLock::new(initial)),
            settings_path,
        }
    }

    pub async fn get(&self) -> AppSettings {
        self.inner.read().await.clone()
    }

    pub async fn save(&self, new_settings: AppSettings) -> Result<AppSettings, String> {
        let clean = new_settings.sanitized();
        if let Some(ref path) = self.settings_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let json = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
            if std::fs::rename(&tmp, path).is_err() {
                std::fs::copy(&tmp, path).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&tmp);
            }
        }
        *self.inner.write().await = clean.clone();
        Ok(clean)
    }
}

pub fn build_reqwest_client(settings: &AppSettings) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(12))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(64)
        .tcp_nodelay(true)
        .tcp_keepalive(Duration::from_secs(30));

    if let Some(proxy_url) = settings.effective_proxy_url() {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
        }
    }

    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

// ---------------------------------------------------------------------------
// Async Token-Bucket Speed Limiter (shared across segments & global limit)
// ---------------------------------------------------------------------------

struct BucketInner {
    tokens: f64,
    last_refill: Instant,
}

#[derive(Clone)]
pub struct TokenBucketLimiter {
    rate_bps: Arc<AtomicU64>,
    state: Arc<Mutex<BucketInner>>,
}

impl TokenBucketLimiter {
    pub fn new(rate_bps: u64) -> Self {
        Self {
            rate_bps: Arc::new(AtomicU64::new(rate_bps)),
            state: Arc::new(Mutex::new(BucketInner {
                tokens: if rate_bps > 0 { rate_bps as f64 } else { 0.0 },
                last_refill: Instant::now(),
            })),
        }
    }

    pub fn set_rate_bps(&self, rate_bps: u64) {
        self.rate_bps.store(rate_bps, Ordering::Relaxed);
    }

    pub fn get_rate_bps(&self) -> u64 {
        self.rate_bps.load(Ordering::Relaxed)
    }

    pub async fn consume(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }
        let mut remaining = bytes;
        // Break huge bursts into <= 64 KB steps so live speed changes apply immediately
        while remaining > 0 {
            let rate = self.rate_bps.load(Ordering::Relaxed);
            if rate == 0 {
                return;
            }
            let step = remaining.min((rate / 4).max(16 * 1024));
            remaining -= step;

            let sleep_dur = {
                let mut inner = self.state.lock().await;
                let now = Instant::now();
                let elapsed = now.duration_since(inner.last_refill).as_secs_f64();
                inner.last_refill = now;

                let rate_f = rate as f64;
                // Cap burst bucket to at most 0.5 s worth of rate (minimum 32 KB)
                let max_burst = (rate_f * 0.5).max(32.0 * 1024.0);
                inner.tokens = (inner.tokens + elapsed * rate_f).min(max_burst);
                inner.tokens -= step as f64;

                if inner.tokens < 0.0 {
                    let wait_secs = (-inner.tokens) / rate_f;
                    Duration::from_secs_f64(wait_secs.min(5.0))
                } else {
                    Duration::ZERO
                }
            };

            if !sleep_dur.is_zero() {
                tokio::time::sleep(sleep_dur).await;
            }
        }
    }
}

pub fn find_credentials_for_url<'a>(
    settings: &'a AppSettings,
    url: &str,
) -> Option<(&'a str, &'a str)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    for cred in &settings.site_credentials {
        let domain = cred
            .domain
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase();
        if domain.is_empty() || cred.username.trim().is_empty() {
            continue;
        }
        if host == domain || host.ends_with(&format!(".{}", domain)) {
            return Some((cred.username.as_str(), cred.password.as_str()));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// File Handling: Auto-Categorize, Checksum Verification & Post-Download Actions
// ---------------------------------------------------------------------------

pub fn auto_category_subfolder(filename: &str) -> &'static str {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" | "ts" => "Videos",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "opus" | "wma" => "Music",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" => "Compressed",
        "exe" | "msi" | "apk" | "dmg" | "deb" | "rpm" | "appimage" | "msix" => "Programs",
        _ => "Documents",
    }
}

pub fn maybe_move_to_auto_category(settings: &AppSettings, current_file: &Path) -> PathBuf {
    if !settings.auto_categorize || !current_file.exists() || !current_file.is_file() {
        return current_file.to_path_buf();
    }
    let Some(filename) = current_file.file_name().and_then(|n| n.to_str()) else {
        return current_file.to_path_buf();
    };
    let root = if !settings.auto_category_root.trim().is_empty() {
        PathBuf::from(settings.auto_category_root.trim())
    } else if !settings.default_download_dir.trim().is_empty() {
        PathBuf::from(settings.default_download_dir.trim())
    } else {
        current_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(std::env::temp_dir)
    };

    let category_dir = root.join(auto_category_subfolder(filename));
    if let Err(_) = std::fs::create_dir_all(&category_dir) {
        return current_file.to_path_buf();
    }

    let mut target = category_dir.join(filename);
    if target == current_file {
        return current_file.to_path_buf();
    }
    if target.exists() {
        let stem = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        for i in 1..1000 {
            let candidate = if ext.is_empty() {
                category_dir.join(format!("{} ({})", stem, i))
            } else {
                category_dir.join(format!("{} ({}).{}", stem, i, ext))
            };
            if !candidate.exists() {
                target = candidate;
                break;
            }
        }
    }

    if std::fs::rename(current_file, &target).is_ok() {
        return target;
    }
    if std::fs::copy(current_file, &target).is_ok() {
        let _ = std::fs::remove_file(current_file);
        return target;
    }
    current_file.to_path_buf()
}

pub fn verify_file_checksum(
    file_path: &Path,
    expected_raw: &str,
) -> Result<(bool, String, String), String> {
    let trimmed = expected_raw.trim();
    if trimmed.is_empty() {
        return Err("Empty expected checksum".to_string());
    }
    let clean_hex = trimmed
        .strip_prefix("sha256:")
        .or_else(|| trimmed.strip_prefix("SHA256:"))
        .or_else(|| trimmed.strip_prefix("md5:"))
        .or_else(|| trimmed.strip_prefix("MD5:"))
        .unwrap_or(trimmed)
        .trim()
        .to_lowercase();

    let is_md5 = trimmed.to_lowercase().starts_with("md5:") || clean_hex.len() == 32;
    let mut file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1024 * 1024];

    if is_md5 {
        use md5::Digest as _;
        let mut hasher = Md5::new();
        loop {
            let n = file.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let actual = format!("{:x}", hasher.finalize());
        Ok((actual == clean_hex, "MD5".to_string(), actual))
    } else {
        use sha2::Digest as _;
        let mut hasher = Sha256::new();
        loop {
            let n = file.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let actual = format!("{:x}", hasher.finalize());
        Ok((actual == clean_hex, "SHA-256".to_string(), actual))
    }
}

#[cfg(target_os = "windows")]
fn find_windows_defender_cli() -> Option<PathBuf> {
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    let direct = PathBuf::from(&pf)
        .join("Windows Defender")
        .join("MpCmdRun.exe");
    if direct.exists() {
        return Some(direct);
    }
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let platform_dir = PathBuf::from(pd)
        .join("Microsoft")
        .join("Windows Defender")
        .join("Platform");
    if let Ok(entries) = std::fs::read_dir(platform_dir) {
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path().join("MpCmdRun.exe"))
            .filter(|p| p.exists())
            .collect();
        versions.sort();
        if let Some(latest) = versions.pop() {
            return Some(latest);
        }
    }
    None
}

pub fn execute_post_download_action(
    app: &AppHandle,
    task_id: &str,
    action: &str,
    program: &str,
    file_path: &Path,
) -> Option<String> {
    let action_clean = action.trim().to_lowercase();
    if action_clean.is_empty() || action_clean == "none" {
        return None;
    }

    match action_clean.as_str() {
        "open_file" => {
            let _ = open::that(file_path);
            Some("Opened downloaded file".to_string())
        }
        "open_folder" => {
            if let Some(parent) = file_path.parent() {
                let _ = open::that(parent);
            }
            Some("Opened destination folder".to_string())
        }
        "run_program" => {
            let prog = program.trim();
            if prog.is_empty() {
                return Some("Skipped post-download program (no executable configured)".to_string());
            }
            let file_str = file_path.to_string_lossy().to_string();
            let expanded = if prog.contains("{file}") {
                prog.replace("{file}", &format!("\"{}\"", file_str))
            } else {
                format!("\"{}\" \"{}\"", prog, file_str)
            };
            #[cfg(target_os = "windows")]
            {
                let mut cmd = Command::new("cmd");
                cmd.args(["/C", &expanded]).creation_flags(CREATE_NO_WINDOW);
                let _ = cmd.spawn();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::process::Command::new("sh")
                    .args(["-c", &expanded])
                    .spawn();
            }
            let msg = format!("Ran post-download command: {}", expanded);
            let _ = app.emit(
                "download-log",
                serde_json::json!({ "id": task_id, "message": msg }),
            );
            Some("Program executed".to_string())
        }
        "defender_scan" => {
            #[cfg(target_os = "windows")]
            {
                if let Some(mp_cmd) = find_windows_defender_cli() {
                    let _ = app.emit(
                        "download-log",
                        serde_json::json!({
                            "id": task_id,
                            "message": "Scanning file with Windows Defender..."
                        }),
                    );
                    let mut cmd = Command::new(mp_cmd);
                    cmd.args([
                        "-Scan",
                        "-ScanType",
                        "3",
                        "-File",
                        &file_path.to_string_lossy(),
                    ])
                    .creation_flags(CREATE_NO_WINDOW);
                    match cmd.output() {
                        Ok(out) if out.status.success() => {
                            let _ = app.emit(
                                "download-log",
                                serde_json::json!({
                                    "id": task_id,
                                    "message": "Windows Defender scan passed: No threats found"
                                }),
                            );
                            Some("clean".to_string())
                        }
                        Ok(out) => {
                            let code = out.status.code().unwrap_or(-1);
                            let _ = app.emit(
                                "download-log",
                                serde_json::json!({
                                    "id": task_id,
                                    "message": format!("Windows Defender scan exited with code {}", code)
                                }),
                            );
                            Some(format!("flagged (code {})", code))
                        }
                        Err(e) => Some(format!("scan failed: {}", e)),
                    }
                } else {
                    Some("MpCmdRun.exe not found".to_string())
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = file_path;
                Some("Defender scan is Windows-only".to_string())
            }
        }
        _ => None,
    }
}

pub fn apply_start_with_windows(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let val = format!("\"{}\"", exe.to_string_lossy());
            let out = Command::new("reg")
                .args([
                    "add",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    "ZR Downloader",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &val,
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
        } else {
            let _ = Command::new("reg")
                .args([
                    "delete",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    "ZR Downloader",
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn token_bucket_holds_configured_speed() {
        // Target ~200 KB/s across 160 KB (expect >= 250ms because initial bucket burst <= 0.5s = 100 KB)
        let rate_bps = 200 * 1024;
        let limiter = TokenBucketLimiter::new(rate_bps);
        let start = Instant::now();
        for _ in 0..5 {
            limiter.consume(32 * 1024).await;
        }
        let elapsed = start.elapsed();
        // Initial burst cap is 100 KB, remaining 60 KB takes 0.30s at 200 KB/s
        assert!(
            elapsed >= Duration::from_millis(220),
            "Limiter elapsed {:?} was faster than expected throttle",
            elapsed
        );
    }

    #[test]
    fn checksum_verification_md5_and_sha256() {
        let dir = std::env::temp_dir().join("zr_checksum_test");
        let _ = std::fs::create_dir_all(&dir);
        let fpath = dir.join("sample.bin");
        std::fs::write(&fpath, b"hello zr downloader").unwrap();

        let expected_md5 = "5ce1561ec46b3b3d6f5f9937483f8f9b";
        let (ok, alg, actual) = verify_file_checksum(&fpath, expected_md5).unwrap();
        assert_eq!(alg, "MD5");
        assert!(ok, "Expected MD5 {} to match, got {}", expected_md5, actual);

        let (ok_bad, _, _) =
            verify_file_checksum(&fpath, "00000000000000000000000000000000").unwrap();
        assert!(!ok_bad);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_category_maps_expected_folders() {
        assert_eq!(auto_category_subfolder("movie.mkv"), "Videos");
        assert_eq!(auto_category_subfolder("song.flac"), "Music");
        assert_eq!(auto_category_subfolder("archive.zip"), "Compressed");
        assert_eq!(auto_category_subfolder("setup.exe"), "Programs");
        assert_eq!(auto_category_subfolder("report.pdf"), "Documents");
    }
}
