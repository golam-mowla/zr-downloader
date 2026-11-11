import React, { useState, useEffect } from "react";
import {
  X,
  Settings,
  Folder,
  RefreshCw,
  CheckCircle2,
  HardDrive,
  Cpu,
  Zap,
  Globe,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { ToolVersionInfo } from "../types";

const TOOLS_CACHE_TTL_MS = 10 * 60 * 1000;
let lastChecked = 0;
let cachedTools: ToolVersionInfo[] = [];

interface EngineProgressPayload {
  tool: string;
  stage: "downloading" | "extracting" | "done" | "error";
  percent?: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speedBps?: number;
  message?: string;
}

const matchesToolName = (a: string, b: string) => {
  const la = a.toLowerCase();
  const lb = b.toLowerCase();
  return (
    la === lb ||
    (la === "aria2" && lb === "aria2c") ||
    (la === "aria2c" && lb === "aria2")
  );
};

const formatBytesPair = (downloaded?: number, total?: number) => {
  const dlMb = ((downloaded ?? 0) / (1024 * 1024)).toFixed(1);
  if (total && total > 0) {
    const totVal = total / (1024 * 1024);
    const totMb = totVal >= 100 ? totVal.toFixed(0) : totVal.toFixed(1);
    return `${dlMb} / ${totMb} MB`;
  }
  return `${dlMb} MB`;
};

interface PreferencesModalProps {
  isOpen: boolean;
  onClose: () => void;
  defaultDir: string;
  onChangeDefaultDir: (dir: string) => void;
  theme?: "dark" | "light";
}

export const PreferencesModal: React.FC<PreferencesModalProps> = ({
  isOpen,
  onClose,
  defaultDir,
  onChangeDefaultDir,
}) => {
  const [activeTab, setActiveTab] = useState<"general" | "connection" | "tools">("general");
  const [savePath, setSavePath] = useState(defaultDir);
  const [maxConcurrent, setMaxConcurrent] = useState(3);
  const [defaultConnections, setDefaultConnections] = useState(8);
  const [speedLimitEnabled, setSpeedLimitEnabled] = useState(false);
  const [speedLimitKb, setSpeedLimitKb] = useState(5120); // 5 MB/s
  const [lowDiskSpace, setLowDiskSpace] = useState(() => {
    return localStorage.getItem("zr_low_disk_space") === "true";
  });

  // Core Tools Info
  const [tools, setTools] = useState<ToolVersionInfo[]>([]);
  const [checkingTools, setCheckingTools] = useState(false);
  const [toolMessage, setToolMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [toolErrors, setToolErrors] = useState<Record<string, string>>({});

  useEffect(() => {
    setSavePath(defaultDir);
  }, [defaultDir]);

  useEffect(() => {
    if (isOpen) {
      loadToolsInfo(false);
    }
  }, [isOpen]);

  useEffect(() => {
    let unlistenFn: (() => void) | undefined;
    let disposed = false;

    listen<EngineProgressPayload>("engine-progress", (event) => {
      if (disposed) return;
      const p = event.payload;
      if (!p || !p.tool) return;

      const isActive = p.stage === "downloading" || p.stage === "extracting";

      setTools((prev) => {
        const next = prev.map((t) => {
          if (!matchesToolName(t.name, p.tool)) return t;
          return {
            ...t,
            isUpdating: isActive,
            progress: typeof p.percent === "number" ? p.percent : t.progress,
            downloadedBytes:
              typeof p.downloadedBytes === "number" ? p.downloadedBytes : t.downloadedBytes,
            totalBytes:
              typeof p.totalBytes === "number" ? p.totalBytes : t.totalBytes,
            speedBps: typeof p.speedBps === "number" ? p.speedBps : t.speedBps,
            stage: p.stage,
          };
        });
        cachedTools = next;
        return next;
      });

      if (p.stage === "downloading" || p.stage === "extracting") {
        setToolErrors((prev) => {
          const next = { ...prev };
          for (const key of Object.keys(next)) {
            if (matchesToolName(key, p.tool)) delete next[key];
          }
          return next;
        });
      } else if (p.stage === "done") {
        lastChecked = 0;
        if (p.message) {
          setToolMessage({ ok: true, text: p.message });
        }
        invoke<ToolVersionInfo[]>("get_installed_tools")
          .then((fresh) => {
            if (disposed || !Array.isArray(fresh)) return;
            setTools((prev) => {
              const merged = fresh.map((item) => {
                const old = prev.find((o) => matchesToolName(o.name, item.name));
                const isTarget = matchesToolName(item.name, p.tool);
                return {
                  ...item,
                  latestVersion: old?.latestVersion || item.latestVersion,
                  hasUpdate: isTarget ? false : Boolean(old?.hasUpdate),
                  isUpdating: false,
                  stage: isTarget ? "done" : old?.stage,
                };
              });
              cachedTools = merged;
              return merged;
            });
          })
          .catch(() => {});
      } else if (p.stage === "error") {
        const errText = p.message || `${p.tool} installation failed`;
        setToolMessage({ ok: false, text: errText });
        setToolErrors((prev) => ({ ...prev, [p.tool.toLowerCase()]: errText }));
      }
    })
      .then((fn) => {
        if (disposed) {
          fn();
        } else {
          unlistenFn = fn;
        }
      })
      .catch(() => {});

    return () => {
      disposed = true;
      if (unlistenFn) unlistenFn();
    };
  }, []);

  const mergeToolUpdates = (prev: ToolVersionInfo[], incoming: ToolVersionInfo[]) => {
    const merged = incoming.map((item) => {
      const existing = prev.find((o) => matchesToolName(o.name, item.name));
      const active =
        existing?.isUpdating ||
        existing?.stage === "downloading" ||
        existing?.stage === "extracting";
      return {
        ...item,
        latestVersion: item.latestVersion || existing?.latestVersion || "",
        hasUpdate: item.hasUpdate ?? existing?.hasUpdate ?? false,
        isUpdating: active ? existing?.isUpdating : false,
        progress: active ? existing?.progress : undefined,
        downloadedBytes: active ? existing?.downloadedBytes : undefined,
        totalBytes: active ? existing?.totalBytes : undefined,
        speedBps: active ? existing?.speedBps : undefined,
        stage: active ? existing?.stage : existing?.stage,
      };
    });
    return merged;
  };

  const loadToolsInfo = async (force = false) => {
    const now = Date.now();
    if (!force && cachedTools.length > 0 && now - lastChecked < TOOLS_CACHE_TTL_MS) {
      setTools((prev) => (prev.length > 0 ? prev : cachedTools));
      return;
    }

    try {
      setCheckingTools(true);
      const localRes = await invoke<ToolVersionInfo[]>("get_installed_tools");
      if (Array.isArray(localRes)) {
        setTools((prev) => {
          const merged = mergeToolUpdates(prev, localRes);
          cachedTools = merged;
          return merged;
        });
      }

      const remoteRes = await invoke<ToolVersionInfo[]>("check_tool_updates");
      if (Array.isArray(remoteRes)) {
        lastChecked = Date.now();
        setTools((prev) => {
          const merged = mergeToolUpdates(prev, remoteRes);
          cachedTools = merged;
          return merged;
        });
      }
    } catch {
      // fallback
    } finally {
      setCheckingTools(false);
    }
  };

  const handleUpdateTool = async (name: string) => {
    try {
      setToolErrors((prev) => {
        const next = { ...prev };
        for (const key of Object.keys(next)) {
          if (matchesToolName(key, name)) delete next[key];
        }
        return next;
      });
      setTools((prev) =>
        prev.map((t) =>
          t.name === name
            ? {
                ...t,
                isUpdating: true,
                progress: 0,
                downloadedBytes: 0,
                totalBytes: 0,
                speedBps: 0,
                stage: "downloading",
              }
            : t
        )
      );
      setToolMessage(null);
      const msg = await invoke<string>("update_tool", { toolName: name, name });
      lastChecked = 0;
      setToolMessage({ ok: true, text: String(msg || `${name} is up to date`) });
      const fresh = await invoke<ToolVersionInfo[]>("get_installed_tools").catch(() => null);
      if (Array.isArray(fresh)) {
        setTools((prev) => {
          const merged = mergeToolUpdates(prev, fresh).map((item) =>
            matchesToolName(item.name, name)
              ? { ...item, isUpdating: false, hasUpdate: false, stage: "done" }
              : item
          );
          cachedTools = merged;
          return merged;
        });
      }
    } catch (err: any) {
      console.error("Update error:", err);
      const errText = String(err?.message || err || `${name} update failed`);
      setToolMessage({ ok: false, text: errText });
      setToolErrors((prev) => ({ ...prev, [name.toLowerCase()]: errText }));
      setTools((prev) =>
        prev.map((t) =>
          t.name === name ? { ...t, isUpdating: false, stage: "error" } : t
        )
      );
    }
  };

  if (!isOpen) return null;

  const handleBrowseFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: savePath || undefined,
      });
      if (selected && typeof selected === "string") {
        setSavePath(selected);
        onChangeDefaultDir(selected);
      }
    } catch (err) {
      console.error("Folder select error:", err);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 select-none">
      <div className="w-full max-w-xl rounded-xl border border-zr bg-zr-surface shadow-2xl overflow-hidden transition-colors">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-zr bg-zr-tableHead transition-colors">
          <div className="flex items-center space-x-2.5">
            <Settings className="w-5 h-5 text-blue-600 dark:text-blue-400" />
            <h2 className="font-semibold text-[14px] text-zr-primary">
              ZR Downloader Options & Configuration
            </h2>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="flex border-b border-zr bg-zr-surface px-5 pt-2.5 space-x-6 transition-colors">
          <button
            onClick={() => setActiveTab("general")}
            className={`pb-2.5 text-[13px] font-semibold flex items-center space-x-1.5 border-b-2 transition-all ${
              activeTab === "general"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <HardDrive className="w-4 h-4" />
            <span>General & Storage</span>
          </button>

          <button
            onClick={() => setActiveTab("connection")}
            className={`pb-2.5 text-[13px] font-semibold flex items-center space-x-1.5 border-b-2 transition-all ${
              activeTab === "connection"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Cpu className="w-4 h-4" />
            <span>Connection & Multi-Threading</span>
          </button>

          <button
            onClick={() => setActiveTab("tools")}
            className={`pb-2.5 text-[13px] font-semibold flex items-center space-x-1.5 border-b-2 transition-all ${
              activeTab === "tools"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Zap className="w-4 h-4" />
            <span>Core Engines</span>
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-4 max-h-[65vh] overflow-y-auto">
          {activeTab === "general" && (
            <div className="space-y-4">
              {/* Default Downloads Directory */}
              <div className="space-y-1.5">
                <label className="text-[13px] font-medium text-zr-secondary">
                  Default Download Directory
                </label>
                <div className="flex space-x-2">
                  <input
                    type="text"
                    value={savePath}
                    readOnly
                    className="flex-1 px-3 py-2 rounded-lg font-mono text-[13px] border border-zr bg-zr-surface text-zr-primary"
                  />
                  <button
                    onClick={handleBrowseFolder}
                    className="px-4 py-2 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-zr-secondary hover:text-zr-primary font-semibold text-[13px] flex items-center space-x-1.5 transition-colors"
                  >
                    <Folder className="w-4 h-4 text-zr-accent" />
                    <span>Browse</span>
                  </button>
                </div>
              </div>

              {/* Low Disk Space Optimization Mode */}
              <div className="p-4 rounded-xl border border-blue-500/20 bg-blue-500/5 dark:bg-blue-900/10 space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2">
                    <HardDrive className="w-4 h-4 text-blue-500" />
                    <span className="font-semibold text-[13px] text-zr-primary">
                      Low Disk Space Mode (Zero-Temp In-Place Direct Write)
                    </span>
                  </div>
                  <input
                    type="checkbox"
                    checked={lowDiskSpace}
                    onChange={(e) => {
                      setLowDiskSpace(e.target.checked);
                      localStorage.setItem("zr_low_disk_space", String(e.target.checked));
                    }}
                    className="w-4 h-4 rounded text-blue-600 focus:ring-blue-500 cursor-pointer"
                  />
                </div>
                <p className="text-zr-secondary text-[12px] leading-relaxed">
                  Saves 50% SSD space on large downloads (e.g. 20 GB file takes strictly 20 GB instead of 40 GB). Chunks write directly into the final destination file without creating duplicate temporary segment files or assembly delay. Ideal for small SSDs (e.g. 220 GB SSD).
                </p>
              </div>

              {/* Browser Integration Info */}
              <div className="p-4 rounded-lg bg-slate-50 dark:bg-slate-900/40 border border-zr space-y-2">
                <div className="flex items-center space-x-2 text-zr-accent font-semibold text-[13px]">
                  <Globe className="w-4 h-4" />
                  <span>Browser Integration & Bridge</span>
                </div>
                <p className="text-zr-secondary text-[12px] leading-relaxed">
                  ZR Downloader listens on <b>http://127.0.0.1:9666</b> and handles the custom protocol <b>zr://</b>.
                  Standard browser downloads (.zip, .exe, .iso, .pdf, .mp4) are caught automatically when using the ZR Downloader extension.
                </p>
              </div>
            </div>
          )}

          {activeTab === "connection" && (
            <div className="space-y-4">
              {/* Connections / Threads per download */}
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <label className="text-[13px] font-medium text-zr-secondary">
                    Default Simultaneous Chunk Connections
                  </label>
                  <span className="font-semibold tabular-nums text-zr-accent text-[13px]">
                    {defaultConnections} Threads
                  </span>
                </div>
                <div className="grid grid-cols-4 gap-2 pt-1">
                  {[4, 8, 12, 16].map((num) => (
                    <button
                      key={num}
                      type="button"
                      onClick={() => setDefaultConnections(num)}
                      className={`py-2 rounded-lg font-semibold text-[13px] border transition-all ${
                        defaultConnections === num
                          ? "bg-blue-600 border-blue-600 text-white"
                          : "bg-zr-surface border-zr text-zr-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-zr-primary"
                      }`}
                    >
                      {num} Chunks
                    </button>
                  ))}
                </div>
              </div>

              {/* Max Concurrent Downloads */}
              <div className="space-y-1.5 pt-2 border-t border-zr">
                <div className="flex justify-between">
                  <label className="text-[13px] font-medium text-zr-secondary">
                    Maximum Active Concurrent Tasks
                  </label>
                  <span className="font-semibold tabular-nums text-zr-accent text-[13px]">
                    {maxConcurrent} Active
                  </span>
                </div>
                <div className="grid grid-cols-4 gap-2 pt-1">
                  {[1, 2, 3, 5].map((num) => (
                    <button
                      key={num}
                      type="button"
                      onClick={() => setMaxConcurrent(num)}
                      className={`py-2 rounded-lg font-semibold text-[13px] border transition-all ${
                        maxConcurrent === num
                          ? "bg-blue-600 border-blue-600 text-white"
                          : "bg-zr-surface border-zr text-zr-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-zr-primary"
                      }`}
                    >
                      {num} Downloads
                    </button>
                  ))}
                </div>
              </div>

              {/* Speed Limiter */}
              <div className="space-y-2 pt-2 border-t border-zr">
                <div className="flex items-center justify-between">
                  <label className="text-[13px] font-medium text-zr-secondary">
                    Global Bandwidth Limiter
                  </label>
                  <input
                    type="checkbox"
                    checked={speedLimitEnabled}
                    onChange={(e) => setSpeedLimitEnabled(e.target.checked)}
                    className="w-4 h-4 accent-blue-600 rounded"
                  />
                </div>
                {speedLimitEnabled && (
                  <div className="flex items-center space-x-2">
                    <input
                      type="number"
                      value={speedLimitKb}
                      onChange={(e) => setSpeedLimitKb(parseInt(e.target.value) || 0)}
                      className="w-32 px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[13px] font-mono"
                    />
                    <span className="text-zr-secondary text-[13px]">
                      KB/s ({(speedLimitKb / 1024).toFixed(1)} MB/s)
                    </span>
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === "tools" && (
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-medium text-zr-secondary">
                  Core Media Engine Binaries
                </span>
                <button
                  onClick={() => loadToolsInfo(true)}
                  disabled={checkingTools}
                  className="flex items-center space-x-1.5 text-zr-accent hover:underline text-[12px] font-semibold"
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${checkingTools ? "animate-spin" : ""}`} />
                  <span>Check Updates</span>
                </button>
              </div>

              {tools.length === 0 ? (
                <div className="p-4 text-zr-muted text-[13px] text-center">
                  Checking installed tools...
                </div>
              ) : (
                <div className="space-y-2">
                  {toolMessage && (
                    <div
                      className={`p-2.5 rounded-lg text-[12px] font-medium ${
                        toolMessage.ok
                          ? "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300"
                          : "bg-red-50 text-red-700 dark:bg-red-900/30 dark:text-red-300"
                      }`}
                    >
                      {toolMessage.text}
                    </div>
                  )}
                  {tools.map((tool) => {
                    const missing = !tool.installedVersion || tool.installedVersion === "Not Found";
                    const showProgress =
                      tool.isUpdating ||
                      tool.stage === "downloading" ||
                      tool.stage === "extracting";
                    const pct = Math.min(100, Math.max(0, tool.progress ?? 0));
                    const speedMb = ((tool.speedBps ?? 0) / (1024 * 1024)).toFixed(1);
                    const errorKey = Object.keys(toolErrors).find((k) =>
                      matchesToolName(k, tool.name)
                    );
                    const rowError = errorKey ? toolErrors[errorKey] : undefined;

                    return (
                      <div
                        key={tool.name}
                        className="p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2"
                      >
                        <div className="flex items-center justify-between">
                          <div>
                            <div className="font-semibold text-[14px] text-zr-primary">
                              {tool.name}
                            </div>
                            <div className="text-[12px] text-zr-secondary font-mono">
                              Installed: {tool.installedVersion || "Bundled"} &bull; Latest:{" "}
                              {tool.latestVersion || (checkingTools ? "Checking..." : "Up to date")}
                            </div>
                          </div>
                          <div className="flex items-center space-x-2">
                            {missing ? (
                              <span className="text-red-500 text-[11.5px] font-semibold">
                                Not Installed
                              </span>
                            ) : tool.hasUpdate ? (
                              <span className="text-amber-500 text-[11.5px] font-semibold">
                                Update Available
                              </span>
                            ) : (
                              <span className="text-emerald-600 dark:text-emerald-400 text-[11.5px] font-semibold flex items-center space-x-1">
                                <CheckCircle2 className="w-3.5 h-3.5" />
                                <span>Active</span>
                              </span>
                            )}
                            <button
                              onClick={() => handleUpdateTool(tool.name)}
                              disabled={tool.isUpdating}
                              className="px-3 py-1 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white font-semibold text-[12px] transition-colors"
                            >
                              {tool.isUpdating
                                ? missing
                                  ? "Installing..."
                                  : "Updating..."
                                : missing
                                ? "Install"
                                : "Update"}
                            </button>
                          </div>
                        </div>

                        {showProgress && (
                          <div className="space-y-1">
                            <div className="flex items-center justify-between text-[11px] text-zr-secondary font-mono">
                              <span>
                                {tool.stage === "extracting"
                                  ? "Extracting..."
                                  : `Downloading... ${pct.toFixed(0)}%`}
                              </span>
                              <span>
                                {formatBytesPair(tool.downloadedBytes, tool.totalBytes)}
                                {tool.stage !== "extracting" && (tool.speedBps ?? 0) > 0
                                  ? ` • ${speedMb} MB/s`
                                  : ""}
                              </span>
                            </div>
                            <div className="w-full h-1.5 rounded-full bg-slate-200 dark:bg-slate-800 overflow-hidden">
                              <div
                                className={`h-full rounded-full transition-all duration-150 ${
                                  tool.stage === "extracting"
                                    ? "bg-amber-500 animate-pulse"
                                    : "bg-blue-600"
                                }`}
                                style={{
                                  width: `${
                                    tool.stage === "extracting" ? 100 : Math.max(2, pct)
                                  }%`,
                                }}
                              />
                            </div>
                          </div>
                        )}

                        {tool.stage === "error" && rowError && (
                          <div className="text-[11.5px] text-red-600 dark:text-red-400 font-medium">
                            {rowError}
                          </div>
                        )}
                      </div>
                    );
                  })}
                  {tools.some((t) => t.isUpdating) && (
                    <div className="text-[11.5px] text-zr-muted text-center">
                      Downloading from the official release — FFmpeg is large (~150 MB), please keep this window open.
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end px-6 py-3.5 border-t border-zr bg-zr-tableHead">
          <button
            onClick={onClose}
            className="px-5 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white font-semibold text-[13px] shadow-xs transition-colors"
          >
            Save & Close
          </button>
        </div>
      </div>
    </div>
  );
};

