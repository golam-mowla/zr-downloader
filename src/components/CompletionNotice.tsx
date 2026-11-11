import React, { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CheckCircle2, FolderOpen, FileCheck, X } from "lucide-react";

export interface CompletionNoticeProps {
  name: string;
  folder: string;
}

const AUTO_CLOSE_MS = 9000;

/** Resolve the full file path whether `folder` is a directory or already includes `name` (HTTP tasks). */
function resolveTargetPath(folder: string, name: string): string {
  const cleanFolder = folder.replace(/[/\\]+$/, "");
  if (!cleanFolder) return name;
  const lowerFolder = cleanFolder.toLowerCase();
  const lowerName = name.toLowerCase();
  if (
    lowerName &&
    (lowerFolder === lowerName ||
      lowerFolder.endsWith(`\\${lowerName}`) ||
      lowerFolder.endsWith(`/${lowerName}`))
  ) {
    return cleanFolder;
  }
  return `${cleanFolder}\\${name}`;
}

/** Small always-on-top toast window shown when a download finishes. */
export const CompletionNotice: React.FC<CompletionNoticeProps> = ({ name, folder }) => {
  const [hover, setHover] = useState(false);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    const saved = localStorage.getItem("zr-theme") || "dark";
    document.documentElement.classList.add(saved === "dark" ? "dark" : "light");
    document.documentElement.classList.remove(saved === "dark" ? "light" : "dark");
    document.body.style.margin = "0";
    document.body.style.overflow = "hidden";
  }, []);

  const close = () => {
    getCurrentWindow()
      .close()
      .catch(() => getCurrentWindow().destroy().catch(() => {}));
  };

  // Auto-close, but never while the pointer is over the toast
  useEffect(() => {
    if (timer.current) window.clearTimeout(timer.current);
    if (!hover) timer.current = window.setTimeout(close, AUTO_CLOSE_MS);
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, [hover]);

  const openFile = async () => {
    try {
      const targetPath = resolveTargetPath(folder, name);
      await invoke("open_file", { path: targetPath });
    } catch {
      if (folder) await invoke("open_folder", { path: folder }).catch(() => {});
    }
    close();
  };

  const openFolder = async () => {
    if (folder) await invoke("open_folder", { path: folder }).catch(() => {});
    close();
  };

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      className="w-screen h-screen flex items-center px-4 gap-3 select-none bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 border border-emerald-500/40"
    >
      <CheckCircle2 className="w-8 h-8 text-emerald-500 shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-semibold uppercase tracking-wide text-emerald-600 dark:text-emerald-400">
          Download complete
        </div>
        <div className="text-[13px] font-medium truncate" title={name}>
          {name}
        </div>
        <div className="mt-2 flex gap-2">
          <button
            onClick={openFile}
            className="px-2.5 py-1 rounded-md bg-blue-600 hover:bg-blue-700 text-white text-[12px] font-semibold flex items-center gap-1"
          >
            <FileCheck className="w-3.5 h-3.5" /> Open
          </button>
          <button
            onClick={openFolder}
            className="px-2.5 py-1 rounded-md bg-slate-200 hover:bg-slate-300 dark:bg-slate-700 dark:hover:bg-slate-600 text-[12px] font-semibold flex items-center gap-1"
          >
            <FolderOpen className="w-3.5 h-3.5" /> Folder
          </button>
        </div>
      </div>
      <button onClick={close} className="self-start mt-2 p-1 rounded hover:bg-slate-200 dark:hover:bg-slate-700" title="Close">
        <X className="w-4 h-4" />
      </button>
    </div>
  );
};
