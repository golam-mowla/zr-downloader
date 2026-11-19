import React, { useState } from "react";
import {
  ClipboardPaste,
  Search,
  FolderOpen,
  Scissors,
  Download,
  PauseCircle,
  Film,
  Music,
  Subtitles,
  Sparkles,
  CheckCircle2,
  AlertCircle,
  Loader2,
  ExternalLink,
  TerminalSquare,
  FileEdit,
  ChevronDown,
  ChevronUp,
} from "lucide-react";
import {
  ALL_CONTAINERS,
  ContainerFormat,
  DownloadMode,
  DownloadOptions,
  DownloadProgress,
  DownloadTask,
  MediaInfo,
} from "../types";

interface DownloadTabProps {
  options: DownloadOptions;
  setOptions: React.Dispatch<React.SetStateAction<DownloadOptions>>;
  mediaInfo: MediaInfo | null;
  isFetching: boolean;
  fetchError: string | null;
  onFetchInfo: () => void;
  onBrowseFolder: () => void;
  onOpenFolder: () => void;
  onStartDownload: (overrideMode?: DownloadMode) => void;
  onCancelDownload: (taskId?: string) => void;
  onResumeTask?: (task: DownloadTask) => void;
  tasks: DownloadTask[];
  progress: DownloadProgress;
  logs: string[];
}

export const DownloadTab: React.FC<DownloadTabProps> = ({
  options,
  setOptions,
  mediaInfo,
  isFetching,
  fetchError,
  onFetchInfo,
  onBrowseFolder,
  onOpenFolder,
  onStartDownload,
  onCancelDownload,
  onResumeTask,
  tasks,
  progress,
  logs,
}) => {
  const [showLogs, setShowLogs] = useState(false);

  const updateOption = <K extends keyof DownloadOptions>(
    key: K,
    value: DownloadOptions[K]
  ) => {
    setOptions((prev) => ({ ...prev, [key]: value }));
  };

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        updateOption("url", text.trim());
      }
    } catch {
      // Fallback if clipboard permission denied
    }
  };

  const handleModeChange = (mode: DownloadMode) => {
    setOptions((prev) => {
      let nextContainer = prev.container;
      if (mode === "audio" && prev.container.includes("(Video)")) {
        nextContainer = "MP3 (Audio)";
      } else if (mode === "video" && prev.container.includes("(Audio)")) {
        nextContainer = "MP4 (Video)";
      }
      return {
        ...prev,
        downloadMode: mode,
        container: nextContainer,
        enableSubtitles: mode === "subtitles" ? true : prev.enableSubtitles,
      };
    });
  };

  const handleContainerChange = (container: ContainerFormat) => {
    setOptions((prev) => {
      let nextMode = prev.downloadMode;
      if (container.includes("(Audio)")) {
        nextMode = "audio";
      } else if (container.includes("(Video)")) {
        nextMode = "video";
      }
      return {
        ...prev,
        container,
        downloadMode: nextMode,
      };
    });
  };

  const formatDuration = (seconds?: number) => {
    if (!seconds || isNaN(seconds)) return "00:00";
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) {
      return `${h.toString().padStart(2, "0")}:${m
        .toString()
        .padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
    return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  };

  const formatFullDuration = (seconds?: number) => {
    if (!seconds || isNaN(seconds)) return "00:00:00";
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    return `${h.toString().padStart(2, "0")}:${m
      .toString()
      .padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  };

  const isBusy =
    progress.status === "downloading" || progress.status === "processing";

  return (
    <div className="space-y-4">
      {/* 1. MEDIA SOURCE URL CARD */}
      <div className="neon-card neon-border-cyan p-4 sm:p-5">
        <div className="flex items-center justify-between mb-2.5">
          <label className="text-xs font-extrabold uppercase tracking-wider text-slate-600 dark:text-slate-300 flex items-center gap-1.5">
            <Sparkles className="w-3.5 h-3.5 text-cyan-500" />
            Media Source Link (YouTube, Facebook, Instagram, TikTok, X, 1000+ Sites)
          </label>
          {mediaInfo && (
            <span className="text-[11px] font-bold text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-950/60 px-2.5 py-0.5 rounded-full border border-emerald-200 dark:border-emerald-800">
              ✔ {mediaInfo.videoStreams.length} Video &bull;{" "}
              {mediaInfo.audioStreams.length} Audio &bull;{" "}
              {mediaInfo.subtitles.length} Subtitles Found
            </span>
          )}
        </div>

        <div className="flex flex-col sm:flex-row gap-2.5">
          <button
            type="button"
            onClick={handlePaste}
            className="px-3.5 py-2.5 rounded-xl text-xs font-bold bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 flex items-center justify-center gap-1.5 shrink-0 transition-all"
          >
            <ClipboardPaste className="w-4 h-4 text-cyan-600" />
            Paste
          </button>

          <input
            type="text"
            value={options.url}
            onChange={(e) => updateOption("url", e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && options.url.trim() && !isFetching) {
                onFetchInfo();
              }
            }}
            placeholder="https://www.youtube.com/watch?v=... or paste any video URL"
            className="neon-input flex-1 px-4 py-2.5 text-sm font-medium"
          />

          <button
            type="button"
            onClick={() => onFetchInfo()}
            disabled={isFetching || !options.url.trim()}
            className="neon-btn-primary px-5 py-2.5 rounded-xl text-xs sm:text-sm font-extrabold flex items-center justify-center gap-2 shrink-0 disabled:opacity-50"
          >
            {isFetching ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Fetching Formats...
              </>
            ) : (
              <>
                <Search className="w-4 h-4" />
                Fetch Link Qualities
              </>
            )}
          </button>
        </div>

        {/* Fetch Error Alert */}
        {fetchError && !mediaInfo && (
          <div className="mt-3 p-3 rounded-xl bg-rose-50 dark:bg-rose-950/50 border border-rose-200 dark:border-rose-800 flex items-start gap-2.5 text-xs text-rose-700 dark:text-rose-300">
            <AlertCircle className="w-4 h-4 text-rose-600 shrink-0 mt-0.5" />
            <div className="flex-1">
              <p className="font-bold">Could not fetch link metadata:</p>
              <p className="mt-0.5 break-all opacity-90">{fetchError}</p>
              <p className="mt-1 text-[11px] text-rose-600 dark:text-rose-400 font-semibold">
                Tip: You can still click &ldquo;Start Download&rdquo; directly, or enable Browser Cookies inside ⚙️ Options &rarr; Downloader for private/age-restricted links.
              </p>
            </div>
          </div>
        )}

        {/* Fetched Media Info Preview Banner */}
        {mediaInfo && (
          <div className="mt-3.5 p-3 rounded-xl bg-gradient-to-r from-cyan-50/90 via-indigo-50/60 to-purple-50/80 dark:from-slate-800/90 dark:to-slate-900/90 border border-cyan-200 dark:border-cyan-800/70 flex flex-col sm:flex-row items-center gap-3.5">
            {mediaInfo.thumbnail ? (
              <img
                src={mediaInfo.thumbnail}
                alt={mediaInfo.title}
                className="w-32 h-20 object-cover rounded-lg border border-cyan-300 dark:border-cyan-700 shadow-sm shrink-0"
              />
            ) : (
              <div className="w-32 h-20 rounded-lg bg-cyan-100 dark:bg-slate-800 flex items-center justify-center shrink-0">
                <Film className="w-7 h-7 text-cyan-600" />
              </div>
            )}
            <div className="flex-1 min-w-0 text-center sm:text-left">
              <div className="flex flex-wrap items-center justify-center sm:justify-start gap-1.5 mb-1">
                <span className="px-2 py-0.5 text-[10px] font-extrabold uppercase rounded-md bg-emerald-100 dark:bg-emerald-950 text-emerald-700 dark:text-emerald-300 border border-emerald-300 dark:border-emerald-700">
                  ✔ Link Fetched
                </span>
                {mediaInfo.duration ? (
                  <span className="px-2 py-0.5 text-[10px] font-bold rounded-md bg-cyan-100 dark:bg-cyan-950 text-cyan-800 dark:text-cyan-300">
                    ⏱ Duration: {formatDuration(mediaInfo.duration)}
                  </span>
                ) : null}
                {mediaInfo.uploader ? (
                  <span className="px-2 py-0.5 text-[10px] font-bold rounded-md bg-purple-100 dark:bg-purple-950 text-purple-800 dark:text-purple-300">
                    👤 {mediaInfo.uploader}
                  </span>
                ) : null}
              </div>
              <div className="mt-1.5 space-y-1">
                <div className="flex items-center justify-between">
                  <label className="text-[11px] font-extrabold uppercase tracking-wide text-slate-700 dark:text-slate-300 flex items-center gap-1">
                    <FileEdit className="w-3 h-3 text-cyan-600" />
                    File Name (Rename Before Download)
                  </label>
                  {options.customFilename && options.customFilename !== mediaInfo.title && (
                    <button
                      type="button"
                      onClick={() => updateOption("customFilename", mediaInfo.title)}
                      className="text-[10px] font-bold text-indigo-600 dark:text-cyan-400 hover:underline"
                    >
                      Reset to detected title
                    </button>
                  )}
                </div>
                <input
                  type="text"
                  value={options.customFilename !== undefined ? options.customFilename : mediaInfo.title}
                  onChange={(e) => updateOption("customFilename", e.target.value)}
                  placeholder={mediaInfo.title}
                  className="neon-input w-full px-3 py-1.5 text-xs sm:text-sm font-bold text-slate-900 dark:text-white"
                  title="Edit output file name before downloading"
                />
              </div>
              <p className="text-[11px] text-slate-500 dark:text-slate-400 mt-1">
                All exact qualities, codecs &amp; subtitle tracks for this link are loaded below.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* 2. DYNAMIC QUALITIES & ALL 13 CONTAINERS + SUBTITLES + CLIPPER */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-4">
        {/* Left 7 Cols: Dynamic Quality & Container Picker */}
        <div className="lg:col-span-7 neon-card p-4 sm:p-5 space-y-4">
          <div className="flex items-center justify-between border-b border-slate-100 dark:border-slate-800 pb-2.5">
            <span className="text-xs font-extrabold uppercase tracking-wider text-indigo-600 dark:text-indigo-400 flex items-center gap-1.5">
              <Film className="w-4 h-4" />
              Dynamic Quality &amp; Container Format
            </span>
            <span className="text-[11px] font-bold text-slate-400">
              13 Containers Supported
            </span>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3.5">
            {/* Download Mode 3-Way Toggle */}
            <div>
              <label className="block text-xs font-bold text-slate-600 dark:text-slate-300 mb-1.5">
                Download Mode
              </label>
              <div className="grid grid-cols-3 gap-1.5 bg-slate-100 dark:bg-slate-800/90 p-1 rounded-xl border border-slate-200 dark:border-slate-700">
                <button
                  type="button"
                  onClick={() => handleModeChange("video")}
                  className={`py-2 px-2 rounded-lg text-xs font-extrabold flex items-center justify-center gap-1 transition-all ${
                    options.downloadMode === "video"
                      ? "bg-white dark:bg-slate-900 text-indigo-600 dark:text-cyan-400 shadow-sm border border-cyan-300 dark:border-cyan-700"
                      : "text-slate-600 dark:text-slate-400 hover:text-slate-900"
                  }`}
                >
                  <Film className="w-3.5 h-3.5" />
                  Video
                </button>
                <button
                  type="button"
                  onClick={() => handleModeChange("audio")}
                  className={`py-2 px-2 rounded-lg text-xs font-extrabold flex items-center justify-center gap-1 transition-all ${
                    options.downloadMode === "audio"
                      ? "bg-white dark:bg-slate-900 text-indigo-600 dark:text-cyan-400 shadow-sm border border-cyan-300 dark:border-cyan-700"
                      : "text-slate-600 dark:text-slate-400 hover:text-slate-900"
                  }`}
                >
                  <Music className="w-3.5 h-3.5" />
                  Audio
                </button>
                <button
                  type="button"
                  onClick={() => handleModeChange("subtitles")}
                  className={`py-2 px-2 rounded-lg text-xs font-extrabold flex items-center justify-center gap-1 transition-all ${
                    options.downloadMode === "subtitles"
                      ? "bg-white dark:bg-slate-900 text-emerald-600 dark:text-emerald-400 shadow-sm border border-emerald-300 dark:border-emerald-700"
                      : "text-slate-600 dark:text-slate-400 hover:text-slate-900"
                  }`}
                >
                  <Subtitles className="w-3.5 h-3.5" />
                  Subtitles
                </button>
              </div>
            </div>

            {/* Container Dropdown (All 13 Options from User's Screenshot) */}
            <div>
              <label className="block text-xs font-bold text-slate-600 dark:text-slate-300 mb-1.5">
                Container Format
              </label>
              <select
                value={options.container}
                onChange={(e) =>
                  handleContainerChange(e.target.value as ContainerFormat)
                }
                className="neon-input w-full px-3 py-2 text-xs sm:text-sm font-bold"
              >
                {ALL_CONTAINERS.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Dynamic Video Quality Dropdown (shown when mode !== audio && mode !== subtitles) */}
          {options.downloadMode === "video" && (
            <div>
              <div className="flex items-center justify-between mb-1.5">
                <label className="text-xs font-bold text-slate-600 dark:text-slate-300">
                  Available Video Qualities (Detected from Link)
                </label>
                {mediaInfo && mediaInfo.videoStreams.length > 0 && (
                  <span className="text-[11px] font-bold text-cyan-600 dark:text-cyan-400">
                    {mediaInfo.videoStreams.length} exact video streams
                  </span>
                )}
              </div>
              <select
                value={options.videoStreamId}
                onChange={(e) => updateOption("videoStreamId", e.target.value)}
                className="neon-input w-full px-3 py-2.5 text-xs sm:text-sm font-semibold"
              >
                <option value="best">
                  🌟 Best Available Video (Auto Highest Resolution &amp; FPS)
                </option>
                {mediaInfo && mediaInfo.videoStreams.length > 0 ? (
                  mediaInfo.videoStreams.map((stream) => (
                    <option key={stream.id} value={stream.id}>
                      🎬 {stream.label}
                    </option>
                  ))
                ) : (
                  <>
                    <option value="bestvideo[height<=2160]">
                      🎬 2160p (4K UHD)
                    </option>
                    <option value="bestvideo[height<=1440]">
                      🎬 1440p (2K QHD)
                    </option>
                    <option value="bestvideo[height<=1080]">
                      🎬 1080p (Full HD)
                    </option>
                    <option value="bestvideo[height<=720]">
                      🎬 720p (HD)
                    </option>
                    <option value="bestvideo[height<=480]">
                      🎬 480p (Standard)
                    </option>
                    <option value="bestvideo[height<=360]">
                      🎬 360p (Data Saver)
                    </option>
                  </>
                )}
              </select>
            </div>
          )}

          {/* Dynamic Audio Quality Dropdown */}
          {options.downloadMode !== "subtitles" && (
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div className="sm:col-span-2">
                <div className="flex items-center justify-between mb-1.5">
                  <label className="text-xs font-bold text-slate-600 dark:text-slate-300">
                    Available Audio Stream (Detected from Link)
                  </label>
                  {mediaInfo && mediaInfo.audioStreams.length > 0 && (
                    <span className="text-[11px] font-bold text-purple-600 dark:text-purple-400">
                      {mediaInfo.audioStreams.length} audio tracks
                    </span>
                  )}
                </div>
                <select
                  value={options.audioStreamId}
                  onChange={(e) => updateOption("audioStreamId", e.target.value)}
                  className="neon-input w-full px-3 py-2.5 text-xs sm:text-sm font-semibold"
                >
                  <option value="ba">
                    🎵 Best Available Audio Stream (Auto Highest Bitrate)
                  </option>
                  {mediaInfo &&
                    mediaInfo.audioStreams.map((stream) => (
                      <option key={stream.id} value={stream.id}>
                        🎧 {stream.label}
                      </option>
                    ))}
                </select>
              </div>

              <div>
                <label className="block text-xs font-bold text-slate-600 dark:text-slate-300 mb-1.5">
                  Audio Bitrate / Quality
                </label>
                <select
                  value={options.audioBitrate}
                  onChange={(e) => updateOption("audioBitrate", e.target.value)}
                  className="neon-input w-full px-3 py-2.5 text-xs sm:text-sm font-semibold"
                >
                  <option value="0">320 kbps (Best)</option>
                  <option value="192K">192 kbps (High)</option>
                  <option value="128K">128 kbps (Standard)</option>
                </select>
              </div>
            </div>
          )}
        </div>

        {/* Right 5 Cols: Separate Subtitle Download + Timeframe Clipper */}
        <div className="lg:col-span-5 space-y-4">
          {/* SEPARATE SUBTITLE DOWNLOAD CARD */}
          <div className="neon-card neon-border-emerald p-4 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-extrabold uppercase tracking-wider text-emerald-700 dark:text-emerald-400 flex items-center gap-1.5">
                <Subtitles className="w-4 h-4 text-emerald-600" />
                Separate Subtitle Download
              </span>
              <label className="inline-flex items-center gap-1.5 text-xs font-bold text-emerald-700 dark:text-emerald-300 cursor-pointer">
                <input
                  type="checkbox"
                  checked={
                    options.enableSubtitles ||
                    options.downloadMode === "subtitles"
                  }
                  onChange={(e) =>
                    updateOption("enableSubtitles", e.target.checked)
                  }
                  className="w-4 h-4 accent-emerald-600 rounded"
                />
                Include Subtitles
              </label>
            </div>

            <div className="space-y-2.5">
              <div>
                <label className="block text-[11px] font-bold text-slate-600 dark:text-slate-300 mb-1">
                  Available Subtitle Tracks (from Link)
                </label>
                <select
                  value={options.subtitleLang}
                  onChange={(e) => updateOption("subtitleLang", e.target.value)}
                  className="neon-input w-full px-3 py-2 text-xs font-semibold"
                >
                  {mediaInfo && mediaInfo.subtitles.length > 0 && (
                    <optgroup label="Detected on This Link">
                      {mediaInfo.subtitles.map((sub) => (
                        <option key={sub.code} value={sub.code}>
                          💬 {sub.name}
                        </option>
                      ))}
                    </optgroup>
                  )}
                  <optgroup label="Standard Language Presets">
                    <option value="en,bn">
                      English + Bengali (en, bn)
                    </option>
                    <option value="en">English (en)</option>
                    <option value="bn">Bengali / বাংলা (bn)</option>
                    <option value="hi">Hindi (hi)</option>
                    <option value="ar">Arabic (ar)</option>
                    <option value="es">Spanish (es)</option>
                    <option value="all">All Available Subtitles (all)</option>
                  </optgroup>
                </select>
              </div>

              <div className="grid grid-cols-2 gap-2.5">
                <div>
                  <label className="block text-[11px] font-bold text-slate-600 dark:text-slate-300 mb-1">
                    Subtitle Format
                  </label>
                  <select
                    value={options.subtitleFormat}
                    onChange={(e) =>
                      updateOption(
                        "subtitleFormat",
                        e.target.value as DownloadOptions["subtitleFormat"]
                      )
                    }
                    className="neon-input w-full px-2.5 py-1.5 text-xs font-semibold"
                  >
                    <option value="srt">SRT (.srt)</option>
                    <option value="vtt">WebVTT (.vtt)</option>
                    <option value="ass">ASS (.ass)</option>
                    <option value="lrc">Lyrics (.lrc)</option>
                  </select>
                </div>

                <div>
                  <label className="block text-[11px] font-bold text-slate-600 dark:text-slate-300 mb-1">
                    Save Mode
                  </label>
                  <select
                    value={options.subtitleSaveMode}
                    onChange={(e) =>
                      updateOption(
                        "subtitleSaveMode",
                        e.target.value as DownloadOptions["subtitleSaveMode"]
                      )
                    }
                    className="neon-input w-full px-2.5 py-1.5 text-xs font-semibold"
                  >
                    <option value="separate">Save Separate File</option>
                    <option value="both">Both (Separate + Embed)</option>
                    <option value="embed">Embed in Video</option>
                  </select>
                </div>
              </div>

              <button
                type="button"
                disabled={isBusy || !options.url.trim()}
                onClick={() => onStartDownload("subtitles")}
                className="w-full py-2 px-3 rounded-xl text-xs font-extrabold bg-emerald-50 hover:bg-emerald-100 dark:bg-emerald-950/80 dark:hover:bg-emerald-900 text-emerald-700 dark:text-emerald-300 border border-emerald-300 dark:border-emerald-700 flex items-center justify-center gap-1.5 transition-all disabled:opacity-50"
              >
                <Subtitles className="w-3.5 h-3.5" />
                Download Subtitle File (.{options.subtitleFormat.toUpperCase()}) Only
              </button>
            </div>
          </div>

          {/* TIMEFRAME CLIPPER CARD */}
          <div className="neon-card neon-border-violet p-4 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-extrabold uppercase tracking-wider text-purple-700 dark:text-purple-400 flex items-center gap-1.5">
                <Scissors className="w-4 h-4 text-purple-600" />
                Timeframe Clipper (Start &amp; End)
              </span>
              <label className="inline-flex items-center gap-1.5 text-xs font-bold text-purple-700 dark:text-purple-300 cursor-pointer">
                <input
                  type="checkbox"
                  checked={options.enableClip}
                  onChange={(e) => updateOption("enableClip", e.target.checked)}
                  className="w-4 h-4 accent-purple-600 rounded"
                />
                Enable Cut
              </label>
            </div>

            <div
              className={`space-y-2.5 transition-opacity ${
                options.enableClip ? "opacity-100" : "opacity-50 pointer-events-none"
              }`}
            >
              <div className="grid grid-cols-3 gap-2.5">
                <div>
                  <label className="block text-[11px] font-bold text-slate-600 dark:text-slate-300 mb-1">
                    Start Time
                  </label>
                  <input
                    type="text"
                    value={options.startTime}
                    onChange={(e) => updateOption("startTime", e.target.value)}
                    placeholder="00:00:00"
                    className="neon-input w-full px-2.5 py-1.5 text-xs font-mono font-bold text-center"
                    title="Start time: HH:MM:SS or seconds"
                  />
                </div>
                <div>
                  <label className="block text-[11px] font-bold text-slate-600 dark:text-slate-300 mb-1">
                    End Time
                  </label>
                  <input
                    type="text"
                    value={options.endTime}
                    onChange={(e) => updateOption("endTime", e.target.value)}
                    placeholder="00:02:00"
                    className="neon-input w-full px-2.5 py-1.5 text-xs font-mono font-bold text-center"
                    title="End time: HH:MM:SS or seconds"
                  />
                </div>
                <div className="flex items-end pb-1">
                  <label
                    title="Forces keyframes at cut points via multithreaded FFmpeg for frame-accurate cuts"
                    className="inline-flex items-center gap-1.5 text-xs font-bold text-slate-700 dark:text-slate-300 cursor-pointer"
                  >
                    <input
                      type="checkbox"
                      checked={options.preciseCut}
                      onChange={(e) =>
                        updateOption("preciseCut", e.target.checked)
                      }
                      className="w-4 h-4 accent-purple-600 rounded"
                    />
                    Precise Cut
                  </label>
                </div>
              </div>

              {/* Quick Preset Buttons */}
              <div className="flex flex-wrap items-center gap-1.5 pt-0.5">
                <span className="text-[10px] font-bold text-slate-400">Presets:</span>
                {mediaInfo?.duration && mediaInfo.duration > 0 && (
                  <button
                    type="button"
                    onClick={() => {
                      updateOption("startTime", "00:00:00");
                      updateOption("endTime", formatFullDuration(mediaInfo.duration));
                    }}
                    className="px-2 py-0.5 rounded text-[10px] font-bold bg-purple-100 hover:bg-purple-200 dark:bg-purple-950/80 dark:hover:bg-purple-900 text-purple-700 dark:text-purple-300 border border-purple-300 dark:border-purple-700 transition-all"
                  >
                    Full ({formatFullDuration(mediaInfo.duration)})
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => {
                    updateOption("startTime", "00:00:00");
                    updateOption("endTime", "00:00:30");
                  }}
                  className="px-2 py-0.5 rounded text-[10px] font-bold bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 transition-all"
                >
                  00:00 - 00:30
                </button>
                <button
                  type="button"
                  onClick={() => {
                    updateOption("startTime", "00:00:00");
                    updateOption("endTime", "00:01:00");
                  }}
                  className="px-2 py-0.5 rounded text-[10px] font-bold bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 transition-all"
                >
                  00:00 - 01:00
                </button>
                <button
                  type="button"
                  onClick={() => {
                    updateOption("startTime", "00:00:00");
                    updateOption("endTime", "00:02:00");
                  }}
                  className="px-2 py-0.5 rounded text-[10px] font-bold bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 transition-all"
                >
                  00:00 - 02:00
                </button>
              </div>

              <p className="text-[10px] text-slate-500 dark:text-slate-400">
                ⚡ <strong>Precise Cut:</strong> Re-encodes keyframes at exact millisecond borders via FFmpeg.
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* 2.5 ACTIVE DOWNLOADS PANEL (Separate progress cards for each individual download) */}
      {(() => {
        const activeTasks = tasks.filter(
          (t) => t.status === "downloading" || t.status === "paused"
        );
        if (activeTasks.length === 0) return null;

        return (
          <div className="neon-card p-4 sm:p-5 space-y-3 border-2 border-cyan-400/40 bg-gradient-to-br from-cyan-500/5 via-slate-500/5 to-purple-500/5">
            <div className="flex items-center justify-between border-b border-slate-200 dark:border-slate-800 pb-2.5">
              <h3 className="text-xs sm:text-sm font-black uppercase tracking-wider text-slate-900 dark:text-white flex items-center gap-2">
                <span className="relative flex h-2.5 w-2.5">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-cyan-400 opacity-75"></span>
                  <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-cyan-500"></span>
                </span>
                Active Downloads ({activeTasks.length})
              </h3>
              <span className="text-[11px] font-bold text-slate-500 dark:text-slate-400">
                Multi-Task Progress &bull; Independent Resumes
              </span>
            </div>

            <div className="space-y-3">
              {activeTasks.map((t) => {
                const isPaused = t.status === "paused";
                return (
                  <div
                    key={t.id}
                    className="p-3.5 rounded-xl bg-white dark:bg-slate-900/90 border border-slate-200 dark:border-slate-800 space-y-2.5 shadow-sm"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex items-center gap-3 min-w-0">
                        {t.thumbnail ? (
                          <img
                            src={t.thumbnail}
                            alt=""
                            className="w-14 h-9 object-cover rounded-lg border border-slate-200 dark:border-slate-700 shrink-0"
                          />
                        ) : (
                          <div className="w-14 h-9 rounded-lg bg-slate-100 dark:bg-slate-800 flex items-center justify-center shrink-0 border border-slate-200 dark:border-slate-700">
                            <Film className="w-4 h-4 text-cyan-600" />
                          </div>
                        )}
                        <div className="min-w-0">
                          <h4
                            className="text-xs sm:text-sm font-black text-slate-900 dark:text-white truncate"
                            title={t.title}
                          >
                            {t.title}
                          </h4>
                          <div className="flex flex-wrap items-center gap-1.5 mt-0.5 text-[11px] text-slate-500 dark:text-slate-400 font-bold">
                            <span className="px-1.5 py-0.2 rounded bg-cyan-100 dark:bg-cyan-950 text-cyan-800 dark:text-cyan-300">
                              {t.container} &bull; {t.qualityLabel}
                            </span>
                            {t.options.enableClip && (
                              <span className="px-1.5 py-0.2 rounded bg-purple-100 dark:bg-purple-950 text-purple-800 dark:text-purple-300">
                                ✂️ {t.options.startTime} - {t.options.endTime}
                                {t.options.preciseCut ? " (Precise)" : ""}
                              </span>
                            )}
                          </div>
                        </div>
                      </div>

                      <div className="flex items-center gap-2 shrink-0">
                        {isPaused ? (
                          <button
                            type="button"
                            onClick={() => onResumeTask && onResumeTask(t)}
                            className="px-3 py-1.5 rounded-lg text-xs font-black bg-emerald-500 hover:bg-emerald-600 text-white flex items-center gap-1 shadow-sm transition-all"
                            title="Resume download"
                          >
                            Resume
                          </button>
                        ) : (
                          <button
                            type="button"
                            onClick={() => onCancelDownload(t.id)}
                            className="px-3 py-1.5 rounded-lg text-xs font-black bg-amber-50 hover:bg-amber-100 dark:bg-amber-950/60 text-amber-700 dark:text-amber-300 border border-amber-300 dark:border-amber-700 flex items-center gap-1 transition-all"
                            title="Pause this specific download"
                          >
                            Pause
                          </button>
                        )}
                      </div>
                    </div>

                    {/* Independent Progress Bar */}
                    <div className="space-y-1">
                      <div className="flex items-center justify-between text-[11px] font-bold">
                        <span className="text-slate-600 dark:text-slate-300 truncate max-w-md">
                          {t.progressText || (isPaused ? "⏸ Download paused (.part saved)" : "Downloading fragments...")}
                        </span>
                        <span className="text-indigo-600 dark:text-cyan-400 font-black shrink-0">
                          {t.speed ? `${t.speed} • ` : ""}
                          {t.eta ? `ETA ${t.eta} • ` : ""}
                          {(t.status === "completed" ? 100 : Math.min(99.0, Math.max(0, t.percent))).toFixed(1)}%
                        </span>
                      </div>
                      <div className="w-full h-2 rounded-full bg-slate-100 dark:bg-slate-800 overflow-hidden border border-slate-200 dark:border-slate-700 p-0.5">
                        <div
                          className="h-full rounded-full transition-all duration-300"
                          style={{
                            width: `${Math.min(100, Math.max(0, t.status === "completed" ? 100 : Math.min(99.0, t.percent)))}%`,
                            background: isPaused
                              ? "#f59e0b"
                              : "linear-gradient(90deg, #06b6d4, #6366f1, #10b981)",
                            boxShadow: "0 0 8px rgba(6, 182, 212, 0.4)",
                          }}
                        />
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })()}

      {/* 3. SAVE LOCATION + ACTION BAR + PROGRESS */}
      <div className="neon-card p-4 sm:p-5 space-y-4">
        {/* Output File Name Rename Input */}
        <div>
          <div className="flex items-center justify-between mb-1.5">
            <label className="block text-xs font-bold text-slate-600 dark:text-slate-300 flex items-center gap-1.5">
              <FileEdit className="w-3.5 h-3.5 text-cyan-600" />
              Output File Name (Rename before download)
            </label>
            {options.customFilename && mediaInfo?.title && options.customFilename !== mediaInfo.title && (
              <button
                type="button"
                onClick={() => updateOption("customFilename", mediaInfo.title)}
                className="text-[10px] font-bold text-indigo-600 dark:text-cyan-400 hover:underline"
              >
                Reset to original title
              </button>
            )}
          </div>
          <input
            type="text"
            value={options.customFilename !== undefined ? options.customFilename : (mediaInfo?.title || "")}
            onChange={(e) => updateOption("customFilename", e.target.value)}
            placeholder={mediaInfo?.title || "e.g. My Custom Video Name (Leave empty to use stream title)"}
            className="neon-input w-full px-3.5 py-2 text-xs sm:text-sm font-semibold"
          />
        </div>

        <div className="flex flex-col md:flex-row items-stretch md:items-end gap-3">
          <div className="flex-1">
            <label className="block text-xs font-bold text-slate-600 dark:text-slate-300 mb-1.5">
              Save Destination Folder
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={options.saveFolder}
                onChange={(e) => updateOption("saveFolder", e.target.value)}
                className="neon-input flex-1 px-3.5 py-2.5 text-xs sm:text-sm font-medium"
              />
              <button
                type="button"
                onClick={onBrowseFolder}
                className="px-3.5 py-2.5 rounded-xl text-xs font-bold bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 flex items-center gap-1.5 shrink-0"
              >
                <FolderOpen className="w-4 h-4 text-indigo-600" />
                Browse
              </button>
              <button
                type="button"
                onClick={onOpenFolder}
                title="Open destination folder in Explorer/Finder"
                className="px-3 py-2.5 rounded-xl text-xs font-bold bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 flex items-center gap-1 shrink-0"
              >
                <ExternalLink className="w-4 h-4 text-cyan-600" />
              </button>
            </div>
          </div>

          <div className="flex gap-2.5">
            {isBusy && (
              <button
                type="button"
                onClick={() => onCancelDownload()}
                className="px-4 py-3 rounded-xl text-xs sm:text-sm font-extrabold bg-amber-50 hover:bg-amber-100 text-amber-700 border border-amber-300 flex items-center justify-center gap-2 shrink-0"
                title="Pause active download"
              >
                <PauseCircle className="w-4 h-4" />
                Pause / Stop
              </button>
            )}

            <button
              type="button"
              disabled={!options.url.trim()}
              onClick={() => onStartDownload()}
              className="neon-btn-primary px-6 py-3 rounded-xl text-sm font-black flex items-center justify-center gap-2 shrink-0 disabled:opacity-50 shadow-md"
              title="Add this download to queue anytime"
            >
              <Download className="w-4 h-4" />
              <span>{isBusy ? "+ Add to Queue" : "Start Download"}</span>
            </button>
          </div>
        </div>

        {/* Live Neon Progress Bar */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between text-xs font-bold">
            <span className="flex items-center gap-1.5 text-slate-700 dark:text-slate-200">
              {progress.status === "completed" ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-500" />
              ) : progress.status === "error" ? (
                <AlertCircle className="w-4 h-4 text-rose-500" />
              ) : null}
              {progress.text}
            </span>
            <span className="text-indigo-600 dark:text-cyan-400 font-extrabold">
              {progress.speed ? `${progress.speed} • ` : ""}
              {progress.eta ? `ETA ${progress.eta} • ` : ""}
              {progress.percent.toFixed(1)}%
            </span>
          </div>

          <div className="w-full h-3 rounded-full bg-slate-100 dark:bg-slate-800 overflow-hidden border border-slate-200 dark:border-slate-700 p-0.5">
            <div
              className="h-full rounded-full transition-all duration-300"
              style={{
                width: `${Math.min(100, Math.max(0, progress.percent))}%`,
                background:
                  progress.status === "error"
                    ? "linear-gradient(90deg, #f43f5e, #e11d48)"
                    : "linear-gradient(90deg, #06b6d4, #6366f1, #a855f7, #10b981)",
                boxShadow: "0 0 12px rgba(6, 182, 212, 0.5)",
              }}
            />
          </div>
        </div>

        {/* Collapsible Live Terminal Log */}
        <div className="pt-1 border-t border-slate-100 dark:border-slate-800">
          <button
            type="button"
            onClick={() => setShowLogs((prev) => !prev)}
            className="text-[11px] font-bold text-slate-500 hover:text-indigo-600 flex items-center gap-1"
          >
            <TerminalSquare className="w-3.5 h-3.5" />
            {showLogs ? "Hide Live Engine Log" : "Show Live Engine Log"}
            {showLogs ? (
              <ChevronUp className="w-3 h-3" />
            ) : (
              <ChevronDown className="w-3 h-3" />
            )}
          </button>

          {showLogs && (
            <div className="mt-2 p-3 rounded-xl bg-slate-900 text-cyan-300 font-mono text-[11px] max-h-40 overflow-y-auto space-y-1 border border-slate-700">
              {logs.length === 0 ? (
                <div className="text-slate-500">
                  No log output yet. Paste a link and click Fetch or Start Download.
                </div>
              ) : (
                logs.slice(-60).map((line, i) => (
                  <div key={i} className="break-all leading-relaxed">
                    {line}
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
