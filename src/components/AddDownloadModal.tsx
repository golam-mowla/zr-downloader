import React, { useState, useEffect } from "react";
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
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { MediaInfo, ContainerFormat, ALL_CONTAINERS } from "../types";

interface AddDownloadModalProps {
  isOpen: boolean;
  onClose: () => void;
  defaultUrl?: string;
  defaultFilename?: string;
  onAddDirectDownload: (
    url: string,
    savePath: string,
    filename: string,
    connections: number
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
  onAddDirectDownload,
  onAddMediaDownload,
  onAddTorrentDownload,
}) => {
  const [tab, setTab] = useState<"direct" | "media" | "torrent">("direct");

  // Direct & General State
  const [url, setUrl] = useState(defaultUrl);
  const [customFilename, setCustomFilename] = useState(defaultFilename);
  const [saveFolder, setSaveFolder] = useState("");
  const [connections, setConnections] = useState(16);
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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

  if (!isOpen) return null;

  const handlePasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text && text.trim()) {
        const clean = text.trim();
        handleUrlChange(clean);
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

  const handleDirectSubmit = async () => {
    const cleanUrl = url.trim();
    if (!cleanUrl) return;

    try {
      setSubmitting(true);
      setErrorMessage(null);
      await onAddDirectDownload(cleanUrl, saveFolder, customFilename.trim(), connections);
      onClose();
    } catch (err: any) {
      setErrorMessage(err?.message || err?.toString() || "Failed to start direct download");
    } finally {
      setSubmitting(false);
    }
  };

  const handleFetchMediaInfo = async () => {
    const cleanUrl = url.trim();
    if (!cleanUrl) return;
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
      setErrorMessage("Could not inspect stream. You can still click 'Download Media' directly: " + (err?.message || err?.toString() || "Stream check failed"));
    } finally {
      setIsFetchingMedia(false);
    }
  };

  const handleMediaSubmit = async () => {
    const cleanUrl = url.trim();
    if (!cleanUrl) return;
    try {
      setSubmitting(true);
      setErrorMessage(null);
      await onAddMediaDownload({
        url: cleanUrl,
        saveFolder,
        customFilename: customFilename.trim() || mediaInfo?.title || "",
        videoStreamId: selectedFormatId || "best",
        audioStreamId: selectedAudioId || "best",
        container,
        enableClip,
        startTime: enableClip ? startTime : undefined,
        endTime: enableClip ? endTime : undefined,
      });
      onClose();
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
      <div className="w-full max-w-xl rounded-xl border border-gdm bg-gdm-surface shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150 transition-colors">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-gdm bg-gdm-tableHead transition-colors">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-full overflow-hidden flex items-center justify-center bg-transparent shrink-0">
              <img src="/icon.png" alt="ZR Downloader" className="w-full h-full object-contain" />
            </div>
            <div>
              <h2 className="font-semibold text-[14px] text-gdm-primary">
                Add New Download
              </h2>
              <p className="text-[12px] text-gdm-secondary">
                Segmented HTTP Engine, Media Extractor & BitTorrent
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center text-gdm-secondary hover:text-gdm-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Selection */}
        <div className="flex border-b border-gdm bg-gdm-surface px-5 pt-2.5 space-x-5 transition-colors">
          <button
            onClick={() => setTab("direct")}
            className={`pb-2.5 text-[13px] font-semibold flex items-center space-x-2 border-b-2 transition-all ${
              tab === "direct"
                ? "border-blue-600 dark:border-blue-400 text-blue-600 dark:text-blue-400"
                : "border-transparent text-gdm-secondary hover:text-gdm-primary"
            }`}
          >
            <Zap className="w-4 h-4 text-blue-600 dark:text-blue-400" />
            <span>Direct File (Multi-Threaded)</span>
          </button>
          <button
            onClick={() => setTab("media")}
            className={`pb-2.5 text-[13px] font-semibold flex items-center space-x-2 border-b-2 transition-all ${
              tab === "media"
                ? "border-purple-600 dark:border-purple-400 text-purple-600 dark:text-purple-400"
                : "border-transparent text-gdm-secondary hover:text-gdm-primary"
            }`}
          >
            <Film className="w-4 h-4 text-purple-600 dark:text-purple-400" />
            <span>Social & Video (yt-dlp)</span>
          </button>
          <button
            onClick={() => setTab("torrent")}
            className={`pb-2.5 text-[13px] font-semibold flex items-center space-x-2 border-b-2 transition-all ${
              tab === "torrent"
                ? "border-orange-500 text-orange-500"
                : "border-transparent text-gdm-secondary hover:text-gdm-primary"
            }`}
          >
            <Radio className="w-4 h-4 text-orange-500" />
            <span>BitTorrent (Magnet / .torrent)</span>
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

          {/* URL / Path Input with Actions */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <label className="text-[13px] font-medium text-gdm-secondary">
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
                  className="text-[12px] font-semibold text-gdm-accent hover:underline flex items-center space-x-1"
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
                placeholder={
                  tab === "direct"
                    ? "Paste direct link (.zip, .exe, .mkv, .iso, .pdf)..."
                    : tab === "torrent"
                    ? "Paste magnet:?xt=urn:btih:... or click Browse .torrent"
                    : "Paste YouTube, Kick, Facebook, Instagram, or TikTok link..."
                }
                className="flex-1 px-3 py-2 rounded-lg text-[13px] font-mono border border-gdm bg-gdm-surface text-gdm-primary placeholder:text-gdm-muted focus:border-gdm-accent focus:outline-hidden transition-colors"
                autoFocus
              />
              {tab === "media" && (
                <button
                  type="button"
                  onClick={handleFetchMediaInfo}
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

          {/* Save Destination Folder */}
          <div className="space-y-1.5">
            <label className="text-[13px] font-medium text-gdm-secondary">
              Save Destination
            </label>
            <div className="flex space-x-2">
              <input
                type="text"
                value={saveFolder}
                onChange={(e) => setSaveFolder(e.target.value)}
                className="flex-1 px-3 py-2 rounded-lg text-[13px] font-mono border border-gdm bg-gdm-surface text-gdm-primary focus:border-gdm-accent focus:outline-hidden transition-colors"
              />
              <button
                type="button"
                onClick={handleSelectFolder}
                className="w-10 h-10 rounded-lg border border-gdm bg-gdm-surface hover:bg-slate-100 dark:hover:bg-slate-800 text-gdm-secondary hover:text-gdm-primary flex items-center justify-center transition-colors shrink-0"
                title="Browse folder"
                aria-label="Browse folder"
              >
                <Folder className="w-4 h-4 text-gdm-accent" />
              </button>
            </div>
          </div>

          {/* Custom Filename (for Direct & Media) */}
          {tab !== "torrent" && (
            <div className="space-y-1.5">
              <label className="text-[13px] font-medium text-gdm-secondary">
                Custom Filename <span className="text-gdm-muted font-normal">(Optional)</span>
              </label>
              <input
                type="text"
                value={customFilename}
                onChange={(e) => setCustomFilename(e.target.value)}
                placeholder="Leave empty to use server original name"
                className="w-full px-3 py-2 rounded-lg text-[14px] font-semibold border border-gdm bg-gdm-surface text-gdm-primary placeholder:text-gdm-muted placeholder:font-normal focus:border-gdm-accent focus:outline-hidden transition-colors"
              />
            </div>
          )}

          {/* Direct Mode Specifics: Simultaneous Segments */}
          {tab === "direct" && (
            <div className="space-y-2 pt-2 border-t border-gdm">
              <div className="flex items-center justify-between text-[13px]">
                <span className="font-medium text-gdm-secondary">
                  Simultaneous Download Segments (Threads)
                </span>
                <span className="font-semibold tabular-nums text-gdm-accent">
                  {connections} Parallel Connections
                </span>
              </div>
              <div className="grid grid-cols-4 gap-2">
                {[4, 8, 12, 16].map((num) => (
                  <button
                    key={num}
                    type="button"
                    onClick={() => setConnections(num)}
                    className={`py-2 rounded-lg text-[13px] font-semibold border transition-colors ${
                      connections === num
                        ? "bg-blue-600 border-blue-600 text-white"
                        : "bg-gdm-surface border-gdm text-gdm-secondary hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-gdm-primary"
                    }`}
                  >
                    {num} Threads
                  </button>
                ))}
              </div>
              <p className="text-[12px] text-gdm-muted">
                Splits file into concurrent HTTP Range slices for maximum bandwidth saturation.
              </p>
            </div>
          )}

          {/* Torrent Mode Specifics */}
          {tab === "torrent" && (
            <div className="space-y-3 pt-2 border-t border-gdm">
              <div className="p-3 rounded-lg border border-gdm bg-orange-500/5 dark:bg-orange-500/10 space-y-2">
                <div className="flex items-center space-x-2">
                  <Radio className="w-4 h-4 text-orange-500" />
                  <span className="font-semibold text-[13.5px] text-gdm-primary">
                    BitTorrent Swarm Engine
                  </span>
                </div>
                <p className="text-[12px] text-gdm-secondary">
                  Connects to Mainline DHT, Peer Exchange (PEX), and UDP trackers to download multi-file torrents.
                </p>
              </div>

              <div className="flex items-center space-x-2 pt-1">
                <input
                  type="checkbox"
                  id="seq-torrent"
                  checked={sequential}
                  onChange={(e) => setSequential(e.target.checked)}
                  className="rounded border-gdm text-orange-500 focus:ring-orange-500"
                />
                <label htmlFor="seq-torrent" className="text-[13px] font-medium text-gdm-primary cursor-pointer">
                  Sequential download (prioritize first and last pieces for media preview)
                </label>
              </div>
            </div>
          )}

          {/* Media Mode Specifics */}
          {tab === "media" && mediaInfo && (
            <div className="space-y-3 pt-2 border-t border-gdm">
              <div className="flex items-center space-x-3 p-3 rounded-lg border border-gdm bg-slate-50 dark:bg-slate-900/40">
                {mediaInfo.thumbnail && (
                  <img
                    src={mediaInfo.thumbnail}
                    alt="Thumbnail"
                    className="w-16 h-10 object-cover rounded shrink-0"
                  />
                )}
                <div className="min-w-0 flex-1">
                  <h4 className="text-[14px] font-semibold text-gdm-primary truncate">
                    {mediaInfo.title}
                  </h4>
                  <p className="text-[12px] text-gdm-secondary truncate">
                    {mediaInfo.uploader}
                  </p>
                </div>
              </div>

              {/* Stream Picker */}
              {mediaInfo.videoStreams.length > 0 && (
                <div className="space-y-1.5">
                  <label className="text-[13px] font-medium text-gdm-secondary">
                    Video Quality
                  </label>
                  <select
                    value={selectedFormatId}
                    onChange={(e) => setSelectedFormatId(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg text-[13px] font-semibold border border-gdm bg-gdm-surface text-gdm-primary focus:border-gdm-accent focus:outline-hidden transition-colors"
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
                <label className="text-[13px] font-medium text-gdm-secondary">
                  Format Container
                </label>
                <select
                  value={container}
                  onChange={(e) => setContainer(e.target.value as ContainerFormat)}
                  className="w-full px-3 py-2 rounded-lg text-[13px] font-semibold border border-gdm bg-gdm-surface text-gdm-primary focus:border-gdm-accent focus:outline-hidden transition-colors"
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
                  className="flex items-center space-x-1.5 text-[12.5px] font-semibold text-gdm-accent hover:underline"
                >
                  <Scissors className="w-3.5 h-3.5" />
                  <span>{enableClip ? "Disable Timeframe Clipper" : "Enable Timeframe Clipper"}</span>
                </button>

                {enableClip && (
                  <div className="grid grid-cols-2 gap-3 mt-2 p-3 rounded-lg border border-gdm bg-slate-50 dark:bg-slate-900/40">
                    <div>
                      <span className="text-[12px] font-medium text-gdm-secondary">
                        Start (HH:MM:SS)
                      </span>
                      <input
                        type="text"
                        value={startTime}
                        onChange={(e) => setStartTime(e.target.value)}
                        className="w-full px-3 py-1.5 rounded-lg text-[13px] font-mono border border-gdm bg-gdm-surface text-gdm-primary focus:border-gdm-accent focus:outline-hidden transition-colors"
                      />
                    </div>
                    <div>
                      <span className="text-[12px] font-medium text-gdm-secondary">
                        End (HH:MM:SS)
                      </span>
                      <input
                        type="text"
                        value={endTime}
                        onChange={(e) => setEndTime(e.target.value)}
                        className="w-full px-3 py-1.5 rounded-lg text-[13px] font-mono border border-gdm bg-gdm-surface text-gdm-primary focus:border-gdm-accent focus:outline-hidden transition-colors"
                      />
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="flex items-center justify-end space-x-3 px-5 py-3 border-t border-gdm bg-gdm-tableHead transition-colors">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-[13px] font-semibold text-gdm-secondary hover:text-gdm-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
          >
            Cancel
          </button>

          {tab === "direct" ? (
            <button
              type="button"
              onClick={() => handleDirectSubmit()}
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
              onClick={handleMediaSubmit}
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
