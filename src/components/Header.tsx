import React, { useState, useRef, useEffect } from "react";
import {
  Sparkles,
  Terminal,
  ArrowRight,
  Plus,
  HelpCircle,
  Github,
  Bug,
  MessageSquare,
  Info,
  RefreshCw,
  X,
} from "lucide-react";
import { ToolVersionInfo } from "../types";

interface HeaderProps {
  tools: ToolVersionInfo[];
  appHasUpdate?: boolean;
  latestAppVersion?: string;
  onOpenUpdates: () => void;
  onAddNewDownload: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  tools,
  appHasUpdate = false,
  latestAppVersion,
  onOpenUpdates,
  onAddNewDownload,
}) => {
  const [showHelpMenu, setShowHelpMenu] = useState(false);
  const [showAboutModal, setShowAboutModal] = useState(false);
  const helpMenuRef = useRef<HTMLDivElement>(null);

  const ytdlp = tools.find((t) => t.name.toLowerCase().includes("yt-dlp"));
  const hasAnyEngineUpdate = tools.some((t) => t.hasUpdate);
  const showUpdateAlert = appHasUpdate || hasAnyEngineUpdate;

  // Close help menu on outside click
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (helpMenuRef.current && !helpMenuRef.current.contains(e.target as Node)) {
        setShowHelpMenu(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <>
      <header className="flex flex-wrap items-center justify-between gap-3 px-3 sm:px-6 py-2.5 bg-white/95 dark:bg-slate-900/95 backdrop-blur-md border-b border-slate-200 dark:border-slate-800 sticky top-0 z-30">
        {/* Left Side: Brand + FULL PARROT + "+ Add" Button (Image 2) */}
        <div className="flex items-center gap-3">
          {/* ZR Downloader Icon */}
          <div className="relative shrink-0 flex items-center justify-center">
            <img
              src="/icon.png"
              alt="ZR Downloader"
              className="w-10 h-10 object-contain drop-shadow-[0_4px_10px_rgba(6,182,212,0.4)] hover:scale-105 transition-transform"
            />
          </div>

          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-base sm:text-lg font-black tracking-tight bg-gradient-to-r from-slate-900 via-indigo-700 to-cyan-600 dark:from-white dark:via-cyan-300 dark:to-indigo-400 bg-clip-text text-transparent">
                ZR Downloader
              </h1>
              <span className="px-2 py-0.5 text-[10px] font-black uppercase tracking-wider rounded-full bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-300 border border-cyan-300 dark:border-cyan-700 flex items-center gap-1">
                v1.2.4
              </span>
            </div>
            <p className="text-[11px] font-semibold text-slate-500 dark:text-slate-400">
              Universal High-Speed Downloader &amp; Media Grabber
            </p>
          </div>

          {/* PARABOLIC STYLE "+ Add" BUTTON (Matches User's Image 2!) */}
          <button
            type="button"
            onClick={onAddNewDownload}
            className="ml-2 flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-black bg-blue-600 hover:bg-blue-700 text-white shadow-md shadow-blue-500/25 transition-all"
            title="Add a new download anytime (even while another download is running)"
          >
            <Plus className="w-3.5 h-3.5 stroke-[3]" />
            <span>Add</span>
          </button>
        </div>

        {/* Right Side: Update Pill + Status + Help Dropdown Menu (Image 4) */}
        <div className="flex items-center gap-2 flex-wrap">
          {/* PROMINENT BLUE UPDATE BUTTON (Matches User's Image 3) */}
          {showUpdateAlert ? (
            <button
              type="button"
              onClick={onOpenUpdates}
              title="Click to view update details and update core engines"
              className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-full text-xs font-black bg-gradient-to-r from-blue-600 to-cyan-600 hover:from-blue-700 hover:to-cyan-700 text-white shadow-md shadow-blue-500/30 border border-blue-400 animate-pulse transition-all"
            >
              <Sparkles className="w-3.5 h-3.5" />
              <span>Update Available{latestAppVersion ? ` (${latestAppVersion})` : ""}</span>
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          ) : (
            /* Yt-dlp Up to Date (No update available) Badge */
            <button
              type="button"
              onClick={onOpenUpdates}
              title="yt-dlp is fully updated to the latest version"
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-bold bg-cyan-50 dark:bg-cyan-950/60 text-cyan-800 dark:text-cyan-200 border border-cyan-300 dark:border-cyan-800 hover:bg-cyan-100 transition-colors"
            >
              <span className="w-2 h-2 rounded-full bg-emerald-500" />
              <Terminal className="w-3.5 h-3.5 text-cyan-600" />
              <span>yt-dlp: Up to date (No update available) ✔</span>
            </button>
          )}

          {/* HELP MENU DROPDOWN (Matches User's Image 4: Parabolic Help Menu) */}
          <div className="relative" ref={helpMenuRef}>
            <button
              type="button"
              onClick={() => setShowHelpMenu((prev) => !prev)}
              className="p-1.5 rounded-xl bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 border border-slate-300 dark:border-slate-700 transition-colors flex items-center gap-1 text-xs font-bold"
              title="Help Menu"
            >
              <HelpCircle className="w-4 h-4 text-cyan-600" />
              <span className="hidden sm:inline">Help</span>
            </button>

            {showHelpMenu && (
              <div className="absolute right-0 mt-2 w-56 bg-white dark:bg-slate-900 rounded-2xl shadow-xl border border-slate-200 dark:border-slate-800 py-1.5 z-50 text-xs font-bold animate-in fade-in zoom-in-95 duration-150">
                <button
                  type="button"
                  onClick={() => {
                    setShowHelpMenu(false);
                    onOpenUpdates();
                  }}
                  className="w-full px-3.5 py-2 flex items-center gap-2.5 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 text-left transition-colors"
                >
                  <RefreshCw className="w-3.5 h-3.5 text-cyan-600" />
                  <span>Check for Updates</span>
                </button>

                <a
                  href="https://github.com/golam-mowla/zr-downloader"
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={() => setShowHelpMenu(false)}
                  className="w-full px-3.5 py-2 flex items-center gap-2.5 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 text-left transition-colors"
                >
                  <Github className="w-3.5 h-3.5 text-slate-800 dark:text-slate-100" />
                  <span>GitHub Repo</span>
                </a>

                <a
                  href="https://github.com/golam-mowla/zr-downloader/issues"
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={() => setShowHelpMenu(false)}
                  className="w-full px-3.5 py-2 flex items-center gap-2.5 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 text-left transition-colors"
                >
                  <Bug className="w-3.5 h-3.5 text-rose-500" />
                  <span>Report a Bug</span>
                </a>

                <a
                  href="https://github.com/golam-mowla/zr-downloader/discussions"
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={() => setShowHelpMenu(false)}
                  className="w-full px-3.5 py-2 flex items-center gap-2.5 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 text-left transition-colors"
                >
                  <MessageSquare className="w-3.5 h-3.5 text-indigo-500" />
                  <span>Discussions</span>
                </a>

                <div className="my-1 border-t border-slate-100 dark:border-slate-800" />

                <button
                  type="button"
                  onClick={() => {
                    setShowHelpMenu(false);
                    setShowAboutModal(true);
                  }}
                  className="w-full px-3.5 py-2 flex items-center gap-2.5 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 text-left transition-colors"
                >
                  <Info className="w-3.5 h-3.5 text-blue-500" />
                  <span>About GM Downloader</span>
                </button>
              </div>
            )}
          </div>
        </div>
      </header>

      {/* ABOUT MODAL */}
      {showAboutModal && (
        <div className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="neon-card max-w-md w-full p-6 space-y-4 relative animate-in fade-in zoom-in-95 duration-200">
            <button
              type="button"
              onClick={() => setShowAboutModal(false)}
              className="absolute top-4 right-4 p-1 rounded-lg text-slate-400 hover:text-slate-700 dark:hover:text-slate-200"
            >
              <X className="w-4 h-4" />
            </button>

            <div className="text-center space-y-2">
              <img
                src="/icon.png"
                alt="ZR Downloader"
                className="w-16 h-16 object-contain mx-auto drop-shadow-md"
              />
              <h3 className="text-lg font-black text-slate-900 dark:text-white">
                ZR Downloader v1.2.36
              </h3>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                Created by Golam Mowla &bull; Powered by Multi-Threaded Engine, BitTorrent &amp; yt-dlp
              </p>
            </div>

            <div className="text-xs text-slate-600 dark:text-slate-300 space-y-2 border-y border-slate-100 dark:border-slate-800 py-3">
              <p>
                <strong>ZR Downloader</strong> is an advanced, high-performance download accelerator, media extractor, and BitTorrent client built for Windows and Android.
              </p>
              <p>
                Includes full Parabolic options, 13 audio/video container formats, automatic 18+ age restriction bypass, and crash-resilient resuming.
              </p>
            </div>

            <div className="flex justify-end pt-1">
              <button
                type="button"
                onClick={() => setShowAboutModal(false)}
                className="neon-btn-primary px-5 py-2 rounded-xl text-xs font-bold"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
};
