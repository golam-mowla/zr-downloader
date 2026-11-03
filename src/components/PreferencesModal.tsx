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

  // Core Tools Info
  const [tools, setTools] = useState<ToolVersionInfo[]>([]);
  const [checkingTools, setCheckingTools] = useState(false);

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
      await invoke("update_tool", { tool_name: name, name });
      await loadToolsInfo();
    } catch (err) {
      console.error("Update error:", err);
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
      <div className="w-full max-w-xl rounded-xl border border-gdm bg-gdm-surface shadow-2xl overflow-hidden transition-colors">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-gdm bg-gdm-tableHead transition-colors">
          <div className="flex items-center space-x-2.5">
            <Settings className="w-5 h-5 text-blue-600 dark:text-blue-400" />
            <h2 className="font-semibold text-[14px] text-gdm-primary">
              GDM Options & Configuration
            </h2>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-gdm-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="flex border-b border-gdm bg-gdm-surface px-5 pt-2.5 space-x-6 transition-colors">
          <button
            onClick={() => setActiveTab("general")}
            className={`pb-2.5 text-[13px] font-semibold flex items-center space-x-1.5 border-b-2 transition-all ${
              activeTab === "general"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-gdm-secondary hover:text-gdm-primary"
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
                : "border-transparent text-gdm-secondary hover:text-gdm-primary"
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
                : "border-transparent text-gdm-secondary hover:text-gdm-primary"
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
                <label className="text-[13px] font-medium text-gdm-secondary">
                  Default Download Directory
                </label>
                <div className="flex space-x-2">
                  <input
                    type="text"
                    value={savePath}
                    readOnly
                    className="flex-1 px-3 py-2 rounded-lg font-mono text-[13px] border border-gdm bg-gdm-surface text-gdm-primary"
                  />
                  <button
                    onClick={handleBrowseFolder}
                    className="px-4 py-2 rounded-lg border border-gdm bg-gdm-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-gdm-secondary hover:text-gdm-primary font-semibold text-[13px] flex items-center space-x-1.5 transition-colors"
                  >
                    <Folder className="w-4 h-4 text-gdm-accent" />
                    <span>Browse</span>
                  </button>
                </div>
              </div>

              {/* Browser Integration Info */}
              <div className="p-4 rounded-lg bg-slate-50 dark:bg-slate-900/40 border border-gdm space-y-2">
                <div className="flex items-center space-x-2 text-gdm-accent font-semibold text-[13px]">
                  <Globe className="w-4 h-4" />
                  <span>Browser Integration & Bridge</span>
                </div>
                <p className="text-gdm-secondary text-[12px] leading-relaxed">
                  ZR Downloader listens on <b>http://127.0.0.1:9666</b> and handles the custom protocol <b>gdm://</b>.
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
                  <label className="text-[13px] font-medium text-gdm-secondary">
                    Default Simultaneous Chunk Connections
                  </label>
                  <span className="font-semibold tabular-nums text-gdm-accent text-[13px]">
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
                          : "bg-gdm-surface border-gdm text-gdm-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-gdm-primary"
                      }`}
                    >
                      {num} Chunks
                    </button>
                  ))}
                </div>
              </div>

              {/* Max Concurrent Downloads */}
              <div className="space-y-1.5 pt-2 border-t border-gdm">
                <div className="flex justify-between">
                  <label className="text-[13px] font-medium text-gdm-secondary">
                    Maximum Active Concurrent Tasks
                  </label>
                  <span className="font-semibold tabular-nums text-gdm-accent text-[13px]">
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
                          : "bg-gdm-surface border-gdm text-gdm-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-gdm-primary"
                      }`}
                    >
                      {num} Downloads
                    </button>
                  ))}
                </div>
              </div>

              {/* Speed Limiter */}
              <div className="space-y-2 pt-2 border-t border-gdm">
                <div className="flex items-center justify-between">
                  <label className="text-[13px] font-medium text-gdm-secondary">
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
                      className="w-32 px-3 py-1.5 rounded-lg border border-gdm bg-gdm-surface text-gdm-primary text-[13px] font-mono"
                    />
                    <span className="text-gdm-secondary text-[13px]">
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
                <span className="text-[13px] font-medium text-gdm-secondary">
                  Core Media Engine Binaries
                </span>
                <button
                  onClick={loadToolsInfo}
                  disabled={checkingTools}
                  className="flex items-center space-x-1.5 text-gdm-accent hover:underline text-[12px] font-semibold"
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${checkingTools ? "animate-spin" : ""}`} />
                  <span>Check Updates</span>
                </button>
              </div>

              {tools.length === 0 ? (
                <div className="p-4 text-gdm-muted text-[13px] text-center">
                  Checking installed tools...
                </div>
              ) : (
                <div className="space-y-2">
                  {tools.map((tool) => (
                    <div
                      key={tool.name}
                      className="p-3 rounded-lg border border-gdm bg-slate-50 dark:bg-slate-900/40 flex items-center justify-between"
                    >
                      <div>
                        <div className="font-semibold text-[14px] text-gdm-primary">
                          {tool.name}
                        </div>
                        <div className="text-[12px] text-gdm-secondary font-mono">
                          Installed: {tool.installedVersion || "Bundled"} &bull; Latest: {tool.latestVersion || "Up to date"}
                        </div>
                      </div>
                      <div className="flex items-center space-x-2">
                        {tool.hasUpdate ? (
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
                          {tool.isUpdating ? "Updating..." : "Update"}
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end px-6 py-3.5 border-t border-gdm bg-gdm-tableHead">
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
