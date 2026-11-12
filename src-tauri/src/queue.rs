use std::path::Path;
use tauri::{AppHandle, Emitter};
use crate::downloader::{self, DownloadTask, EngineState};

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
    process_queue(app, state);
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
            ctrl.is_paused.store(true, std::sync::atomic::Ordering::Relaxed);
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
    process_queue(app, state);
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
            if task.status == "paused" || task.status == "cancelled" || task.status == "error" {
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
        process_queue(app, state);
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
    process_queue(app, state);
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
        let meta_path = downloader::sparse_meta_path_for(&task.save_path, &task.id);
        let _ = std::fs::remove_file(meta_path);
        if let Some(parent) = Path::new(&task.save_path).parent() {
            let tmp_dir = parent.join(format!(".zr_tmp_{}", task.id));
            if tmp_dir.exists() {
                let _ = std::fs::remove_dir_all(tmp_dir);
            }
        }
        if delete_file {
            let path = Path::new(&task.save_path);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    crate::spawn_save_all_tasks(&app);
    Ok(())
}

pub fn process_queue(app: AppHandle, state: EngineState) {
    let app_handle = app.clone();
    let engine_state = state.clone();

    tauri::async_runtime::spawn(async move {
        const MAX_CONCURRENT: usize = 3;

        let mut tasks_guard = engine_state.tasks.write().await;
        let mut active_count = 0;
        for t in tasks_guard.values() {
            if t.status == "downloading" {
                active_count += 1;
            }
        }

        if active_count >= MAX_CONCURRENT {
            return;
        }

        let slots = MAX_CONCURRENT - active_count;
        let queued_ids: Vec<String> = tasks_guard
            .values()
            .filter(|t| t.status == "queued")
            .map(|t| t.id.clone())
            .take(slots)
            .collect();

        for tid in &queued_ids {
            if let Some(t) = tasks_guard.get_mut(tid) {
                t.status = "downloading".to_string();
                let _ = app_handle.emit("download-progress", t.clone());
            }
        }
        drop(tasks_guard);

        for tid in queued_ids {
            let app_c = app_handle.clone();
            let state_c = engine_state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = downloader::execute_multithreaded_download(app_c.clone(), state_c.clone(), tid).await;
                process_queue(app_c, state_c);
            });
        }
    });
}
