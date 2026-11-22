use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

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

pub fn generate_bridge_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
fn default_bridge_port() -> u16 {
    9666
}
fn default_bridge_token() -> String {
    generate_bridge_token()
}
fn default_name_conflict() -> String {
    "keep_both".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeaderPreset {
    pub name: String,
    pub user_agent: String,
    pub headers: std::collections::HashMap<String, String>,
}

impl Default for HeaderPreset {
    fn default() -> Self {
        Self {
            name: "Chrome (Windows)".to_string(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string(),
            headers: [
                ("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".to_string()),
                ("Accept-Language".to_string(), "en-US,en;q=0.9".to_string()),
            ].into_iter().collect(),
        }
    }
}

pub fn default_header_presets() -> Vec<HeaderPreset> {
    vec![
        HeaderPreset {
            name: "Chrome (Windows)".to_string(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string(),
            headers: [
                ("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".to_string()),
                ("Accept-Language".to_string(), "en-US,en;q=0.9".to_string()),
            ].into_iter().collect(),
        },
        HeaderPreset {
            name: "Firefox (Windows)".to_string(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:130.0) Gecko/20100101 Firefox/130.0".to_string(),
            headers: [
                ("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".to_string()),
                ("Accept-Language".to_string(), "en-US,en;q=0.5".to_string()),
            ].into_iter().collect(),
        },
        HeaderPreset {
            name: "Edge (Windows)".to_string(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0".to_string(),
            headers: [
                ("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".to_string()),
                ("Accept-Language".to_string(), "en-US,en;q=0.9".to_string()),
            ].into_iter().collect(),
        },
        HeaderPreset {
            name: "Android Chrome".to_string(),
            user_agent: "Mozilla/5.0 (Linux; Android 14; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36".to_string(),
            headers: [
                ("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".to_string()),
                ("Accept-Language".to_string(), "en-US,en;q=0.9".to_string()),
            ].into_iter().collect(),
        },
        HeaderPreset {
            name: "curl".to_string(),
            user_agent: "curl/8.9.1".to_string(),
            headers: [
                ("Accept".to_string(), "*/*".to_string()),
            ].into_iter().collect(),
        },
    ]
}

fn default_active_header_preset() -> String {
    "Chrome (Windows)".to_string()
}
fn default_aria2_port() -> u16 {
    16800
}
fn default_github_mirror() -> String {
    "https://ghproxy.net/".to_string()
}
fn default_hf_mirror() -> String {
    "https://hf-mirror.com/".to_string()
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
    #[serde(default = "default_name_conflict")]
    pub name_conflict: String, // "keep_both" | "overwrite" | "ask"
    #[serde(default)]
    pub preserve_last_modified: bool,
    #[serde(default = "default_true")]
    pub verify_ssl: bool,
    #[serde(default = "default_true")]
    pub use_system_dns: bool,
    #[serde(default)]
    pub delete_files_on_remove: bool,
    #[serde(default)]
    pub remove_missing_tasks: bool,

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

    // Browser Bridge Integration
    #[serde(default = "default_bridge_port")]
    pub bridge_port: u16,
    #[serde(default = "default_true")]
    pub bridge_auto_port: bool,
    #[serde(default = "default_bridge_token")]
    pub bridge_pair_token: String,

    // Header & User-Agent Presets
    #[serde(default = "default_header_presets")]
    pub header_presets: Vec<HeaderPreset>,
    #[serde(default = "default_active_header_preset")]
    pub active_header_preset: String,

    // aria2 JSON-RPC
    #[serde(default)]
    pub aria2_rpc_enabled: bool,
    #[serde(default = "default_aria2_port")]
    pub aria2_rpc_port: u16,
    #[serde(default)]
    pub aria2_rpc_token: String,

    // Mirror Rewriting
    #[serde(default)]
    pub github_mirror_enabled: bool,
    #[serde(default = "default_github_mirror")]
    pub github_mirror_url: String,
    #[serde(default)]
    pub huggingface_mirror_enabled: bool,
    #[serde(default = "default_hf_mirror")]
    pub huggingface_mirror_url: String,
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
            name_conflict: "keep_both".to_string(),
            preserve_last_modified: false,
            verify_ssl: true,
            use_system_dns: true,
            delete_files_on_remove: false,
            remove_missing_tasks: false,
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
            bridge_port: 9666,
            bridge_auto_port: true,
            bridge_pair_token: generate_bridge_token(),
            header_presets: default_header_presets(),
            active_header_preset: default_active_header_preset(),
            aria2_rpc_enabled: false,
            aria2_rpc_port: 16800,
            aria2_rpc_token: String::new(),
            github_mirror_enabled: false,
            github_mirror_url: default_github_mirror(),
            huggingface_mirror_enabled: false,
            huggingface_mirror_url: default_hf_mirror(),
        }
    }
}

impl AppSettings {
    pub fn get_active_user_agent(&self) -> String {
        self.header_presets
            .iter()
            .find(|p| p.name == self.active_header_preset)
            .map(|p| p.user_agent.clone())
            .unwrap_or_else(|| {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string()
            })
    }

    pub fn get_active_headers(&self) -> std::collections::HashMap<String, String> {
        self.header_presets
            .iter()
            .find(|p| p.name == self.active_header_preset)
            .map(|p| p.headers.clone())
            .unwrap_or_default()
    }

    pub fn effective_gh_mirror(&self) -> Option<&str> {
        if self.github_mirror_enabled && !self.github_mirror_url.trim().is_empty() {
            Some(self.github_mirror_url.trim())
        } else {
            None
        }
    }

    pub fn effective_hf_mirror(&self) -> Option<&str> {
        if self.huggingface_mirror_enabled && !self.huggingface_mirror_url.trim().is_empty() {
            Some(self.huggingface_mirror_url.trim())
        } else {
            None
        }
    }

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
        if !matches!(self.name_conflict.as_str(), "keep_both" | "overwrite" | "ask") {
            self.name_conflict = "keep_both".to_string();
        }
        if self.bridge_port == 0 {
            self.bridge_port = 9666;
        }
        if self.bridge_pair_token.trim().is_empty() {
            self.bridge_pair_token = generate_bridge_token();
        }
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
        let (initial, should_persist) = match settings_path.as_ref() {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<AppSettings>(&content) {
                    Ok(parsed) => {
                        let token_was_empty = parsed.bridge_pair_token.trim().is_empty();
                        let sanitized = parsed.sanitized();
                        (sanitized, token_was_empty)
                    }
                    Err(_) => {
                        let default = AppSettings::default().sanitized();
                        (default, true)
                    }
                },
                Err(_) => {
                    let default = AppSettings::default().sanitized();
                    (default, true)
                }
            },
            None => (AppSettings::default().sanitized(), false),
        };

        if should_persist {
            if let Some(ref path) = settings_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok(json) = serde_json::to_string_pretty(&initial) {
                    let tmp = path.with_extension("json.tmp");
                    if std::fs::write(&tmp, json.as_bytes()).is_ok() {
                        if std::fs::rename(&tmp, path).is_err() {
                            let _ = std::fs::copy(&tmp, path);
                            let _ = std::fs::remove_file(&tmp);
                        }
                    }
                }
            }
        }

        Self {
            inner: Arc::new(RwLock::new(initial)),
            settings_path,
        }
    }

    pub fn get_blocking(&self) -> AppSettings {
        self.inner
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub async fn get(&self) -> AppSettings {
        self.get_blocking()
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
        if let Ok(mut guard) = self.inner.write() {
            *guard = clean.clone();
        }
        Ok(clean)
    }
}

pub fn build_reqwest_client(settings: &AppSettings) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(64)
        .tcp_nodelay(true)
        .tcp_keepalive(Duration::from_secs(30));

    if !settings.verify_ssl {
        builder = builder.danger_accept_invalid_certs(true);
    }

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

#[cfg(not(test))]
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

        let expected_md5 = "fd05801ed4d2fce4609db320f670fc4c";
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

    #[test]
    fn bridge_settings_defaults_and_backward_compatibility() {
        // Deserializing empty JSON or legacy settings.json without bridge fields
        let legacy_json = r#"{"defaultDownloadDir":"C:\\Downloads","maxActiveDownloads":3}"#;
        let settings: AppSettings = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(settings.bridge_port, 9666);
        assert!(settings.bridge_auto_port);
        assert!(!settings.bridge_pair_token.is_empty());
        assert!(settings.bridge_pair_token.len() >= 16);

        let sanitized = settings.sanitized();
        assert_eq!(sanitized.bridge_port, 9666);
        assert!(!sanitized.bridge_pair_token.is_empty());
    }

    #[test]
    fn test_load_or_default_persists_token() {
        let temp_dir = std::env::temp_dir().join(format!("zr_test_settings_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let settings_file = temp_dir.join("settings.json");

        // 1. Initial load from non-existent file
        assert!(!settings_file.exists());
        let state1 = SettingsState::load_or_default(Some(settings_file.clone()));
        let token1 = state1.get_blocking().bridge_pair_token;
        assert!(!token1.is_empty(), "Token should be generated");
        assert!(settings_file.exists(), "settings.json should be persisted immediately");

        // 2. Second load from the now-existing file
        let state2 = SettingsState::load_or_default(Some(settings_file.clone()));
        let token2 = state2.get_blocking().bridge_pair_token;
        assert_eq!(token1, token2, "Token must remain stable across restarts");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
