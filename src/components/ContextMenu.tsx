import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Play, Pause, FolderOpen, FileCheck, Trash2, Link2, RotateCcw } from "lucide-react";
import { DownloadTask } from "../types";

interface ContextMenuProps {
  x: number;
  y: number;
  task: DownloadTask;
  onClose: () => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onDelete: (id: string) => void;
  onOpenFile: (path: string) => void;
  onOpenFolder: (path: string) => void;
  onCopyLink: (url: string) => void;
}

/**
 * Right-click menu for a download row (IDM / Free Download Manager style).
 * Rendered only while open, so all hooks below run on every render of this component.
 */
export const ContextMenu: React.FC<ContextMenuProps> = ({
  x,
  y,
  task,
  onClose,
  onPause,
  onResume,
  onDelete,
  onOpenFile,
  onOpenFolder,
  onCopyLink,
}) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  // Keep the latest onClose without re-attaching window listeners on every parent render.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  // Keep the menu fully inside the window.
  useLayoutEffect(() => {
    const el = menuRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    setPos({
      x: Math.max(8, Math.min(x, window.innerWidth - rect.width - 8)),
      y: Math.max(8, Math.min(y, window.innerHeight - rect.height - 8)),
    });
  }, [x, y]);

  // Close on outside click, Escape, resize, blur or scroll.
  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) onCloseRef.current();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCloseRef.current();
    };
    const close = () => onCloseRef.current();
    window.addEventListener("mousedown", onMouseDown);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("scroll", close, true);
    };
  }, []);

  const path = task.save_path || task.savePath || "";
  const isFileReady = task.status === "completed" || task.status === "completed_with_warning";
  const isDownloading = task.status === "downloading";
  const isError = task.status === "error";
  const canResume =
    task.status === "paused" ||
    task.status === "interrupted" ||
    task.status === "queued" ||
    isError;

  const run = (action: () => void) => () => {
    action();
    onClose();
  };

  const itemClass =
    "w-full flex items-center gap-2.5 px-3 py-2 text-[13px] text-left text-zr-primary hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors";

  return (
    <div
      ref={menuRef}
      role="menu"
      onContextMenu={(e) => e.preventDefault()}
      style={{ left: pos.x, top: pos.y }}
      className="fixed z-[100] min-w-[190px] py-1 rounded-lg border border-zr bg-zr-surface shadow-2xl select-none"
    >
      {isFileReady && (
        <button
          type="button"
          role="menuitem"
          className={itemClass}
          onClick={run(() => onOpenFile(path))}
        >
          <FileCheck className="w-4 h-4 text-emerald-500" />
          <span>Open file</span>
        </button>
      )}
      {isDownloading && (
        <button
          type="button"
          role="menuitem"
          className={itemClass}
          onClick={run(() => onPause(task.id))}
        >
          <Pause className="w-4 h-4 text-amber-500" />
          <span>Pause</span>
        </button>
      )}
      {canResume && (
        <button
          type="button"
          role="menuitem"
          className={itemClass}
          onClick={run(() => onResume(task.id))}
        >
          {isError ? (
            <RotateCcw className="w-4 h-4 text-blue-500" />
          ) : (
            <Play className="w-4 h-4 text-emerald-500 fill-current" />
          )}
          <span>{isError ? "Retry" : "Resume"}</span>
        </button>
      )}
      <button
        type="button"
        role="menuitem"
        className={itemClass}
        onClick={run(() => onOpenFolder(path))}
      >
        <FolderOpen className="w-4 h-4 text-blue-500" />
        <span>Show in folder</span>
      </button>
      <button
        type="button"
        role="menuitem"
        className={itemClass}
        onClick={run(() => onCopyLink(task.url))}
      >
        <Link2 className="w-4 h-4 text-slate-500" />
        <span>Copy download link</span>
      </button>
      <div className="my-1 border-t border-zr" />
      <button
        type="button"
        role="menuitem"
        className={`${itemClass} !text-red-600 dark:!text-red-400`}
        onClick={run(() => onDelete(task.id))}
      >
        <Trash2 className="w-4 h-4" />
        <span>Delete…</span>
      </button>
    </div>
  );
};
