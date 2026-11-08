import React, { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window";
import {
  Download,
  Plus,
  Zap,
  Sparkles,
  LayoutGrid,
  List,
} from "lucide-react";
import { Sidebar, CategoryType, StatusFilterType } from "./components/Sidebar";
import { Toolbar } from "./components/Toolbar";
import { TopBar } from "./components/TopBar";
import { DownloadTableView } from "./components/DownloadTableView";
import { DownloadItemCard } from "./components/DownloadItemCard";
import { AddDownloadModal } from "./components/AddDownloadModal";
import { PreferencesModal } from "./components/PreferencesModal";
import { DownloadStatusDialog } from "./components/DownloadStatusDialog";
import { TorrentDetailsDrawer } from "./components/TorrentDetailsDrawer";
import { DownloadTask } from "./types";

function cleanDownloadUrl(url: string): string {
  if (!url) return url;
  try {
    const u = new URL(url);
    let changed = false;
    if (u.searchParams.has("bytestart") || u.searchParams.has("byteend")) {
      u.searchParams.delete("bytestart");
      u.searchParams.delete("byteend");
      changed = true;
    }
    if (u.searchParams.has("range")) {
      u.searchParams.delete("range");
      changed = true;
    }
    return changed ? u.toString() : url;
  } catch {
    return url;
  }
}

export const App: React.FC = () => {
  // State
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [selectedTorrentId, setSelectedTorrentId] = useState<string | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<CategoryType>("all");
  const [selectedStatus, setSelectedStatus] = useState<StatusFilterType>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [viewMode, setViewMode] = useState<"table" | "cards">("table");
  const [windowWidth, setWindowWidth] = useState<number>(
    typeof window !== "undefined" ? window.innerWidth : 1200
  );

  useEffect(() => {
    const handleResize = () => {
      const w = window.innerWidth;
      setWindowWidth(w);
      if (w < 700) {
        setViewMode("cards");
      }
    };
    window.addEventListener("resize", handleResize);
    if (typeof window !== "undefined" && window.innerWidth < 700) {
      setViewMode("cards");
    }
    return () => window.removeEventListener("resize", handleResize);
  }, []);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [modalInitialUrl, setModalInitialUrl] = useState("");
  const [modalInitialFilename, setModalInitialFilename] = useState("");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [defaultDir, setDefaultDir] = useState("D:\\Downloads");
  const [bridgeOnline, setBridgeOnline] = useState(true);
  const [theme, setTheme] = useState<"dark" | "light">(() => {
    return (localStorage.getItem("zr-theme") as "dark" | "light") || "dark";
  });
  const [activePopupTaskIds, setActivePopupTaskIds] = useState<string[]>([]);

  const openDownloadPopup = async (task: DownloadTask) => {
    try {
      await invoke("open_download_window", {
        taskId: task.id,
        initialTask: {
          id: task.id,
          url: task.url || "",
          filename: task.filename || "download.mp4",
          save_path: task.save_path || task.savePath || defaultDirRef.current || "",
          savePath: task.save_path || task.savePath || defaultDirRef.current || "",
          total_bytes: task.total_bytes || task.totalBytes || 0,
          totalBytes: task.total_bytes || task.totalBytes || 0,
          downloaded_bytes: task.downloaded_bytes || task.downloadedBytes || 0,
          downloadedBytes: task.downloaded_bytes || task.downloadedBytes || 0,
          percent: task.percent || 1.0,
          speed_bps: task.speed_bps || task.speedBps || 0,
          speedBps: task.speed_bps || task.speedBps || 0,
          speed_formatted: task.speed_formatted || task.speedFormatted || "Connecting...",
          speedFormatted: task.speed_formatted || task.speedFormatted || "Connecting...",
          eta_formatted: task.eta_formatted || task.etaFormatted || "--",
          etaFormatted: task.eta_formatted || task.etaFormatted || "--",
          status: task.status || "downloading",
          category: task.category || "video",
          chunk_count: task.chunk_count || 1,
          is_resumable: true,
          low_disk_space: Boolean(task.low_disk_space || task.lowDiskSpace),
        },
        title: `${task.percent?.toFixed(1) || 0}% of ${task.filename || 'Downloading...'}`,
      });
    } catch (err) {
      console.warn("Native window open error:", err);
      if ((window as any).GMAndroidBridge) {
        setActivePopupTaskIds((prev) => (prev.includes(task.id) ? prev : [...prev, task.id]));
      }
    }
  };

  const defaultDirRef = useRef(defaultDir);
  defaultDirRef.current = defaultDir;
  const handleAddMediaDownloadRef = useRef<(options: any) => Promise<void>>(() => Promise.resolve());
  const handleAddDirectDownloadRef = useRef<(url: string, savePath: string, filename: string, connections: number, lowDiskSpace?: boolean) => Promise<void>>(() => Promise.resolve());
  const handleAddTorrentDownloadRef = useRef<(urlOrPath: string, savePath: string, sequential: boolean) => Promise<void>>(() => Promise.resolve());
  const handleOpenUrlRef = useRef<(payload: any) => void>(() => {});

  useEffect(() => {
    if (theme === "dark") {
      document.documentElement.classList.add("dark");
      document.documentElement.classList.remove("light");
    } else {
      document.documentElement.classList.add("light");
      document.documentElement.classList.remove("dark");
    }
  }, [theme]);

  const handleToggleTheme = () => {
    const next = theme === "dark" ? "light" : "dark";
    setTheme(next);
    localStorage.setItem("zr-theme", next);
  };

  // Load existing tasks and default download folder on mount
  useEffect(() => {
    const loadInitialData = async () => {
      try {
        const initialTasks = await invoke<DownloadTask[]>("get_all_downloads");
        if (Array.isArray(initialTasks)) {
          setTasks(initialTasks);
          if (initialTasks.length > 0 && !selectedTaskId) {
            setSelectedTaskId(initialTasks[0].id);
          }
        }
      } catch (err) {
        console.error("Failed to load downloads:", err);
      }

      try {
        const androidBridge = (window as any).GMAndroidBridge;
        if (androidBridge?.getDownloadPath) {
          const aDir = androidBridge.getDownloadPath();
          if (aDir) setDefaultDir(aDir);
        } else {
          const dir = await invoke<string>("get_default_download_dir");
          if (dir) setDefaultDir(dir);
        }
      } catch {}

      // Check if started with pending URL (e.g. via zr:// protocol or Android Intent)
      try {
        const androidBridge = (window as any).GMAndroidBridge;
        if (androidBridge?.getPendingSharedUrl) {
          const shared = androidBridge.getPendingSharedUrl();
          if (shared && typeof shared === "string" && shared.trim()) {
            setModalInitialUrl(shared.trim());
            setIsAddModalOpen(true);
          }
        }
        const pending = await invoke<any>("get_pending_url");
        if (pending) {
          handleOpenUrlRef.current(pending);
        }
      } catch {}
    };
    loadInitialData();
  }, []);

  // Listen to Tauri backend events
  useEffect(() => {
    const handleProgress = (event: { payload: any }) => {
      const updated = event.payload;
      const id = updated?.id || updated?.taskId || updated?.task_id;
      if (!updated || !id) return;
      updated.id = id;
      setTasks((prev) => {
        const index = prev.findIndex((t) => t.id === updated.id);
        const now = new Date();
        const timeStr = `${now.getMinutes()}:${now.getSeconds()}`;
        const speedBps = updated.speed_bps || updated.speedBps || updated.speed_bytes_per_sec || 0;
        const speedKb = Math.round(speedBps / 1024);
        const downloadedBytes = updated.downloaded_bytes ?? updated.downloadedBytes ?? updated.downloaded ?? 0;
        const totalBytes = updated.total_bytes ?? updated.totalBytes ?? updated.total ?? 0;
        const isResumable = Boolean(updated.is_resumable ?? updated.isResumable ?? updated.supports_ranges ?? false);

        if (index >= 0) {
          const current = prev[index];
          const prevHistory = current.speedHistory || [];
          const newHistory = [...prevHistory, { time: timeStr, speedKb }].slice(-25);

          const updatedList = [...prev];
          updatedList[index] = {
            ...current,
            ...updated,
            downloaded_bytes: downloadedBytes || current.downloaded_bytes || 0,
            downloadedBytes: downloadedBytes || current.downloadedBytes || 0,
            total_bytes: totalBytes || current.total_bytes || current.totalBytes || 0,
            totalBytes: totalBytes || current.total_bytes || current.totalBytes || 0,
            is_resumable: isResumable || current.is_resumable || current.isResumable || false,
            isResumable: isResumable || current.isResumable || current.is_resumable || false,
            speed_bytes_per_sec: speedBps,
            speedBps,
            speed_formatted: updated.speed_formatted || updated.speedFormatted || updated.speed || (speedBps > 0 ? (speedBps >= 1048576 ? `${(speedBps / 1048576).toFixed(2)} MB/s` : `${(speedBps / 1024).toFixed(1)} KB/s`) : "0 B/s"),
            speedFormatted: updated.speed_formatted || updated.speedFormatted || updated.speed || (speedBps > 0 ? (speedBps >= 1048576 ? `${(speedBps / 1048576).toFixed(2)} MB/s` : `${(speedBps / 1024).toFixed(1)} KB/s`) : "0 B/s"),
            speed: updated.speed_formatted || updated.speedFormatted || updated.speed || (speedBps > 0 ? (speedBps >= 1048576 ? `${(speedBps / 1048576).toFixed(2)} MB/s` : `${(speedBps / 1024).toFixed(1)} KB/s`) : "0 B/s"),
            eta_formatted: updated.eta_formatted || updated.etaFormatted || updated.eta || "--",
            etaFormatted: updated.eta_formatted || updated.etaFormatted || updated.eta || "--",
            eta: updated.eta_formatted || updated.etaFormatted || updated.eta || "--",
            errorMessage: updated.error_message || updated.errorMessage || current.errorMessage,
            speedHistory: newHistory,
          };
          return updatedList;
        } else {
          return [
            {
              ...updated,
              downloaded_bytes: downloadedBytes,
              downloadedBytes,
              total_bytes: totalBytes,
              totalBytes,
              is_resumable: isResumable,
              isResumable,
              speed_bytes_per_sec: speedBps,
              speedBps,
              speed_formatted: updated.speed_formatted || updated.speedFormatted || updated.speed || (speedBps > 0 ? (speedBps >= 1048576 ? `${(speedBps / 1048576).toFixed(2)} MB/s` : `${(speedBps / 1024).toFixed(1)} KB/s`) : "0 B/s"),
              speedFormatted: updated.speed_formatted || updated.speedFormatted || updated.speed || (speedBps > 0 ? (speedBps >= 1048576 ? `${(speedBps / 1048576).toFixed(2)} MB/s` : `${(speedBps / 1024).toFixed(1)} KB/s`) : "0 B/s"),
              speed: updated.speed_formatted || updated.speedFormatted || updated.speed || (speedBps > 0 ? (speedBps >= 1048576 ? `${(speedBps / 1048576).toFixed(2)} MB/s` : `${(speedBps / 1024).toFixed(1)} KB/s`) : "0 B/s"),
              eta_formatted: updated.eta_formatted || updated.etaFormatted || updated.eta || "--",
              etaFormatted: updated.eta_formatted || updated.etaFormatted || updated.eta || "--",
              eta: updated.eta_formatted || updated.etaFormatted || updated.eta || "--",
              speedHistory: [{ time: timeStr, speedKb }],
            },
            ...prev,
          ];
        }
      });
    };

    // 1. Download progress event
    const unlistenProgress = listen<any>("download-progress", handleProgress);
    const unlistenZrProgress = listen<any>("zr://progress", handleProgress);

    // 2. Download complete event
    const handleComplete = (event: { payload: any }) => {
      const completed = event.payload;
      const id = completed?.id || completed?.taskId;
      if (!completed || !id) return;
      setTasks((prev) =>
        prev.map((t) =>
          t.id === id
            ? {
                ...t,
                ...completed,
                status: "completed",
                total_bytes: completed.total_bytes || t.total_bytes || 0,
                totalBytes: completed.total_bytes || t.totalBytes || 0,
                downloaded_bytes: completed.downloaded_bytes || completed.total_bytes || t.downloaded_bytes || 0,
                downloadedBytes: completed.downloaded_bytes || completed.total_bytes || t.downloadedBytes || 0,
              }
            : t
        )
      );
    };
    const unlistenComplete = listen<any>("download-complete", handleComplete);
    const unlistenZrComplete = listen<any>("zr://completed", handleComplete);

    // 3. Download error event
    const unlistenError = listen<any>("download-error", (event) => {
      const payload = event.payload || {};
      const id = payload.id;
      const error = payload.error || "Download error";
      if (id) {
        setTasks((prev) =>
          prev.map((t) => (t.id === id ? { ...t, status: "error", error, errorMessage: error } : t))
        );
      }
    });

    // 4. URL received from browser extension (via bridge / protocol)
    let lastHandledTime = 0;
    let lastHandledUrl = "";
    const handleOpenUrl = (event: { payload: any }) => {
      const raw = event.payload;
      let targetUrl = "";
      let targetFilename = "";
      let targetQuality = "";
      let autoStart = false;
      let isTorrent = false;
      if (typeof raw === "string") {
        targetUrl = raw;
      } else if (raw && typeof raw === "object" && raw.url) {
        targetUrl = String(raw.url);
        if (raw.filename) {
          targetFilename = String(raw.filename);
        }
        if (raw.quality) {
          targetQuality = String(raw.quality);
        }
        if (raw.autoStart || raw.autostart) {
          autoStart = true;
        }
        if (raw.isTorrent) {
          isTorrent = true;
        }
      }
      if (targetUrl && targetUrl.trim()) {
        const now = Date.now();
        const urlKey = `${targetUrl.trim()}_${targetQuality}_${Boolean(raw.isDirectFile)}`;
        if (urlKey === lastHandledUrl && now - lastHandledTime < 2500) {
          return; // Ignore immediate duplicate event from dual emission
        }
        lastHandledUrl = urlKey;
        lastHandledTime = now;

        let clean = cleanDownloadUrl(targetUrl.trim());
        if (clean.startsWith("@[") && clean.endsWith("]")) {
          clean = clean.slice(2, -1).trim();
        }
        clean = clean.replace(/^["'`[]+|["'`\]]+$/g, "").trim();

        // Check for Instagram direct CDN stream where audio is separate:
        if (
          !raw.isDirectFile && (clean.includes("cdninstagram.com") || clean.includes("fbcdn.net") || (clean.includes("instagram.com") && clean.includes(".mp4"))) && raw.referrer &&
          (raw.referrer.includes("/reel/") || raw.referrer.includes("/p/"))
        ) {
          clean = raw.referrer;
        }

        const lower = clean.toLowerCase();
        if (isTorrent || lower.startsWith("magnet:") || lower.endsWith(".torrent") || lower.includes(".torrent")) {
          if (autoStart) {
            handleAddTorrentDownloadRef.current(clean, defaultDirRef.current, false);
          } else {
            setModalInitialUrl(clean);
            setIsAddModalOpen(true);
          }
          return;
        }

        if (autoStart) {
          const isDirectFile =
            raw.isDirectFile ||
            targetQuality?.toLowerCase() === "thumbnail" ||
            targetQuality?.toLowerCase() === "subtitle" ||
            ((lower.endsWith(".zip") ||
              lower.endsWith(".rar") ||
              lower.endsWith(".7z") ||
              lower.endsWith(".tar") ||
              lower.endsWith(".gz") ||
              lower.endsWith(".iso") ||
              lower.endsWith(".exe") ||
              lower.endsWith(".msi") ||
              lower.endsWith(".pdf") ||
              lower.endsWith(".dmg") ||
              lower.endsWith(".jpg") ||
              lower.endsWith(".jpeg") ||
              lower.endsWith(".png") ||
              lower.endsWith(".webp") ||
              lower.endsWith(".vtt") ||
              lower.endsWith(".srt") ||
              lower.includes("timedtext")) &&
              !targetQuality);

          if (isDirectFile) {
            handleAddDirectDownloadRef.current(
              clean,
              defaultDirRef.current,
              targetFilename,
              16,
              Boolean(raw.lowDiskSpace)
            );
          } else {
            const isAudio = targetQuality.toLowerCase().includes("audio");
            const isKick = clean.toLowerCase().includes("kick.com");
            const targetContainer = raw.container
              ? String(raw.container).toUpperCase().includes("TS")
                ? "TS (Video)"
                : isAudio
                ? "MP3 (Audio)"
                : "MP4 (Video)"
              : isAudio
              ? "MP3 (Audio)"
              : "MP4 (Video)";

            let videoStreamId = isKick ? "best" : "bv*";
            if (targetQuality.includes("1080")) {
              videoStreamId = isKick ? "1080p60/1080p/best" : "bv*[height<=1080]";
            } else if (targetQuality.includes("720")) {
              videoStreamId = isKick ? "720p60/720p/best" : "bv*[height<=720]";
            } else if (targetQuality.includes("480")) {
              videoStreamId = isKick ? "480p/best" : "bv*[height<=480]";
            } else if (targetQuality.includes("360")) {
              videoStreamId = isKick ? "360p/best" : "bv*[height<=360]";
            } else if (targetQuality.includes("240")) {
              videoStreamId = isKick ? "240p/best" : "bv*[height<=240]";
            } else if (targetQuality.includes("160")) {
              videoStreamId = isKick ? "160p/best" : "bv*[height<=160]";
            } else if (targetQuality.includes("144")) {
              videoStreamId = isKick ? "144p/best" : "bv*[height<=144]";
            }

            handleAddMediaDownloadRef.current({
              url: clean,
              saveFolder: defaultDirRef.current,
              customFilename: targetFilename || undefined,
              downloadMode: isAudio ? "audio" : "video",
              container: targetContainer,
              videoStreamId: isAudio ? "" : videoStreamId,
              audioStreamId: isKick ? "" : "ba",
              audioBitrate: "Best",
              enableSubtitles: false,
              subtitleLang: "en",
              subtitleFormat: "srt",
              subtitleSaveMode: "separate",
              enableClip: false,
              startTime: "",
              endTime: "",
              preciseCut: false,
              maxActiveDownloads: 3,
              overwriteFiles: false,
              includeMediaId: false,
              includeAutoSubtitles: false,
              includeSuperRes: false,
              preferredVideoCodec: "default",
              preferredAudioCodec: "default",
              preferredSubtitleFormat: "default",
              preferredFps: "default",
              usePartFiles: !Boolean(raw.lowDiskSpace || raw.low_disk_space || localStorage.getItem("zr_low_disk_space") === "true"),
              lowDiskSpace: Boolean(raw.lowDiskSpace || raw.low_disk_space || localStorage.getItem("zr_low_disk_space") === "true"),
              concurrentConnections: 8,
            });
          }
        } else {
          setModalInitialUrl(clean);
          setModalInitialFilename(targetFilename.trim());
          setIsAddModalOpen(true);
        }
      }
    };
    const unlistenOpenUrl = listen<any>("open-url", handleOpenUrl);
    // zr open url listener removed

    // Register Android Bridge Dynamic Callbacks
    (window as any).onAndroidSharedUrl = (url: string) => {
      if (url && typeof url === "string" && url.trim()) {
        handleOpenUrl({ payload: url.trim() });
      }
    };

    (window as any).onAndroidFolderSelected = (folder: string) => {
      if (folder && typeof folder === "string" && folder.trim()) {
        setDefaultDir(folder.trim());
      }
    };

    (window as any).onAndroidProgress = (taskId: string, progress: number, eta: string, line: string) => {
      setTasks((prev) => {
        const index = prev.findIndex((t) => t.id === taskId);
        if (index === -1) return prev;
        const current = prev[index];
        const updated = [...prev];
        const percent = typeof progress === "number" ? Math.min(100, Math.max(0, progress)) : current.percent;
        updated[index] = {
          ...current,
          percent,
          eta_formatted: eta || current.eta_formatted,
          status: percent >= 100 ? "completed" : "downloading",
          progressText: line || current.progressText,
        };
        return updated;
      });
    };

    (window as any).onAndroidComplete = (taskId: string, success: boolean, errorMsg?: string) => {
      setTasks((prev) => {
        const index = prev.findIndex((t) => t.id === taskId);
        if (index === -1) return prev;
        const current = prev[index];
        const updated = [...prev];
        updated[index] = {
          ...current,
          percent: success ? 100 : current.percent,
          status: success ? "completed" : "error",
          errorMessage: errorMsg || current.errorMessage,
        };
        return updated;
      });
    };

    return () => {
      unlistenProgress.then((f) => f()).catch(() => {});
      unlistenZrProgress.then((f) => f()).catch(() => {});
      unlistenComplete.then((f) => f()).catch(() => {});
      unlistenZrComplete.then((f) => f()).catch(() => {});
      unlistenError.then((f) => f()).catch(() => {});
      unlistenOpenUrl.then((f) => f()).catch(() => {});
      // unlistenZrOpenUrl removed
      delete (window as any).onAndroidSharedUrl;
      delete (window as any).onAndroidFolderSelected;
      delete (window as any).onAndroidProgress;
      delete (window as any).onAndroidComplete;
    };
  }, []);

  // Check local bridge status periodically
  useEffect(() => {
    const checkBridge = async () => {
      try {
        const resp = await fetch("http://127.0.0.1:9666/ping");
        setBridgeOnline(resp.ok);
      } catch {
        setBridgeOnline(false);
      }
    };
    checkBridge();
    const interval = setInterval(checkBridge, 4000);
    return () => clearInterval(interval);
  }, []);

  // Windows Taskbar Dynamic Progress Bar Sync
  useEffect(() => {
    try {
      const win = getCurrentWindow();
      const activeDownloading = tasks.filter(
        (t) => t.status === "downloading" || t.status === "assembling"
      );
      const activePaused = tasks.filter((t) => t.status === "paused");
      const activeErrors = tasks.filter((t) => t.status === "error");

      if (activeDownloading.length > 0) {
        let totalBytes = 0;
        let downloadedBytes = 0;
        let knownBytesCount = 0;
        let totalPercent = 0;

        for (const t of activeDownloading) {
          const total = t.total_bytes || t.totalBytes || 0;
          const down = t.downloaded_bytes || t.downloadedBytes || 0;
          const pct = typeof t.percent === "number" ? t.percent : parseFloat(String(t.percent || 0)) || 0;
          if (total > 0) {
            totalBytes += total;
            downloadedBytes += down;
            knownBytesCount++;
          }
          totalPercent += pct;
        }

        let overallPercent = 0;
        if (knownBytesCount > 0 && totalBytes > 0) {
          overallPercent = Math.min(100, Math.max(0, (downloadedBytes / totalBytes) * 100));
        } else {
          overallPercent = Math.min(100, Math.max(0, totalPercent / activeDownloading.length));
        }

        win.setProgressBar({
          status: ProgressBarStatus.Normal,
          progress: Math.round(overallPercent),
        }).catch(() => {});
      } else if (activePaused.length > 0 && activeDownloading.length === 0) {
        win.setProgressBar({
          status: ProgressBarStatus.Paused,
          progress: 50,
        }).catch(() => {});
      } else if (activeErrors.length > 0 && activeDownloading.length === 0) {
        win.setProgressBar({
          status: ProgressBarStatus.Error,
          progress: 100,
        }).catch(() => {});
      } else {
        win.setProgressBar({
          status: ProgressBarStatus.None,
        }).catch(() => {});
      }
    } catch {
      // Non-tauri context fallback
    }
  }, [tasks]);

  // Helper to check if task is a yt-dlp media extraction task
  const isMediaTask = (task?: DownloadTask, id?: string) => {
    const targetId = id || task?.id || "";
    if (targetId.startsWith("media-") || targetId.startsWith("ytdlp-")) return true;
    if (!task) return false;
    const u = (task.url || "").toLowerCase();
    return (
      u.includes("kick.com") ||
      u.includes("youtube.com") ||
      u.includes("youtu.be") ||
      u.includes("instagram.com") ||
      u.includes("twitch.tv") ||
      u.includes("facebook.com") ||
      u.includes("tiktok.com") ||
      u.includes("twitter.com") ||
      u.includes("x.com") ||
      u.includes(".m3u8")
    );
  };

  const isTorrentTask = (task?: DownloadTask, id?: string) => {
    if (task?.category === "torrents") return true;
    const targetId = id || task?.id || "";
    if (targetId.startsWith("torrent-") || targetId.startsWith("bt-") || targetId.length === 40) return true;
    const u = (task?.url || "").toLowerCase();
    return u.startsWith("magnet:") || u.endsWith(".torrent");
  };

  // IDM Action Handlers
  const handlePause = async (id: string) => {
    try {
      const task = tasks.find((t) => t.id === id);
      if (task && isTorrentTask(task, id)) {
        try {
          await invoke("pause_torrent", { idOrHash: task.id });
        } catch (e) {
          console.warn("pause_torrent error:", e);
        }
      } else if (isMediaTask(task, id)) {
        if ((window as any).GMAndroidBridge?.cancelDownload) {
          try { (window as any).GMAndroidBridge.cancelDownload(id); } catch {}
        }
        try {
          await invoke("cancel_download", { taskId: id });
        } catch (e) {
          console.warn("Media cancel_download error:", e);
        }
      } else {
        try {
          await invoke("pause_http_download", { taskId: id });
        } catch (e) {
          console.warn("pause_http_download error:", e);
        }
      }
      setTasks((prev) =>
        prev.map((t) => (t.id === id ? { ...t, status: "paused" } : t))
      );
    } catch (err) {
      console.error("Pause failed:", err);
    }
  };

  const handleResume = async (id: string) => {
    try {
      const task = tasks.find((t) => t.id === id);
      setTasks((prev) =>
        prev.map((t) =>
          t.id === id
            ? { ...t, status: "downloading", error: undefined, errorMessage: undefined }
            : t
        )
      );

      if (task && isTorrentTask(task, id)) {
        try {
          await invoke("resume_torrent", { idOrHash: task.id });
        } catch (e) {
          console.warn("resume_torrent error:", e);
        }
      } else if (task && isMediaTask(task, id)) {
        // Resume media task via start_download with saved task options
        try {
          const opts = task.options || {
            taskId: task.id,
            url: task.url,
            saveFolder: task.save_path || task.savePath || defaultDirRef.current,
            customFilename: task.filename,
            downloadMode: task.category === "audio" ? "audio" : "video",
            container: task.category === "audio" ? "MP3 (Audio)" : "MP4 (Video)",
            videoStreamId: "best",
            audioStreamId: "ba",
            audioBitrate: "Best",
            enableSubtitles: false,
            subtitleLang: "en",
            subtitleFormat: "srt",
            subtitleSaveMode: "separate",
            enableClip: false,
            startTime: "",
            endTime: "",
            preciseCut: false,
            maxActiveDownloads: 3,
            overwriteFiles: false,
            includeMediaId: false,
            includeAutoSubtitles: false,
            includeSuperRes: false,
            preferredVideoCodec: "default",
            preferredAudioCodec: "default",
            preferredSubtitleFormat: "default",
            preferredFps: "default",
          };
          await invoke("start_download", { options: opts });
        } catch (e) {
          console.error("start_download resume error:", e);
        }
      } else {
        await invoke("resume_http_download", { taskId: id });
      }
    } catch (err) {
      console.error("Resume failed:", err);
    }
  };

  const handleCancel = async (id: string) => {
    try {
      const task = tasks.find((t) => t.id === id);
      if (task && isTorrentTask(task, id)) {
        try {
          await invoke("pause_torrent", { idOrHash: task.id });
        } catch {}
      } else if (isMediaTask(task, id)) {
        if ((window as any).GMAndroidBridge?.cancelDownload) {
          try { (window as any).GMAndroidBridge.cancelDownload(id); } catch {}
        }
        try {
          await invoke("cancel_download", { taskId: id });
        } catch {}
      } else {
        try {
          await invoke("cancel_http_download", { taskId: id });
        } catch {}
      }
      setTasks((prev) =>
        prev.map((t) => (t.id === id ? { ...t, status: "cancelled" } : t))
      );
    } catch (err) {
      console.error("Cancel failed:", err);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      const task = tasks.find((t) => t.id === id);
      if (task && isTorrentTask(task, id)) {
        try {
          await invoke("remove_torrent", { idOrHash: task.id, deleteFiles: false });
        } catch (e) {
          console.warn("remove_torrent error:", e);
        }
      } else {
        await invoke("remove_http_download", { taskId: id, deleteFile: false });
      }
      setTasks((prev) => prev.filter((t) => t.id !== id));
      if (selectedTaskId === id) setSelectedTaskId(null);
      if (selectedTorrentId === id) setSelectedTorrentId(null);
    } catch (err) {
      console.error("Delete failed:", err);
    }
  };

  const handleStopAll = async () => {
    tasks.forEach((t) => {
      if (t.status === "downloading") {
        if (isMediaTask(t, t.id)) {
          invoke("cancel_download", { taskId: t.id }).catch(() => {});
        } else {
          invoke("pause_http_download", { taskId: t.id }).catch(() => {});
        }
      }
    });
    setTasks((prev) =>
      prev.map((t) => (t.status === "downloading" ? { ...t, status: "paused" } : t))
    );
  };

  const handleClearCompleted = async () => {
    const completedTasks = tasks.filter((t) => t.status === "completed");
    for (const t of completedTasks) {
      await invoke("remove_http_download", { taskId: t.id, deleteFile: false }).catch(() => {});
    }
    setTasks((prev) => prev.filter((t) => t.status !== "completed"));
    if (selectedTaskId && completedTasks.some((t) => t.id === selectedTaskId)) {
      setSelectedTaskId(null);
    }
  };

  const handleOpenFile = async (path: string) => {
    try {
      await invoke("open_file", { path });
    } catch (err) {
      console.error("Open file error, attempting fallback to open_folder:", err);
      try {
        await invoke("open_folder", { path });
      } catch {}
    }
  };

  const handleOpenFolder = async (path: string) => {
    try {
      await invoke("open_folder", { path });
    } catch (err) {
      console.error("Open folder error:", err);
    }
  };

  const handleOpenDefaultFolder = async () => {
    if (defaultDir) {
      handleOpenFolder(defaultDir);
    }
  };



  const handleAddDirectDownload = async (
    url: string,
    savePath: string,
    filename: string,
    connections: number,
    lowDiskSpace?: boolean
  ) => {
    try {
      const isLowSpace = lowDiskSpace !== undefined
        ? lowDiskSpace
        : (localStorage.getItem("zr_low_disk_space") === "true");

      const androidBridge = (window as any).GMAndroidBridge;
      if (androidBridge?.startDownload) {
        const taskId = `direct-${Date.now()}`;
        let finalFilename = filename?.trim() || url.split("/").pop()?.split("?")[0] || "download";
        if (!/\.[a-zA-Z0-9]{2,5}$/.test(finalFilename)) {
          finalFilename = `${finalFilename}.mp4`;
        }
        const newDirectTask: DownloadTask = {
          id: taskId,
          url,
          filename: finalFilename,
          save_path: savePath || defaultDir || "Downloads",
          total_bytes: 0,
          downloaded_bytes: 0,
          percent: 1.0,
          speed_bps: 0,
          speed_formatted: "Connecting...",
          eta_formatted: "--",
          status: "downloading",
          category: "compressed",
          chunk_count: connections || 8,
          is_resumable: true,
          low_disk_space: isLowSpace,
          lowDiskSpace: isLowSpace,
          chunks: [],
          speedHistory: [],
          errorMessage: undefined,
          createdAt: new Date().toLocaleTimeString(),
        };
        setTasks((prev) => [newDirectTask, ...prev.filter((t) => t.id !== newDirectTask.id)]);
        setActivePopupTaskIds((prev) => prev.includes(newDirectTask.id) ? prev : [...prev, newDirectTask.id]);
        androidBridge.startDownload(taskId, url, "best", "", false, "", "", false, "", savePath || "", finalFilename);
        return;
      }
      const taskId = await invoke<string>("start_http_download", {
        url,
        saveFolder: savePath || null,
        filename: filename || null,
        chunkCount: connections,
        lowDiskSpace: isLowSpace,
      });
      const updated = await invoke<DownloadTask[]>("get_all_downloads");
      setTasks(updated);
      if (taskId) {
        const created = updated.find((t) => t.id === taskId);
        if (created) openDownloadPopup(created);
      }
    } catch (e) {
      console.error("Direct download failed to start:", e);
    }
  };

  const handleAddMediaDownload = async (options: any) => {
    try {
      const androidBridge = (window as any).GMAndroidBridge;
      const taskId = options.taskId || `media-${Date.now()}`;

      const isAudio =
        options.downloadMode === "audio" ||
        (options.container && String(options.container).toLowerCase().includes("audio"));

      let finalFilename = options.customFilename?.trim() || options.title?.trim() || "Online Media";
      if (!/\.[a-zA-Z0-9]{2,5}$/.test(finalFilename)) {
        finalFilename = `${finalFilename}${isAudio ? ".mp3" : ".mp4"}`;
      }

      const newMediaTask: DownloadTask = {
        id: taskId,
        url: options.url,
        filename: finalFilename,
        save_path: options.saveFolder || defaultDir || "Downloads",
        total_bytes: 0,
        downloaded_bytes: 0,
        percent: 1.0,
        speed_bps: 0,
        speed_formatted: "Connecting...",
        eta_formatted: "--",
        status: "downloading",
        category: isAudio ? "audio" : "video",
        chunk_count: 1,
        is_resumable: true,
        chunks: [],
        speedHistory: [],
        errorMessage: undefined,
        createdAt: new Date().toLocaleTimeString(),
        options: { ...options, taskId },
      };

      setTasks((prev) => [newMediaTask, ...prev.filter((t) => t.id !== newMediaTask.id)]);
      openDownloadPopup(newMediaTask);

      if (androidBridge?.startDownload) {
        androidBridge.startDownload(
          taskId,
          options.url,
          options.formatId || "best",
          options.container || "MP4",
          options.enableClip || false,
          options.startTime || "",
          options.endTime || "",
          options.enableSubs || false,
          options.subLangs || "",
          options.saveFolder || "",
          finalFilename
        );
      } else {
        const isLowSpace = Boolean(
          options.lowDiskSpace ||
          options.low_disk_space ||
          localStorage.getItem("zr_low_disk_space") === "true"
        );
        const invokeOptions = {
          ...options,
          taskId,
          task_id: taskId,
          saveFolder: options.saveFolder || defaultDirRef.current,
          save_folder: options.saveFolder || defaultDirRef.current,
          customFilename: finalFilename,
          custom_filename: finalFilename,
          lowDiskSpace: isLowSpace,
          low_disk_space: isLowSpace,
          usePartFiles: !isLowSpace,
        };
        try {
          await invoke<string>("ytdlp_download", { options: invokeOptions });
        } catch {
          await invoke<string>("start_download", { options: invokeOptions });
        }
      }
    } catch (e) {
      console.error("Media download failed to start:", e);
    }
  };

  const handleAddTorrentDownload = async (
    urlOrPath: string,
    savePath: string,
    sequential: boolean
  ) => {
    try {
      const resp: any = await invoke("add_torrent", {
        request: {
          source: urlOrPath,
          saveFolder: savePath || defaultDir || "Downloads",
          paused: false,
          sequential,
        },
      });

      const infoHash = resp?.info_hash || `torrent-${Date.now()}`;
      const name = resp?.name || (urlOrPath.startsWith("magnet:") ? "Magnet Torrent" : urlOrPath.split(/[/\\]/).pop() || "Torrent");

      const newTorrentTask: DownloadTask = {
        id: infoHash,
        url: urlOrPath,
        filename: name,
        save_path: savePath || defaultDir || "Downloads",
        total_bytes: resp?.total_bytes || 0,
        downloaded_bytes: 0,
        percent: 0.0,
        speed_bps: 0,
        speed_formatted: "Connecting to swarm...",
        eta_formatted: "--",
        status: "downloading",
        category: "torrents",
        chunk_count: 1,
        is_resumable: true,
        chunks: [],
        speedHistory: [],
        errorMessage: undefined,
        createdAt: new Date().toLocaleTimeString(),
      };

      setTasks((prev) => [newTorrentTask, ...prev.filter((t) => t.id !== newTorrentTask.id)]);
      setSelectedTaskId(infoHash);
      setSelectedTorrentId(infoHash);
    } catch (e: any) {
      console.error("Torrent download failed to start:", e);
      throw e;
    }
  };

  useEffect(() => {
    handleAddDirectDownloadRef.current = handleAddDirectDownload;
    handleAddMediaDownloadRef.current = handleAddMediaDownload;
    handleAddTorrentDownloadRef.current = handleAddTorrentDownload;
    defaultDirRef.current = defaultDir;
  });

  // Native window drag-and-drop listener for .torrent files
  useEffect(() => {
    const handleDragOver = (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    const handleDrop = (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const files = e.dataTransfer?.files;
      if (files && files.length > 0) {
        for (let i = 0; i < files.length; i++) {
          const file = files[i];
          const path = (file as any).path || file.name;
          if (path && (path.toLowerCase().endsWith(".torrent") || path.toLowerCase().includes(".torrent"))) {
            setModalInitialUrl(path);
            setIsAddModalOpen(true);
            break;
          }
        }
      }
    };

    window.addEventListener("dragover", handleDragOver);
    window.addEventListener("drop", handleDrop);
    return () => {
      window.removeEventListener("dragover", handleDragOver);
      window.removeEventListener("drop", handleDrop);
    };
  }, []);

  // Category counts
  const categoryCounts = useMemo(() => {
    const counts: Record<CategoryType, number> = {
      all: tasks.length,
      torrents: 0,
      compressed: 0,
      docs: 0,
      audio: 0,
      programs: 0,
      video: 0,
    };
    tasks.forEach((t) => {
      const cat = (t.category || "docs").toLowerCase() as CategoryType;
      if (counts[cat] !== undefined) counts[cat]++;
    });
    return counts;
  }, [tasks]);

  // Status counts
  const statusCounts = useMemo(() => {
    const counts: Record<StatusFilterType, number> = {
      all: tasks.length,
      downloading: 0,
      completed: 0,
      paused: 0,
      error: 0,
    };
    tasks.forEach((t) => {
      const st = t.status as StatusFilterType;
      if (counts[st] !== undefined) counts[st]++;
    });
    return counts;
  }, [tasks]);

  // Overall speed and active count
  const { totalSpeedFormatted, activeCount } = useMemo(() => {
    let totalBytesPerSec = 0;
    let count = 0;
    tasks.forEach((t) => {
      if (t.status === "downloading") {
        totalBytesPerSec += t.speed_bytes_per_sec || 0;
        count++;
      }
    });

    let formatted = "0 B/s";
    if (totalBytesPerSec >= 1024 * 1024 * 1024) {
      formatted = `${(totalBytesPerSec / (1024 * 1024 * 1024)).toFixed(2)} GB/s`;
    } else if (totalBytesPerSec >= 1024 * 1024) {
      formatted = `${(totalBytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`;
    } else if (totalBytesPerSec >= 1024) {
      formatted = `${(totalBytesPerSec / 1024).toFixed(0)} KB/s`;
    } else if (totalBytesPerSec > 0) {
      formatted = `${totalBytesPerSec} B/s`;
    }

    return { totalSpeedFormatted: formatted, activeCount: count };
  }, [tasks]);

  // Filtered tasks
  const filteredTasks = useMemo(() => {
    return tasks.filter((t) => {
      if (selectedCategory !== "all") {
        const cat = (t.category || "").toLowerCase();
        if (cat !== selectedCategory) return false;
      }
      if (selectedStatus !== "all") {
        if (t.status !== selectedStatus) return false;
      }
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase();
        const matchName = t.filename.toLowerCase().includes(q);
        const matchUrl = t.url.toLowerCase().includes(q);
        if (!matchName && !matchUrl) return false;
      }
      return true;
    });
  }, [tasks, selectedCategory, selectedStatus, searchQuery]);

  const selectedTask = useMemo(() => {
    return tasks.find((t) => t.id === selectedTaskId) || null;
  }, [tasks, selectedTaskId]);

  const isDark = theme === "dark";

  return (
    <div className="flex h-screen w-screen overflow-hidden font-sans bg-zr-app text-zr-primary transition-colors duration-150">
      {/* Left Sidebar (IDM Category Tree + Status) */}
      <Sidebar
        selectedCategory={selectedCategory}
        onSelectCategory={setSelectedCategory}
        selectedStatus={selectedStatus}
        onSelectStatus={setSelectedStatus}
        categoryCounts={categoryCounts}
        statusCounts={statusCounts}
        bridgeOnline={bridgeOnline}
        onOpenSettings={() => setIsSettingsOpen(true)}
        onOpenMediaModal={() => {
          setModalInitialUrl("");
          setModalInitialFilename("");
          setIsAddModalOpen(true);
        }}
        theme={theme}
      />

      {/* Main Workspace */}
      <div className="flex-1 flex flex-col h-screen min-w-0 overflow-hidden">
        {/* Top IDM Action Ribbon / Toolbar */}
        <Toolbar
          onAddUrl={() => {
            setModalInitialUrl("");
            setModalInitialFilename("");
            setIsAddModalOpen(true);
          }}
          onResumeSelected={() => selectedTaskId && handleResume(selectedTaskId)}
          onPauseSelected={() => selectedTaskId && handlePause(selectedTaskId)}
          onStopAll={handleStopAll}
          onDeleteSelected={() => selectedTaskId && handleDelete(selectedTaskId)}
          onClearCompleted={handleClearCompleted}
          onOpenMediaGrabber={() => {
            setModalInitialUrl("");
            setModalInitialFilename("");
            setIsAddModalOpen(true);
          }}
          onOpenSettings={() => setIsSettingsOpen(true)}
          onOpenDefaultFolder={handleOpenDefaultFolder}
          hasSelected={Boolean(selectedTaskId)}
          hasActive={activeCount > 0}
          hasCompleted={statusCounts.completed > 0}
          theme={theme}
        />

        {/* Top Search, Status & View Toggle Bar */}
        <div className="px-5 py-2.5 border-b border-zr bg-zr-surface flex items-center justify-between gap-4 z-10 select-none transition-colors">
          {/* Search Bar */}
          <div className="flex-1 max-w-md">
            <TopBar
              searchQuery={searchQuery}
              onSearchChange={setSearchQuery}
              statusFilter={selectedStatus}
              onStatusFilterChange={(st) => setSelectedStatus(st as StatusFilterType)}
              totalSpeedFormatted={totalSpeedFormatted}
              activeCount={activeCount}
              totalTasksCount={tasks.length}
              onOpenAddModal={() => {
                setModalInitialUrl("");
                setModalInitialFilename("");
                setIsAddModalOpen(true);
              }}
              onClearCompleted={handleClearCompleted}
              theme={theme}
              onToggleTheme={handleToggleTheme}
            />
          </div>

          {/* View Mode Toggle (Table / Card) */}
          <div className="flex items-center space-x-1 p-1 rounded-xl border border-zr bg-zr-surface transition-colors">
            <button
              onClick={() => setViewMode("table")}
              className={`p-1.5 rounded-lg transition-all ${
                viewMode === "table"
                  ? "bg-blue-600 text-white shadow-xs"
                  : "text-zr-secondary hover:text-zr-primary"
              }`}
              title="Table View"
              aria-label="Table View"
            >
              <List className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode("cards")}
              className={`p-1.5 rounded-lg transition-all ${
                viewMode === "cards"
                  ? "bg-blue-600 text-white shadow-xs"
                  : "text-zr-secondary hover:text-zr-primary"
              }`}
              title="Modern Card View"
              aria-label="Card View"
            >
              <LayoutGrid className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Main Content Area */}
        <main className="flex-1 overflow-y-auto p-3 sm:p-5 space-y-4">
          {filteredTasks.length === 0 ? (
            /* Empty State */
            <div className="h-[65vh] flex flex-col items-center justify-center text-center p-8 space-y-4">
              <div className="w-20 h-20 rounded-2xl overflow-hidden flex items-center justify-center shadow-lg border border-white/20 bg-[#D40203]">
                <img
                  src="/icon.png"
                  alt="ZR Downloader"
                  className="w-full h-full object-cover"
                />
              </div>

              <div className="max-w-md space-y-2">
                <h2 className="text-xl font-bold text-zr-primary">
                  ZR Downloader
                </h2>
                <p className="text-[13px] leading-relaxed text-zr-secondary">
                  Ultra-fast multi-threaded download acceleration, media stream grabber and BitTorrent client.
                  Paste any download link above or use the browser extension to intercept downloads.
                </p>
                <div className="pt-2">
                  <button
                    onClick={() => {
                      setModalInitialUrl("");
                      setModalInitialFilename("");
                      setIsAddModalOpen(true);
                    }}
                    className="px-5 py-2.5 rounded-xl bg-gradient-to-b from-[#3b9bff] to-[#0a6cf0] hover:brightness-105 text-white font-semibold text-[13px] shadow-[0_4px_14px_rgba(10,108,240,0.4)] transition-all inline-flex items-center space-x-2"
                  >
                    <Plus className="w-4 h-4" />
                    <span>Add New Download</span>
                  </button>
                </div>
              </div>
            </div>
          ) : viewMode === "table" ? (
            /* IDM High-Density Table View */
            <DownloadTableView
              tasks={filteredTasks}
              selectedTaskId={selectedTaskId}
              onSelectTask={(id) => {
                setSelectedTaskId(id);
                const task = tasks.find((t) => t.id === id);
                if (task) {
                  if (task.category === "torrents") {
                    setSelectedTorrentId(id);
                  } else {
                    openDownloadPopup(task);
                  }
                }
              }}
              onPause={handlePause}
              onResume={handleResume}
              onDelete={handleDelete}
              onOpenFile={handleOpenFile}
              onOpenFolder={handleOpenFolder}
              theme={theme}
              windowWidth={windowWidth}
            />
          ) : (
            /* Card View */
            <div className="space-y-3">
              {filteredTasks.map((task) => (
                <DownloadItemCard
                  key={task.id}
                  task={task}
                  onPause={handlePause}
                  onResume={handleResume}
                  onCancel={handleCancel}
                  onDelete={handleDelete}
                  onOpenFile={handleOpenFile}
                  onOpenFolder={handleOpenFolder}
                  theme={theme}
                />
              ))}
            </div>
          )}
        </main>

        {/* Torrent Details Drawer (Phase 1) */}
        {selectedTorrentId && (
          <TorrentDetailsDrawer
            torrentId={selectedTorrentId}
            isOpen={Boolean(selectedTorrentId)}
            onClose={() => setSelectedTorrentId(null)}
            theme={theme}
          />
        )}
      </div>

      {/* IDM-Style Independent Download Status Dialogs (Only on Android mobile fallback) */}
      {(window as any).GMAndroidBridge && activePopupTaskIds.map((taskId, index) => {
        const task = tasks.find((t) => t.id === taskId);
        if (!task) return null;
        return (
          <DownloadStatusDialog
            key={task.id}
            task={task}
            isOpen={true}
            dialogIndex={index}
            totalDialogs={activePopupTaskIds.length}
            onClose={() => setActivePopupTaskIds((prev) => prev.filter((id) => id !== task.id))}
            onBringToFront={() => {
              setActivePopupTaskIds((prev) => [...prev.filter((id) => id !== task.id), task.id]);
            }}
            onPause={handlePause}
            onResume={handleResume}
            onCancel={handleCancel}
            onOpenFile={handleOpenFile}
            onOpenFolder={handleOpenFolder}
            theme={theme}
          />
        );
      })}

      {/* Add Download Modal */}
      <AddDownloadModal
        isOpen={isAddModalOpen}
        onClose={() => {
          setIsAddModalOpen(false);
          setModalInitialUrl("");
          setModalInitialFilename("");
        }}
        defaultUrl={modalInitialUrl}
        defaultFilename={modalInitialFilename}
        onAddDirectDownload={handleAddDirectDownload}
        onAddMediaDownload={handleAddMediaDownload}
        onAddTorrentDownload={handleAddTorrentDownload}
        theme={theme}
      />

      {/* Preferences / Settings Modal */}
      <PreferencesModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        defaultDir={defaultDir}
        onChangeDefaultDir={setDefaultDir}
        theme={theme}
      />
    </div>
  );
};

export default App;

