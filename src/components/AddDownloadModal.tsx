import React, { useState, useEffect, useRef } from "react";
import {
  X,
  Zap,
  Film,
  Folder,
  Download,
  AlertCircle,
  Scissors,
  Loader2,
  Clipboard,
  Radio,
  FileCode,
  HardDrive,
  ListPlus,
  AlertTriangle,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  MediaInfo,
  ContainerFormat,
  ALL_CONTAINERS,
  DuplicateCheckResult,
} from "../types";
import { expandBatchUrlInput } from "../lib/batchUrls";

export interface DirectDownloadExtras {
  autoConnections?: boolean;
  speedLimitKb?: number;
  priority?: string;
  expectedChecksum?: string;
  originPageUrl?: string;
  postDownloadAction?: string;
  postDownloadProgram?: string;
  authUsername?: string;
  authPassword?: string;
}

interface AddDownloadModalProps {
  isOpen: boolean;
  onClose: () => void;
  defaultUrl?: string;
  defaultFilename?: string;
  defaultBatchText?: string;
  onAddDirectDownload: (
    url: string,
    savePath: string,
    filename: string,
    connections: number,
    lowDiskSpace?: boolean,
    extras?: DirectDownloadExtras
  ) => Promise<void>;
  onAddMediaDownload: (options: any) => Promise<void>;
  onAddTorrentDownload?: (
    urlOrPath: string,
    savePath: string,
    sequential: boolean
  ) => Promise<void>;
  theme?: "dark" | "light";
}

export const AddDownloadModal: React.FC<AddDownloadModalProps> = ({
  isOpen,
  onClose,
  defaultUrl = "",
  defaultFilename = "",
  defaultBatchText = "",
  onAddDirectDownload,
  onAddMediaDownload,
  onAddTorrentDownload,
}) => {
  const [tab, setTab] = useState<"direct" | "media" | "batch" | "torrent">("direct");

  // Direct & General State
  const [url, setUrl] = useState(defaultUrl);
  const [customFilename, setCustomFilename] = useState(defaultFilename);
  const [saveFolder, setSaveFolder] = useState("");
  const [connections, setConnections] = useState(8);
  const [autoConnections, setAutoConnections] = useState(false);
  const [priority, setPriority] = useState<"high" | "normal" | "low">("normal");
  const [speedLimitKb, setSpeedLimitKb] = useState(0);
  const [expectedChecksum, setExpectedChecksum] = useState("");
  const [postDownloadAction, setPostDownloadAction] = useState("");
  const [authUsername, setAuthUsername] = useState("");
  const [authPassword, setAuthPassword] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [lowDiskSpace, setLowDiskSpace] = useState(() => {
    return localStorage.getItem("zr_low_disk_space") === "true";
  });
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Batch Import State
  const [batchText, setBatchText] = useState(defaultBatchText);
  const batchPreviewUrls = React.useMemo(() => expandBatchUrlInput(batchText), [batchText]);

  // Duplicate Prompt State
  const [duplicatePrompt, setDuplicatePrompt] = useState<{
    mode: "direct" | "media";
    result: DuplicateCheckResult;
  } | null>(null);

  // Media (yt-dlp) State
  const [mediaInfo, setMediaInfo] = useState<MediaInfo | null>(null);
  const [isFetchingMedia, setIsFetchingMedia] = useState(false);
  const [selectedFormatId, setSelectedFormatId] = useState("");
  const [selectedAudioId, setSelectedAudioId] = useState("");
  const [container, setContainer] = useState<ContainerFormat>("MP4 (Video)");
  const [enableClip, setEnableClip] = useState(false);
  const [startTime, setStartTime] = useState("00:00:00");
  const [endTime, setEndTime] = useState("00:00:10");

  // Torrent State
  const [sequential, setSequential] = useState(false);

  const lastInspectedUrl = useRef("");
  const inspectSeq = useRef(0);

  const shouldAutoInspectUrl = (rawUrl: string): boolean => {
    let clean = rawUrl.trim();
    if (clean.startsWith("@[") && clean.endsWith("]")) {
      clean = clean.slice(2, -1).trim();
    }
    if (!/^https?:\/\//i.test(clean)) return false;
    const lower = clean.toLowerCase();
    if (lower.startsWith("magnet:") || lower.includes(".torrent")) return false;
    if (/\.(zip|rar|7z|exe|msi|iso|pdf|apk)(\?|#|$)/i.test(lower)) return false;
    return true;
  };

  useEffect(() => {
    if (isOpen) {
      const cleanUrl = typeof defaultUrl === "string" ? defaultUrl.trim() : (defaultUrl as any)?.url || "";
      setUrl(cleanUrl);
      setCustomFilename(defaultFilename || "");
      setMediaInfo(null);
      setSelectedFormatId("");
      setSelectedAudioId("");
      setErrorMessage(null);
      setSubmitting(false);
      setSequential(false);
      if (cleanUrl) {
        handleUrlDetection(cleanUrl);
      }
    }
  }, [isOpen, defaultUrl, defaultFilename]);

  const handleUrlChange = (newUrl: string) => {
    setUrl(newUrl);
    setMediaInfo(null);
    setCustomFilename("");
    setSelectedFormatId("");
    setSelectedAudioId("");
    setErrorMessage(null);
    handleUrlDetection(newUrl);
  };

  const handleUrlDetection = (inputUrl: string) => {
    let clean = inputUrl.trim();
    if (clean.startsWith("@[") && clean.endsWith("]")) {
      clean = clean.slice(2, -1).trim();
    }
    clean = clean.replace(/^["'`[]+|["'`\]]+$/g, "").trim();

    const lower = clean.toLowerCase();

    if (
      lower.startsWith("magnet:") ||
      lower.endsWith(".torrent") ||
      lower.includes(".torrent")
    ) {
      setTab("torrent");
      return;
    }

    const isDirectFile =
      lower.endsWith(".mkv") ||
      lower.endsWith(".mp4") ||
      lower.endsWith(".avi") ||
      lower.endsWith(".zip") ||
      lower.endsWith(".rar") ||
      lower.endsWith(".7z") ||
      lower.endsWith(".tar") ||
      lower.endsWith(".gz") ||
      lower.endsWith(".iso") ||
      lower.endsWith(".exe") ||
      lower.endsWith(".msi") ||
      lower.endsWith(".pdf");

    if (
      !isDirectFile &&
      (lower.includes("youtube.com") ||
        lower.includes("youtu.be") ||
        lower.includes("kick.com") ||
        lower.includes("twitch.tv") ||
        lower.includes("instagram.com") ||
        lower.includes("facebook.com") ||
        lower.includes("tiktok.com") ||
        lower.includes("twitter.com") ||
        lower.includes("x.com") ||
        lower.includes("vimeo.com") ||
        lower.includes("reddit.com") ||
        lower.includes("dailymotion.com") ||
        lower.includes("bilibili.com") ||
        lower.includes("m3u8") ||
        lower.includes("/watch") ||
        lower.includes("/video") ||
        lower.includes("/reel") ||
        !lower.includes("."))
    ) {
      setTab("media");
    } else {
      setTab("direct");
    }
  };

  // Load default downloads directory
  useEffect(() => {
    const fetchDefaultDir = async () => {
      try {
        const androidBridge = (window as any).GMAndroidBridge;
        if (androidBridge?.getDownloadPath) {
          const dir = androidBridge.getDownloadPath();
          if (dir) {
            setSaveFolder(dir);
            return;
          }
        }
        const dir = await invoke<string>("get_default_download_dir");
        if (dir) setSaveFolder(dir);
      } catch (err) {
        setSaveFolder("D:\\Downloads");
      }
    };
    fetchDefaultDir();
  }, []);

  // Auto-inspect URL when modal is open and URL changes (600 ms debounce)
  useEffect(() => {
    if (!isOpen) {
      lastInspectedUrl.current = "";
      return;
    }
    const cleanUrl = url.trim();
    if (!shouldAutoInspectUrl(cleanUrl)) return;
    if (cleanUrl === lastInspectedUrl.current || isFetchingMedia) return;

    const timer = window.setTimeout(() => {
      if (cleanUrl === lastInspectedUrl.current || isFetchingMedia) return;
      lastInspectedUrl.current = cleanUrl;
      void handleFetchMediaInfo(cleanUrl);
    }, 600);

    return () => window.clearTimeout(timer);
  }, [url, isOpen]);

  useEffect(() => {
    if (isOpen && defaultBatchText) {
      setBatchText(defaultBatchText);
      setTab("batch");
    }
    if (isOpen) {
      setDuplicatePrompt(null);
    }
  }, [isOpen, defaultBatchText]);

  if (!isOpen) return null;

  const handlePasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text && text.trim()) {
        const clean = text.trim();
        handleUrlChange(clean);
        if (shouldAutoInspectUrl(clean)) {
          lastInspectedUrl.current = clean;
          void handleFetchMediaInfo(clean);
        }
      }
    } catch (e) {
      console.error("Paste clipboard error:", e);
    }
  };

  const handleSelectFolder = async () => {
    try {
      const androidBridge = (window as any).GMAndroidBridge;
      if (androidBridge?.chooseFolder) {
        (window as any).onAndroidFolderSelected = (folder: string) => {
          if (folder) setSaveFolder(folder);
        };
        androidBridge.chooseFolder();
        return;
      }
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: saveFolder || undefined,
      });
      if (selected && typeof selected === "string") {
        setSaveFolder(selected);
      }
    } catch (err) {
      console.error("Failed to select folder:", err);
    }
  };

  const handleBrowseTorrentFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Torrent Files (*.torrent)", extensions: ["torrent"] }],
      });
      if (selected && typeof selected === "string") {
        setUrl(selected);
        setTab("torrent");
      }
    } catch (e) {
      console.error("Browse torrent file error:", e);
    }
  };

  const executeDirectSubmit = async (filenameOverride?: string) => {
    const cleanUrl = url.trim();
    if (!cleanUrl) return;
    await onAddDirectDownload(
      cleanUrl,
      saveFolder,
      filenameOverride !== undefined ? filenameOverride : customFilename.trim(),
      autoConnections ? 8 : connections,
      lowDiskSpace,
      {
        autoConnections,
        speedLimitKb: speedLimitKb > 0 ? speedLimitKb : 0,
        priority,
        expectedChecksum: expectedChecksum.trim(),
        postDownloadAction,
        authUsername: authUsername.trim(),
        authPassword,
      }
    );
    onClose();
  };

  const handleDirectSubmit = async (skipDuplicateCheck = false) => {
    const cleanUrl = url.trim();
    if (!cleanUrl) return;

    try {
      setSubmitting(true);
      setErrorMessage(null);
      if (!skipDuplicateCheck) {
        try {
          const dup = await invoke<DuplicateCheckResult>("check_duplicate_download", {
            url: cleanUrl,
            filename: customFilename.trim(),
            saveFolder: saveFolder || null,
            expectedSize: null,
          });
          if (dup?.isDuplicate) {
            setDuplicatePrompt({ mode: "direct", result: dup });
            setSubmitting(false);
            return;
          }
        } catch {}
      }
      await executeDirectSubmit();
    } catch (err: any) {
      setErrorMessage(err?.message || err?.toString() || "Failed to start direct download");
    } finally {
      setSubmitting(false);
    }
  };

  const handleBatchSubmit = async () => {
    if (batchPreviewUrls.length === 0) return;
    try {
      setSubmitting(true);
      setErrorMessage(null);
      for (const batchUrl of batchPreviewUrls) {
        await onAddDirectDownload(
          batchUrl,
          saveFolder,
          "",
          autoConnections ? 8 : connections,
          lowDiskSpace,
          {
            autoConnections,
            speedLimitKb: speedLimitKb > 0 ? speedLimitKb : 0,
            priority,
            postDownloadAction,
            authUsername: authUsername.trim(),
            authPassword,
          }
        );
      }
      onClose();
    } catch (err: any) {
      setErrorMessage(err?.message || err?.toString() || "Batch import failed");
    } finally {
      setSubmitting(false);
    }
  };

  const handleFetchMediaInfo = async (overrideUrl?: string) => {
    let cleanUrl = (overrideUrl ?? url).trim();
    if (cleanUrl.startsWith("@[") && cleanUrl.endsWith("]")) {
      cleanUrl = cleanUrl.slice(2, -1).trim();
    }
    if (!cleanUrl) return;
    lastInspectedUrl.current = cleanUrl;
    const seq = ++inspectSeq.current;
    try {
      setIsFetchingMedia(true);
      setErrorMessage(null);
      let parsed: any;
      const androidBridge = (window as any).GMAndroidBridge;
      if (androidBridge?.fetchMediaInfo) {
        const callbackId = `fetch-${Date.now()}`;
        parsed = await new Promise((resolve, reject) => {
          (window as any).onAndroidFetchSuccess = (cbId: string, info: any) => {
            if (cbId === callbackId) resolve(info);
          };
          (window as any).onAndroidFetchError = (cbId: string, err: string) => {
            if (cbId === callbackId) reject(new Error(err));
          };
          androidBridge.fetchMediaInfo(cleanUrl, callbackId);
        });
      } else {
        try {
          parsed = await invoke("fetch_media_info", {
            url: cleanUrl,
            cookiesBrowser: null,
            cookiesFile: null,
            proxy: null,
            discoveryArgs: null,
          });
        } catch (err) {
          parsed = await invoke("ytdlp_get_info", { url: cleanUrl });
        }
      }

      if (inspectSeq.current !== seq) return;

      if (typeof parsed === "string") {
        try { parsed = JSON.parse(parsed); } catch {}
      }

      const rawFormats = parsed.videoStreams || parsed.video_streams || parsed.formats || [];
      const rawAudio = parsed.audioStreams || parsed.audio_streams || [];

      const videoStreams = rawFormats.map((f: any) => ({
        id: String(f.id || f.format_id),
        label: f.label || `${f.resolution || (f.height ? `${f.height}p` : "Video")} • ${(f.ext || "mp4").toUpperCase()} • ${f.fps ? `${Math.round(f.fps)}fps` : ""}`,
        ext: f.ext || "mp4",
      }));

      const audioStreams = rawAudio.map((f: any) => ({
        id: String(f.id || f.format_id),
        label: f.label || `${f.ext || "audio"} • ${f.abr ? `${Math.round(f.abr)}kbps` : "Audio"}`,
        ext: f.ext || "m4a",
      }));

      let titleToUse = parsed.title || "";
      if (
        titleToUse.toLowerCase() === "master" ||
        titleToUse.toLowerCase() === "playlist" ||
        titleToUse.toLowerCase() === "index"
      ) {
        if (cleanUrl.includes("kick.com")) {
          titleToUse = "Kick Stream / VOD";
        } else {
          titleToUse = "";
        }
      }

      const info: MediaInfo = {
        title: titleToUse || "Online Media",
        uploader: parsed.uploader || "",
        duration: parsed.duration || 0,
        thumbnail: parsed.thumbnail || "",
        videoStreams,
        audioStreams,
        subtitles: parsed.subtitles || [],
      };

      setMediaInfo(info);
      setCustomFilename(titleToUse);
      if (videoStreams.length > 0) setSelectedFormatId(videoStreams[0].id);
      if (audioStreams.length > 0) setSelectedAudioId(audioStreams[0].id);
    } catch (err: any) {
      if (inspectSeq.current !== seq) return;
      setErrorMessage("Could not inspect stream. You can still click 'Download Media' directly: " + (err?.message || err?.toString() || "Stream check failed"));
    } finally {
      if (inspectSeq.current === seq) {
        setIsFetchingMedia(false);
      }
    }
  };

  const executeMediaSubmit = async (filenameOverride?: string, forceOverwrite = false) => {
    let cleanUrl = url.trim();
    if (cleanUrl.startsWith("@[") && cleanUrl.endsWith("]")) {
      cleanUrl = cleanUrl.slice(2, -1).trim();
    }
    if (!cleanUrl) return;
    const isAudio = container.toLowerCase().includes("audio");
    await onAddMediaDownload({
      url: cleanUrl,
      saveFolder: saveFolder || "",
      customFilename:
        filenameOverride !== undefined
          ? filenameOverride
          : customFilename.trim() || mediaInfo?.title || "",
      downloadMode: isAudio ? "audio" : "video",
      videoStreamId: isAudio ? "" : selectedFormatId || "best",
      audioStreamId: selectedAudioId || "best",
      container: container || "MP4 (Video)",
      enableClip: Boolean(enableClip),
      startTime: enableClip ? startTime || "00:00:00" : "00:00:00",
      endTime: enableClip ? endTime || "00:00:10" : "00:00:10",
      lowDiskSpace: Boolean(lowDiskSpace),
      overwriteFiles: forceOverwrite,
      aria2Connections: autoConnections ? 16 : Math.min(16, connections),
      limitSpeed: speedLimitKb > 0,
      speedLimitKb: speedLimitKb > 0 ? speedLimitKb : 0,
      priority,
      expectedChecksum: expectedChecksum.trim(),
      postDownloadAction,
      authUsername: authUsername.trim(),
      authPassword,
      duration: mediaInfo?.duration && mediaInfo.duration > 0 ? mediaInfo.duration : undefined,
      proxy: "",
      cookiesFile: "",
      cookiesBrowser: "",
      ytdlpDiscoveryArgs: "",
      ytdlpDownloadArgs: "",
      postProcessorArgs: "",
    });
    onClose();
  };

  const handleMediaSubmit = async (skipDuplicateCheck = false) => {
    let cleanUrl = url.trim();
    if (cleanUrl.startsWith("@[") && cleanUrl.endsWith("]")) {
      cleanUrl = cleanUrl.slice(2, -1).trim();
    }
    if (!cleanUrl) return;
    try {
      setSubmitting(true);
      setErrorMessage(null);
      if (!skipDuplicateCheck) {
        try {
          const dup = await invoke<DuplicateCheckResult>("check_duplicate_download", {
            url: cleanUrl,
            filename: customFilename.trim() || mediaInfo?.title || "",
            saveFolder: saveFolder || null,
            expectedSize: null,
          });
          if (dup?.isDuplicate) {
            setDuplicatePrompt({ mode: "media", result: dup });
            setSubmitting(false);
            return;
          }
        } catch {}
      }
      await executeMediaSubmit();
    } catch (err: any) {
      setErrorMessage(err?.message || err?.toString() || "Failed to start media download");
    } finally {
      setSubmitting(false);
    }
  };

  const handleTorrentSubmit = async () => {
    let cleanUrl = url.trim();
    if (cleanUrl.startsWith("@[") && cleanUrl.endsWith("]")) {
      cleanUrl = cleanUrl.slice(2, -1).trim();
    }
    cleanUrl = cleanUrl.replace(/^["'`[]+|["'`\]]+$/g, "").trim();
    if (!cleanUrl) return;
    try {
      setSubmitting(true);
      setErrorMessage(null);
      if (onAddTorrentDownload) {
        await onAddTorrentDownload(cleanUrl, saveFolder, sequential);
      } else {
        await invoke("add_torrent", {
          request: {
            source: cleanUrl,
            saveFolder: saveFolder || null,
            paused: false,
            sequential,
          },
        });
      }
      onClose();
    } catch (err: any) {
      setErrorMessage("Failed to start torrent: " + (err?.message || err?.toString() || "Unknown error"));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
      <div className="w-full max-w-xl rounded-xl border border-zr bg-zr-surface shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150 transition-colors">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-zr bg-zr-tableHead transition-colors">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-full overflow-hidden flex items-center justify-center bg-transparent shrink-0">
              <img src="/icon.png" alt="ZR Downloader" className="w-full h-full object-contain" />
            </div>
            <div>
              <h2 className="font-semibold text-[14px] text-zr-primary">
                Add New Download
              </h2>
              <p className="text-[12px] text-zr-secondary">
                Segmented HTTP Engine, Media Extractor & BitTorrent
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Selection */}
        <div className="flex border-b border-zr bg-zr-surface px-5 pt-2.5 space-x-4 overflow-x-auto transition-colors">
          <button
            onClick={() => setTab("direct")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              tab === "direct"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Zap className="w-4 h-4 text-blue-600 dark:text-blue-400" />
            <span>Direct File</span>
          </button>
          <button
            onClick={() => setTab("media")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              tab === "media"
                ? "border-purple-600 dark:border-purple-400 text-purple-600 dark:text-purple-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Film className="w-4 h-4 text-purple-600 dark:text-purple-400" />
            <span>Social & Video</span>
          </button>
          <button
            onClick={() => setTab("batch")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              tab === "batch"
                ? "border-emerald-600 dark:border-emerald-400 text-emerald-600 dark:text-emerald-400"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <ListPlus className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
            <span>Batch Import</span>
          </button>
          <button
            onClick={() => setTab("torrent")}
            className={`pb-2.5 text-[12.5px] font-semibold flex items-center space-x-1.5 border-b-2 whitespace-nowrap transition-all ${
              tab === "torrent"
                ? "border-orange-500 text-orange-500"
                : "border-transparent text-zr-secondary hover:text-zr-primary"
            }`}
          >
            <Radio className="w-4 h-4 text-orange-500" />
            <span>BitTorrent</span>
          </button>
        </div>

        {/* Form Body */}
        <div className="p-5 space-y-4 max-h-[75vh] overflow-y-auto">
          {errorMessage && (
            <div className="p-3 rounded-lg bg-red-50 dark:bg-red-950/40 border border-red-300 dark:border-red-900/60 text-red-700 dark:text-red-400 text-[13px] flex items-center space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 text-red-600 dark:text-red-400" />
              <span>{errorMessage}</span>
            </div>
          )}

          {duplicatePrompt && (
            <div className="p-3.5 rounded-xl bg-amber-500/10 border border-amber-500/40 space-y-2.5">
              <div className="flex items-start space-x-2">
                <AlertTriangle className="w-4 h-4 text-amber-500 mt-0.5 shrink-0" />
                <div className="text-[12.5px] text-zr-primary">
                  <div className="font-semibold">Duplicate Download Detected</div>
                  <div className="text-zr-secondary mt-0.5">{duplicatePrompt.result.reason}</div>
                </div>
              </div>
              <div className="flex flex-wrap gap-2 justify-end">
                <button
                  type="button"
                  onClick={() => {
                    setDuplicatePrompt(null);
                    onClose();
                  }}
                  className="px-3 py-1.5 rounded-lg border border-zr bg-zr-surface text-[12px] font-semibold text-zr-secondary hover:text-zr-primary"
                >
                  Skip
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    const suggested = duplicatePrompt.result.suggestedFilename;
                    const mode = duplicatePrompt.mode;
                    setDuplicatePrompt(null);
                    setCustomFilename(suggested);
                    if (mode === "direct") await executeDirectSubmit(suggested);
                    else await executeMediaSubmit(suggested, false);
                  }}
                  className="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-[12px] font-semibold"
                >
                  Rename ({duplicatePrompt.result.suggestedFilename})
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    const mode = duplicatePrompt.mode;
                    setDuplicatePrompt(null);
                    if (mode === "direct") await executeDirectSubmit();
                    else await executeMediaSubmit(undefined, true);
                  }}
                  className="px-3 py-1.5 rounded-lg bg-amber-600 hover:bg-amber-700 text-white text-[12px] font-semibold"
                >
                  Overwrite
                </button>
              </div>
            </div>
          )}

          {tab === "batch" ? (
            <div className="space-y-3">
              <div className="space-y-1">
                <label className="text-[12.5px] font-medium text-zr-secondary">
                  URLs or Patterns (e.g. https://example.com/file[001-100].zip)
                </label>
                <textarea
                  rows={5}
                  value={batchText}
                  onChange={(e) => setBatchText(e.target.value)}
                  placeholder={"https://example.com/archive[001-010].zip\nhttps://example.com/another-file.iso"}
                  className="w-full px-3 py-2 rounded-lg text-[12.5px] font-mono border border-zr bg-zr-surface text-zr-primary focus:border-zr-accent focus:outline-hidden"
                />
              </div>
              <div className="rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40 p-3 space-y-1.5">
                <div className="flex justify-between text-[12px] font-semibold text-zr-primary">
                  <span>Expanded Preview List</span>
                  <span className="text-emerald-600 dark:text-emerald-400">
                    {batchPreviewUrls.length} URL{batchPreviewUrls.length === 1 ? "" : "s"}
                  </span>
                </div>
                <div className="max-h-32 overflow-y-auto space-y-0.5 font-mono text-[11px] text-zr-secondary">
                  {batchPreviewUrls.slice(0, 50).map((u, idx) => (
                    <div key={idx} className="truncate">
                      {idx + 1}. {u}
                    </div>
                  ))}
                  {batchPreviewUrls.length > 50 && (
                    <div className="text-zr-muted italic">
                      ...and {batchPreviewUrls.length - 50} more URLs
                    </div>
                  )}
                </div>
              </div>
            </div>
          ) : (
            <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <label className="text-[13px] font-medium text-zr-secondary">
                {tab === "torrent" ? "Magnet Link or .torrent File Path" : "Target Download URL"}
              </label>
              <div className="flex items-center space-x-3">
                {tab === "torrent" && (
                  <button
                    type="button"
                    onClick={handleBrowseTorrentFile}
                    className="text-[12px] font-semibold text-orange-500 hover:underline flex items-center space-x-1"
                  >
                    <FileCode className="w-3.5 h-3.5" />
                    <span>Browse .torrent</span>
                  </button>
                )}
                <button
                  type="button"
                  onClick={handlePasteClipboard}
                  className="text-[12px] font-semibold text-zr-accent hover:underline flex items-center space-x-1"
                >
                  <Clipboard className="w-3.5 h-3.5" />
                  <span>Paste from Clipboard</span>
                </button>
              </div>
            </div>
            <div className="flex space-x-2">
              <input
                type="text"
                value={url}
                onChange={(e) => handleUrlChange(e.target.value)}
                onPaste={(e) => {
                  const pasted = e.clipboardData.getData("text")?.trim();
                  if (pasted) {
                    e.preventDefault();
                    handleUrlChange(pasted);
                    if (shouldAutoInspectUrl(pasted)) {
                      lastInspectedUrl.current = pasted;
                      void handleFetchMediaInfo(pasted);
                    }
                  }
                }}
                placeholder={
                  tab === "direct"
                    ? "Paste direct link (.zip, .exe, .mkv, .iso, .pdf)..."
                    : tab === "torrent"
                    ? "Paste magnet:?xt=urn:btih:... or click Browse .torrent"
                    : "Paste YouTube, Kick, Facebook, Instagram, or TikTok link..."
                }
                className="flex-1 px-3 py-2 rounded-lg text-[13px] font-mono border border-zr bg-zr-surface text-zr-primary placeholder:text-zr-muted focus:border-zr-accent focus:outline-hidden transition-colors"
                autoFocus
              />
              {tab === "media" && (
                <button
                  type="button"
                  onClick={() => {
                    lastInspectedUrl.current = "";
                    void handleFetchMediaInfo();
                  }}
                  disabled={isFetchingMedia || !url.trim()}
                  className="px-4 py-2 rounded-lg bg-purple-600 hover:bg-purple-700 text-white text-[13px] font-semibold flex items-center space-x-1.5 transition-colors disabled:opacity-60"
                >
                  {isFetchingMedia ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <span>Inspect</span>
                  )}
                </button>
              )}
            </div>
          </div>
          )}

          {/* Save Destination Folder */}
          <div className="space-y-1.5">
            <label className="text-[13px] font-medium text-zr-secondary">
              Save Destination
            </label>
            <div className="flex space-x-2">
              <input
                type="text"
                value={saveFolder}
                onChange={(e) => setSaveFolder(e.target.value)}
                className="flex-1 px-3 py-2 rounded-lg text-[13px] font-mono border border-zr bg-zr-surface text-zr-primary focus:border-zr-accent focus:outline-hidden transition-colors"
              />
              <button
                type="button"
                onClick={handleSelectFolder}
                className="w-10 h-10 rounded-lg border border-zr bg-zr-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-zr-secondary hover:text-zr-primary flex items-center justify-center transition-colors shrink-0"
                title="Browse folder"
                aria-label="Browse folder"
              >
                <Folder className="w-4 h-4 text-zr-accent" />
              </button>
            </div>
          </div>

          {/* Custom Filename (for Direct & Media) */}
          {tab !== "torrent" && (
            <div className="space-y-1.5">
              <label className="text-[13px] font-medium text-zr-secondary">
                Custom Filename <span className="text-zr-muted font-normal">(Optional)</span>
              </label>
              <input
                type="text"
                value={customFilename}
                onChange={(e) => setCustomFilename(e.target.value)}
                placeholder="Leave empty to use server original name"
                className="w-full px-3 py-2 rounded-lg text-[14px] font-semibold border border-zr bg-zr-surface text-zr-primary placeholder:text-zr-muted placeholder:font-normal focus:border-zr-accent focus:outline-hidden transition-colors"
              />
            </div>
          )}

          {/* Low Disk Space Optimization Switch (Direct & Media) */}
          {tab !== "torrent" && (
            <div className="p-3 rounded-xl border border-blue-500/25 bg-blue-500/5 dark:bg-blue-900/10 space-y-1.5">
              <div className="flex items-center justify-between">
                <div className="flex items-center space-x-2">
                  <HardDrive className="w-4 h-4 text-blue-500" />
                  <span className="font-semibold text-[13px] text-zr-primary">
                    Low Disk Space Mode (Zero-Temp In-Place)
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
              <p className="text-[12px] text-zr-secondary leading-relaxed">
                Saves 50% SSD space on large downloads (e.g. 20 GB file uses strictly 20 GB instead of 40 GB). Data streams directly into the final destination file without duplicate temporary files or assembly delay.
              </p>
            </div>
          )}

          {/* Direct & Batch Mode Specifics: Simultaneous Segments (4/8/16/24/32 + Auto) */}
          {(tab === "direct" || tab === "batch") && (
            <div className="space-y-2 pt-2 border-t border-zr">
              <div className="flex items-center justify-between text-[13px]">
                <span className="font-medium text-zr-secondary">
                  Simultaneous Connections
                </span>
                <span className="font-semibold tabular-nums text-zr-accent">
                  {autoConnections ? "Auto (8 -> 32)" : `${connections} Connections`}
                </span>
              </div>
              <div className="grid grid-cols-6 gap-1.5">
                {[4, 8, 16, 24, 32].map((num) => (
                  <button
                    key={num}
                    type="button"
                    title={num > 16 ? "some servers may block" : `${num} connections`}
                    onClick={() => {
                      setAutoConnections(false);
                      setConnections(num);
                    }}
                    className={`py-2 rounded-lg text-[12.5px] font-semibold border transition-colors ${
                      !autoConnections && connections === num
                        ? "bg-blue-600 border-blue-600 text-white"
                        : "bg-zr-surface border-zr text-zr-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-zr-primary"
                    }`}
                  >
                    {num}
                  </button>
                ))}
                <button
                  type="button"
                  title="Start 8, +4 every 5s while speed grows >10%, cap 32"
                  onClick={() => {
                    setAutoConnections(true);
                    setConnections(8);
                  }}
                  className={`py-2 rounded-lg text-[12.5px] font-semibold border transition-colors ${
                    autoConnections
                      ? "bg-blue-600 border-blue-600 text-white"
                      : "bg-zr-surface border-zr text-zr-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-zr-primary"
                  }`}
                >
                  Auto
                </button>
              </div>
              {(autoConnections || connections > 16) && (
                <p
                  className="text-[11.5px] text-amber-600 dark:text-amber-400 flex items-center space-x-1.5"
                  title="some servers may block"
                >
                  <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                  <span>Warning: &gt;16 connections — some servers may block excessive segments.</span>
                </p>
              )}
            </div>
          )}

          {/* Optional Advanced Settings: Priority, Task Speed Limit, Checksum (MD5/SHA-256), Auth, Post-Download Action */}
          {tab !== "torrent" && (
            <div className="pt-2 border-t border-zr space-y-2">
              <button
                type="button"
                onClick={() => setShowAdvanced((v) => !v)}
                className="text-[12px] font-semibold text-zr-accent hover:underline"
              >
                {showAdvanced
                  ? "Hide Advanced (Checksum, Priority, Task Limit, Auth & Post-Action)"
                  : "+ Advanced (Checksum, Priority, Task Limit, Auth & Post-Action)"}
              </button>
              {showAdvanced && (
                <div className="p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40 space-y-2.5 text-[12px]">
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="text-zr-secondary font-medium block mb-1">
                        Queue Priority
                      </label>
                      <select
                        value={priority}
                        onChange={(e) =>
                          setPriority(e.target.value as "high" | "normal" | "low")
                        }
                        className="w-full px-2.5 py-1.5 rounded border border-zr bg-zr-surface text-zr-primary"
                      >
                        <option value="high">High</option>
                        <option value="normal">Normal</option>
                        <option value="low">Low</option>
                      </select>
                    </div>
                    <div>
                      <label className="text-zr-secondary font-medium block mb-1">
                        Per-Task Speed Limit (KB/s, 0 = default)
                      </label>
                      <input
                        type="number"
                        min={0}
                        value={speedLimitKb}
                        onChange={(e) =>
                          setSpeedLimitKb(Math.max(0, parseInt(e.target.value, 10) || 0))
                        }
                        className="w-full px-2.5 py-1.5 rounded border border-zr bg-zr-surface text-zr-primary font-mono"
                      />
                    </div>
                  </div>
                  {tab !== "batch" && (
                    <div>
                      <label className="text-zr-secondary font-medium block mb-1">
                        Expected MD5 / SHA-256 Checksum (Optional)
                      </label>
                      <input
                        type="text"
                        value={expectedChecksum}
                        onChange={(e) => setExpectedChecksum(e.target.value)}
                        placeholder="Optional hex MD5 (32 chars) or SHA-256 (64 chars)..."
                        className="w-full px-2.5 py-1.5 rounded border border-zr bg-zr-surface text-zr-primary font-mono"
                      />
                    </div>
                  )}
                  <div className="grid grid-cols-2 gap-2">
                    <input
                      type="text"
                      value={authUsername}
                      onChange={(e) => setAuthUsername(e.target.value)}
                      placeholder="HTTP Basic Auth Username (optional)"
                      className="px-2.5 py-1.5 rounded border border-zr bg-zr-surface text-zr-primary"
                    />
                    <input
                      type="password"
                      value={authPassword}
                      onChange={(e) => setAuthPassword(e.target.value)}
                      placeholder="HTTP Basic Auth Password (optional)"
                      className="px-2.5 py-1.5 rounded border border-zr bg-zr-surface text-zr-primary"
                    />
                  </div>
                  <div>
                    <label className="text-zr-secondary font-medium block mb-1">
                      Post-Download Action Override
                    </label>
                    <select
                      value={postDownloadAction}
                      onChange={(e) => setPostDownloadAction(e.target.value)}
                      className="w-full px-2.5 py-1.5 rounded border border-zr bg-zr-surface text-zr-primary"
                    >
                      <option value="">Use Global Setting</option>
                      <option value="none">Do Nothing</option>
                      <option value="open_file">Open Downloaded File</option>
                      <option value="open_folder">Open Folder</option>
                      <option value="defender_scan">Scan with Windows Defender</option>
                    </select>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Torrent Mode Specifics */}
          {tab === "torrent" && (
            <div className="space-y-3 pt-2 border-t border-zr">
              <div className="p-3 rounded-lg border border-zr bg-orange-500/5 dark:bg-orange-500/10 space-y-2">
                <div className="flex items-center space-x-2">
                  <Radio className="w-4 h-4 text-orange-500" />
                  <span className="font-semibold text-[13.5px] text-zr-primary">
                    BitTorrent Swarm Engine
                  </span>
                </div>
                <p className="text-[12px] text-zr-secondary">
                  Connects to Mainline DHT, Peer Exchange (PEX), and UDP trackers to download multi-file torrents.
                </p>
              </div>

              <div className="flex items-center space-x-2 pt-1">
                <input
                  type="checkbox"
                  id="seq-torrent"
                  checked={sequential}
                  onChange={(e) => setSequential(e.target.checked)}
                  className="rounded border-zr text-orange-500 focus:ring-orange-500"
                />
                <label htmlFor="seq-torrent" className="text-[13px] font-medium text-zr-primary cursor-pointer">
                  Sequential download (prioritize first and last pieces for media preview)
                </label>
              </div>
            </div>
          )}

          {/* Media Mode Specifics */}
          {tab === "media" && mediaInfo && (
            <div className="space-y-3 pt-2 border-t border-zr">
              <div className="flex items-center space-x-3 p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40">
                {mediaInfo.thumbnail && (
                  <img
                    src={mediaInfo.thumbnail}
                    alt="Thumbnail"
                    className="w-16 h-10 object-cover rounded shrink-0"
                  />
                )}
                <div className="min-w-0 flex-1">
                  <h4 className="text-[14px] font-semibold text-zr-primary truncate">
                    {mediaInfo.title}
                  </h4>
                  <p className="text-[12px] text-zr-secondary truncate">
                    {mediaInfo.uploader}
                  </p>
                </div>
              </div>

              {/* Stream Picker */}
              {mediaInfo.videoStreams.length > 0 && (
                <div className="space-y-1.5">
                  <label className="text-[13px] font-medium text-zr-secondary">
                    Video Quality
                  </label>
                  <select
                    value={selectedFormatId}
                    onChange={(e) => setSelectedFormatId(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg text-[13px] font-semibold border border-zr bg-zr-surface text-zr-primary focus:border-zr-accent focus:outline-hidden transition-colors"
                  >
                    {mediaInfo.videoStreams.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.label}
                      </option>
                    ))}
                  </select>
                </div>
              )}

              {/* Container Format */}
              <div className="space-y-1.5">
                <label className="text-[13px] font-medium text-zr-secondary">
                  Format Container
                </label>
                <select
                  value={container}
                  onChange={(e) => setContainer(e.target.value as ContainerFormat)}
                  className="w-full px-3 py-2 rounded-lg text-[13px] font-semibold border border-zr bg-zr-surface text-zr-primary focus:border-zr-accent focus:outline-hidden transition-colors"
                >
                  {ALL_CONTAINERS.map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
              </div>

              {/* Clipper Toggle */}
              <div className="pt-1">
                <button
                  type="button"
                  onClick={() => setEnableClip(!enableClip)}
                  className="flex items-center space-x-1.5 text-[12.5px] font-semibold text-zr-accent hover:underline"
                >
                  <Scissors className="w-3.5 h-3.5" />
                  <span>{enableClip ? "Disable Timeframe Clipper" : "Enable Timeframe Clipper"}</span>
                </button>

                {enableClip && (
                  <div className="grid grid-cols-2 gap-3 mt-2 p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40">
                    <div>
                      <span className="text-[12px] font-medium text-zr-secondary">
                        Start (HH:MM:SS)
                      </span>
                      <input
                        type="text"
                        value={startTime}
                        onChange={(e) => setStartTime(e.target.value)}
                        className="w-full px-3 py-1.5 rounded-lg text-[13px] font-mono border border-zr bg-zr-surface text-zr-primary focus:border-zr-accent focus:outline-hidden transition-colors"
                      />
                    </div>
                    <div>
                      <span className="text-[12px] font-medium text-zr-secondary">
                        End (HH:MM:SS)
                      </span>
                      <input
                        type="text"
                        value={endTime}
                        onChange={(e) => setEndTime(e.target.value)}
                        className="w-full px-3 py-1.5 rounded-lg text-[13px] font-mono border border-zr bg-zr-surface text-zr-primary focus:border-zr-accent focus:outline-hidden transition-colors"
                      />
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="flex items-center justify-end space-x-3 px-5 py-3 border-t border-zr bg-zr-tableHead transition-colors">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-[13px] font-semibold text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
          >
            Cancel
          </button>

          {tab === "direct" ? (
            <button
              type="button"
              onClick={() => handleDirectSubmit(false)}
              disabled={submitting || !url.trim()}
              className="flex items-center space-x-2 px-5 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white font-semibold text-[13px] shadow-xs transition-colors disabled:opacity-60"
            >
              {submitting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Download className="w-4 h-4" />
              )}
              <span>Start High-Speed Download</span>
            </button>
          ) : tab === "batch" ? (
            <button
              type="button"
              onClick={handleBatchSubmit}
              disabled={submitting || batchPreviewUrls.length === 0}
              className="flex items-center space-x-2 px-5 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white font-semibold text-[13px] shadow-xs transition-colors disabled:opacity-60"
            >
              {submitting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <ListPlus className="w-4 h-4" />
              )}
              <span>Start All ({batchPreviewUrls.length})</span>
            </button>
          ) : tab === "torrent" ? (
            <button
              type="button"
              onClick={handleTorrentSubmit}
              disabled={submitting || !url.trim()}
              className="flex items-center space-x-2 px-5 py-2 rounded-lg bg-orange-600 hover:bg-orange-700 text-white font-semibold text-[13px] shadow-xs transition-colors disabled:opacity-60"
            >
              {submitting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Radio className="w-4 h-4" />
              )}
              <span>Start Torrent Download</span>
            </button>
          ) : (
            <button
              type="button"
              onClick={() => handleMediaSubmit(false)}
              disabled={submitting || !url.trim()}
              className="flex items-center space-x-2 px-5 py-2 rounded-lg bg-purple-600 hover:bg-purple-700 text-white font-semibold text-[13px] shadow-xs transition-colors disabled:opacity-60"
            >
              {submitting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Film className="w-4 h-4" />
              )}
              <span>Download Media</span>
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

