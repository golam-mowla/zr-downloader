import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, ExternalLink, RefreshCw, Sparkles, X } from "lucide-react";

/** Mirrors `AppUpdateInfo` in src-tauri/src/media.rs (camelCase). */
export interface AppUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  releaseName: string;
  releaseNotes: string;
  releaseUrl: string;
  publishedAt: string;
  assetName: string;
  assetUrl: string;
  assetSize: number;
}

const DISMISS_KEY = "zr-update-dismissed-version";
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000; // re-check every 6 hours
const FIRST_CHECK_DELAY_MS = 8000; // do not slow down app start-up

function errText(e: unknown): string {
  if (typeof e === "string") return e;
  const m = (e as { message?: unknown } | null)?.message;
  return typeof m === "string" ? m : String(e);
}

function fmtSize(bytes: number): string {
  if (!bytes || bytes <= 0) return "";
  const mb = bytes / (1024 * 1024);
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${(bytes / 1024).toFixed(0)} KB`;
}

/**
 * Checks GitHub Releases (via the Rust `check_app_update` command).
 * With `autoCheck` it checks shortly after start-up and then every few hours.
 */
export function useAppUpdate(autoCheck: boolean) {
  const [info, setInfo] = useState<AppUpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [checkedOnce, setCheckedOnce] = useState(false);

  const check = useCallback(async () => {
    setChecking(true);
    setError(null);
    try {
      const res = await invoke<AppUpdateInfo>("check_app_update");
      setInfo(res);
    } catch (e) {
      setError(errText(e));
    } finally {
      setChecking(false);
      setCheckedOnce(true);
    }
  }, []);

  const install = useCallback(async () => {
    if (!info?.assetUrl) return;
    setInstalling(true);
    setError(null);
    try {
      // The app closes itself and starts the installer when this succeeds.
      await invoke("install_app_update", { downloadUrl: info.assetUrl });
    } catch (e) {
      setError(errText(e));
      setInstalling(false);
    }
  }, [info]);

  const openReleasePage = useCallback(async () => {
    const url = info?.releaseUrl || "https://github.com/golam-mowla/zr-downloader/releases";
    try {
      await invoke("open_release_page", { url });
    } catch (e) {
      setError(errText(e));
    }
  }, [info]);

  useEffect(() => {
    if (!autoCheck) return;
    const first = window.setTimeout(() => {
      void check();
    }, FIRST_CHECK_DELAY_MS);
    const iv = window.setInterval(() => {
      void check();
    }, CHECK_INTERVAL_MS);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(iv);
    };
  }, [autoCheck, check]);

  return { info, checking, installing, error, checkedOnce, check, install, openReleasePage };
}

type UpdateState = ReturnType<typeof useAppUpdate>;

/** Slim bar at the top of the window: shown only when a newer GitHub release exists. */
export const AppUpdateBanner: React.FC<{ update: UpdateState }> = ({ update }) => {
  const { info, installing, error, install, openReleasePage } = update;
  const [dismissedVersion, setDismissedVersion] = useState<string>(() => {
    try {
      return localStorage.getItem(DISMISS_KEY) || "";
    } catch {
      return "";
    }
  });

  if (!info?.updateAvailable) return null;
  if (dismissedVersion === info.latestVersion) return null;

  const dismiss = () => {
    try {
      localStorage.setItem(DISMISS_KEY, info.latestVersion);
    } catch {
      /* ignore */
    }
    setDismissedVersion(info.latestVersion);
  };

  return (
    <div className="shrink-0 flex flex-wrap items-center gap-2 px-4 py-2 text-[12.5px] font-semibold text-white bg-gradient-to-r from-blue-600 to-cyan-600 border-b border-blue-400">
      <Sparkles className="w-4 h-4 shrink-0" />
      <span className="min-w-0">
        New version <b>v{info.latestVersion}</b> is available (you have v{info.currentVersion})
        {info.assetSize > 0 ? ` - ${fmtSize(info.assetSize)}` : ""}
      </span>
      {error && <span className="text-red-100 font-medium">{error}</span>}
      <div className="ml-auto flex items-center gap-2">
        {info.assetUrl ? (
          <button
            type="button"
            onClick={() => void install()}
            disabled={installing}
            className="px-3 py-1 rounded-md bg-white text-blue-700 hover:bg-blue-50 disabled:opacity-60 flex items-center gap-1.5"
          >
            {installing ? (
              <RefreshCw className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Download className="w-3.5 h-3.5" />
            )}
            <span>{installing ? "Downloading..." : "Update now"}</span>
          </button>
        ) : null}
        <button
          type="button"
          onClick={() => void openReleasePage()}
          className="px-3 py-1 rounded-md bg-white/15 hover:bg-white/25 flex items-center gap-1.5"
        >
          <ExternalLink className="w-3.5 h-3.5" />
          <span>What's new</span>
        </button>
        <button
          type="button"
          onClick={dismiss}
          title="Hide until the next release"
          className="p-1 rounded-md hover:bg-white/20"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
};

/** Card for Settings: manual check + release notes + 1-click update. */
export const AppUpdatePanel: React.FC<{ update: UpdateState }> = ({ update }) => {
  const { info, checking, installing, error, checkedOnce, check, install, openReleasePage } = update;

  useEffect(() => {
    // open the tab -> check once if nothing was checked yet
    if (!checkedOnce && !checking) void check();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-700 space-y-2.5">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="text-[13px] font-semibold text-zr-primary">ZR Downloader app updates</div>
          <div className="text-[12px] text-zr-muted">
            {info
              ? info.updateAvailable
                ? `New release v${info.latestVersion} is available (installed: v${info.currentVersion}).`
                : info.latestVersion
                  ? `You are on the latest release (v${info.currentVersion}).`
                  : `Installed: v${info.currentVersion}. No GitHub release published yet.`
              : checking
                ? "Checking GitHub Releases..."
                : "Not checked yet."}
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            type="button"
            onClick={() => void check()}
            disabled={checking || installing}
            className="flex items-center space-x-1.5 text-zr-accent hover:underline text-[12px] font-semibold disabled:opacity-60"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${checking ? "animate-spin" : ""}`} />
            <span>Check for updates</span>
          </button>
        </div>
      </div>

      {info?.updateAvailable && (
        <div className="space-y-2">
          {info.releaseNotes && (
            <pre className="max-h-40 overflow-y-auto whitespace-pre-wrap text-[11.5px] leading-snug p-2 rounded-md bg-slate-100 dark:bg-slate-800/70 text-zr-secondary">
              {info.releaseNotes}
            </pre>
          )}
          <div className="flex items-center gap-2 flex-wrap">
            {info.assetUrl ? (
              <button
                type="button"
                onClick={() => void install()}
                disabled={installing}
                className="px-3.5 py-1.5 rounded-md text-[12.5px] font-bold text-white bg-blue-600 hover:bg-blue-700 disabled:opacity-60 flex items-center gap-1.5"
              >
                {installing ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Download className="w-3.5 h-3.5" />
                )}
                <span>
                  {installing
                    ? "Downloading installer..."
                    : `Update to v${info.latestVersion}${info.assetSize > 0 ? ` (${fmtSize(info.assetSize)})` : ""}`}
                </span>
              </button>
            ) : (
              <span className="text-[12px] text-zr-muted">
                No installer for this OS in the release - open the release page to download it.
              </span>
            )}
            <button
              type="button"
              onClick={() => void openReleasePage()}
              className="px-3 py-1.5 rounded-md text-[12.5px] font-semibold border border-slate-300 dark:border-slate-600 text-zr-secondary hover:text-zr-primary flex items-center gap-1.5"
            >
              <ExternalLink className="w-3.5 h-3.5" />
              <span>Release page</span>
            </button>
          </div>
        </div>
      )}

      {error && (
        <div className="p-2 rounded-md text-[12px] font-medium bg-red-50 text-red-700 dark:bg-red-900/30 dark:text-red-300">
          {error}
        </div>
      )}
    </div>
  );
};
