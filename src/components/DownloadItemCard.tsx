import React, { useState } from "react";
import {
  Pause,
  Play,
  FolderOpen,
  FileCheck,
  ChevronDown,
  ChevronUp,
  Trash2,
  FileText,
  Video,
  Music,
  Archive,
  Binary,
  Layers,
  Zap,
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
import { SpeedChart } from "./SpeedChart";

interface DownloadItemCardProps {
  task: DownloadTask;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onCancel: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenFile: (path: string) => void;
  onOpenFolder: (path: string) => void;
  theme?: "dark" | "light";
}

export const DownloadItemCard: React.FC<DownloadItemCardProps> = ({
  task,
  onPause,
  onResume,
  onCancel,
  onDelete,
  onOpenFile,
  onOpenFolder,
}) => {
  const [showChunks, setShowChunks] = useState(false);
  const [showSpeedChart, setShowSpeedChart] = useState(false);

  const getCategoryIcon = () => {
    switch (task.category) {
      case "video":
        return <Video className="w-5 h-5 text-purple-600 dark:text-purple-400" />;
      case "audio":
        return <Music className="w-5 h-5 text-pink-600 dark:text-pink-400" />;
      case "docs":
        return <FileText className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />;
      case "compressed":
        return <Archive className="w-5 h-5 text-amber-600 dark:text-amber-400" />;
      case "programs":
        return <Binary className="w-5 h-5 text-teal-600 dark:text-teal-400" />;
      default:
        return <Layers className="w-5 h-5 text-blue-600 dark:text-blue-400" />;
    }
  };

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

  const isDownloading = task.status === "downloading";
  const isPaused = task.status === "paused";
  const isCompleted = task.status === "completed";
  const isError = task.status === "error";

  const downloaded = getDownloadedBytes(task);
  const total = getTotalBytes(task);
  const isResumable = getIsResumable(task);
  const chunkCount = getChunkCount(task);

  return (
    <div className="bg-zr-surface rounded-xl p-4 border border-zr shadow-xs transition-all relative overflow-hidden">
      {/* Main Row */}
      <div className="flex items-start justify-between gap-4">
        {/* Left: Icon & Title */}
        <div className="flex items-start space-x-3.5 flex-1 min-w-0">
          <div className="p-2.5 rounded-lg bg-slate-100 dark:bg-slate-800 border border-zr shrink-0">
            {getCategoryIcon()}
          </div>

          <div className="min-w-0 flex-1 space-y-1">
            <div className="flex items-center space-x-2">
              <h3
                className="font-semibold text-[14px] text-zr-primary truncate cursor-pointer hover:underline"
                title={task.filename}
                onClick={() => {
                  if (!isCompleted) return;
                  const folder = task.save_path || task.savePath || "";
                  const fn = task.filename || "";
                  const full = folder ? `${folder.replace(/[/\\]+$/, "")}\\${fn}` : fn;
                  onOpenFile(full);
                }}
              >
                {task.filename}
              </h3>
              <span
                className={`text-[12px] font-semibold uppercase tracking-wide px-2 py-0.5 rounded-md ${getStatusBadgeClass(
                  task.status
                )}`}
              >
                {task.status}
              </span>
              {chunkCount > 1 && (
                <span className="text-[12px] font-semibold text-zr-accent bg-blue-50 dark:bg-blue-950/40 px-2 py-0.5 rounded-md border border-blue-200 dark:border-blue-900/60">
                  {chunkCount} Segments
                </span>
              )}
              <span className="text-[12px] text-zr-secondary">
                Resume:{" "}
                <strong className={isResumable ? "text-emerald-700 dark:text-emerald-400" : "text-red-700 dark:text-red-400"}>
                  {isResumable ? "Yes" : "No"}
                </strong>
              </span>
            </div>

            {/* Truncated Monospace URL */}
            <p className="text-[13px] font-mono text-zr-accent truncate max-w-xl">
              {task.url}
            </p>
          </div>
        </div>

        {/* Right: Action Buttons (32px hit area, 18px icons, text-secondary) */}
        <div className="flex items-center space-x-1 shrink-0">
          {isDownloading && (
            <button
              onClick={() => onPause(task.id)}
              className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-amber-800 dark:hover:text-amber-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
              title="Pause"
              aria-label="Pause"
            >
              <Pause className="w-[18px] h-[18px]" />
            </button>
          )}

          {(isPaused || isError) && (
            <button
              onClick={() => onResume(task.id)}
              className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-blue-600 dark:hover:text-blue-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
              title={isError ? "Retry" : "Resume"}
              aria-label={isError ? "Retry" : "Resume"}
            >
              <Play className="w-[18px] h-[18px] fill-current" />
            </button>
          )}

          {isCompleted && (
            <button
              onClick={() => {
                const folder = task.save_path || task.savePath || "";
                const fn = task.filename || "";
                const full = folder ? `${folder.replace(/[/\\]+$/, "")}\\${fn}` : fn;
                onOpenFile(full);
              }}
              className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-emerald-700 dark:hover:text-emerald-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
              title="Open File"
              aria-label="Open File"
            >
              <FileCheck className="w-[18px] h-[18px]" />
            </button>
          )}

          <button
            onClick={() => onOpenFolder(task.save_path || task.savePath || "")}
            className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-zr-primary hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
            title="Open Folder"
            aria-label="Open Folder"
          >
            <FolderOpen className="w-[18px] h-[18px]" />
          </button>

          {!isCompleted && (
            <button
              onClick={() => onCancel(task.id)}
              className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-red-700 dark:hover:text-red-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
              title="Cancel"
              aria-label="Cancel"
            >
              <Trash2 className="w-[18px] h-[18px]" />
            </button>
          )}

          {isCompleted && (
            <button
              onClick={() => onDelete(task.id)}
              className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-red-700 dark:hover:text-red-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
              title="Delete from list"
              aria-label="Delete"
            >
              <Trash2 className="w-[18px] h-[18px]" />
            </button>
          )}
        </div>
      </div>

      {/* Progress Bar (8px high, visible track, solid accent fill) */}
      <div className="mt-3.5 space-y-1.5">
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
            style={{ width: `${Math.min(100, Math.max(0, task.percent))}%` }}
          />
        </div>

        {/* Progress Metrics Row: sans + tabular-nums 13px */}
        <div className="flex items-center justify-between text-[13px] tabular-nums font-sans">
          <div className="flex items-center space-x-2 text-zr-secondary">
            <span className="font-semibold text-zr-primary">{task.percent.toFixed(1)}%</span>
            <span>&bull;</span>
            <span>
              {formatBytes(downloaded)} / {total > 0 ? formatBytes(total) : "Unknown"}
            </span>
          </div>

          <div className="flex items-center space-x-4">
            {isDownloading && (
              <div className="flex items-center space-x-1 font-semibold text-amber-800 dark:text-amber-400">
                <Zap className="w-3.5 h-3.5 fill-current" />
                <span>{getSpeedFormatted(task)}</span>
              </div>
            )}
            <div className="text-zr-secondary">
              {isDownloading ? getEtaFormatted(task) : isCompleted ? "Complete" : "--"}
            </div>
            {task.speedHistory && task.speedHistory.length > 0 && (
              <button
                onClick={() => setShowSpeedChart(!showSpeedChart)}
                className="text-[12px] font-semibold text-zr-accent hover:underline cursor-pointer"
              >
                {showSpeedChart ? "Hide Graph" : "Speed Graph"}
              </button>
            )}
            {task.chunks && task.chunks.length > 0 && (
              <button
                onClick={() => setShowChunks(!showChunks)}
                className="text-[12px] font-semibold text-zr-accent hover:underline flex items-center space-x-0.5 cursor-pointer"
              >
                <span>{showChunks ? "Hide Chunks" : "Chunks"}</span>
                {showChunks ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Speed Chart */}
      {showSpeedChart && task.speedHistory && task.speedHistory.length > 0 && (
        <div className="mt-3 pt-3 border-t border-zr">
          <SpeedChart data={task.speedHistory} height={60} />
        </div>
      )}

      {/* Chunks Progress Drawer */}
      {showChunks && task.chunks && task.chunks.length > 0 && (
        <div className="mt-3 pt-3 border-t border-zr space-y-2">
          <div className="text-[12px] font-semibold text-zr-secondary">
            Segmented HTTP Range Connections ({task.chunks.length} Threads)
          </div>
          <div className="grid grid-cols-2 sm:grid-cols-4 md:grid-cols-8 gap-2">
            {task.chunks.map((chk, i) => (
              <div key={i} className="bg-slate-50 dark:bg-slate-900/60 p-2 rounded-lg border border-zr space-y-1">
                <div className="flex justify-between text-[12px] tabular-nums text-zr-secondary">
                  <span className="font-semibold text-zr-primary">#{i + 1}</span>
                  <span>{chk.percent.toFixed(0)}%</span>
                </div>
                <div className="h-2 w-full bg-slate-200 dark:bg-slate-700 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-blue-600 dark:bg-blue-400 rounded-full transition-all duration-150"
                    style={{ width: `${chk.percent}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

