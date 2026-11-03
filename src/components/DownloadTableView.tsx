import React, { useState } from "react";
import {
  Play,
  Pause,
  FolderOpen,
  FileCheck,
  Trash2,
  ChevronDown,
  ChevronUp,
  FileText,
  Video,
  Music,
  Archive,
  Binary,
  Layers,
  Zap,
  Radio,
  Info,
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

interface DownloadTableViewProps {
  tasks: DownloadTask[];
  selectedTaskId: string | null;
  onSelectTask: (id: string) => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenFile: (path: string) => void;
  onOpenFolder: (path: string) => void;
  theme?: "dark" | "light";
}

export const DownloadTableView: React.FC<DownloadTableViewProps> = ({
  tasks,
  selectedTaskId,
  onSelectTask,
  onPause,
  onResume,
  onDelete,
  onOpenFile,
  onOpenFolder,
}) => {
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const getCategoryIcon = (category: string) => {
    switch (category) {
      case "video":
        return <Video className="w-[18px] h-[18px] text-purple-600 dark:text-purple-400 shrink-0" />;
      case "audio":
        return <Music className="w-[18px] h-[18px] text-pink-600 dark:text-pink-400 shrink-0" />;
      case "docs":
        return <FileText className="w-[18px] h-[18px] text-emerald-600 dark:text-emerald-400 shrink-0" />;
      case "compressed":
        return <Archive className="w-[18px] h-[18px] text-amber-600 dark:text-amber-400 shrink-0" />;
      case "programs":
        return <Binary className="w-[18px] h-[18px] text-teal-600 dark:text-teal-400 shrink-0" />;
      case "torrents":
        return <Radio className="w-[18px] h-[18px] text-cyan-600 dark:text-cyan-400 shrink-0" />;
      default:
        return <Layers className="w-[18px] h-[18px] text-blue-600 dark:text-blue-400 shrink-0" />;
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

  return (
    <div className="w-full bg-gdm-surface border border-gdm rounded-xl overflow-hidden shadow-xs">
      <div className="overflow-x-auto scrollbar-thin">
        <table className="w-full min-w-[760px] text-left border-collapse">
          <thead>
            <tr className="border-b border-gdm bg-gdm-tableHead text-gdm-secondary text-[12px] font-semibold uppercase tracking-wide select-none">
              <th className="py-3 px-3 w-10 text-center whitespace-nowrap">#</th>
              <th className="py-3 px-3 whitespace-nowrap min-w-[200px]">File Name</th>
              <th className="py-3 px-3 w-32 whitespace-nowrap">Size</th>
              <th className="py-3 px-3 w-48 whitespace-nowrap">Progress</th>
              <th className="py-3 px-3 w-28 whitespace-nowrap">Status</th>
              <th className="py-3 px-3 w-28 whitespace-nowrap hidden sm:table-cell">Speed</th>
              <th className="py-3 px-3 w-24 whitespace-nowrap hidden md:table-cell">Time Left</th>
              <th className="py-3 px-3 w-36 whitespace-nowrap text-right shrink-0">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gdm">
            {tasks.map((task, idx) => {
              const isSelected = selectedTaskId === task.id;
              const isExpanded = expandedId === task.id;
              const isDownloading = task.status === "downloading";
              const isCompleted = task.status === "completed";
              const isPaused = task.status === "paused";
              const isError = task.status === "error";

              return (
                <React.Fragment key={task.id}>
                  <tr
                    onClick={() => onSelectTask(task.id)}
                    className={`min-h-[56px] py-3 cursor-pointer transition-colors ${
                      isSelected
                        ? "bg-blue-50 dark:bg-blue-950/40"
                        : "hover:bg-slate-50 dark:hover:bg-slate-800/40"
                    }`}
                  >
                    {/* Index */}
                    <td className="py-3 px-3 text-center text-gdm-muted text-[13px] tabular-nums font-sans">
                      {idx + 1}
                    </td>

                    {/* File Name */}
                    <td className="py-3 px-3 max-w-xs">
                      <div className="flex items-center space-x-2.5">
                        {getCategoryIcon(task.category)}
                        <span
                          className="truncate text-[14px] font-semibold text-gdm-primary hover:underline"
                          title={task.filename}
                          onDoubleClick={() => {
                            if (!isCompleted) return;
                            const folder = task.save_path || task.savePath || "";
                            const fn = task.filename || "";
                            const full = folder ? `${folder.replace(/[/\\]+$/, "")}\\${fn}` : fn;
                            onOpenFile(full);
                          }}
                        >
                          {task.filename}
                        </span>
                      </div>
                    </td>

                    {/* Size & Resume Capability / Torrent Swarm */}
                    <td className="py-3 px-3 text-[13px] tabular-nums font-sans text-gdm-primary">
                      <div>
                        {formatBytes(getDownloadedBytes(task))} /{" "}
                        {getTotalBytes(task) > 0 ? formatBytes(getTotalBytes(task)) : "Unknown"}
                      </div>
                      {task.category === "torrents" ? (
                        <div className="flex items-center space-x-1.5 mt-0.5">
                          <span className="text-[12px] text-gdm-secondary">Seeds:</span>
                          <span className="text-[12px] font-semibold text-emerald-600 dark:text-emerald-400">
                            {task.seeders || 0}
                          </span>
                          <span className="text-[12px] text-gdm-secondary">| Peers:</span>
                          <span className="text-[12px] font-semibold text-blue-600 dark:text-blue-400">
                            {task.peers || 0}
                          </span>
                        </div>
                      ) : (
                        <div className="flex items-center space-x-1.5 mt-0.5">
                          <span className="text-[12px] text-gdm-secondary">Resume:</span>
                          {getIsResumable(task) ? (
                            <span className="text-[12px] font-semibold text-emerald-700 dark:text-emerald-400">
                              Yes
                            </span>
                          ) : (
                            <span className="text-[12px] font-semibold text-red-700 dark:text-red-400">
                              No
                            </span>
                          )}
                        </div>
                      )}
                    </td>

                    {/* Progress Bar & Percent */}
                    <td className="py-3 px-3">
                      <div className="space-y-1.5">
                        <div className="flex justify-between items-center text-[13px] font-semibold tabular-nums">
                          <span className="text-gdm-primary">{task.percent.toFixed(1)}%</span>
                          {getChunkCount(task) > 1 && (
                            <span className="text-gdm-accent text-[12px] font-medium">
                              {getChunkCount(task)} Chunks
                            </span>
                          )}
                        </div>
                        {/* 8px high solid accent progress bar */}
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
                      </div>
                    </td>

                    {/* Status Badge */}
                    <td className="py-3 px-3 whitespace-nowrap">
                      <span
                        className={`inline-block text-[12px] font-semibold uppercase tracking-wide px-2.5 py-1 rounded-md ${getStatusBadgeClass(
                          task.status
                        )}`}
                      >
                        {task.status}
                      </span>
                    </td>

                    {/* Speed: Transfer rate */}
                    <td className="py-3 px-3 text-[13px] font-semibold tabular-nums text-amber-800 dark:text-amber-400 whitespace-nowrap hidden sm:table-cell">
                      {isDownloading || task.status === "downloading" || task.status === "seeding" ? (
                        <div>
                          <div>{getSpeedFormatted(task)}</div>
                          {task.up_speed_formatted && task.up_speed_formatted !== "0 B/s" && (
                            <div className="text-[11px] font-medium text-emerald-600 dark:text-emerald-400">
                              ↑ {task.up_speed_formatted}
                            </div>
                          )}
                        </div>
                      ) : (
                        "--"
                      )}
                    </td>

                    {/* Time Left (ETA) */}
                    <td className="py-3 px-3 text-[13px] tabular-nums font-sans text-gdm-secondary whitespace-nowrap hidden md:table-cell">
                      {isDownloading ? getEtaFormatted(task) : "--"}
                    </td>

                    {/* Action Buttons: 32px hit area, 18px icons, text-secondary */}
                    <td className="py-3 px-3 text-right whitespace-nowrap shrink-0">
                      <div
                        className="flex items-center justify-end space-x-1 shrink-0"
                        onClick={(e) => e.stopPropagation()}
                      >
                        {isDownloading && (
                          <button
                            onClick={() => onPause(task.id)}
                            className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-amber-800 dark:hover:text-amber-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                            title="Pause"
                            aria-label="Pause"
                          >
                            <Pause className="w-[18px] h-[18px]" />
                          </button>
                        )}

                        {(isPaused || isError) && (
                          <button
                            onClick={() => onResume(task.id)}
                            className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-blue-600 dark:hover:text-blue-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                            title={isError ? "Retry Download" : "Resume Download"}
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
                            className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-emerald-700 dark:hover:text-emerald-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                            title="Open File"
                            aria-label="Open File"
                          >
                            <FileCheck className="w-[18px] h-[18px]" />
                          </button>
                        )}

                        {/* Open Download Details Dialog */}
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            onSelectTask(task.id);
                          }}
                          className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-gdm-primary hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                          title="View Download Details"
                          aria-label="Open Download Dialog"
                        >
                          <Info className="w-[18px] h-[18px]" />
                        </button>

                        <button
                          onClick={() => onOpenFolder(task.save_path || task.savePath || "")}
                          className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-gdm-primary hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                          title="Open Folder"
                          aria-label="Open Folder"
                        >
                          <FolderOpen className="w-[18px] h-[18px]" />
                        </button>

                        <button
                          onClick={() => onDelete(task.id)}
                          className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-red-700 dark:hover:text-red-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                          title="Delete"
                          aria-label="Delete"
                        >
                          <Trash2 className="w-[18px] h-[18px]" />
                        </button>

                        {/* Expand Chunk Drawer */}
                        {task.chunks && task.chunks.length > 0 && (
                          <button
                            onClick={() => setExpandedId(isExpanded ? null : task.id)}
                            className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-gdm-primary hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                            title="Toggle Chunks Drawer"
                            aria-label="Toggle Chunks"
                          >
                            {isExpanded ? (
                              <ChevronUp className="w-[18px] h-[18px]" />
                            ) : (
                              <ChevronDown className="w-[18px] h-[18px]" />
                            )}
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>

                  {/* IDM 16-Chunk Drawer */}
                  {isExpanded && task.chunks && task.chunks.length > 0 && (
                    <tr className="bg-slate-50 dark:bg-slate-900/60 border-b border-gdm">
                      <td colSpan={8} className="p-4">
                        <div className="space-y-3">
                          <div className="flex items-center justify-between text-[12px] text-gdm-secondary">
                            <span className="font-semibold flex items-center space-x-1.5 text-gdm-primary">
                              <Zap className="w-4 h-4 text-amber-800 dark:text-amber-400" />
                              <span>Active Segments ({task.chunks.length} Threads)</span>
                            </span>
                            <span className="text-gdm-muted text-[12px]">
                              Direct Stream Connections
                            </span>
                          </div>

                          {/* Grid of Chunk Progress Bars */}
                          <div className="grid grid-cols-2 sm:grid-cols-4 md:grid-cols-8 gap-2">
                            {task.chunks.map((chk, i) => (
                              <div
                                key={i}
                                className="bg-gdm-surface p-2 rounded-lg border border-gdm space-y-1.5"
                              >
                                <div className="flex justify-between text-[12px] tabular-nums text-gdm-secondary">
                                  <span className="font-semibold text-gdm-primary">T{i + 1}</span>
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

                          {/* Speed Chart Telemetry */}
                          {task.speedHistory && task.speedHistory.length > 0 && (
                            <div className="pt-2">
                              <SpeedChart data={task.speedHistory} height={40} compact />
                            </div>
                          )}
                        </div>
                      </td>
                    </tr>
                  )}
                </React.Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};
