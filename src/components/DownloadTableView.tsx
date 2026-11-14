import React, { useState, useEffect } from "react";
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
  GripVertical,
  ShieldCheck,
  ShieldAlert,
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
  onContextMenuTask?: (e: React.MouseEvent, id: string) => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenFile: (path: string) => void;
  onOpenFolder: (path: string) => void;
  onChangePriority?: (id: string, priority: string) => void;
  onReorderTasks?: (orderedIds: string[]) => void;
  onViewErrorLog?: (task: DownloadTask) => void;
  theme?: "dark" | "light";
  windowWidth?: number;
}

export const DownloadTableView: React.FC<DownloadTableViewProps> = ({
  tasks,
  selectedTaskId,
  onSelectTask,
  onContextMenuTask,
  onPause,
  onResume,
  onDelete,
  onOpenFile,
  onOpenFolder,
  onChangePriority,
  onReorderTasks,
  onViewErrorLog,
  windowWidth,
}) => {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [draggedTaskId, setDraggedTaskId] = useState<string | null>(null);
  const [dragOverTaskId, setDragOverTaskId] = useState<string | null>(null);
  const [internalWidth, setInternalWidth] = useState<number>(
    typeof window !== "undefined" ? window.innerWidth : 1200
  );

  useEffect(() => {
    const handleResize = () => setInternalWidth(window.innerWidth);
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  const width = windowWidth ?? internalWidth;

  // Responsive Breakpoints matching exact user requirements:
  // below ~1000px: hide TIME LEFT
  // below ~900px: hide SPEED
  // below ~800px: hide the "# " column and show Size under the file name
  // below ~700px: automatically switch to card view (handled in App.tsx)
  // Sidebar (w-64 = 256px) is always visible, so base the breakpoints on the table's real width
  const SIDEBAR_W = 256;
  const avail = width - SIDEBAR_W;
  const showTimeLeft = avail >= 1030;
  const showSpeed = avail >= 930;
  const showIndex = avail >= 820;
  const showSizeColumn = avail >= 820;

  const totalVisibleCols =
    (showIndex ? 1 : 0) +
    1 + // File Name
    (showSizeColumn ? 1 : 0) +
    1 + // Progress
    1 + // Status
    (showSpeed ? 1 : 0) +
    (showTimeLeft ? 1 : 0) +
    1; // Actions

  const getCategoryIcon = (category: string) => {
    switch (category) {
      case "video":
        return <Video className="w-4 h-4 text-purple-600 dark:text-purple-400 shrink-0" />;
      case "audio":
        return <Music className="w-4 h-4 text-pink-600 dark:text-pink-400 shrink-0" />;
      case "docs":
        return <FileText className="w-4 h-4 text-emerald-600 dark:text-emerald-400 shrink-0" />;
      case "compressed":
        return <Archive className="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0" />;
      case "programs":
        return <Binary className="w-4 h-4 text-teal-600 dark:text-teal-400 shrink-0" />;
      case "torrents":
        return <Radio className="w-4 h-4 text-cyan-600 dark:text-cyan-400 shrink-0" />;
      default:
        return <Layers className="w-4 h-4 text-blue-600 dark:text-blue-400 shrink-0" />;
    }
  };

  const getStatusBadgeClass = (status: string) => {
    switch (status) {
      case "downloading":
        return "badge-status-downloading";
      case "completed":
        return "badge-status-completed";
      case "completed_with_warning":
        return "badge-status-warning";
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
    <div className="w-full bg-zr-surface border border-zr rounded-xl overflow-hidden shadow-xs">
      {/* Container: Vertical scroll only, zero horizontal scrollbar */}
      <div className="w-full overflow-x-hidden overflow-y-auto">
        <table className="w-full table-fixed border-separate border-spacing-0 text-left">
          {/* Explicit column width allocations */}
          <colgroup>
            {showIndex && <col className="w-10" />}
            <col /> {/* File Name takes remaining 1fr */}
            {showSizeColumn && <col className="w-32" />}
            <col className="w-36 sm:w-44" /> {/* Progress */}
            <col className="w-28 sm:w-32" /> {/* Status */}
            {showSpeed && <col className="w-28" />}
            {showTimeLeft && <col className="w-24" />}
            <col className="w-[120px]" /> {/* Actions */}
          </colgroup>

          <thead>
            <tr className="bg-zr-tableHead text-zr-secondary text-[12px] font-semibold uppercase tracking-wide select-none">
              {showIndex && (
                <th className="py-3 px-2 text-center whitespace-nowrap border-b border-zr">#</th>
              )}
              <th className="py-3 px-3 whitespace-nowrap truncate border-b border-zr">File Name</th>
              {showSizeColumn && (
                <th className="py-3 px-3 whitespace-nowrap border-b border-zr">Size</th>
              )}
              <th className="py-3 px-3 whitespace-nowrap border-b border-zr">Progress</th>
              <th className="py-3 px-3 whitespace-nowrap border-b border-zr">Status</th>
              {showSpeed && (
                <th className="py-3 px-3 whitespace-nowrap border-b border-zr">Speed</th>
              )}
              {showTimeLeft && (
                <th className="py-3 px-3 whitespace-nowrap border-b border-zr">Time Left</th>
              )}
              <th className="py-3 px-3 w-[120px] min-w-[120px] max-w-[120px] text-right whitespace-nowrap sticky right-0 z-20 bg-zr-tableHead/95 backdrop-blur-md border-b border-zr">
                Actions
              </th>
            </tr>
          </thead>

          <tbody>
            {tasks.map((task, idx) => {
              const isSelected = selectedTaskId === task.id;
              const isExpanded = expandedId === task.id;
              const isDownloading = task.status === "downloading";
              const isWarning = task.status === "completed_with_warning";
              const isCompleted = task.status === "completed";
              const isPaused = task.status === "paused";
              const isError = task.status === "error" && !isWarning;
              const isFileReady = isCompleted || isWarning;

              return (
                <React.Fragment key={task.id}>
                  <tr
                    draggable={Boolean(onReorderTasks)}
                    onDragStart={(e) => {
                      setDraggedTaskId(task.id);
                      e.dataTransfer.effectAllowed = "move";
                      e.dataTransfer.setData("text/plain", task.id);
                    }}
                    onDragOver={(e) => {
                      if (!onReorderTasks || !draggedTaskId || draggedTaskId === task.id) return;
                      e.preventDefault();
                      setDragOverTaskId(task.id);
                    }}
                    onDragLeave={() => {
                      if (dragOverTaskId === task.id) setDragOverTaskId(null);
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      const srcId = draggedTaskId || e.dataTransfer.getData("text/plain");
                      setDraggedTaskId(null);
                      setDragOverTaskId(null);
                      if (!onReorderTasks || !srcId || srcId === task.id) return;
                      const ids = tasks.map((t) => t.id);
                      const fromIdx = ids.indexOf(srcId);
                      const toIdx = ids.indexOf(task.id);
                      if (fromIdx < 0 || toIdx < 0) return;
                      ids.splice(fromIdx, 1);
                      ids.splice(toIdx, 0, srcId);
                      onReorderTasks(ids);
                    }}
                    onDragEnd={() => {
                      setDraggedTaskId(null);
                      setDragOverTaskId(null);
                    }}
                    onClick={() => onSelectTask(task.id)}
                    onContextMenu={(e) => {
                      if (!onContextMenuTask) return;
                      e.preventDefault();
                      onContextMenuTask(e, task.id);
                    }}
                    className={`group min-h-[56px] cursor-pointer transition-colors ${
                      dragOverTaskId === task.id
                        ? "border-t-2 border-blue-500 bg-blue-50/40 dark:bg-blue-950/30"
                        : isSelected
                        ? "bg-blue-50/80 dark:bg-blue-950/40"
                        : "hover:bg-slate-50/80 dark:hover:bg-slate-800/40"
                    }`}
                  >
                    {/* Index + Drag Grip */}
                    {showIndex && (
                      <td className="py-2.5 px-2 text-center text-zr-muted text-[12px] tabular-nums font-sans border-b border-zr">
                        <div className="inline-flex items-center justify-center gap-0.5">
                          {onReorderTasks && (
                            <GripVertical className="w-3.5 h-3.5 opacity-0 group-hover:opacity-60 cursor-grab shrink-0" />
                          )}
                          <span>{idx + 1}</span>
                        </div>
                      </td>
                    )}

                    {/* File Name (flexible 1fr, shrinks with ellipsis) + Badges */}
                    <td className="py-2.5 px-3 min-w-0 max-w-0 truncate border-b border-zr">
                      <div className="flex items-center space-x-2 min-w-0">
                        {getCategoryIcon(task.category)}
                        <div className="min-w-0 flex-1 truncate">
                          <div className="flex items-center space-x-1.5 min-w-0">
                            <span
                              className="truncate text-[13px] font-semibold text-zr-primary hover:underline"
                              title={task.filename}
                              onDoubleClick={() => {
                                if (!isFileReady) return;
                                const folder = task.save_path || task.savePath || "";
                                const fn = task.filename || "";
                                const full = folder ? `${folder.replace(/[/\\]+$/, "")}\\${fn}` : fn;
                                onOpenFile(full);
                              }}
                            >
                              {task.filename}
                            </span>
                            {(task.checksumStatus || task.checksum_status) === "verified" && (
                              <span
                                className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 text-[10px] font-bold shrink-0"
                                title="Checksum verified"
                              >
                                <ShieldCheck className="w-3 h-3" />
                                <span>Verified</span>
                              </span>
                            )}
                            {(task.checksumStatus || task.checksum_status) === "mismatch" && (
                              <span
                                className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-red-500/15 text-red-600 dark:text-red-400 text-[10px] font-bold shrink-0"
                                title="Checksum mismatch!"
                              >
                                <ShieldAlert className="w-3 h-3" />
                                <span>Mismatch</span>
                              </span>
                            )}
                          </div>

                          {/* When Size column is hidden (<800px), display Size cleanly under file name */}
                          {!showSizeColumn && (
                            <div className="text-[11px] text-zr-secondary tabular-nums truncate pt-0.5">
                              {formatBytes(getDownloadedBytes(task))} /{" "}
                              {getTotalBytes(task) > 0 ? formatBytes(getTotalBytes(task)) : "Unknown"}
                              {task.category === "torrents"
                                ? ` • S:${task.seeders || 0} P:${task.peers || 0}`
                                : getIsResumable(task)
                                ? " • Resume"
                                : ""}
                            </div>
                          )}
                        </div>
                      </div>
                    </td>

                    {/* Dedicated Size Column (visible >= 800px) */}
                    {showSizeColumn && (
                      <td className="py-2.5 px-3 text-[12px] tabular-nums font-sans text-zr-primary whitespace-nowrap border-b border-zr">
                        <div>
                          {formatBytes(getDownloadedBytes(task))} /{" "}
                          {getTotalBytes(task) > 0 ? formatBytes(getTotalBytes(task)) : "Unknown"}
                        </div>
                        {task.category === "torrents" ? (
                          <div className="flex items-center space-x-1.5 mt-0.5 text-[11px] text-zr-secondary">
                            <span>Seeds:</span>
                            <span className="font-semibold text-emerald-600 dark:text-emerald-400">
                              {task.seeders || 0}
                            </span>
                            <span>| Peers:</span>
                            <span className="font-semibold text-blue-600 dark:text-blue-400">
                              {task.peers || 0}
                            </span>
                          </div>
                        ) : (
                          <div className="flex items-center space-x-1.5 mt-0.5 text-[11px] text-zr-secondary">
                            <span>Resume:</span>
                            {getIsResumable(task) ? (
                              <span className="font-semibold text-emerald-700 dark:text-emerald-400">
                                Yes
                              </span>
                            ) : (
                              <span className="font-semibold text-red-700 dark:text-red-400">
                                No
                              </span>
                            )}
                          </div>
                        )}
                      </td>
                    )}

                    {/* Progress Bar & Percent */}
                    <td className="py-2.5 px-3 border-b border-zr">
                      <div className="space-y-1">
                        <div className="flex justify-between items-center text-[12px] font-semibold tabular-nums">
                          <span className="text-zr-primary">
                            {(isError && task.percent >= 100 ? 99 : task.percent).toFixed(1)}%
                          </span>
                          {getChunkCount(task) > 1 && (
                            <span className="text-zr-accent text-[11px] font-medium hidden sm:inline">
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
                                : isWarning
                                ? "bg-amber-600 dark:bg-amber-500"
                                : isError
                                ? "bg-red-600 dark:bg-red-500"
                                : isPaused
                                ? "bg-amber-600 dark:bg-amber-500"
                                : "bg-blue-600 dark:bg-blue-400"
                            }`}
                            style={{
                              width: `${Math.min(
                                100,
                                Math.max(0, isError && task.percent >= 100 ? 99 : task.percent)
                              )}%`,
                            }}
                          />
                        </div>
                        {/* Responsive inline speed and ETA when columns are hidden (<900px or <1000px) */}
                        {(!showSpeed || !showTimeLeft) && (
                          <div className="flex items-center justify-between text-[11px] tabular-nums text-zr-secondary pt-0.5 truncate">
                            <span>{!showSpeed && isDownloading ? getSpeedFormatted(task) : ""}</span>
                            <span>{!showTimeLeft && isDownloading ? getEtaFormatted(task) : ""}</span>
                          </div>
                        )}
                      </div>
                    </td>

                    {/* Status Badge + Priority Selector (stacked so Priority never overlaps Speed) */}
                    <td className="py-2.5 px-3 whitespace-nowrap overflow-hidden border-b border-zr">
                      <div className="flex flex-col items-start gap-1">
                        <span
                          className={`inline-block max-w-full truncate text-[11px] font-semibold uppercase tracking-wide px-2 py-0.5 rounded-md ${getStatusBadgeClass(
                            task.status
                          )}`}
                          title={isWarning ? "Downloaded, but conversion failed — file kept" : task.status}
                        >
                          {isWarning
                            ? "Warning"
                            : task.status}
                        </span>
                        <div className="flex items-center gap-1">
                          {onChangePriority && !isFileReady && !isError && task.status !== "cancelled" && (
                            <select
                              value={task.priority || "normal"}
                              onClick={(e) => e.stopPropagation()}
                              onChange={(e) => onChangePriority(task.id, e.target.value)}
                              className="text-[10px] font-semibold px-1.5 py-0.5 rounded border border-zr bg-zr-surface text-zr-secondary cursor-pointer focus:outline-none focus:border-blue-500"
                              title="Queue priority"
                            >
                              <option value="high">High</option>
                              <option value="normal">Normal</option>
                              <option value="low">Low</option>
                            </select>
                          )}
                          {isError && onViewErrorLog && (
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                onViewErrorLog(task);
                              }}
                              className="text-[10px] font-semibold px-1.5 py-0.5 rounded bg-red-500/15 text-red-600 dark:text-red-400 hover:bg-red-500/25 border border-red-500/30 transition-colors"
                              title={task.errorMessage || (task as any).error || "View failure log"}
                            >
                              Log
                            </button>
                          )}
                        </div>
                      </div>
                    </td>

                    {/* Speed: Transfer rate (visible >= 900px) */}
                    {showSpeed && (
                      <td className="py-2.5 px-3 text-[12px] font-semibold tabular-nums text-amber-800 dark:text-amber-400 whitespace-nowrap overflow-hidden border-b border-zr">
                        {isDownloading || task.status === "assembling" || task.status === "downloading" || task.status === "seeding" ? (
                          <div>
                            <div className="truncate">{getSpeedFormatted(task)}</div>
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
                    )}

                    {/* Time Left (visible >= 1000px) */}
                    {showTimeLeft && (
                      <td className="py-2.5 px-3 text-[12px] tabular-nums font-sans text-zr-secondary whitespace-nowrap border-b border-zr">
                        {isDownloading ? getEtaFormatted(task) : "--"}
                      </td>
                    )}

                    {/* Action Buttons: ALWAYS fully visible, sticky on right, fixed 120px, icon-only, no wrap */}
                    <td
                      className={`py-2.5 px-3 w-[120px] min-w-[120px] max-w-[120px] text-right whitespace-nowrap sticky right-0 z-10 border-b border-zr backdrop-blur-md transition-colors ${
                        isSelected
                          ? "bg-blue-50/95 dark:bg-blue-950/85"
                          : "bg-zr-surface/95 dark:bg-[#0f172a]/95 group-hover:bg-slate-100/95 dark:group-hover:bg-slate-800/90"
                      }`}
                    >
                      <div
                        className="flex items-center justify-end space-x-1 w-full shrink-0 flex-nowrap"
                        onClick={(e) => e.stopPropagation()}
                      >
                        {isDownloading && (
                          <button
                            onClick={() => onPause(task.id)}
                            className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 text-zr-secondary hover:text-amber-800 dark:hover:text-amber-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 transition-colors"
                            title="Pause"
                            aria-label="Pause"
                          >
                            <Pause className="w-4 h-4" />
                          </button>
                        )}

                        {(isPaused || isError) && (
                          <button
                            onClick={() => onResume(task.id)}
                            className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 text-zr-secondary hover:text-blue-600 dark:hover:text-blue-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 transition-colors"
                            title={isError ? "Retry Download" : "Resume Download"}
                            aria-label={isError ? "Retry" : "Resume"}
                          >
                            <Play className="w-4 h-4 fill-current" />
                          </button>
                        )}

                        {isFileReady && (
                          <button
                            onClick={() => {
                              const folder = task.save_path || task.savePath || "";
                              const fn = task.filename || "";
                              const full = folder ? `${folder.replace(/[/\\]+$/, "")}\\${fn}` : fn;
                              onOpenFile(full);
                            }}
                            className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 text-zr-secondary hover:text-emerald-700 dark:hover:text-emerald-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 transition-colors"
                            title="Open File"
                            aria-label="Open File"
                          >
                            <FileCheck className="w-4 h-4" />
                          </button>
                        )}

                        <button
                          onClick={() => onOpenFolder(task.save_path || task.savePath || "")}
                          className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 text-zr-secondary hover:text-zr-primary hover:bg-slate-200/60 dark:hover:bg-slate-700/60 transition-colors"
                          title="Open Folder"
                          aria-label="Open Folder"
                        >
                          <FolderOpen className="w-4 h-4" />
                        </button>

                        <button
                          onClick={() => onDelete(task.id)}
                          className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 text-zr-secondary hover:text-red-700 dark:hover:text-red-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 transition-colors"
                          title="Delete"
                          aria-label="Delete"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>

                        {/* Expand Chunk Drawer */}
                        {task.chunks && task.chunks.length > 0 && (
                          <button
                            onClick={() => setExpandedId(isExpanded ? null : task.id)}
                            className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 text-zr-secondary hover:text-zr-primary hover:bg-slate-200/60 dark:hover:bg-slate-700/60 transition-colors"
                            title="Toggle Chunks Drawer"
                            aria-label="Toggle Chunks"
                          >
                            {isExpanded ? (
                              <ChevronUp className="w-4 h-4" />
                            ) : (
                              <ChevronDown className="w-4 h-4" />
                            )}
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>

                  {/* IDM 16-Chunk Drawer */}
                  {isExpanded && task.chunks && task.chunks.length > 0 && (
                    <tr className="bg-slate-50 dark:bg-slate-900/60">
                      <td colSpan={totalVisibleCols} className="p-4 border-b border-zr">
                        <div className="space-y-3">
                          <div className="flex items-center justify-between text-[12px] text-zr-secondary">
                            <span className="font-semibold flex items-center space-x-1.5 text-zr-primary">
                              <Zap className="w-4 h-4 text-amber-800 dark:text-amber-400" />
                              <span>Active Segments ({task.chunks.length} Threads)</span>
                            </span>
                            <span className="text-zr-muted text-[12px]">
                              Direct Stream Connections
                            </span>
                          </div>

                          {/* Grid of Chunk Progress Bars */}
                          <div className="grid grid-cols-2 sm:grid-cols-4 md:grid-cols-8 gap-2">
                            {task.chunks.map((chk, i) => (
                              <div
                                key={i}
                                className="bg-zr-surface p-2 rounded-lg border border-zr space-y-1.5"
                              >
                                <div className="flex justify-between text-[12px] tabular-nums text-zr-secondary">
                                  <span className="font-semibold text-zr-primary">T{i + 1}</span>
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

