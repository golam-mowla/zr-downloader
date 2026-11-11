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
import { open } from "@tauri-apps/plugin-dialog";
import { ToolVersionInfo } from "../types";

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

  useEffect(() => {
    setSavePath(defaultDir);
  }, [defaultDir]);

  useEffect(() => {
    if (isOpen) {
      loadToolsInfo();
    }
  }, [isOpen]);

  const loadToolsInfo = async () => {
    try {
      setCheckingTools(true);
      const res = await invoke<ToolVersionInfo[]>("check_tool_updates");
      if (Array.isArray(res)) setTools(res);
    } catch {
      // fallback
    } finally {
      setCheckingTools(false);
    }
  };

  const handleUpdateTool = async (name: string) => {
    try {
      setTools((prev) =>
        prev.map((t) => (t.name === name ? { ...t, isUpdating: true } : t))
      );
      setToolMessage(null);
      const msg = await invoke<string>("update_tool", { toolName: name, name });
      setToolMessage({ ok: true, text: String(msg || `${name} is up to date`) });
      await loadToolsInfo();
    } catch (err: any) {
      console.error("Update error:", err);
      setToolMessage({ ok: false, text: String(err?.message || err || `${name} update failed`) });
    } finally {
      setTools((prev) =>
        prev.map((t) => (t.name === name ? { ...t, isUpdating: false } : t))
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
                  onClick={loadToolsInfo}
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
                    return (
                    <div
                      key={tool.name}
                      className="p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40 flex items-center justify-between"
                    >
                      <div>
                        <div className="font-semibold text-[14px] text-zr-primary">
                          {tool.name}
                        </div>
                        <div className="text-[12px] text-zr-secondary font-mono">
                          Installed: {tool.installedVersion || "Bundled"} &bull; Latest: {tool.latestVersion || "Up to date"}
                        </div>
                      </div>
                      <div className="flex items-center space-x-2">
                        {missing ? (
                          <span className="text-red-500 text-[11.5px] font-semibold">Not Installed</span>
                        ) : tool.hasUpdate ? (
                          <span className="text-amber-500 text-[11.5px] font-semibold">Update Available</span>
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
                          {tool.isUpdating ? (missing ? "Installing..." : "Updating...") : missing ? "Install" : "Update"}
                        </button>
                      </div>
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

