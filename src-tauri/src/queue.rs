use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::downloader::{self, DownloadTask, EngineState};
use crate::media::DownloadState;
use crate::settings::SettingsState;

static QUEUE_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueCandidate {
    pub id: String,
    pub is_media: bool,
    pub priority: String,
    pub queue_order: i64,
    pub created_at: String,
}

pub fn priority_rank(priority: &str) -> i32 {
    match priority.trim().to_lowercase().as_str() {
        "high" => 2,
        "low" => 0,
        _ => 1,
    }
}

pub fn sort_queue_candidates(candidates: &mut [QueueCandidate]) {
    candidates.sort_by(|a, b| {
        priority_rank(&b.priority)
            .cmp(&priority_rank(&a.priority))
            .then_with(|| a.queue_order.cmp(&b.queue_order))
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
}

pub async fn enqueue_download(
    app: AppHandle,
    state: EngineState,
    task: DownloadTask,
) -> Result<String, String> {
    let task_id = task.id.clone();
    {
        let mut tasks = state.tasks.write().await;
        tasks.insert(task_id.clone(), task.clone());
    }
    let _ = app.emit("download-progress", task);
    crate::spawn_save_all_tasks(&app);
    process_queue(&app).await;
    Ok(task_id)
}

pub async fn pause_download(
    app: AppHandle,
    state: EngineState,
    task_id: String,
) -> Result<(), String> {
    {
        let controls = state.controls.read().await;
        if let Some(ctrl) = controls.get(&task_id) {
            ctrl.is_paused
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = ctrl.cancel_sender.send(());
        }
    }
    let updated_task = {
        let mut tasks = state.tasks.write().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = "paused".to_string();
            task.speed_bps = 0;
            task.speed_formatted = "0 B/s".to_string();
            task.eta_formatted = "--".to_string();
            Some(task.clone())
        } else {
            None
        }
    };
    if let Some(task) = updated_task {
        let _ = app.emit("download-progress", task);
    }
    crate::spawn_save_all_tasks(&app);
    process_queue(&app).await;
    Ok(())
}

pub async fn resume_download(
    app: AppHandle,
    state: EngineState,
    task_id: String,
) -> Result<(), String> {
    let updated_task = {
        let mut tasks = state.tasks.write().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            if matches!(
                task.status.as_str(),
                "paused" | "cancelled" | "error" | "queued"
            ) {
                task.status = "queued".to_string();
                task.error_message = None;
                Some(task.clone())
            } else {
                None
            }
        } else {
            None
        }
    };
    if let Some(mut task) = updated_task {
        downloader::hydrate_task_from_sparse_meta(&mut task).await;
        {
            let mut tasks = state.tasks.write().await;
            tasks.insert(task.id.clone(), task.clone());
        }
        let _ = app.emit("download-progress", task);
        crate::spawn_save_all_tasks(&app);
        process_queue(&app).await;
    }
    Ok(())
}

pub async fn cancel_download(
    app: AppHandle,
    state: EngineState,
    task_id: String,
) -> Result<(), String> {
    {
        let controls = state.controls.read().await;
        if let Some(ctrl) = controls.get(&task_id) {
            let _ = ctrl.cancel_sender.send(());
        }
    }
    let updated_task = {
        let mut tasks = state.tasks.write().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = "cancelled".to_string();
            task.speed_bps = 0;
            task.speed_formatted = "0 B/s".to_string();
            task.eta_formatted = "--".to_string();
            Some(task.clone())
        } else {
            None
        }
    };
    if let Some(task) = updated_task {
        let _ = app.emit("download-progress", task);
    }
    crate::spawn_save_all_tasks(&app);
    process_queue(&app).await;
    Ok(())
}

pub async fn remove_download(
    app: AppHandle,
    state: EngineState,
    task_id: String,
    delete_file: bool,
) -> Result<(), String> {
    {
        let controls = state.controls.read().await;
        if let Some(ctrl) = controls.get(&task_id) {
            let _ = ctrl.cancel_sender.send(());
        }
    }
    let removed = {
        let mut tasks = state.tasks.write().await;
        tasks.remove(&task_id)
    };
    if let Some(task) = removed {
        let was_active = matches!(
            task.status.as_str(),
            "downloading" | "assembling" | "connecting"
        );
        let is_finished = matches!(task.status.as_str(), "completed" | "completed_with_warning");
        let cleanup = move || {
            let meta_path = downloader::sparse_meta_path_for(&task.save_path, &task.id);
            let _ = std::fs::remove_file(meta_path);
            if let Some(parent) = Path::new(&task.save_path).parent() {
                let tmp_dir = parent.join(format!(".zr_tmp_{}", task.id));
                if tmp_dir.exists() {
                    let _ = std::fs::remove_dir_all(tmp_dir);
                }
            }
            // An unfinished low-disk-space download is a full-size pre-allocated file that is
            // useless on its own, so it is always removed together with the task.
            if delete_file || (!is_finished && task.low_disk_space) {
                let path = Path::new(&task.save_path);
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
            }
        };
        if was_active {
            // Give the cancelled workers a moment to release their file handles (Windows cannot
            // delete open files) and let the progress monitor stop writing the resume metadata.
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                cleanup();
            });
        } else {
            cleanup();
        }
    }
    crate::spawn_save_all_tasks(&app);
    process_queue(&app).await;
    Ok(())
}

/// Unified queue admission across HTTP EngineState + Media DownloadState using SettingsState.max_active_downloads.
pub async fn process_queue(app: &AppHandle) {
    let _guard = QUEUE_LOCK.lock().await;

    let max_active = app
        .try_state::<SettingsState>()
        .map(|s| s.get_blocking().max_active_downloads.clamp(1, 10))
        .unwrap_or(3);

    let engine_state = app.try_state::<EngineState>().map(|s| s.inner().clone());
    let media_state = app.try_state::<DownloadState>();

    let mut active_count = 0usize;
    let mut candidates: Vec<QueueCandidate> = Vec::new();

    if let Some(ref eng) = engine_state {
        let tasks = eng.tasks.read().await;
        for t in tasks.values() {
            if matches!(t.status.as_str(), "downloading" | "assembling" | "connecting") {
                active_count += 1;
            } else if t.status == "queued" {
                candidates.push(QueueCandidate {
                    id: t.id.clone(),
                    is_media: false,
                    priority: t.priority.clone(),
                    queue_order: t.queue_order,
                    created_at: t.created_at.clone(),
                });
            }
        }
    }

    if let Some(ref med) = media_state {
        let tasks = med.tasks.lock().unwrap();
        for t in tasks.values() {
            if matches!(t.status.as_str(), "downloading" | "assembling" | "connecting") {
                active_count += 1;
            } else if t.status == "queued" {
                candidates.push(QueueCandidate {
                    id: t.id.clone(),
                    is_media: true,
                    priority: t.priority.clone(),
                    queue_order: t.queue_order,
                    created_at: t.created_at.clone(),
                });
            }
        }
    }

    if active_count >= max_active || candidates.is_empty() {
        return;
    }

    sort_queue_candidates(&mut candidates);
    let slots = max_active - active_count;
    let selected: Vec<QueueCandidate> = candidates.into_iter().take(slots).collect();

    let mut http_to_start = Vec::new();
    let mut media_to_start = Vec::new();

    if let Some(ref eng) = engine_state {
        let mut tasks = eng.tasks.write().await;
        for cand in &selected {
            if !cand.is_media {
                if let Some(t) = tasks.get_mut(&cand.id) {
                    t.status = "downloading".to_string();
                    let _ = app.emit("download-progress", t.clone());
                    http_to_start.push(cand.id.clone());
                }
            }
        }
    }

    if let Some(ref med) = media_state {
        let mut tasks = med.tasks.lock().unwrap();
        for cand in &selected {
            if cand.is_media {
                if let Some(t) = tasks.get_mut(&cand.id) {
                    t.status = "downloading".to_string();
                    t.speed_formatted = "Connecting...".to_string();
                    let _ = app.emit("download-progress", t.clone());
                    media_to_start.push(cand.id.clone());
                }
            }
        }
    }

    crate::spawn_save_all_tasks(app);

    if let Some(eng) = engine_state {
        for tid in http_to_start {
            let app_c = app.clone();
            let state_c = eng.clone();
            tauri::async_runtime::spawn(async move {
                let _ = downloader::execute_multithreaded_download(app_c.clone(), state_c, tid).await;
                kick_queue(app_c);
            });
        }
    }

    for tid in media_to_start {
        crate::media::launch_media_worker(app.clone(), tid);
    }
}

/// Non-async launcher to trigger queue progression without creating recursive `async fn` cycles.
pub fn kick_queue(app: AppHandle) {
    let fut: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> =
        Box::pin(async move {
            process_queue(&app).await;
        });
    tauri::async_runtime::spawn(fut);
}

pub async fn count_active_and_queued(app: &AppHandle) -> (usize, usize) {
    let mut active = 0usize;
    let mut queued = 0usize;
    if let Some(eng) = app.try_state::<EngineState>() {
        let tasks = eng.tasks.read().await;
        for t in tasks.values() {
            if matches!(t.status.as_str(), "downloading" | "assembling" | "connecting") {
                active += 1;
            } else if t.status == "queued" {
                queued += 1;
            }
        }
    }
    if let Some(med) = app.try_state::<DownloadState>() {
        let tasks = med.tasks.lock().unwrap();
        for t in tasks.values() {
            if matches!(t.status.as_str(), "downloading" | "assembling" | "connecting") {
                active += 1;
            } else if t.status == "queued" {
                queued += 1;
            }
        }
    }
    (active, queued)
}

pub async fn pause_all_downloads(app: &AppHandle) {
    if let Some(eng) = app.try_state::<EngineState>() {
        let controls = eng.controls.read().await;
        for ctrl in controls.values() {
            ctrl.is_paused
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = ctrl.cancel_sender.send(());
        }
        drop(controls);
        let mut tasks = eng.tasks.write().await;
        for t in tasks.values_mut() {
            if matches!(t.status.as_str(), "downloading" | "queued" | "connecting") {
                t.status = "paused".to_string();
                t.speed_bps = 0;
                t.speed_formatted = "0 B/s".to_string();
                t.eta_formatted = "--".to_string();
                let _ = app.emit("download-progress", t.clone());
            }
        }
    }

    if let Some(med) = app.try_state::<DownloadState>() {
        let _ = crate::media::cancel_download(app.clone(), med.clone(), None).await;
        let mut tasks = med.tasks.lock().unwrap();
        for t in tasks.values_mut() {
            if matches!(t.status.as_str(), "downloading" | "queued" | "connecting") {
                t.status = "paused".to_string();
                t.speed_bps = 0;
                t.speed_formatted = "0 B/s".to_string();
                t.eta_formatted = "--".to_string();
                let _ = app.emit("download-progress", t.clone());
            }
        }
    }

    crate::spawn_save_all_tasks(app);
}

pub async fn resume_scheduled_queue(app: &AppHandle) {
    if let Some(eng) = app.try_state::<EngineState>() {
        let mut tasks = eng.tasks.write().await;
        for t in tasks.values_mut() {
            if t.status == "paused" {
                t.status = "queued".to_string();
                let _ = app.emit("download-progress", t.clone());
            }
        }
    }
    if let Some(med) = app.try_state::<DownloadState>() {
        let mut tasks = med.tasks.lock().unwrap();
        for t in tasks.values_mut() {
            if t.status == "paused" {
                t.status = "queued".to_string();
                let _ = app.emit("download-progress", t.clone());
            }
        }
    }
    crate::spawn_save_all_tasks(app);
    process_queue(app).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_by_priority_then_order_across_http_and_media() {
        let mut items = vec![
            QueueCandidate {
                id: "low_1".into(),
                is_media: false,
                priority: "low".into(),
                queue_order: 1,
                created_at: "2026-01-01 00:00:01".into(),
            },
            QueueCandidate {
                id: "norm_2".into(),
                is_media: true,
                priority: "normal".into(),
                queue_order: 2,
                created_at: "2026-01-01 00:00:02".into(),
            },
            QueueCandidate {
                id: "norm_1".into(),
                is_media: false,
                priority: "normal".into(),
                queue_order: 1,
                created_at: "2026-01-01 00:00:03".into(),
            },
            QueueCandidate {
                id: "high_2".into(),
                is_media: true,
                priority: "high".into(),
                queue_order: 2,
                created_at: "2026-01-01 00:00:04".into(),
            },
            QueueCandidate {
                id: "high_1".into(),
                is_media: false,
                priority: "high".into(),
                queue_order: 1,
                created_at: "2026-01-01 00:00:05".into(),
            },
        ];

        sort_queue_candidates(&mut items);
        let ids: Vec<&str> = items.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["high_1", "high_2", "norm_1", "norm_2", "low_1"]);
    }
}
