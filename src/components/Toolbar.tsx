import React from "react";
import {
  Plus,
  Play,
  Pause,
  Square,
  Trash2,
  CheckCheck,
  Settings,
  Sparkles,
  FolderOpen,
} from "lucide-react";

interface ToolbarProps {
  onAddUrl: () => void;
  onResumeSelected: () => void;
  onPauseSelected: () => void;
  onStopAll: () => void;
  onDeleteSelected: () => void;
  onClearCompleted: () => void;
  onOpenMediaGrabber: () => void;
  onOpenSettings: () => void;
  onOpenDefaultFolder: () => void;
  hasSelected: boolean;
  hasActive: boolean;
  hasCompleted: boolean;
  theme?: "dark" | "light";
}

export const Toolbar: React.FC<ToolbarProps> = ({
  onAddUrl,
  onResumeSelected,
  onPauseSelected,
  onStopAll,
  onDeleteSelected,
  onClearCompleted,
  onOpenMediaGrabber,
  onOpenSettings,
  onOpenDefaultFolder,
  hasSelected,
  hasActive,
  hasCompleted,
}) => {
  return (
    <div className="h-14 px-3 sm:px-4 border-b border-gdm bg-gdm-surface flex items-center justify-between gap-1.5 sm:gap-2 z-10 select-none overflow-x-auto transition-colors">
      {/* Primary Action Buttons */}
      <div className="flex items-center space-x-1.5 sm:space-x-2 shrink-0">
        {/* Add URL */}
        <button
          onClick={onAddUrl}
          className="flex items-center space-x-1.5 min-h-[32px] px-2.5 sm:px-3.5 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-[13px] font-semibold shadow-xs transition-colors active:scale-95"
          title="Add new download URL"
        >
          <Plus className="w-[18px] h-[18px]" />
          <span>Add URL</span>
        </button>

        {/* Resume */}
        <button
          onClick={onResumeSelected}
          disabled={!hasSelected}
          className="flex items-center space-x-1 sm:space-x-1.5 min-h-[32px] px-2 sm:px-3 py-1.5 rounded-lg text-[13px] font-semibold border border-gdm bg-gdm-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-emerald-700 dark:text-emerald-400 disabled:opacity-60 disabled:pointer-events-none transition-colors"
          title="Resume selected download"
        >
          <Play className="w-[18px] h-[18px] fill-current" />
          <span className="hidden md:inline">Resume</span>
        </button>

        {/* Stop */}
        <button
          onClick={onPauseSelected}
          disabled={!hasSelected}
          className="flex items-center space-x-1 sm:space-x-1.5 min-h-[32px] px-2 sm:px-3 py-1.5 rounded-lg text-[13px] font-semibold border border-gdm bg-gdm-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-amber-800 dark:text-amber-400 disabled:opacity-60 disabled:pointer-events-none transition-colors"
          title="Pause selected download"
        >
          <Pause className="w-[18px] h-[18px]" />
          <span className="hidden md:inline">Stop</span>
        </button>

        {/* Stop All */}
        <button
          onClick={onStopAll}
          disabled={!hasActive}
          className="flex items-center space-x-1 sm:space-x-1.5 min-h-[32px] px-2 sm:px-3 py-1.5 rounded-lg text-[13px] font-semibold border border-red-300 dark:border-red-900/60 bg-red-50 dark:bg-red-950/30 hover:bg-red-100 dark:hover:bg-red-950/50 text-red-700 dark:text-red-400 disabled:opacity-60 disabled:pointer-events-none transition-colors"
          title="Stop all active downloads"
        >
          <Square className="w-[18px] h-[18px]" />
          <span className="hidden lg:inline">Stop All</span>
        </button>

        {/* Delete */}
        <button
          onClick={onDeleteSelected}
          disabled={!hasSelected}
          className="flex items-center space-x-1 sm:space-x-1.5 min-h-[32px] px-2 sm:px-3 py-1.5 rounded-lg text-[13px] font-semibold border border-gdm bg-gdm-surface hover:bg-red-50 dark:hover:bg-red-950/30 text-gdm-secondary hover:text-red-600 dark:hover:text-red-400 disabled:opacity-60 disabled:pointer-events-none transition-colors"
          title="Delete selected download"
        >
          <Trash2 className="w-[18px] h-[18px]" />
          <span className="hidden md:inline">Delete</span>
        </button>

        {/* Clear Completed */}
        <button
          onClick={onClearCompleted}
          disabled={!hasCompleted}
          className="hidden sm:flex items-center space-x-1.5 min-h-[32px] px-2.5 sm:px-3 py-1.5 rounded-lg text-[13px] font-semibold border border-gdm bg-gdm-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-gdm-secondary hover:text-gdm-primary disabled:opacity-60 disabled:pointer-events-none transition-colors"
          title="Clear finished downloads from list"
        >
          <CheckCheck className="w-[18px] h-[18px]" />
          <span className="hidden xl:inline">Clean Finished</span>
        </button>
      </div>

      {/* Right Controls */}
      <div className="flex items-center space-x-2 shrink-0">
        {/* Media Grabber */}
        <button
          onClick={onOpenMediaGrabber}
          className="flex items-center space-x-1.5 min-h-[32px] px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-[13px] font-semibold transition-colors shadow-xs"
          title="Open Media Extractor"
        >
          <Sparkles className="w-[18px] h-[18px] text-amber-300" />
          <span className="hidden md:inline">Media Grabber</span>
        </button>

        {/* Open Download Folder */}
        <button
          onClick={onOpenDefaultFolder}
          className="w-8 h-8 rounded-lg flex items-center justify-center border border-gdm bg-gdm-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-gdm-secondary hover:text-gdm-primary transition-colors"
          title="Open Downloads Folder"
          aria-label="Open Downloads Folder"
        >
          <FolderOpen className="w-[18px] h-[18px] text-blue-600 dark:text-blue-400" />
        </button>

        {/* Settings */}
        <button
          onClick={onOpenSettings}
          className="w-8 h-8 rounded-lg flex items-center justify-center border border-gdm bg-gdm-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-gdm-secondary hover:text-gdm-primary transition-colors"
          title="Preferences & Settings"
          aria-label="Preferences & Settings"
        >
          <Settings className="w-[18px] h-[18px]" />
        </button>
      </div>
    </div>
  );
};
