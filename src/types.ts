export interface ChunkProgress {
  id: number;
  start: number;
  end: number;
  downloaded: number;
  total: number;
  percent: number;
}

export type TaskStatus =
  | "queued"
  | "downloading"
  | "assembling"
  | "seeding"
  | "paused"
  | "interrupted"
  | "completed"
  | "error"
  | "cancelled";

export interface DownloadTask {
  id: string;
  url: string;
  filename: string;
  title?: string;
  customFilename?: string;
  save_path?: string;
  savePath?: string;
  saveFolder?: string;
  total_bytes?: number;
  totalBytes?: number;
  downloaded_bytes?: number;
  downloadedBytes?: number;
  percent: number;
  speed_bytes_per_sec?: number;
  speed_bps?: number;
  speedBps?: number;
  speed_formatted?: string;
  speedFormatted?: string;
  speed?: string;
  eta_seconds?: number;
  eta_formatted?: string;
  etaFormatted?: string;
  eta?: string;
  progressText?: string;
  status: TaskStatus;
  category: "all" | "video" | "audio" | "docs" | "compressed" | "programs" | "torrents" | string;
  chunk_count?: number;
  chunkCount?: number;
  connections?: number;
  is_resumable?: boolean;
  isResumable?: boolean;
  supports_ranges?: boolean;
  supportsRanges?: boolean;
  low_disk_space?: boolean;
  lowDiskSpace?: boolean;
  error?: string | null;
  errorMessage?: string | null;
  created_at?: string | number;
  createdAt?: string | number;
  completed_at?: number | null;
  chunks?: ChunkProgress[];
  speedHistory?: { time: string; speedKb: number }[];
  thumbnail?: string;
  container?: string;
  qualityLabel?: string;
  options?: any;
  peers?: number;
  seeders?: number;
  total_peers?: number;
  totalPeers?: number;
  up_speed?: string;
  up_speed_formatted?: string;
}

export const getDownloadedBytes = (t: DownloadTask | any): number => {
  if (!t) return 0;
  const val = t.downloaded_bytes ?? t.downloadedBytes ?? t.downloaded ?? 0;
  return typeof val === "number" && !isNaN(val) ? val : 0;
};

export const getTotalBytes = (t: DownloadTask | any): number => {
  if (!t) return 0;
  const val = t.total_bytes ?? t.totalBytes ?? t.total ?? 0;
  return typeof val === "number" && !isNaN(val) ? val : 0;
};

export const getChunkCount = (t: DownloadTask | any): number => {
  if (!t) return 1;
  return t.chunk_count ?? t.chunkCount ?? t.connections ?? (t.chunks?.length || 1);
};

export const getIsResumable = (t: DownloadTask | any): boolean => {
  if (!t) return false;
  return Boolean(t.is_resumable ?? t.isResumable ?? t.supports_ranges ?? t.supportsRanges ?? false);
};

export const getSpeedFormatted = (t: DownloadTask | any): string => {
  if (!t) return "0 B/s";
  if (t.speed_formatted && t.speed_formatted !== "0 B/s" && t.speed_formatted !== "--") return t.speed_formatted;
  if (t.speedFormatted && t.speedFormatted !== "0 B/s" && t.speedFormatted !== "--") return t.speedFormatted;
  if (t.speed && t.speed !== "0 B/s" && t.speed !== "--") return t.speed;
  const bps = t.speed_bps ?? t.speedBps ?? t.speed_bytes_per_sec ?? 0;
  if (typeof bps === "number" && bps > 0) {
    return `${formatBytes(bps)}/s`;
  }
  return "0 B/s";
};

export const getEtaFormatted = (t: DownloadTask | any): string => {
  if (!t) return "--";
  if (t.eta_formatted && t.eta_formatted !== "--") return t.eta_formatted;
  if (t.etaFormatted && t.etaFormatted !== "--") return t.etaFormatted;
  if (t.eta && t.eta !== "--") return t.eta;
  return "--";
};

export const formatBytes = (bytes: number | null | undefined): string => {
  if (bytes === undefined || bytes === null || isNaN(bytes) || bytes <= 0) {
    return "0 B";
  }
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  if (i < 0 || i >= sizes.length || isNaN(i)) return "0 B";
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
};

export interface DownloadProgress {
  percent: number;
  text?: string;
  speed?: string;
  eta?: string;
  status?: string;
}

// Media Engine (yt-dlp) types
export interface MediaStream {
  id: string;
  label: string;
  height?: number;
  fps?: number;
  ext?: string;
  vcodec?: string;
  acodec?: string;
  filesize?: string | number;
  hasAudio?: boolean;
}

export interface SubtitleTrack {
  code: string;
  name: string;
  isAuto: boolean;
}

export interface MediaInfo {
  id?: string;
  title: string;
  uploader?: string;
  duration?: number;
  thumbnail?: string;
  videoStreams: MediaStream[];
  audioStreams: MediaStream[];
  subtitles: SubtitleTrack[];
}

export type DownloadMode = "video" | "audio" | "subtitles";

export type ContainerFormat =
  | "Video (Generic)"
  | "MP4 (Video)"
  | "WEBM (Video)"
  | "MKV (Video)"
  | "MOV (Video)"
  | "AVI (Video)"
  | "Audio (Generic)"
  | "MP3 (Audio)"
  | "M4A (Audio)"
  | "OPUS (Audio)"
  | "FLAC (Audio)"
  | "WAV (Audio)"
  | "OGG (Audio)";

export const ALL_CONTAINERS: ContainerFormat[] = [
  "Video (Generic)",
  "MP4 (Video)",
  "WEBM (Video)",
  "MKV (Video)",
  "MOV (Video)",
  "AVI (Video)",
  "Audio (Generic)",
  "MP3 (Audio)",
  "M4A (Audio)",
  "OPUS (Audio)",
  "FLAC (Audio)",
  "WAV (Audio)",
  "OGG (Audio)",
];

export interface MediaDownloadOptions {
  taskId?: string;
  url: string;
  saveFolder: string;
  customFilename?: string;
  downloadMode: DownloadMode;
  container: ContainerFormat;
  videoStreamId: string;
  audioStreamId: string;
  audioBitrate: string;
  enableSubtitles: boolean;
  subtitleLang: string;
  subtitleFormat: "srt" | "vtt" | "ass" | "lrc" | "any";
  subtitleSaveMode: "separate" | "embed" | "both";
  enableClip: boolean;
  startTime: string;
  endTime: string;
  preciseCut: boolean;
  [key: string]: any;
}

export type DownloadOptions = MediaDownloadOptions;

export interface ToolVersionInfo {
  name: string;
  installedVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  isUpdating?: boolean;
}

export interface TorrentFileItem {
  index: number;
  path: string;
  filename: string;
  length: number;
  length_formatted: string;
  selected: boolean;
  priority: "skip" | "normal" | "high" | "max";
}

export interface TorrentPeerItem {
  addr: string;
  client: string;
  flags: string;
  down_speed: string;
  up_speed: string;
  progress: number;
}

export interface TorrentTrackerItem {
  url: string;
  status: string;
  peers: number;
  seeds: number;
  interval: number;
}

export interface TorrentDetailsData {
  id: string;
  name: string;
  info_hash: string;
  state: "queued" | "checking" | "downloading" | "seeding" | "paused" | "stalled" | "error";
  total_bytes: number;
  downloaded_bytes: number;
  uploaded_bytes: number;
  progress: number;
  download_speed: string;
  upload_speed: string;
  eta: string;
  ratio: number;
  peers_connected: number;
  seeds_connected: number;
  save_path: string;
  piece_length: number;
  num_pieces: number;
  pieces_downloaded: number;
  files: TorrentFileItem[];
  peers: TorrentPeerItem[];
  trackers: TorrentTrackerItem[];
}
