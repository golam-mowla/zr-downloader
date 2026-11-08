import React from "react";
import {
  X,
  Minus,
  Play,
  Pause,
  FolderOpen,
  FileCheck,
  Zap,
} from "lucide-react";
import {
  DownloadTask,
  formatBytes,
  getDownloadedBytes,
  getTotalBytes,
  getChunkCount,
  getIsResumable,
} from "../types";

export interface DownloadStatusDialogProps {
  task: DownloadTask;
  isOpen: boolean;
  onClose: () => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onCancel: (id: string) => void;
  onOpenFile: (path: string) => void;
  onOpenFolder: (path: string) => void;
  dialogIndex?: number;
  totalDialogs?: number;
  onBringToFront?: () => void;
  theme?: "dark" | "light";
}

export const DownloadStatusDialog: React.FC<DownloadStatusDialogProps> = ({
  task,
  isOpen,
  onClose,
  onPause,
  onResume,
  onCancel,
  onOpenFile,
  onOpenFolder,
  dialogIndex = 0,
  totalDialogs = 1,
  onBringToFront,
  theme = "dark",
}) => {
  if (!isOpen) return null;

  const downloaded = getDownloadedBytes(task);
  const total = getTotalBytes(task);
  const isResumable = getIsResumable(task);
  const chunkCount = getChunkCount(task);
  const isDownloading = task.status === "downloading";
  const isPaused = task.status === "paused";
  const isCompleted = task.status === "completed";
  const isError = task.status === "error";

  const percent = task.percent > 0 ? task.percent : total > 0 ? (downloaded / total) * 100 : 0;
  const clampedPercent = Math.min(100, Math.max(0, percent));

  const getStatusBadgeClass = (status: string) => {
    switch (status) {
      case "downloading":
        return "badge-status-downloading";
      case "completed":
        return "badge-status-completed";
      case "paused":
        return "badge-status-paused";
      case "error":
        return "badge-status-error";
      case "assembling":
        return "badge-status-assembling";
      default:
        return "badge-status-queued";
    }
  };

  const cascadeOffset = (dialogIndex % 8) * 26;

  return (
    <div
      className="fixed inset-0 pointer-events-none flex items-center justify-center p-4 select-none"
      style={{ zIndex: 60 + dialogIndex }}
    >
      <div
        onClick={onBringToFront}
        style={{
          transform: `translate(${cascadeOffset}px, ${cascadeOffset}px)`,
        }}
        className="pointer-events-auto w-full max-w-lg rounded-xl shadow-2xl border border-zr bg-zr-surface overflow-hidden transition-colors">
        {/* Titlebar */}
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-zr bg-zr-tableHead select-none">
          <div className="flex items-center space-x-2 min-w-0">
            <div className="w-5 h-5 rounded-md bg-emerald-500/10 flex items-center justify-center shrink-0 border border-emerald-500/20">
              <Zap className="w-3.5 h-3.5 text-emerald-500 fill-emerald-500/20" />
            </div>
            <span className="font-semibold text-[13px] text-zr-primary truncate">
              {isCompleted
                ? `Download Complete - ${task.filename}`
                : isError
                ? `Download Error - ${task.filename}`
                : `${clampedPercent.toFixed(1)}% of ${task.filename}`}
            </span>
          </div>

          <div className="flex items-center space-x-1">
            <button
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
              title="Minimize to main window"
              aria-label="Minimize"
            >
              <Minus className="w-4 h-4" />
            </button>
            <button
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-zr-secondary hover:text-white hover:bg-red-600 transition-colors"
              title="Close dialog"
              aria-label="Close dialog"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Content Body */}
        <div className="p-4 space-y-4">
          {/* File Name & URL */}
          <div className="p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2">
            <div className="flex items-baseline justify-between gap-3">
              <span className="text-[13px] font-medium text-zr-secondary shrink-0">
                File Name:
              </span>
              <span className="text-[14px] font-semibold text-zr-primary truncate text-right flex-1" title={task.filename}>
                {task.filename}
              </span>
            </div>
            <div className="flex items-baseline justify-between gap-3">
              <span className="text-[13px] font-medium text-zr-secondary shrink-0">
                URL:
              </span>
              <span
                className="truncate font-mono text-[13px] text-zr-accent text-right flex-1"
                title={task.url}
              >
                {task.url}
              </span>
            </div>
          </div>

          {/* Metrics Grid */}
          <div className="grid grid-cols-2 gap-2.5 p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40">
            {/* Error Message Banner */}
            {isError && (task.errorMessage || (task as any).error) && (
              <div className="col-span-2 p-2.5 rounded-md bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900/50">
                <p className="text-[12px] font-medium text-red-700 dark:text-red-400 break-words leading-relaxed">
                  <span className="font-bold">Error: </span>
                  {task.errorMessage || (task as any).error}
                </p>
              </div>
            )}

            {/* Status */}
            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium text-zr-secondary">
                Status:
              </span>
              <span
                className={`text-[11px] font-semibold uppercase tracking-wider px-2 py-0.5 rounded ${getStatusBadgeClass(
                  task.status
                )}`}
              >
                {task.status}
              </span>
            </div>

            {/* Resume Capability */}
            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium text-zr-secondary">
                Resume capability:
              </span>
              <span
                className={`text-[12px] font-semibold ${
                  isResumable
                    ? "text-emerald-700 dark:text-emerald-400"
                    : "text-red-700 dark:text-red-400"
                }`}
              >
                {isResumable ? "Yes" : "No"}
              </span>
            </div>

            {/* File Size */}
            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium text-zr-secondary">
                File size:
              </span>
              <span className="font-semibold tabular-nums text-zr-primary text-[13px]">
                {total > 0 ? formatBytes(total) : "Unknown"}
              </span>
            </div>

            {/* Downloaded */}
            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium text-zr-secondary">
                Downloaded:
              </span>
              <span className="font-semibold tabular-nums text-emerald-700 dark:text-emerald-400 text-[13px]">
                {formatBytes(downloaded)}
              </span>
            </div>

            {/* Transfer Rate */}
            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium text-zr-secondary">
                Transfer rate:
              </span>
              <span className="font-semibold tabular-nums text-amber-800 dark:text-amber-400 text-[13px] flex items-center space-x-1">
                <Zap className="w-3.5 h-3.5 text-amber-500 fill-amber-500" />
                <span>
                  {isDownloading || (task.status as string) === "assembling"
                    ? task.speed_formatted || "Calculating..."
                    : "--"}
                </span>
              </span>
            </div>

            {/* Time Left */}
            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium text-zr-secondary">
                Time left:
              </span>
              <span className="font-semibold tabular-nums text-zr-secondary text-[13px]">
                {isDownloading
                  ? task.eta_formatted || "Calculating..."
                  : isCompleted
                  ? "Finished"
                  : "--"}
              </span>
            </div>
          </div>

          {/* Main Progress Bar: 8px high, visible track, solid accent fill */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-[13px] font-semibold tabular-nums">
              <span className="text-zr-primary">{clampedPercent.toFixed(1)}%</span>
              <span className="text-zr-secondary">
                {formatBytes(downloaded)} of {total > 0 ? formatBytes(total) : "Unknown"}
              </span>
            </div>
            <div className="h-2 w-full bg-slate-200 dark:bg-slate-700 rounded-full overflow-hidden">
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

          {/* Segmented Thread Visualizer */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-[12px] font-semibold text-zr-secondary">
              <span>Segmented Connection:</span>
              <span className="text-zr-accent">
                {chunkCount} Parallel Streams
              </span>
            </div>
            <div className="grid grid-cols-8 sm:grid-cols-16 gap-1">
              {Array.from({ length: Math.max(1, chunkCount) }).map((_, i) => {
                const chunkPercent = isCompleted ? 100 : Math.min(100, Math.max(0, clampedPercent + (i % 3) * 5));
                return (
                  <div
                    key={i}
                    className="h-2.5 rounded-xs bg-slate-200 dark:bg-slate-700 overflow-hidden"
                    title={`Segment #${i + 1}`}
                  >
                    <div
                      className={`h-full transition-all duration-200 ${
                        isCompleted
                          ? "bg-emerald-600 dark:bg-emerald-500"
                          : isError
                          ? "bg-red-600 dark:bg-red-500"
                          : isPaused
                          ? "bg-amber-600 dark:bg-amber-500"
                          : "bg-blue-600 dark:bg-blue-400"
                      }`}
                      style={{ width: `${chunkPercent}%` }}
                    />
                  </div>
                );
              })}
            </div>
          </div>

          {/* Action Buttons Row */}
          <div className="flex items-center justify-between pt-3 border-t border-zr">
            <div className="flex items-center space-x-2">
              {isDownloading && (
                <button
                  onClick={() => onPause(task.id)}
                  className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-lg bg-amber-600 hover:bg-amber-700 text-white font-semibold text-[13px] transition-colors shadow-xs"
                >
                  <Pause className="w-4 h-4" />
                  <span>Pause</span>
                </button>
              )}

              {(isPaused || isError) && (
                <button
                  onClick={() => onResume(task.id)}
                  className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white font-semibold text-[13px] transition-colors shadow-xs"
                >
                  <Play className="w-4 h-4 fill-current" />
                  <span>{isError ? "Retry" : "Resume"}</span>
                </button>
              )}

              {isCompleted ? (
                <>
                  <button
                    onClick={() => {
                      const folder = task.save_path || task.savePath || "";
                      const fn = task.filename || "";
                      const full = folder ? `${folder.replace(/[/\\]+$/, "")}\\${fn}` : fn;
                      onOpenFile(full);
                    }}
                    className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white font-semibold text-[13px] transition-colors shadow-xs"
                  >
                    <FileCheck className="w-4 h-4" />
                    <span>Open File</span>
                  </button>
                  <button
                    onClick={() => onOpenFolder(task.save_path || task.savePath || "")}
                    className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-zr-secondary hover:text-zr-primary font-semibold text-[13px] transition-colors shadow-xs"
                  >
                    <FolderOpen className="w-4 h-4" />
                    <span>Open Folder</span>
                  </button>
                </>
              ) : (
                <button
                  onClick={() => onCancel(task.id)}
                  className="px-3.5 py-1.5 rounded-lg text-red-700 bg-white border border-red-300 hover:bg-red-50 dark:bg-red-950/30 dark:border-red-900/60 dark:text-red-400 dark:hover:bg-red-950/50 font-semibold text-[13px] transition-colors"
                >
                  Cancel
                </button>
              )}
            </div>

            <button
              onClick={onClose}
              className="px-3.5 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-zr-secondary hover:text-zr-primary font-semibold text-[13px] transition-colors"
            >
              Hide / Background
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

// Backwards compatibility alias
export const IdmDownloadDialog = DownloadStatusDialog;
export type IdmDownloadDialogProps = DownloadStatusDialogProps;

