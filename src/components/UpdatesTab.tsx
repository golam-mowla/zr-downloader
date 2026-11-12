import React from "react";
import { ToolVersionInfo } from "../types";
import {
  RefreshCw,
  CheckCircle2,
  AlertTriangle,
  ArrowUpCircle,
  Terminal,
  Sparkles,
  Download,
  ExternalLink,
} from "lucide-react";

interface UpdatesTabProps {
  tools: ToolVersionInfo[];
  checking: boolean;
  onCheckUpdates: () => void;
  onUpdateTool: (name: string) => void;
  onUpdateAll: () => void;
  updateLogs: string[];
  successMessage: string | null;
  appHasUpdate?: boolean;
  latestAppVersion?: string;
  isUpdatingApp?: boolean;
  on1ClickAppUpdate?: () => void;
}

export const UpdatesTab: React.FC<UpdatesTabProps> = ({
  tools,
  checking,
  onCheckUpdates,
  onUpdateTool,
  onUpdateAll,
  updateLogs,
  successMessage,
  appHasUpdate = false,
  latestAppVersion,
  isUpdatingApp = false,
  on1ClickAppUpdate,
}) => {
  return (
    <div className="space-y-4">
      {/* 1-CLICK GITHUB RELEASE APP UPDATER */}
      <div
        className={`neon-card p-5 border-2 transition-all ${
          appHasUpdate
            ? "border-cyan-400 bg-gradient-to-r from-blue-500/10 via-cyan-500/10 to-indigo-500/10 dark:from-blue-950/40 dark:to-indigo-950/40"
            : "border-slate-200 dark:border-slate-800"
        }`}
      >
        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2.5">
              <span className="p-2 rounded-xl bg-gradient-to-tr from-cyan-500 to-blue-600 text-white shadow-md shadow-cyan-500/20">
                <Sparkles className="w-5 h-5" />
              </span>
              <div>
                <h2 className="text-base font-black text-slate-900 dark:text-white flex items-center gap-2">
                  GM Downloader App Updater
                  <span className="px-2 py-0.5 text-[10px] font-black rounded-full bg-cyan-100 dark:bg-cyan-950 text-cyan-800 dark:text-cyan-300 border border-cyan-300 dark:border-cyan-700">
                    Installed: v2.0.1
                  </span>
                </h2>
                <p className="text-xs text-slate-600 dark:text-slate-300">
                  {appHasUpdate
                    ? `🎉 New GitHub Release Available: ${latestAppVersion}! Update now with 1 click.`
                    : "✔ You are running the latest official release from GitHub. Connected to GitHub Releases API."}
                </p>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2.5 w-full sm:w-auto">
            {appHasUpdate && on1ClickAppUpdate ? (
              <button
                type="button"
                onClick={on1ClickAppUpdate}
                disabled={isUpdatingApp}
                className="neon-btn-primary w-full sm:w-auto px-5 py-2.5 rounded-xl text-xs sm:text-sm font-black flex items-center justify-center gap-2 shadow-lg shadow-cyan-500/30 animate-pulse"
              >
                {isUpdatingApp ? (
                  <>
                    <RefreshCw className="w-4 h-4 animate-spin" />
                    <span>Updating App...</span>
                  </>
                ) : (
                  <>
                    <Download className="w-4 h-4 stroke-[3]" />
                    <span>1-Click Update to {latestAppVersion}</span>
                  </>
                )}
              </button>
            ) : (
              <button
                type="button"
                onClick={onCheckUpdates}
                disabled={checking}
                className="px-4 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 text-xs font-bold text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 flex items-center gap-1.5"
              >
                <RefreshCw
                  className={`w-3.5 h-3.5 ${checking ? "animate-spin" : ""}`}
                />
                Check GitHub Releases
              </button>
            )}

            <a
              href="https://github.com/golam-mowla/zr-downloader/releases"
              target="_blank"
              rel="noopener noreferrer"
              className="px-3 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 text-xs font-bold flex items-center gap-1.5 shrink-0"
              title="View all GitHub Releases & Changelogs"
            >
              <ExternalLink className="w-3.5 h-3.5" />
              <span className="hidden sm:inline">Release Notes</span>
            </a>
          </div>
        </div>
      </div>
      <div className="neon-card p-5 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm sm:text-base font-extrabold text-slate-900 dark:text-white flex items-center gap-2">
            <RefreshCw
              className={`w-4 h-4 text-cyan-600 ${
                checking ? "animate-spin" : ""
              }`}
            />
            <span>Core Engine Updater (yt-dlp, Deno JS, FFmpeg)</span>
          </h2>
          <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
            Keep yt-dlp, Deno (YouTube JS challenge solver), and FFmpeg up to date with one click so link fetching and 4K/8K downloads never break.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onCheckUpdates}
            disabled={checking}
            className="px-4 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 text-xs font-bold text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 flex items-center gap-1.5"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${checking ? "animate-spin" : ""}`}
            />
            {checking ? "Checking..." : "Refresh Versions"}
          </button>

          <button
            type="button"
            onClick={onUpdateAll}
            className="neon-btn-primary px-4 py-2 rounded-xl text-xs font-extrabold flex items-center gap-1.5"
          >
            <ArrowUpCircle className="w-4 h-4" />
            Update All Engines
          </button>
        </div>
      </div>

      {successMessage && (
        <div className="neon-card neon-border-emerald p-4 bg-emerald-50/90 dark:bg-emerald-950/60 flex items-center gap-3 text-emerald-800 dark:text-emerald-200">
          <CheckCircle2 className="w-5 h-5 text-emerald-600 shrink-0" />
          <div className="text-xs font-bold">{successMessage}</div>
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {tools.map((tool) => (
          <div
            key={tool.name}
            className="neon-card p-5 flex flex-col justify-between space-y-4"
          >
            <div>
              <div className="flex items-center justify-between">
                <span className="font-extrabold text-sm text-slate-900 dark:text-white">
                  {tool.name}
                </span>
                {tool.installedVersion === "Not Found" ? (
                  <span className="px-2.5 py-0.5 rounded-full bg-amber-50 text-amber-700 border border-amber-300 text-[10px] font-extrabold flex items-center gap-1">
                    <AlertTriangle className="w-3 h-3" />
                    Missing
                  </span>
                ) : (
                  <span className="px-2.5 py-0.5 rounded-full bg-emerald-50 text-emerald-700 border border-emerald-300 text-[10px] font-extrabold flex items-center gap-1">
                    <CheckCircle2 className="w-3 h-3" />
                    Active
                  </span>
                )}
              </div>

              <div className="mt-3 space-y-1 text-xs text-slate-600 dark:text-slate-300">
                <p>
                  Installed Version:{" "}
                  <span className="font-mono font-bold text-indigo-600 dark:text-cyan-400">
                    {tool.installedVersion}
                  </span>
                </p>
              </div>
            </div>

            <button
              type="button"
              onClick={() => onUpdateTool(tool.name)}
              disabled={tool.isUpdating}
              className="neon-btn-primary w-full py-2.5 px-3 rounded-xl text-xs font-extrabold flex items-center justify-center gap-1.5 disabled:opacity-50"
            >
              {tool.isUpdating ? (
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <ArrowUpCircle className="w-3.5 h-3.5" />
              )}
              <span>
                {tool.isUpdating ? "Updating..." : `Update ${tool.name}`}
              </span>
            </button>
          </div>
        ))}
      </div>

      <div className="neon-card p-4 space-y-2">
        <div className="text-xs font-extrabold text-slate-600 dark:text-slate-300 flex items-center gap-1.5">
          <Terminal className="w-3.5 h-3.5 text-cyan-600" />
          <span>Updater Console Output</span>
        </div>
        <div className="bg-slate-900 rounded-xl p-3 font-mono text-[11px] text-emerald-400 h-32 overflow-y-auto space-y-1 border border-slate-800">
          {updateLogs.length === 0 ? (
            <p className="text-slate-500">
              Click &ldquo;Update&rdquo; on any engine above to view live output.
            </p>
          ) : (
            updateLogs.map((log, idx) => <p key={idx}>{log}</p>)
          )}
        </div>
      </div>
    </div>
  );
};
