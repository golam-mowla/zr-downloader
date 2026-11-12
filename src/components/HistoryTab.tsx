import React from "react";
import { DownloadTask } from "../types";
import {
  Play,
  Pause,
  FolderOpen,
  Trash2,
  Clock,
  CheckCircle2,
  AlertTriangle,
  FileVideo,
  FileAudio,
  Subtitles,
} from "lucide-react";

interface HistoryTabProps {
  tasks: DownloadTask[];
  onResumeTask: (task: DownloadTask) => void;
  onPauseTask: (id: string) => void;
  onDeleteTask: (id: string) => void;
  onClearCompleted: () => void;
  onOpenFolder: (path: string) => void;
}

export const HistoryTab: React.FC<HistoryTabProps> = ({
  tasks,
  onResumeTask,
  onPauseTask,
  onDeleteTask,
  onClearCompleted,
  onOpenFolder,
}) => {
  return (
    <div className="space-y-4">
      <div className="neon-card p-4 sm:p-5 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm sm:text-base font-extrabold text-slate-900 dark:text-white flex items-center gap-2">
            <Clock className="w-4 h-4 text-cyan-600" />
            <span>Download Queue &amp; Crash-Safe Resume History</span>
          </h2>
          <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
            Interrupted downloads (network disconnect, power cut, PC restart) can be resumed from the exact byte using saved .part files.
          </p>
        </div>

        {tasks.some((t) => t.status === "completed" || t.status === "completed_with_warning") && (
          <button
            type="button"
            onClick={onClearCompleted}
            className="px-3.5 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-xs font-bold text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 transition-all"
          >
            Clear Completed
          </button>
        )}
      </div>

      {tasks.length === 0 ? (
        <div className="neon-card p-12 text-center space-y-2">
          <Clock className="w-9 h-9 text-cyan-500/60 mx-auto" />
          <h3 className="text-sm font-extrabold text-slate-800 dark:text-slate-100">
            No downloads in history yet
          </h3>
          <p className="text-xs text-slate-500 dark:text-slate-400 max-w-md mx-auto">
            When you start downloads, they appear here automatically. Even if your internet drops or power turns off, you can resume right from where it stopped.
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {tasks.map((task) => {
            const isPaused =
              task.status === "paused" ||
              task.status === "interrupted" ||
              task.status === "error";
            const isDownloading = task.status === "downloading";
            const isWarning = task.status === "completed_with_warning";
            const isCompleted = task.status === "completed";
            const isFileReady = isCompleted || isWarning;

            return (
              <div key={task.id} className="neon-card p-4 space-y-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="flex items-start gap-3 min-w-0">
                    <div className="w-14 h-11 rounded-xl bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 overflow-hidden shrink-0 flex items-center justify-center">
                      {task.thumbnail ? (
                        <img
                          src={task.thumbnail}
                          alt=""
                          className="w-full h-full object-cover"
                        />
                      ) : task.options.downloadMode === "audio" ? (
                        <FileAudio className="w-5 h-5 text-purple-600" />
                      ) : task.options.downloadMode === "subtitles" ? (
                        <Subtitles className="w-5 h-5 text-emerald-600" />
                      ) : (
                        <FileVideo className="w-5 h-5 text-cyan-600" />
                      )}
                    </div>

                    <div className="min-w-0">
                      <h4
                        className="text-xs sm:text-sm font-extrabold text-slate-900 dark:text-white truncate max-w-xl"
                        title={task.title}
                      >
                        {task.title}
                      </h4>
                      <div className="flex flex-wrap items-center gap-1.5 mt-1 text-[11px] text-slate-500 dark:text-slate-400">
                        <span className="px-2 py-0.5 rounded-md bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-300 border border-cyan-200 dark:border-cyan-800 font-bold">
                          {task.container} &bull; {task.qualityLabel}
                        </span>
                        {task.options.enableClip && (
                          <span className="px-2 py-0.5 rounded-md bg-purple-50 dark:bg-purple-950/60 text-purple-700 dark:text-purple-300 border border-purple-200 dark:border-purple-800 font-bold">
                            ✂️ Clip: {task.options.startTime} -{" "}
                            {task.options.endTime}
                          </span>
                        )}
                        <span>
                          &bull; {new Date(task.createdAt || task.created_at || Date.now()).toLocaleString()}
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* Status Badge */}
                  <div>
                    {isCompleted && (
                      <span className="px-2.5 py-1 rounded-full bg-emerald-50 dark:bg-emerald-950 text-emerald-700 dark:text-emerald-300 border border-emerald-300 dark:border-emerald-700 text-[11px] font-extrabold flex items-center gap-1">
                        <CheckCircle2 className="w-3.5 h-3.5" />
                        Completed
                      </span>
                    )}
                    {isWarning && (
                      <span className="px-2.5 py-1 rounded-full bg-amber-50 dark:bg-amber-950 text-amber-700 dark:text-amber-300 border border-amber-300 dark:border-amber-700 text-[11px] font-extrabold flex items-center gap-1">
                        <AlertTriangle className="w-3.5 h-3.5" />
                        Downloaded, but conversion failed — file kept
                      </span>
                    )}
                    {isPaused && (
                      <span className="px-2.5 py-1 rounded-full bg-amber-50 dark:bg-amber-950 text-amber-700 dark:text-amber-300 border border-amber-300 dark:border-amber-700 text-[11px] font-extrabold flex items-center gap-1">
                        <AlertTriangle className="w-3.5 h-3.5" />
                        {task.status === "paused"
                          ? "Paused (.part saved)"
                          : "Interrupted"}
                      </span>
                    )}
                    {isDownloading && (
                      <span className="px-2.5 py-1 rounded-full bg-cyan-50 dark:bg-cyan-950 text-cyan-700 dark:text-cyan-300 border border-cyan-300 dark:border-cyan-700 text-[11px] font-extrabold flex items-center gap-1.5">
                        <span className="w-2 h-2 rounded-full bg-cyan-500 animate-ping" />
                        Downloading
                      </span>
                    )}
                  </div>
                </div>

                {/* Progress Bar */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-[11px] font-bold text-slate-600 dark:text-slate-300">
                    <span className="truncate max-w-sm">
                      {isWarning
                        ? "Downloaded, but conversion failed — file kept"
                        : task.progressText || (task.percent >= 100 ? "Completed ✔" : `${task.percent.toFixed(1)}%`)}
                    </span>
                    <span className="text-indigo-600 dark:text-cyan-400 font-black shrink-0">
                      {task.speed ? `${task.speed} • ` : ""}
                      {task.eta ? `ETA ${task.eta} • ` : ""}
                      {task.percent.toFixed(1)}%
                    </span>
                  </div>
                  <div className="w-full bg-slate-100 dark:bg-slate-800 h-2.5 rounded-full overflow-hidden border border-slate-200 dark:border-slate-700">
                    <div
                      className="h-full rounded-full transition-all duration-300"
                      style={{
                        width: `${Math.min(100, Math.max(0, task.percent))}%`,
                        background: isCompleted
                          ? "#10b981"
                          : isWarning || isPaused
                          ? "#f59e0b"
                          : "linear-gradient(90deg, #06b6d4, #6366f1, #a855f7)",
                      }}
                    />
                  </div>
                </div>

                {/* Actions */}
                <div className="flex items-center justify-between pt-1">
                  <div
                    className="text-[11px] font-medium text-slate-500 truncate max-w-sm"
                    title={task.saveFolder}
                  >
                    📁 {task.saveFolder}
                  </div>

                  <div className="flex items-center gap-2">
                    {isPaused && (
                      <button
                        type="button"
                        onClick={() => onResumeTask(task)}
                        className="neon-btn-emerald px-3.5 py-1.5 rounded-xl text-xs font-extrabold flex items-center gap-1.5"
                      >
                        <Play className="w-3.5 h-3.5 fill-current" />
                        Resume Download
                      </button>
                    )}

                    {isDownloading && (
                      <button
                        type="button"
                        onClick={() => onPauseTask(task.id)}
                        className="px-3.5 py-1.5 rounded-xl bg-amber-500 hover:bg-amber-600 text-white text-xs font-extrabold flex items-center gap-1.5"
                      >
                        <Pause className="w-3.5 h-3.5 fill-current" />
                        Pause
                      </button>
                    )}

                    <button
                      type="button"
                      onClick={() => onOpenFolder(task.saveFolder || task.save_path || "")}
                      className="p-2 rounded-xl bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-200 border border-slate-200 dark:border-slate-700"
                      title="Open Destination Folder"
                    >
                      <FolderOpen className="w-3.5 h-3.5" />
                    </button>

                    <button
                      type="button"
                      onClick={() => onDeleteTask(task.id)}
                      className="p-2 rounded-xl bg-rose-50 hover:bg-rose-100 dark:bg-rose-950/50 text-rose-600 border border-rose-200 dark:border-rose-800"
                      title="Remove from history"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
