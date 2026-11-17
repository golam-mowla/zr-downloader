import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatTime(seconds?: number): string {
  if (seconds === undefined || seconds === null || seconds < 0) return "00:00:00";
  const sec = Math.round(seconds);
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

export function parseTimeToSeconds(timeStr: string): number {
  const parts = timeStr.trim().split(":").map(Number);
  if (parts.some(isNaN)) return 0;
  if (parts.length === 1) return parts[0];
  if (parts.length === 2) return parts[0] * 60 + parts[1];
  if (parts.length === 3) return parts[0] * 3600 + parts[1] * 60 + parts[2];
  return 0;
}

export function formatBytes(bytes?: number): string {
  if (!bytes) return "0 B";
  const k = 1024;
  const sizes = ["B", "KiB", "MiB", "GiB", "TiB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

/**
 * True for streams that Low Disk Space mode cannot help with (HLS / DASH / Kick / Twitch / IVS).
 * Mirrors `is_hls_like_url` in src-tauri/src/media.rs: yt-dlp always needs fragment + merge space there.
 */
export function isHlsLikeUrl(rawUrl?: string): boolean {
  const u = (rawUrl || "").trim().toLowerCase();
  if (!u) return false;
  if (u.includes("m3u8") || u.includes("/hls/") || u.includes(".mpd")) return true;
  let host = u;
  try {
    host = new URL(u).hostname;
  } catch {
    /* not a full URL, fall back to the raw string */
  }
  return host.includes("kick.com") || host.includes("twitch.tv") || host.includes("ivs");
}
