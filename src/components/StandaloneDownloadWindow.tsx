import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Play,
  Pause,
  FolderOpen,
  FileCheck,
  Zap,
  AlertCircle,
  EyeOff,
  Clock,
  HardDrive,
  CheckCircle2,
  X,
} from "lucide-react";
import {
  DownloadTask,
  formatBytes,
  getDownloadedBytes,
  getTotalBytes,
  getChunkCount,
  getIsResumable,
  getSpeedFormatted,
  getEtaFormatted,
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

  // Fetch initial task state from Rust Engine / Media State
  useEffect(() => {
    let isMounted = true;
    const fetchTask = async () => {
      try {
        const directMatch = await invoke<DownloadTask | null>("get_download_task", { taskId });
        if (directMatch && isMounted) {
          setTask(directMatch);
          return;
        }

        const all = await invoke<DownloadTask[]>("get_all_downloads");
        if (Array.isArray(all) && isMounted) {
          const matched = all.find((t) => t.id === taskId);
          if (matched) {
            setTask(matched);
          }
        }
      } catch (err) {
        console.error("Failed to fetch download task:", err);
      }
    };

    fetchTask();
    const interval = setInterval(fetchTask, 1200);
    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, [taskId]);

  // Listen to Tauri progress events
  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenHttp: (() => void) | undefined;
    let unlistenMedia: (() => void) | undefined;
    let unlistenZr: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        const handleUpdate = (updated: any) => {
          const id = updated?.id || updated?.taskId || updated?.task_id;
          if (id === taskId) {
            setTask((prev) => {
              if (!prev) {
                return {
                  id: taskId,
                  url: updated.url || "",
                  filename: updated.filename || "Downloading...",
                  save_path: updated.save_path || updated.savePath || "",
                  savePath: updated.savePath || updated.save_path || "",
                  total_bytes: updated.total_bytes ?? updated.totalBytes ?? 0,
                  downloaded_bytes: updated.downloaded_bytes ?? updated.downloadedBytes ?? 0,
                  percent: updated.percent ?? 0,
                  speed_bps: updated.speed_bps ?? updated.speedBps ?? 0,
                  speed_formatted: updated.speed_formatted ?? updated.speedFormatted ?? updated.speed ?? "0 B/s",
                  eta_formatted: updated.eta_formatted ?? updated.etaFormatted ?? updated.eta ?? "--",
                  status: updated.status || "downloading",
                  category: updated.category || "video",
                  chunk_count: updated.chunk_count ?? updated.chunkCount ?? 1,
                  is_resumable: updated.is_resumable ?? updated.isResumable ?? true,
                  chunks: updated.chunks || [],
                  speed_history: updated.speed_history || updated.speedHistory || [],
                  error_message: updated.error_message || updated.errorMessage,
                  created_at: updated.created_at || new Date().toLocaleTimeString(),
                };
              }
              return {
                ...prev,
                ...updated,
                id: taskId,
                filename: updated.filename || prev.filename,
                url: updated.url || prev.url,
                save_path: updated.save_path || updated.savePath || prev.save_path || prev.savePath,
                savePath: updated.savePath || updated.save_path || prev.savePath || prev.save_path,
                category: updated.category || prev.category,
              };
            });
          }
        };

        unlistenProgress = await listen<any>("progress", (e) => handleUpdate(e.payload));
        unlistenHttp = await listen<any>("download-progress", (e) => handleUpdate(e.payload));
        unlistenMedia = await listen<any>("ytdlp-progress", (e) => handleUpdate(e.payload));
        unlistenZr = await listen<any>("zr://progress", (e) => handleUpdate(e.payload));
      } catch (e) {
        console.warn("Event listen error in popup window:", e);
      }
    };

    setupListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenHttp) unlistenHttp();
      if (unlistenMedia) unlistenMedia();
      if (unlistenZr) unlistenZr();
    };
  }, [taskId]);

  // Update native OS window title dynamically
  useEffect(() => {
    if (!task) return;
    const win = getCurrentWindow();
    const clampedPercent = Math.min(100, Math.max(0, task.percent || 0));
    const fn = task.filename || "Downloading...";
    if (task.status === "completed" || clampedPercent >= 100) {
      win.setTitle(`100% Complete - ${fn}`).catch(() => {});
    } else if (task.status === "error") {
      win.setTitle(`Download Error - ${fn}`).catch(() => {});
    } else if (task.status === "paused") {
      win.setTitle(`Paused (${clampedPercent.toFixed(1)}%) - ${fn}`).catch(() => {});
    } else {
      win.setTitle(`${clampedPercent.toFixed(1)}% of ${fn}`).catch(() => {});
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
  const speedDisplay = getSpeedFormatted(task);
  const etaDisplay = getEtaFormatted(task);

  const clampedPercent = Math.min(100, Math.max(0, task.percent || 0));
  const isCompleted = task.status === "completed" || clampedPercent >= 100;
  const isPaused = task.status === "paused";
  const isError = task.status === "error";
  const isAssembling = task.status === "assembling";

  const handlePause = async () => {
    try {
      if (task.category === "torrents" || task.category === "torrent") {
        await invoke("pause_torrent", { idOrHash: task.id });
      } else {
        await invoke("pause_http_download", { taskId: task.id });
      }
      setTask((prev) => (prev ? { ...prev, status: "paused" } : null));
    } catch (e) {
      console.error("Pause error:", e);
    }
  };

  const handleResume = async () => {
    try {
      if (task.category === "torrents" || task.category === "torrent") {
        await invoke("resume_torrent", { idOrHash: task.id });
      } else {
        await invoke("resume_http_download", { taskId: task.id });
      }
      setTask((prev) => (prev ? { ...prev, status: "downloading" } : null));
    } catch (e) {
      console.error("Resume error:", e);
    }
  };

  const handleCancel = async () => {
    try {
      if (task.category === "torrents" || task.category === "torrent") {
        await invoke("pause_torrent", { idOrHash: task.id });
      } else {
        try {
          await invoke("cancel_download", { taskId: task.id });
        } catch {}
        try {
          await invoke("cancel_http_download", { taskId: task.id, deleteFile: false });
        } catch {}
      }
    } catch (e) {
      console.error("Cancel error:", e);
    }
    try {
      await getCurrentWindow().close();
    } catch {}
  };

  const handleOpenFile = async () => {
    try {
      const folder = task.save_path || task.savePath || "";
      const filename = task.filename || "";
      const fullPath = folder ? `${folder.replace(/[/\\]+$/, "")}\\${filename}` : filename;
      await invoke("open_file", { path: fullPath });
    } catch (e) {
      console.error("Open file error:", e);
      try {
        const folder = task.save_path || task.savePath || "";
        await invoke("open_folder", { path: folder });
      } catch {}
    }
  };

  const handleOpenFolder = async () => {
    try {
      const folder = task.save_path || task.savePath || "";
      await invoke("open_folder", { path: folder });
    } catch (e) {
      console.error("Open folder error:", e);
    }
  };

  const handleHideToBackground = async () => {
    try {
      await getCurrentWindow().close();
    } catch (err) {
      console.error("Close window error:", err);
    }
  };

  return (
    <div className="w-screen h-screen p-4 flex flex-col justify-between bg-zr-surface text-zr-primary select-none overflow-hidden font-sans">
      {/* Top Details Card */}
      <div className="space-y-3 flex-1 overflow-y-auto pr-0.5">
        <div className="p-3.5 rounded-xl bg-slate-100/80 dark:bg-slate-900/80 border border-zr space-y-2 text-[13px]">
          <div className="flex justify-between items-start gap-2">
            <span className="text-zr-secondary font-medium w-24 shrink-0">File Name:</span>
            <span
              className="font-semibold text-zr-primary text-right break-all line-clamp-2 select-text"
              title={task.filename}
            >
              {task.filename}
            </span>
          </div>

          <div className="flex justify-between items-start gap-2">
            <span className="text-zr-secondary font-medium w-24 shrink-0">URL:</span>
            <span
              className="text-blue-600 dark:text-blue-400 text-right truncate flex-1 font-mono text-[11px] select-text"
              title={task.url}
            >
              {task.url}
            </span>
          </div>

          <div className="grid grid-cols-2 gap-3 pt-1 border-t border-zr/40 text-[12px]">
            <div className="flex justify-between items-center pr-2">
              <span className="text-zr-secondary">Status:</span>
              <span
                className={`font-semibold uppercase tracking-wider text-[11px] px-2 py-0.5 rounded ${
                  isCompleted
                    ? "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border border-emerald-500/30"
                    : isError
                    ? "bg-red-500/15 text-red-600 dark:text-red-400 border border-red-500/30"
                    : isPaused
                    ? "bg-amber-500/15 text-amber-600 dark:text-amber-400 border border-amber-500/30"
                    : isAssembling
                    ? "bg-purple-500/15 text-purple-600 dark:text-purple-400 border border-purple-500/30"
                    : "bg-blue-500/15 text-blue-600 dark:text-blue-400 border border-blue-500/30"
                }`}
              >
                {task.status}
              </span>
            </div>

            <div className="flex justify-between items-center pl-2 border-l border-zr/50">
              <span className="text-zr-secondary">Resume capability:</span>
              <span className={`font-semibold ${isResumable ? "text-emerald-600 dark:text-emerald-400" : "text-amber-500"}`}>
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
              <span className="font-semibold text-zr-primary">{formatBytes(downloadedBytes)}</span>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3 text-[12px]">
            <div className="flex justify-between items-center pr-2">
              <span className="text-zr-secondary">Transfer rate:</span>
              <span className="font-semibold text-zr-primary flex items-center gap-1">
                {isPaused ? (
                  <span className="text-amber-500">Paused</span>
                ) : isCompleted ? (
                  <span className="text-emerald-500 font-bold flex items-center gap-1">
                    <CheckCircle2 className="w-3.5 h-3.5" /> Finished
                  </span>
                ) : isError ? (
                  <span className="text-red-500">Error</span>
                ) : (
                  <>
                    <Zap className="w-3.5 h-3.5 text-amber-500 fill-amber-500 animate-pulse" />
                    <span>{speedDisplay}</span>
                  </>
                )}
              </span>
            </div>
            <div className="flex justify-between items-center pl-2 border-l border-zr/50">
              <span className="text-zr-secondary">Time left:</span>
              <span className="font-semibold text-zr-primary flex items-center gap-1">
                <Clock className="w-3.5 h-3.5 text-zr-secondary" />
                <span>{isCompleted ? "00:00:00" : etaDisplay}</span>
              </span>
            </div>
          </div>
        </div>

        {/* Error Message */}
        {isError && (task.errorMessage || (task as any).error_message) && (
          <div className="p-2.5 rounded-lg border border-red-500/20 bg-red-500/10 text-red-600 dark:text-red-400 text-[12px] flex items-start gap-2">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            <span className="break-all">{task.errorMessage || (task as any).error_message}</span>
          </div>
        )}

        {/* Progress Bar */}
        <div className="space-y-1 pt-1">
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
          <div className="space-y-1 pt-1">
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
