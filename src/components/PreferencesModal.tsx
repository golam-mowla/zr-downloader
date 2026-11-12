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
  CalendarClock,
  Shield,
  Download,
  Upload,
  AlertTriangle,
  Plus,
  Trash2,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ToolVersionInfo, AppSettings, SchedulerConfig } from "../types";

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
  onSettingsSaved?: (settings: AppSettings) => void;
  onListImported?: () => void;
  theme?: "dark" | "light";
}

const DEFAULT_APP_SETTINGS: AppSettings = {
  defaultDownloadDir: "",
  maxActiveDownloads: 3,
  connectionMode: "fixed",
  defaultConnections: 8,
  lowDiskSpace: false,
  speedLimitEnabled: false,
  speedLimitValue: 5,
  speedLimitUnit: "MB",
  autoCategorize: false,
  autoCategoryRoot: "",
  postDownloadAction: "none",
  postDownloadProgram: "",
  proxyEnabled: false,
  proxyType: "http",
  proxyHost: "",
  proxyPort: 8080,
  proxyUsername: "",
  proxyPassword: "",
  cookiesBrowser: "None",
  cookiesFile: "",
  siteCredentials: [],
  clipboardMonitor: false,
  startWithWindows: false,
  minimizeToTray: false,
  closeToTray: true,
};

const DEFAULT_SCHEDULER_CONFIG: SchedulerConfig = {
  enabled: false,
  startTime: "00:00",
  stopTime: "06:00",
  days: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
  whenFinished: "none",
};

const ALL_DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

export const PreferencesModal: React.FC<PreferencesModalProps> = ({
  isOpen,
  onClose,
  defaultDir,
  onChangeDefaultDir,
  onSettingsSaved,
  onListImported,
}) => {
  const [activeTab, setActiveTab] = useState<
    "general" | "connection" | "scheduler" | "network" | "tools"
  >("general");
  const [savePath, setSavePath] = useState(defaultDir);
  const [appSettings, setAppSettings] = useState<AppSettings>({
    ...DEFAULT_APP_SETTINGS,
    defaultDownloadDir: defaultDir,
    lowDiskSpace: localStorage.getItem("zr_low_disk_space") === "true",
  });
  const [schedulerCfg, setSchedulerCfg] = useState<SchedulerConfig>(DEFAULT_SCHEDULER_CONFIG);
  const [newCredDomain, setNewCredDomain] = useState("");
  const [newCredUser, setNewCredUser] = useState("");
  const [newCredPass, setNewCredPass] = useState("");
  const [savingSettings, setSavingSettings] = useState(false);
  const [transferMsg, setTransferMsg] = useState<string | null>(null);

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
      setTransferMsg(null);
      invoke<AppSettings>("get_app_settings")
        .then((res) => {
          if (res) {
            setAppSettings({
              ...DEFAULT_APP_SETTINGS,
              ...res,
              defaultDownloadDir: res.defaultDownloadDir || defaultDir,
            });
            if (res.defaultDownloadDir) setSavePath(res.defaultDownloadDir);
          }
        })
        .catch(() => {});
      invoke<SchedulerConfig>("get_scheduler_config")
        .then((res) => {
          if (res) setSchedulerCfg({ ...DEFAULT_SCHEDULER_CONFIG, ...res });
        })
        .catch(() => {});
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
        setAppSettings((prev) => ({ ...prev, defaultDownloadDir: selected }));
        onChangeDefaultDir(selected);
      }
    } catch (err) {
      console.error("Folder select error:", err);
    }
  };

  const handleBrowseCategoryRoot = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: appSettings.autoCategoryRoot || savePath || undefined,
      });
      if (selected && typeof selected === "string") {
        setAppSettings((prev) => ({ ...prev, autoCategoryRoot: selected }));
      }
    } catch (err) {
      console.error("Category root browse error:", err);
    }
  };

  const handleBrowseCookiesFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Cookies Text", extensions: ["txt"] }],
      });
      if (selected && typeof selected === "string") {
        setAppSettings((prev) => ({ ...prev, cookiesFile: selected }));
      }
    } catch (err) {
      console.error("Cookies browse error:", err);
    }
  };

  const handleBrowsePostProgram = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Executables", extensions: ["exe", "bat", "cmd", "ps1"] }],
      });
      if (selected && typeof selected === "string") {
        setAppSettings((prev) => ({ ...prev, postDownloadProgram: selected }));
      }
    } catch {}
  };

  const handleExportList = async () => {
    try {
      const target = await save({
        defaultPath: "zr-downloads-export.json",
        filters: [{ name: "JSON List", extensions: ["json"] }],
      });
      if (target) {
        const count = await invoke<number>("export_download_list", { filePath: target });
        setTransferMsg(`Exported ${count} downloads to JSON.`);
      }
    } catch (e: any) {
      setTransferMsg(`Export failed: ${e?.message || e}`);
    }
  };

  const handleImportList = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON List", extensions: ["json"] }],
      });
      if (selected && typeof selected === "string") {
        const count = await invoke<number>("import_download_list", { filePath: selected });
        setTransferMsg(`Imported ${count} downloads.`);
        onListImported?.();
      }
    } catch (e: any) {
      setTransferMsg(`Import failed: ${e?.message || e}`);
    }
  };

  const handleSaveAndClose = async () => {
    try {
      setSavingSettings(true);
      const payload: AppSettings = {
        ...appSettings,
        defaultDownloadDir: savePath || appSettings.defaultDownloadDir,
      };
      localStorage.setItem("zr_low_disk_space", String(payload.lowDiskSpace));
      const saved = await invoke<AppSettings>("save_app_settings", { newSettings: payload });
      await invoke("save_scheduler_config", { config: schedulerCfg });
      onChangeDefaultDir(saved.defaultDownloadDir || savePath);
      onSettingsSaved?.(saved);
      onClose();
    } catch (err) {
      console.error("Failed to save settings:", err);
      onClose();
    } finally {
      setSavingSettings(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 select-none">
      <div className="w-full max-w-2xl rounded-xl border border-zr bg-zr-surface shadow-2xl overflow-hidden transition-colors">
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
        <div className="flex border-b border-zr bg-zr-surface px-5 pt-2.5 space-x-4 overflow-x-auto transition-colors">
          <button
            onClick={() => setActiveTab("general")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              activeTab === "general"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <HardDrive className="w-4 h-4" />
            <span>General & Files</span>
          </button>

          <button
            onClick={() => setActiveTab("connection")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              activeTab === "connection"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Cpu className="w-4 h-4" />
            <span>Queue & Speed</span>
          </button>

          <button
            onClick={() => setActiveTab("scheduler")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              activeTab === "scheduler"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <CalendarClock className="w-4 h-4" />
            <span>Scheduler</span>
          </button>

          <button
            onClick={() => setActiveTab("network")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              activeTab === "network"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Shield className="w-4 h-4" />
            <span>Network</span>
          </button>

          <button
            onClick={() => setActiveTab("tools")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
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

              {/* Auto Categories (G) */}
              <div className="p-4 rounded-xl border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2.5">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="font-semibold text-[13px] text-zr-primary">
                      Auto-Categorize Completed Files
                    </div>
                    <div className="text-[12px] text-zr-secondary">
                      Move completed files into Videos, Music, Documents, Compressed, or Programs subfolders
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={appSettings.autoCategorize}
                    onChange={(e) =>
                      setAppSettings((prev) => ({ ...prev, autoCategorize: e.target.checked }))
                    }
                    className="w-4 h-4 accent-blue-600 cursor-pointer"
                  />
                </div>
                {appSettings.autoCategorize && (
                  <div className="flex space-x-2 pt-1">
                    <input
                      type="text"
                      value={appSettings.autoCategoryRoot}
                      onChange={(e) =>
                        setAppSettings((prev) => ({ ...prev, autoCategoryRoot: e.target.value }))
                      }
                      placeholder={savePath || "Category root folder"}
                      className="flex-1 px-3 py-1.5 rounded-lg font-mono text-[12px] border border-zr bg-zr-surface text-zr-primary"
                    />
                    <button
                      type="button"
                      onClick={handleBrowseCategoryRoot}
                      className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-[12px] font-semibold text-zr-secondary hover:text-zr-primary"
                    >
                      Browse Root
                    </button>
                  </div>
                )}
              </div>

              {/* Post-Download Action (G) */}
              <div className="p-4 rounded-xl border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2">
                <label className="text-[13px] font-semibold text-zr-primary block">
                  Default Post-Download Action
                </label>
                <select
                  value={appSettings.postDownloadAction}
                  onChange={(e) =>
                    setAppSettings((prev) => ({ ...prev, postDownloadAction: e.target.value }))
                  }
                  className="w-full px-3 py-2 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[13px]"
                >
                  <option value="none">Do Nothing</option>
                  <option value="open_file">Open Downloaded File</option>
                  <option value="open_folder">Open Containing Folder</option>
                  <option value="defender_scan">Antivirus Scan (Windows Defender MpCmdRun)</option>
                  <option value="run_program">Run Custom Program / Script</option>
                </select>
                {appSettings.postDownloadAction === "run_program" && (
                  <div className="flex space-x-2 pt-1">
                    <input
                      type="text"
                      value={appSettings.postDownloadProgram}
                      onChange={(e) =>
                        setAppSettings((prev) => ({ ...prev, postDownloadProgram: e.target.value }))
                      }
                      placeholder="Path to executable (downloaded path passed as first argument)"
                      className="flex-1 px-3 py-1.5 rounded-lg font-mono text-[12px] border border-zr bg-zr-surface text-zr-primary"
                    />
                    <button
                      type="button"
                      onClick={handleBrowsePostProgram}
                      className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-[12px] font-semibold text-zr-secondary hover:text-zr-primary"
                    >
                      Browse
                    </button>
                  </div>
                )}
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
                    checked={appSettings.lowDiskSpace}
                    onChange={(e) =>
                      setAppSettings((prev) => ({ ...prev, lowDiskSpace: e.target.checked }))
                    }
                    className="w-4 h-4 rounded text-blue-600 focus:ring-blue-500 cursor-pointer"
                  />
                </div>
                <p className="text-zr-secondary text-[12px] leading-relaxed">
                  Saves 50% SSD space on large downloads without creating duplicate temporary segment files.
                </p>
              </div>

              {/* App Shell & UX Settings (I, J) */}
              <div className="p-4 rounded-xl border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2.5">
                <div className="text-[13px] font-semibold text-zr-primary">
                  App Shell, Tray & Clipboard
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 text-[12.5px]">
                  <label className="flex items-center justify-between p-2 rounded-lg border border-zr bg-zr-surface cursor-pointer">
                    <span className="text-zr-secondary font-medium">Monitor Clipboard Links</span>
                    <input
                      type="checkbox"
                      checked={appSettings.clipboardMonitor}
                      onChange={(e) =>
                        setAppSettings((p) => ({ ...p, clipboardMonitor: e.target.checked }))
                      }
                      className="w-4 h-4 accent-blue-600"
                    />
                  </label>
                  <label className="flex items-center justify-between p-2 rounded-lg border border-zr bg-zr-surface cursor-pointer">
                    <span className="text-zr-secondary font-medium">Start with Windows</span>
                    <input
                      type="checkbox"
                      checked={appSettings.startWithWindows}
                      onChange={(e) =>
                        setAppSettings((p) => ({ ...p, startWithWindows: e.target.checked }))
                      }
                      className="w-4 h-4 accent-blue-600"
                    />
                  </label>
                  <label className="flex items-center justify-between p-2 rounded-lg border border-zr bg-zr-surface cursor-pointer">
                    <span className="text-zr-secondary font-medium">Minimize to Tray</span>
                    <input
                      type="checkbox"
                      checked={appSettings.minimizeToTray}
                      onChange={(e) =>
                        setAppSettings((p) => ({ ...p, minimizeToTray: e.target.checked }))
                      }
                      className="w-4 h-4 accent-blue-600"
                    />
                  </label>
                  <label className="flex items-center justify-between p-2 rounded-lg border border-zr bg-zr-surface cursor-pointer">
                    <span className="text-zr-secondary font-medium">Close Button Hides to Tray</span>
                    <input
                      type="checkbox"
                      checked={appSettings.closeToTray}
                      onChange={(e) =>
                        setAppSettings((p) => ({ ...p, closeToTray: e.target.checked }))
                      }
                      className="w-4 h-4 accent-blue-600"
                    />
                  </label>
                </div>

                {/* Import / Export Download List (J) */}
                <div className="flex items-center justify-between pt-2 border-t border-zr">
                  <span className="text-[12px] text-zr-secondary">
                    Backup or restore download queue (JSON)
                  </span>
                  <div className="flex items-center space-x-2">
                    <button
                      type="button"
                      onClick={handleImportList}
                      className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-[12px] font-semibold text-zr-primary flex items-center space-x-1"
                    >
                      <Upload className="w-3.5 h-3.5" />
                      <span>Import List</span>
                    </button>
                    <button
                      type="button"
                      onClick={handleExportList}
                      className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-[12px] font-semibold text-zr-primary flex items-center space-x-1"
                    >
                      <Download className="w-3.5 h-3.5" />
                      <span>Export List</span>
                    </button>
                  </div>
                </div>
                {transferMsg && (
                  <div className="text-[12px] text-emerald-600 dark:text-emerald-400 font-medium">
                    {transferMsg}
                  </div>
                )}
              </div>

              {/* Browser Integration Info */}
              <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-900/40 border border-zr flex items-center space-x-2 text-[12px] text-zr-secondary">
                <Globe className="w-4 h-4 text-zr-accent shrink-0" />
                <span>
                  Listening on <b>http://127.0.0.1:9666</b> &amp; <b>zr://</b> protocol handler.
                </span>
              </div>
            </div>
          )}

          {activeTab === "connection" && (
            <div className="space-y-5">
              {/* Connections / Threads per download: 4 / 8 / 16 / 24 / 32 + Auto */}
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <label className="text-[13px] font-semibold text-zr-primary">
                    Connections per Download
                  </label>
                  <span className="font-semibold tabular-nums text-zr-accent text-[13px]">
                    {appSettings.connectionMode === "auto"
                      ? "Auto (8 -> 32)"
                      : `${appSettings.defaultConnections} Connections`}
                  </span>
                </div>
                <div className="grid grid-cols-6 gap-2">
                  {[4, 8, 16, 24, 32].map((num) => (
                    <button
                      key={num}
                      type="button"
                      title={num > 16 ? "some servers may block" : `${num} connections`}
                      onClick={() =>
                        setAppSettings((p) => ({
                          ...p,
                          connectionMode: "fixed",
                          defaultConnections: num,
                        }))
                      }
                      className={`py-2 rounded-lg font-semibold text-[12.5px] border transition-all ${
                        appSettings.connectionMode === "fixed" &&
                        appSettings.defaultConnections === num
                          ? "bg-blue-600 border-blue-600 text-white"
                          : "bg-zr-surface border-zr text-zr-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-zr-primary"
                      }`}
                    >
                      {num}
                    </button>
                  ))}
                  <button
                    type="button"
                    title="Start with 8, add +4 every 5s while total speed grows >10%, cap 32"
                    onClick={() =>
                      setAppSettings((p) => ({
                        ...p,
                        connectionMode: "auto",
                        defaultConnections: 8,
                      }))
                    }
                    className={`py-2 rounded-lg font-semibold text-[12.5px] border transition-all ${
                      appSettings.connectionMode === "auto"
                        ? "bg-blue-600 border-blue-600 text-white"
                        : "bg-zr-surface border-zr text-zr-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-zr-primary"
                    }`}
                  >
                    Auto
                  </button>
                </div>
                {(appSettings.connectionMode === "auto" || appSettings.defaultConnections > 16) && (
                  <div
                    className="flex items-center space-x-2 p-2.5 rounded-lg bg-amber-500/10 border border-amber-500/30 text-[12px] text-amber-700 dark:text-amber-300"
                    title="some servers may block"
                  >
                    <AlertTriangle className="w-4 h-4 shrink-0" />
                    <span>
                      Using more than 16 connections: some servers may block or throttle excessive parallel requests. aria2c stays capped at 16 per server.
                    </span>
                  </div>
                )}
              </div>

              {/* Queue Manager: max_active_downloads 1..10 (applied to HTTP + media together) */}
              <div className="space-y-2 pt-3 border-t border-zr">
                <div className="flex justify-between items-center">
                  <div>
                    <label className="text-[13px] font-semibold text-zr-primary block">
                      Maximum Concurrent Downloads (Queue Limit: 1–10)
                    </label>
                    <span className="text-[11.5px] text-zr-secondary">
                      Applies across both direct HTTP and video/media downloads together.
                    </span>
                  </div>
                  <span className="font-semibold tabular-nums text-zr-accent text-[14px]">
                    {appSettings.maxActiveDownloads}
                  </span>
                </div>
                <input
                  type="range"
                  min={1}
                  max={10}
                  value={appSettings.maxActiveDownloads}
                  onChange={(e) =>
                    setAppSettings((p) => ({
                      ...p,
                      maxActiveDownloads: parseInt(e.target.value, 10) || 1,
                    }))
                  }
                  className="w-full accent-blue-600 cursor-pointer"
                />
                <div className="flex justify-between text-[11px] text-zr-muted px-0.5 tabular-nums">
                  {[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((n) => (
                    <span key={n}>{n}</span>
                  ))}
                </div>
              </div>

              {/* Global Speed Limiter (KB/s or MB/s) */}
              <div className="space-y-2.5 pt-3 border-t border-zr">
                <div className="flex items-center justify-between">
                  <div>
                    <label className="text-[13px] font-semibold text-zr-primary block">
                      Global Speed Limiter
                    </label>
                    <span className="text-[11.5px] text-zr-secondary">
                      Applies to HTTP token-bucket, yt-dlp (--limit-rate), and aria2c (--max-overall-download-limit).
                    </span>
                  </div>
                  <input
                    type="checkbox"
                    checked={appSettings.speedLimitEnabled}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, speedLimitEnabled: e.target.checked }))
                    }
                    className="w-4 h-4 accent-blue-600 rounded cursor-pointer"
                  />
                </div>
                <div className="flex items-center space-x-2">
                  <input
                    type="number"
                    min={1}
                    value={appSettings.speedLimitValue}
                    onChange={(e) =>
                      setAppSettings((p) => ({
                        ...p,
                        speedLimitValue: Math.max(1, parseInt(e.target.value, 10) || 1),
                      }))
                    }
                    className="w-32 px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[13px] font-mono"
                  />
                  <select
                    value={appSettings.speedLimitUnit}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, speedLimitUnit: e.target.value }))
                    }
                    className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[13px] font-semibold"
                  >
                    <option value="KB">KB/s</option>
                    <option value="MB">MB/s</option>
                  </select>
                </div>
              </div>
            </div>
          )}

          {activeTab === "scheduler" && (
            <div className="space-y-4">
              <div className="p-4 rounded-xl border border-zr bg-slate-50 dark:bg-slate-900/40 flex items-center justify-between">
                <div>
                  <div className="font-semibold text-[13px] text-zr-primary">
                    Enable Download Scheduler
                  </div>
                  <div className="text-[12px] text-zr-secondary">
                    Automatically start queued/paused downloads at Start Time and pause active downloads at Stop Time.
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={schedulerCfg.enabled}
                  onChange={(e) =>
                    setSchedulerCfg((prev) => ({ ...prev, enabled: e.target.checked }))
                  }
                  className="w-4 h-4 accent-blue-600 cursor-pointer"
                />
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-1.5">
                  <label className="text-[12.5px] font-semibold text-zr-secondary">
                    Start Queue At
                  </label>
                  <input
                    type="time"
                    value={schedulerCfg.startTime}
                    onChange={(e) =>
                      setSchedulerCfg((prev) => ({ ...prev, startTime: e.target.value }))
                    }
                    className="w-full px-3 py-2 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[13px]"
                  />
                </div>
                <div className="space-y-1.5">
                  <label className="text-[12.5px] font-semibold text-zr-secondary">
                    Pause Everything At
                  </label>
                  <input
                    type="time"
                    value={schedulerCfg.stopTime}
                    onChange={(e) =>
                      setSchedulerCfg((prev) => ({ ...prev, stopTime: e.target.value }))
                    }
                    className="w-full px-3 py-2 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[13px]"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-[12.5px] font-semibold text-zr-secondary block">
                  Active Days
                </label>
                <div className="grid grid-cols-7 gap-1.5">
                  {ALL_DAYS.map((day) => {
                    const active = schedulerCfg.days.includes(day);
                    return (
                      <button
                        key={day}
                        type="button"
                        onClick={() =>
                          setSchedulerCfg((prev) => ({
                            ...prev,
                            days: active
                              ? prev.days.filter((d) => d !== day)
                              : [...prev.days, day],
                          }))
                        }
                        className={`py-2 rounded-lg text-[12px] font-semibold border transition-colors ${
                          active
                            ? "bg-blue-600 border-blue-600 text-white"
                            : "bg-zr-surface border-zr text-zr-secondary hover:text-zr-primary"
                        }`}
                      >
                        {day}
                      </button>
                    );
                  })}
                </div>
              </div>

              <div className="space-y-1.5 pt-2 border-t border-zr">
                <label className="text-[13px] font-semibold text-zr-primary block">
                  When All Downloads Finish
                </label>
                <select
                  value={schedulerCfg.whenFinished}
                  onChange={(e) =>
                    setSchedulerCfg((prev) => ({ ...prev, whenFinished: e.target.value }))
                  }
                  className="w-full px-3 py-2 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[13px]"
                >
                  <option value="none">Do Nothing</option>
                  <option value="close_app">Close ZR Downloader</option>
                  <option value="sleep">Put Computer to Sleep</option>
                  <option value="shutdown">Shut Down Windows (with 30s cancel dialog)</option>
                </select>
              </div>
            </div>
          )}

          {activeTab === "network" && (
            <div className="space-y-4">
              {/* Proxy Section */}
              <div className="p-4 rounded-xl border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-3">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="font-semibold text-[13px] text-zr-primary">
                      HTTP / HTTPS / SOCKS5 Proxy
                    </div>
                    <div className="text-[12px] text-zr-secondary">
                      Applied to HTTP engine, yt-dlp (--proxy), and aria2c (--all-proxy).
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={appSettings.proxyEnabled}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, proxyEnabled: e.target.checked }))
                    }
                    className="w-4 h-4 accent-blue-600 cursor-pointer"
                  />
                </div>
                <div className="grid grid-cols-12 gap-2">
                  <select
                    value={appSettings.proxyType}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, proxyType: e.target.value }))
                    }
                    className="col-span-3 px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12.5px]"
                  >
                    <option value="http">HTTP</option>
                    <option value="https">HTTPS</option>
                    <option value="socks5">SOCKS5</option>
                  </select>
                  <input
                    type="text"
                    value={appSettings.proxyHost}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, proxyHost: e.target.value }))
                    }
                    placeholder="127.0.0.1"
                    className="col-span-6 px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12.5px] font-mono"
                  />
                  <input
                    type="number"
                    value={appSettings.proxyPort}
                    onChange={(e) =>
                      setAppSettings((p) => ({
                        ...p,
                        proxyPort: parseInt(e.target.value, 10) || 8080,
                      }))
                    }
                    placeholder="8080"
                    className="col-span-3 px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12.5px] font-mono"
                  />
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <input
                    type="text"
                    value={appSettings.proxyUsername}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, proxyUsername: e.target.value }))
                    }
                    placeholder="Proxy Username (optional)"
                    className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12.5px]"
                  />
                  <input
                    type="password"
                    value={appSettings.proxyPassword}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, proxyPassword: e.target.value }))
                    }
                    placeholder="Proxy Password (optional)"
                    className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12.5px]"
                  />
                </div>
              </div>

              {/* Cookies for yt-dlp */}
              <div className="p-4 rounded-xl border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2.5">
                <div className="font-semibold text-[13px] text-zr-primary">
                  Cookies (yt-dlp Private / Authenticated Media)
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <select
                    value={appSettings.cookiesBrowser}
                    onChange={(e) =>
                      setAppSettings((p) => ({ ...p, cookiesBrowser: e.target.value }))
                    }
                    className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12.5px]"
                  >
                    <option value="None">Cookies From Browser: None</option>
                    <option value="Chrome">Chrome</option>
                    <option value="Edge">Edge</option>
                    <option value="Firefox">Firefox</option>
                    <option value="Brave">Brave</option>
                    <option value="Opera">Opera</option>
                  </select>
                  <div className="flex space-x-1.5">
                    <input
                      type="text"
                      value={appSettings.cookiesFile}
                      onChange={(e) =>
                        setAppSettings((p) => ({ ...p, cookiesFile: e.target.value }))
                      }
                      placeholder="cookies.txt path..."
                      className="flex-1 min-w-0 px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12px] font-mono"
                    />
                    <button
                      type="button"
                      onClick={handleBrowseCookiesFile}
                      className="px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface text-[12px] font-semibold text-zr-secondary hover:text-zr-primary"
                    >
                      File
                    </button>
                  </div>
                </div>
              </div>

              {/* Per-Site HTTP Basic Auth Credentials */}
              <div className="p-4 rounded-xl border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2.5">
                <div className="font-semibold text-[13px] text-zr-primary">
                  Per-Site Basic Auth Credentials
                </div>
                <div className="grid grid-cols-12 gap-2">
                  <input
                    type="text"
                    value={newCredDomain}
                    onChange={(e) => setNewCredDomain(e.target.value)}
                    placeholder="domain.com"
                    className="col-span-4 px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12px]"
                  />
                  <input
                    type="text"
                    value={newCredUser}
                    onChange={(e) => setNewCredUser(e.target.value)}
                    placeholder="Username"
                    className="col-span-3 px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12px]"
                  />
                  <input
                    type="password"
                    value={newCredPass}
                    onChange={(e) => setNewCredPass(e.target.value)}
                    placeholder="Password"
                    className="col-span-3 px-2.5 py-1.5 rounded-lg border border-zr bg-zr-surface text-zr-primary text-[12px]"
                  />
                  <button
                    type="button"
                    onClick={() => {
                      if (!newCredDomain.trim() || !newCredUser.trim()) return;
                      setAppSettings((p) => ({
                        ...p,
                        siteCredentials: [
                          ...p.siteCredentials,
                          {
                            id: `${Date.now()}`,
                            domain: newCredDomain.trim(),
                            username: newCredUser.trim(),
                            password: newCredPass,
                          },
                        ],
                      }));
                      setNewCredDomain("");
                      setNewCredUser("");
                      setNewCredPass("");
                    }}
                    className="col-span-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-[12px] font-semibold flex items-center justify-center space-x-1"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    <span>Add</span>
                  </button>
                </div>
                {appSettings.siteCredentials.length > 0 && (
                  <div className="space-y-1 max-h-28 overflow-y-auto pt-1">
                    {appSettings.siteCredentials.map((cred) => (
                      <div
                        key={cred.id}
                        className="flex items-center justify-between px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-[12px]"
                      >
                        <span className="font-mono text-zr-primary">
                          {cred.domain} — <b>{cred.username}</b>
                        </span>
                        <button
                          type="button"
                          onClick={() =>
                            setAppSettings((p) => ({
                              ...p,
                              siteCredentials: p.siteCredentials.filter((c) => c.id !== cred.id),
                            }))
                          }
                          className="text-red-500 hover:text-red-600"
                          title="Remove credential"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    ))}
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
        <div className="flex items-center justify-end space-x-2 px-6 py-3.5 border-t border-zr bg-zr-tableHead">
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-lg border border-zr bg-zr-surface text-zr-secondary hover:text-zr-primary font-semibold text-[13px]"
          >
            Cancel
          </button>
          <button
            onClick={handleSaveAndClose}
            disabled={savingSettings}
            className="px-5 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white font-semibold text-[13px] shadow-xs transition-colors disabled:opacity-60"
          >
            {savingSettings ? "Saving..." : "Save & Close"}
          </button>
        </div>
      </div>
    </div>
  );
};

