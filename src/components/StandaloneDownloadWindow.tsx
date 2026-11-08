import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Play,
  Pause,
  X,
  FolderOpen,
  FileCheck,
  Zap,
  AlertCircle,
  EyeOff,
} from "lucide-react";
import {
  DownloadTask,
  formatBytes,
  getDownloadedBytes,
  getTotalBytes,
  getChunkCount,
  getIsResumable,
} from "../types";

export interface StandaloneDownloadWindowProps {
  taskId: string;
}

export const StandaloneDownloadWindow: React.FC<StandaloneDownloadWindowProps> = ({ taskId }) => {
  const [task, setTask] = useState<DownloadTask | null>(null);
  const [theme, setTheme] = useState<"dark" | "light">("dark");

  // Load theme preference
  useEffect(() => {
    const savedTheme = localStorage.getItem("zr-theme") || "dark";
    if (savedTheme === "dark") {
      setTheme("dark");
      document.documentElement.classList.add("dark");
      document.documentElement.classList.remove("light");
    } else {
      setTheme("light");
      document.documentElement.classList.add("light");
      document.documentElement.classList.remove("dark");
    }
  }, []);

  // Fetch initial task state
  useEffect(() => {
    const fetchTask = async () => {
      try {
        const all = await invoke<DownloadTask[]>("get_all_downloads");
        if (Array.isArray(all)) {
          const found = all.find((t) => t.id === taskId);
          if (found) {
            setTask(found);
          }
        }
      } catch (err) {
        console.error("Failed to load initial task:", err);
      }
    };
    fetchTask();
    const interval = setInterval(fetchTask, 1500);
    return () => clearInterval(interval);
  }, [taskId]);

  // Listen to Tauri progress events
  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenHttp: (() => void) | undefined;
    let unlistenMedia: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenProgress = await listen<any>("progress", (event) => {
          const updated = event.payload;
          const id = updated?.id || updated?.taskId || updated?.task_id;
          if (id === taskId) {
            setTask((prev) => (prev ? { ...prev, ...updated, id: taskId } : updated));
          }
        });

        unlistenHttp = await listen<any>("download-progress", (event) => {
          const updated = event.payload;
          const id = updated?.id || updated?.taskId || updated?.task_id;
          if (id === taskId) {
            setTask((prev) => (prev ? { ...prev, ...updated, id: taskId } : updated));
          }
        });

        unlistenMedia = await listen<any>("ytdlp-progress", (event) => {
          const updated = event.payload;
          const id = updated?.id || updated?.taskId || updated?.task_id;
          if (id === taskId) {
            setTask((prev) => (prev ? { ...prev, ...updated, id: taskId } : updated));
          }
        });
      } catch (e) {
        console.warn("Event listen error in popup window:", e);
      }
    };

    setupListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenHttp) unlistenHttp();
      if (unlistenMedia) unlistenMedia();
    };
  }, [taskId]);

  // Update native window title
  useEffect(() => {
    if (!task) return;
    const win = getCurrentWindow();
    const clampedPercent = Math.min(100, Math.max(0, task.percent || 0));
    if (task.status === "completed") {
      win.setTitle(`Download Complete - ${task.filename}`).catch(() => {});
    } else if (task.status === "error") {
      win.setTitle(`Download Error - ${task.filename}`).catch(() => {});
    } else {
      win.setTitle(`${clampedPercent.toFixed(1)}% of ${task.filename}`).catch(() => {});
    }
  }, [task?.percent, task?.status, task?.filename]);

  if (!task) {
    return (
      <div className="w-screen h-screen flex flex-col items-center justify-center p-6 bg-zr-surface text-zr-primary select-none">
        <div className="w-8 h-8 rounded-full border-2 border-emerald-500 border-t-transparent animate-spin mb-3" />
        <span className="text-sm font-medium text-zr-secondary">Connecting to download engine...</span>
      </div>
    );
  }

  const downloadedBytes = getDownloadedBytes(task);
  const totalBytes = getTotalBytes(task);
  const chunkCount = getChunkCount(task);
  const isResumable = getIsResumable(task);

  const clampedPercent = Math.min(100, Math.max(0, task.percent || 0));
  const isCompleted = task.status === "completed" || clampedPercent >= 100;
  const isPaused = task.status === "paused";
  const isError = task.status === "error";
  const isAssembling = task.status === "assembling";

  const handlePause = async () => {
    try {
      if (task.category === "torrent") {
        await invoke("pause_torrent", { id: task.id });
      } else {
        await invoke("pause_http_download", { id: task.id });
      }
      setTask((prev) => prev ? { ...prev, status: "paused" } : null);
    } catch (e) {
      console.error(e);
    }
  };

  const handleResume = async () => {
    try {
      if (task.category === "torrent") {
        await invoke("resume_torrent", { id: task.id });
      } else {
        await invoke("resume_http_download", { id: task.id });
      }
      setTask((prev) => prev ? { ...prev, status: "downloading" } : null);
    } catch (e) {
      console.error(e);
    }
  };

  const handleCancel = async () => {
    try {
      if (task.category === "torrent") {
        await invoke("remove_torrent", { id: task.id, deleteFiles: false });
      } else {
        try {
          await invoke("cancel_http_download", { id: task.id });
        } catch {
          await invoke("cancel_download", { taskId: task.id });
        }
      }
    } catch (e) {}
    // Close window after cancel
    try {
      await getCurrentWindow().close();
    } catch {}
  };

  const handleOpenFile = async () => {
    try {
      const fullPath = `${task.save_path}\\${task.filename}`;
      await invoke("open_file", { path: fullPath });
    } catch (e) {
      console.error(e);
    }
  };

  const handleOpenFolder = async () => {
    try {
      await invoke("open_folder", { path: task.save_path });
    } catch (e) {
      console.error(e);
    }
  };

  const handleHideToBackground = async () => {
    try {
      await getCurrentWindow().close();
    } catch {}
  };

  return (
    <div className="w-screen h-screen flex flex-col justify-between p-4 bg-zr-surface text-zr-primary select-none overflow-y-auto">
      {/* Top Header Card */}
      <div className="space-y-3">
        {/* Info Grid */}
        <div className="p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2">
          <div className="flex items-baseline justify-between gap-3">
            <span className="text-[12px] font-medium text-zr-secondary shrink-0">File Name:</span>
            <span className="text-[13px] font-semibold text-zr-primary truncate text-right flex-1" title={task.filename}>
              {task.filename}
            </span>
          </div>

          <div className="flex items-baseline justify-between gap-3">
            <span className="text-[12px] font-medium text-zr-secondary shrink-0">URL:</span>
            <span className="text-[11px] text-blue-600 dark:text-blue-400 font-mono truncate text-right flex-1" title={task.url}>
              {task.url}
            </span>
          </div>

          <div className="grid grid-cols-2 gap-3 pt-1 border-t border-zr/50 text-[12px]">
            <div className="flex justify-between items-center pr-2">
              <span className="text-zr-secondary">Status:</span>
              <span className={`px-2 py-0.5 rounded text-[11px] font-bold uppercase tracking-wider ${
                isCompleted
                  ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                  : isError
                  ? "bg-red-500/10 text-red-600 dark:text-red-400"
                  : isPaused
                  ? "bg-amber-500/10 text-amber-600 dark:text-amber-400"
                  : "bg-blue-500/10 text-blue-600 dark:text-blue-400"
              }`}>
                {task.status}
              </span>
            </div>
            <div className="flex justify-between items-center pl-2 border-l border-zr/50">
              <span className="text-zr-secondary">Resume capability:</span>
              <span className={`font-semibold ${isResumable ? "text-emerald-600 dark:text-emerald-400" : "text-amber-600 dark:text-amber-400"}`}>
                {isResumable ? "Yes" : "No"}
              </span>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3 text-[12px]">
            <div className="flex justify-between items-center pr-2">
              <span className="text-zr-secondary">File size:</span>
              <span className="font-semibold text-zr-primary">
                {totalBytes > 0 ? formatBytes(totalBytes) : "Unknown"}
              </span>
            </div>
            <div className="flex justify-between items-center pl-2 border-l border-zr/50">
              <span className="text-zr-secondary">Downloaded:</span>
              <span className="font-semibold text-zr-primary">
                {formatBytes(downloadedBytes)}
              </span>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3 text-[12px]">
            <div className="flex justify-between items-center pr-2">
              <span className="text-zr-secondary">Transfer rate:</span>
              <span className="font-semibold text-zr-primary flex items-center gap-1">
                {isPaused ? (
                  <span className="text-amber-500">Paused</span>
                ) : isCompleted ? (
                  <span className="text-emerald-500 font-bold">100%</span>
                ) : (
                  <>
                    <Zap className="w-3.5 h-3.5 text-amber-500 fill-amber-500 animate-pulse" />
                    <span>{task.speed_formatted || "Connecting..."}</span>
                  </>
                )}
              </span>
            </div>
            <div className="flex justify-between items-center pl-2 border-l border-zr/50">
              <span className="text-zr-secondary">Time left:</span>
              <span className="font-semibold text-zr-primary">
                {isCompleted ? "00:00:00" : task.eta_formatted || "--"}
              </span>
            </div>
          </div>
        </div>

        {/* Error Message */}
        {isError && task.errorMessage && (
          <div className="p-2.5 rounded-lg border border-red-500/20 bg-red-500/10 text-red-600 dark:text-red-400 text-[12px] flex items-start gap-2">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            <span className="break-all">{task.errorMessage}</span>
          </div>
        )}

        {/* Progress Bar */}
        <div className="space-y-1">
          <div className="flex justify-between text-[12px] font-semibold">
            <span className="text-zr-primary">{clampedPercent.toFixed(1)}%</span>
            <span className="text-zr-secondary">
              {formatBytes(downloadedBytes)} of {totalBytes > 0 ? formatBytes(totalBytes) : "Unknown"}
            </span>
          </div>
          <div className="h-3.5 w-full bg-slate-200 dark:bg-slate-800 rounded-full overflow-hidden border border-zr/60 p-0.5">
            <div
              className={`h-full rounded-full transition-all duration-200 ${
                isCompleted
                  ? "bg-emerald-600 dark:bg-emerald-500"
                  : isError
                  ? "bg-red-600 dark:bg-red-500"
                  : isPaused
                  ? "bg-amber-600 dark:bg-amber-500"
                  : "bg-blue-600 dark:bg-blue-400"
              }`}
              style={{ width: `${clampedPercent}%` }}
            />
          </div>
        </div>

        {/* Chunk Connection Streams */}
        {!isCompleted && !isError && (
          <div className="space-y-1">
            <div className="flex justify-between text-[11px] text-zr-secondary font-medium">
              <span>Segmented Connection:</span>
              <span className="text-blue-600 dark:text-blue-400 font-semibold">
                {chunkCount} Parallel Streams
              </span>
            </div>
            <div className="grid grid-cols-8 gap-1 h-3 p-1 rounded bg-slate-100 dark:bg-slate-900 border border-zr/40">
              {Array.from({ length: Math.min(16, Math.max(1, chunkCount)) }).map((_, i) => (
                <div key={i} className="h-full bg-slate-200 dark:bg-slate-800 rounded-sm overflow-hidden">
                  <div
                    className={`h-full ${
                      isPaused ? "bg-amber-500" : "bg-blue-500 animate-pulse"
                    }`}
                    style={{ width: `${clampedPercent}%` }}
                  />
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Bottom Controls */}
      <div className="pt-3 border-t border-zr flex items-center justify-between gap-2 mt-2">
        <div className="flex items-center gap-2">
          {!isCompleted && !isError && (
            isPaused ? (
              <button
                onClick={handleResume}
                className="px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white font-medium text-[12px] flex items-center gap-1.5 transition-colors shadow-sm"
              >
                <Play className="w-3.5 h-3.5 fill-current" />
                Resume
              </button>
            ) : (
              <button
                onClick={handlePause}
                className="px-3 py-1.5 rounded-lg bg-amber-600 hover:bg-amber-700 text-white font-medium text-[12px] flex items-center gap-1.5 transition-colors shadow-sm"
              >
                <Pause className="w-3.5 h-3.5 fill-current" />
                Pause
              </button>
            )
          )}

          {!isCompleted && (
            <button
              onClick={handleCancel}
              className="px-3 py-1.5 rounded-lg bg-slate-200 dark:bg-slate-800 hover:bg-red-600 hover:text-white text-zr-secondary font-medium text-[12px] transition-colors"
            >
              Cancel
            </button>
          )}

          {isCompleted && (
            <>
              <button
                onClick={handleOpenFile}
                className="px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white font-medium text-[12px] flex items-center gap-1.5 transition-colors shadow-sm"
              >
                <FileCheck className="w-3.5 h-3.5" />
                Open File
              </button>
              <button
                onClick={handleOpenFolder}
                className="px-3 py-1.5 rounded-lg bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 text-zr-primary font-medium text-[12px] flex items-center gap-1.5 transition-colors"
              >
                <FolderOpen className="w-3.5 h-3.5" />
                Open Folder
              </button>
            </>
          )}
        </div>

        <button
          onClick={handleHideToBackground}
          className="px-3 py-1.5 rounded-lg border border-zr text-zr-secondary hover:text-zr-primary hover:bg-slate-100 dark:hover:bg-slate-800/80 font-medium text-[12px] flex items-center gap-1.5 transition-colors"
          title="Minimize dialog and keep download running in main window"
        >
          <EyeOff className="w-3.5 h-3.5" />
          Hide / Background
        </button>
      </div>
    </div>
  );
};
