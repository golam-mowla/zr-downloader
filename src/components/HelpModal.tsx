import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  X,
  HelpCircle,
  Github,
  ExternalLink,
  Copy,
  Check,
  AlertTriangle,
  RefreshCw,
  Trash2,
  Bug,
  Tag,
  Keyboard,
  FileWarning,
} from "lucide-react";
import { DownloadTask } from "../types";

interface HelpModalProps {
  isOpen: boolean;
  initialTab?: "help" | "logs";
  focusedTaskId?: string | null;
  tasks: DownloadTask[];
  onClose: () => void;
}

const REPO_URL = "https://github.com/golam-mowla/zr-downloader";
const RELEASES_URL = "https://github.com/golam-mowla/zr-downloader/releases";
const ISSUES_URL = "https://github.com/golam-mowla/zr-downloader/issues";

export const HelpModal: React.FC<HelpModalProps> = ({
  isOpen,
  initialTab = "help",
  focusedTaskId = null,
  tasks,
  onClose,
}) => {
  const [activeTab, setActiveTab] = useState<"help" | "logs">(initialTab);
  const [logText, setLogText] = useState<string>("");
  const [loadingLogs, setLoadingLogs] = useState(false);
  const [copiedRepo, setCopiedRepo] = useState(false);
  const [copiedLogs, setCopiedLogs] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setActiveTab(initialTab);
    }
  }, [isOpen, initialTab]);

  const loadLogs = async () => {
    setLoadingLogs(true);
    try {
      const content = await invoke<string>("get_error_logs");
      setLogText(content || "");
    } catch (err) {
      setLogText(`Unable to read failure log file: ${String(err)}`);
    } finally {
      setLoadingLogs(false);
    }
  };

  useEffect(() => {
    if (isOpen && activeTab === "logs") {
      loadLogs();
    }
  }, [isOpen, activeTab]);

  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const failedTasks = tasks.filter(
    (t) =>
      t.status === "error" ||
      Boolean(t.errorMessage || t.error) ||
      Boolean((t.errorLog && t.errorLog.length > 0) || (t.error_log && t.error_log.length > 0))
  );

  const focusedTask = focusedTaskId
    ? tasks.find((t) => t.id === focusedTaskId) || null
    : null;

  const handleClearLogs = async () => {
    try {
      await invoke("clear_error_logs");
      setLogText("");
    } catch {}
  };

  const handleCopyAllLogs = () => {
    const sections: string[] = [];
    if (focusedTask) {
      const lines = focusedTask.errorLog || focusedTask.error_log || [];
      sections.push(
        `=== Selected Task: ${focusedTask.filename} ===\nError: ${
          focusedTask.errorMessage || focusedTask.error || "Unknown error"
        }\n${lines.join("\n")}`
      );
    }
    if (logText.trim()) {
      sections.push(`=== Persistent Failure Log (zr_error_logs.log) ===\n${logText}`);
    }
    navigator.clipboard.writeText(sections.join("\n\n") || "No failure logs recorded.");
    setCopiedLogs(true);
    setTimeout(() => setCopiedLogs(false), 1800);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px] p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-3xl max-h-[86vh] flex flex-col rounded-2xl border border-zr bg-zr-surface shadow-2xl overflow-hidden text-zr-primary"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="px-5 py-4 border-b border-zr flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <div className="w-9 h-9 rounded-xl bg-blue-600/15 border border-blue-500/30 flex items-center justify-center text-blue-600 dark:text-blue-400">
              <HelpCircle className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-[15px] font-bold text-zr-primary leading-tight">
                Help, Repository &amp; Failure Logs
              </h2>
              <p className="text-[11.5px] text-zr-secondary">
                ZR Downloader v1.2.37 &bull; by Golam Mowla
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center hover:bg-slate-200/70 dark:hover:bg-slate-800 text-zr-secondary hover:text-zr-primary transition-colors"
            aria-label="Close Help Modal"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Tabs */}
        <div className="px-5 pt-3 border-b border-zr flex items-center gap-2 bg-slate-50/50 dark:bg-slate-900/30">
          <button
            type="button"
            onClick={() => setActiveTab("help")}
            className={`px-3.5 py-2 text-[12.5px] font-semibold rounded-t-lg border-b-2 transition-colors flex items-center gap-1.5 ${
              activeTab === "help"
                ? "border-blue-600 text-blue-600 dark:text-blue-400 bg-zr-surface"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Github className="w-3.5 h-3.5" />
            <span>Help &amp; Repository</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("logs")}
            className={`px-3.5 py-2 text-[12.5px] font-semibold rounded-t-lg border-b-2 transition-colors flex items-center gap-1.5 ${
              activeTab === "logs"
                ? "border-red-500 text-red-600 dark:text-red-400 bg-zr-surface"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <FileWarning className="w-3.5 h-3.5" />
            <span>Failure Logs</span>
            {failedTasks.length > 0 && (
              <span className="px-1.5 py-0.2 rounded-full text-[10px] font-bold bg-red-500/20 text-red-600 dark:text-red-400">
                {failedTasks.length}
              </span>
            )}
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-5 space-y-5">
          {activeTab === "help" ? (
            <>
              {/* Repository Card */}
              <div className="p-4 rounded-xl border border-blue-500/25 bg-blue-500/5 space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <Github className="w-5 h-5 text-zr-primary" />
                    <div>
                      <h3 className="text-[13.5px] font-bold text-zr-primary">
                        Official GitHub Repository
                      </h3>
                      <p className="text-[11.5px] text-zr-secondary font-mono break-all">
                        {REPO_URL}
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        navigator.clipboard.writeText(REPO_URL);
                        setCopiedRepo(true);
                        setTimeout(() => setCopiedRepo(false), 1800);
                      }}
                      className="px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-[12px] font-medium flex items-center gap-1.5"
                    >
                      {copiedRepo ? (
                        <Check className="w-3.5 h-3.5 text-emerald-500" />
                      ) : (
                        <Copy className="w-3.5 h-3.5" />
                      )}
                      <span>{copiedRepo ? "Copied" : "Copy URL"}</span>
                    </button>
                    <a
                      href={REPO_URL}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-[12px] font-semibold flex items-center gap-1.5"
                    >
                      <span>Open Repo</span>
                      <ExternalLink className="w-3.5 h-3.5" />
                    </a>
                  </div>
                </div>

                <div className="flex flex-wrap gap-2 pt-1">
                  <a
                    href={RELEASES_URL}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-[12px] font-medium flex items-center gap-1.5"
                  >
                    <Tag className="w-3.5 h-3.5 text-emerald-500" />
                    <span>Releases &amp; Updates</span>
                    <ExternalLink className="w-3 h-3 text-zr-secondary" />
                  </a>
                  <a
                    href={ISSUES_URL}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-[12px] font-medium flex items-center gap-1.5"
                  >
                    <Bug className="w-3.5 h-3.5 text-rose-500" />
                    <span>Report Bug / Request Feature</span>
                    <ExternalLink className="w-3 h-3 text-zr-secondary" />
                  </a>
                </div>
              </div>

              {/* Troubleshooting Guide */}
              <div className="space-y-2.5">
                <h3 className="text-[13px] font-bold text-zr-primary flex items-center gap-1.5">
                  <AlertTriangle className="w-4 h-4 text-amber-500" />
                  <span>Quick Guide &amp; Troubleshooting</span>
                </h3>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-[12px]">
                  <div className="p-3 rounded-xl border border-zr bg-zr-surface space-y-1">
                    <div className="font-semibold text-zr-primary">
                      YouTube / Media Quality Extraction
                    </div>
                    <p className="text-zr-secondary leading-relaxed">
                      Install or update <code className="font-mono">yt-dlp</code>, <code className="font-mono">FFmpeg</code>, and <code className="font-mono">Deno</code> inside{" "}
                      <strong>Options &amp; Settings &rarr; Updates</strong>. For age-restricted or Premium formats, select your browser under <strong>Network &rarr; Cookies</strong>.
                    </p>
                  </div>
                  <div className="p-3 rounded-xl border border-zr bg-zr-surface space-y-1">
                    <div className="font-semibold text-zr-primary">
                      Pause / Resume &amp; Connections
                    </div>
                    <p className="text-zr-secondary leading-relaxed">
                      Direct HTTP downloads save sparse chunk metadata for exact resume. HLS/m3u8 streams preserve partial fragments and continue instead of resetting to 0%.
                    </p>
                  </div>
                  <div className="p-3 rounded-xl border border-zr bg-zr-surface space-y-1">
                    <div className="font-semibold text-zr-primary">
                      Download Failure Diagnostics
                    </div>
                    <p className="text-zr-secondary leading-relaxed">
                      Every failed download saves HTTP status codes, yt-dlp/aria2c/ffmpeg error lines, and timestamps to{" "}
                      <code className="font-mono">zr_error_logs.log</code>. Open the <strong>Failure Logs</strong> tab above anytime to inspect or copy them.
                    </p>
                  </div>
                  <div className="p-3 rounded-xl border border-zr bg-zr-surface space-y-1">
                    <div className="font-semibold text-zr-primary flex items-center gap-1.5">
                      <Keyboard className="w-3.5 h-3.5 text-blue-500" />
                      <span>Keyboard Shortcuts</span>
                    </div>
                    <ul className="text-zr-secondary space-y-0.5">
                      <li><code className="font-mono">Ctrl + V</code> — Paste URL into Add Download</li>
                      <li><code className="font-mono">Space</code> — Pause / Resume selected download</li>
                      <li><code className="font-mono">Enter</code> — Open completed file</li>
                      <li><code className="font-mono">Delete</code> — Delete selected task</li>
                    </ul>
                  </div>
                </div>
              </div>
            </>
          ) : (
            <div className="space-y-4">
              {/* Top Controls */}
              <div className="flex flex-wrap items-center justify-between gap-2">
                <p className="text-[12px] text-zr-secondary">
                  Detailed failure reasons, exit codes, and engine output captured when any download fails.
                </p>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={loadLogs}
                    className="px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-[12px] font-medium flex items-center gap-1.5"
                  >
                    <RefreshCw className={`w-3.5 h-3.5 ${loadingLogs ? "animate-spin" : ""}`} />
                    <span>Refresh</span>
                  </button>
                  <button
                    type="button"
                    onClick={handleCopyAllLogs}
                    className="px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-[12px] font-medium flex items-center gap-1.5"
                  >
                    {copiedLogs ? (
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                    ) : (
                      <Copy className="w-3.5 h-3.5" />
                    )}
                    <span>{copiedLogs ? "Copied" : "Copy All"}</span>
                  </button>
                  <button
                    type="button"
                    onClick={handleClearLogs}
                    className="px-2.5 py-1.5 rounded-lg border border-red-500/30 bg-red-500/10 hover:bg-red-500/20 text-red-600 dark:text-red-400 text-[12px] font-medium flex items-center gap-1.5"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    <span>Clear Log File</span>
                  </button>
                </div>
              </div>

              {/* Focused or Session Failed Tasks */}
              {focusedTask && (
                <div className="p-3.5 rounded-xl border border-red-500/35 bg-red-500/10 space-y-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-[12.5px] font-bold text-red-600 dark:text-red-400 truncate">
                      Selected Task Failure: {focusedTask.filename}
                    </span>
                    <span className="text-[11px] font-mono text-zr-secondary shrink-0">
                      {focusedTask.id}
                    </span>
                  </div>
                  <p className="text-[12px] font-medium text-zr-primary break-words">
                    Reason: {focusedTask.errorMessage || focusedTask.error || "Unknown error"}
                  </p>
                  {(focusedTask.errorLog || focusedTask.error_log || []).length > 0 && (
                    <pre className="p-2.5 rounded-lg bg-black/20 dark:bg-black/45 text-[11px] font-mono text-zr-primary overflow-x-auto max-h-40 overflow-y-auto whitespace-pre-wrap break-all">
                      {(focusedTask.errorLog || focusedTask.error_log || []).join("\n")}
                    </pre>
                  )}
                </div>
              )}

              {failedTasks.length > 0 && (
                <div className="space-y-2">
                  <h4 className="text-[12px] font-bold uppercase tracking-wider text-zr-secondary">
                    Failed Tasks in Current List ({failedTasks.length})
                  </h4>
                  <div className="space-y-2 max-h-56 overflow-y-auto pr-1">
                    {failedTasks.map((t) => {
                      const lines = t.errorLog || t.error_log || [];
                      return (
                        <div
                          key={t.id}
                          className="p-3 rounded-xl border border-zr bg-zr-surface space-y-1.5"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-[12.5px] font-semibold text-zr-primary truncate">
                              {t.filename}
                            </span>
                            <span className="text-[11px] text-red-600 dark:text-red-400 font-semibold shrink-0">
                              ERROR
                            </span>
                          </div>
                          <div className="text-[11.5px] text-red-600 dark:text-red-400 break-words">
                            {t.errorMessage || t.error || "Download failed"}
                          </div>
                          {lines.length > 0 && (
                            <pre className="p-2 rounded bg-slate-900/90 text-slate-100 text-[10.5px] font-mono overflow-x-auto max-h-28 overflow-y-auto whitespace-pre-wrap break-all">
                              {lines.join("\n")}
                            </pre>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Persistent Disk Log */}
              <div className="space-y-1.5">
                <h4 className="text-[12px] font-bold uppercase tracking-wider text-zr-secondary">
                  Persistent History (zr_error_logs.log)
                </h4>
                <pre className="p-3 rounded-xl border border-zr bg-slate-950 text-slate-200 text-[11px] font-mono overflow-x-auto max-h-64 overflow-y-auto whitespace-pre-wrap break-all leading-relaxed">
                  {logText.trim()
                    ? logText
                    : "No failure logs recorded yet. When a download fails, the exact reason and engine output will appear here."}
                </pre>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
