use chrono::{Datelike, Local, Timelike, Weekday};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::RwLock;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn default_start_time() -> String {
    "00:00".to_string()
}

fn default_stop_time() -> String {
    "06:00".to_string()
}

fn default_days() -> Vec<String> {
    vec![
        "Mon".to_string(),
        "Tue".to_string(),
        "Wed".to_string(),
        "Thu".to_string(),
        "Fri".to_string(),
        "Sat".to_string(),
        "Sun".to_string(),
    ]
}

fn default_when_finished() -> String {
    "none".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SchedulerConfig {
    pub enabled: bool,
    #[serde(default = "default_start_time")]
    pub start_time: String, // "HH:MM"
    #[serde(default = "default_stop_time")]
    pub stop_time: String, // "HH:MM"
    #[serde(default = "default_days")]
    pub days: Vec<String>, // ["Mon", "Tue", ...]
    #[serde(default = "default_when_finished")]
    pub when_finished: String, // "none" | "close_app" | "sleep" | "shutdown"
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start_time: default_start_time(),
            stop_time: default_stop_time(),
            days: default_days(),
            when_finished: default_when_finished(),
        }
    }
}

#[derive(Clone)]
pub struct SchedulerState {
    pub config: Arc<RwLock<SchedulerConfig>>,
    pub config_path: PathBuf,
    pub pending_shutdown: Arc<AtomicBool>,
}

impl SchedulerState {
    pub fn load_or_default(app_data_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&app_data_dir);
        let config_path = app_data_dir.join("scheduler.json");
        let cfg = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<SchedulerConfig>(&s).ok())
            .unwrap_or_default();

        Self {
            config: Arc::new(RwLock::new(cfg)),
            config_path,
            pending_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn get(&self) -> SchedulerConfig {
        self.config.read().await.clone()
    }

    pub async fn save(&self, new_cfg: SchedulerConfig) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&new_cfg).map_err(|e| e.to_string())?;
        if let Some(parent) = self.config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = self.config_path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp_path, &self.config_path).map_err(|e| e.to_string())?;
        *self.config.write().await = new_cfg;
        Ok(())
    }
}

pub fn weekday_code(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

pub fn parse_hhmm(input: &str) -> Option<u32> {
    let parts: Vec<&str> = input.trim().split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    if h >= 24 || m >= 60 {
        return None;
    }
    Some(h * 60 + m)
}

/// Evaluates whether the provided `(weekday_code, minute_of_day)` falls inside
/// the configured schedule window (supporting both same-day and overnight windows).
pub fn is_within_schedule(cfg: &SchedulerConfig, day_code: &str, minute_of_day: u32) -> bool {
    if !cfg.enabled {
        return false;
    }
    let day_allowed = cfg.days.is_empty()
        || cfg
            .days
            .iter()
            .any(|d| d.trim().eq_ignore_ascii_case(day_code));
    if !day_allowed {
        return false;
    }

    let Some(start) = parse_hhmm(&cfg.start_time) else {
        return false;
    };
    let Some(stop) = parse_hhmm(&cfg.stop_time) else {
        return false;
    };

    if start == stop {
        // Equal start & stop means any time on allowed days triggers start-of-day.
        return true;
    }
    if start < stop {
        minute_of_day >= start && minute_of_day < stop
    } else {
        // Overnight window (e.g., 23:00 -> 06:00)
        minute_of_day >= start || minute_of_day < stop
    }
}

pub fn cancel_os_shutdown(state: &SchedulerState) -> Result<(), String> {
    state.pending_shutdown.store(false, Ordering::Relaxed);
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("shutdown");
        cmd.args(["/a"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.output();
    }
    Ok(())
}

fn execute_when_finished_action(app: &AppHandle, state: &SchedulerState, action: &str) {
    match action {
        "close_app" => {
            app.exit(0);
        }
        "sleep" => {
            #[cfg(target_os = "windows")]
            {
                let mut cmd = Command::new("rundll32.exe");
                cmd.args(["powrprof.dll,SetSuspendState", "0,1,0"]);
                cmd.creation_flags(CREATE_NO_WINDOW);
                let _ = cmd.spawn();
            }
        }
        "shutdown" => {
            if state.pending_shutdown.swap(true, Ordering::SeqCst) {
                return;
            }
            #[cfg(target_os = "windows")]
            {
                let mut cmd = Command::new("shutdown");
                cmd.args(["/s", "/t", "30"]);
                cmd.creation_flags(CREATE_NO_WINDOW);
                let _ = cmd.spawn();
            }
            let _ = app.emit(
                "scheduler-shutdown-countdown",
                serde_json::json!({ "seconds": 30 }),
            );
        }
        _ => {}
    }
}

pub fn start_scheduler_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut prev_in_window: Option<bool> = None;
        let mut saw_active_downloads = false;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

        loop {
            interval.tick().await;

            let Some(sched_state) = app.try_state::<SchedulerState>() else {
                continue;
            };
            let cfg = sched_state.get().await;

            if cfg.enabled {
                let now = Local::now();
                let day = weekday_code(now.weekday());
                let minute = now.hour() * 60 + now.minute();
                let in_window = is_within_schedule(&cfg, day, minute);

                if let Some(was_in_window) = prev_in_window {
                    if !was_in_window && in_window {
                        let app_c = app.clone();
                        tokio::spawn(async move {
                            crate::queue::resume_scheduled_queue(&app_c).await;
                        });
                    } else if was_in_window && !in_window {
                        let app_c = app.clone();
                        tokio::spawn(async move {
                            crate::queue::pause_all_downloads(&app_c).await;
                        });
                    }
                } else if in_window {
                    // App opened inside schedule window: kick queue once if items are queued.
                    let app_c = app.clone();
                    tokio::spawn(async move {
                        crate::queue::process_queue(&app_c).await;
                    });
                }
                prev_in_window = Some(in_window);
            } else {
                prev_in_window = None;
            }

            // "When finished" monitor: fires once after active downloads drain to 0.
            let (active_count, queued_count) = crate::queue::count_active_and_queued(&app).await;
            if active_count > 0 {
                saw_active_downloads = true;
            } else if saw_active_downloads && queued_count == 0 {
                saw_active_downloads = false;
                if cfg.when_finished != "none" {
                    execute_when_finished_action(&app, &sched_state, &cfg.when_finished);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_and_invalid_times() {
        assert_eq!(parse_hhmm("00:00"), Some(0));
        assert_eq!(parse_hhmm("02:30"), Some(150));
        assert_eq!(parse_hhmm("23:59"), Some(1439));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("12:60"), None);
        assert_eq!(parse_hhmm("bad"), None);
    }

    #[test]
    fn schedule_window_same_day_and_overnight() {
        let mut cfg = SchedulerConfig {
            enabled: true,
            start_time: "09:00".to_string(),
            stop_time: "17:00".to_string(),
            days: vec!["Mon".to_string(), "Wed".to_string()],
            when_finished: "shutdown".to_string(),
        };
        assert!(is_within_schedule(&cfg, "Mon", 9 * 60));
        assert!(is_within_schedule(&cfg, "Wed", 16 * 60 + 59));
        assert!(!is_within_schedule(&cfg, "Mon", 17 * 60));
        assert!(!is_within_schedule(&cfg, "Tue", 12 * 60));

        // Overnight window 23:00 -> 06:00
        cfg.start_time = "23:00".to_string();
        cfg.stop_time = "06:00".to_string();
        assert!(is_within_schedule(&cfg, "Mon", 23 * 60 + 30));
        assert!(is_within_schedule(&cfg, "Mon", 2 * 60));
        assert!(!is_within_schedule(&cfg, "Mon", 12 * 60));

        cfg.enabled = false;
        assert!(!is_within_schedule(&cfg, "Mon", 23 * 60 + 30));
    }
}
