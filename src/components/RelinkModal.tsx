import React, { useState } from "react";
import { X, RefreshCw, AlertCircle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { DownloadTask } from "../types";

interface RelinkModalProps {
  isOpen: boolean;
  task: DownloadTask | null;
  onClose: () => void;
  onSuccess?: () => void;
}

export const RelinkModal: React.FC<RelinkModalProps> = ({
  isOpen,
  task,
  onClose,
  onSuccess,
}) => {
  const [newUrl, setNewUrl] = useState(task?.url || "");
  const [headersText, setHeadersText] = useState("");
  const [proxy, setProxy] = useState("");
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  React.useEffect(() => {
    if (task) {
      setNewUrl(task.url || "");
      setErrorMsg(null);
    }
  }, [task]);

  if (!isOpen || !task) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleanUrl = newUrl.trim();
    if (!cleanUrl) {
      setErrorMsg("Please enter a valid URL.");
      return;
    }
    setLoading(true);
    setErrorMsg(null);

    let headersMap: Record<string, string> | null = null;
    if (headersText.trim()) {
      headersMap = {};
      for (const line of headersText.split("\n")) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith("#")) continue;
        const colonIdx = trimmed.indexOf(":");
        if (colonIdx > 0) {
          const k = trimmed.slice(0, colonIdx).trim();
          const v = trimmed.slice(colonIdx + 1).trim();
          if (k) headersMap[k] = v;
        }
      }
    }

    try {
      await invoke("relink_http_download", {
        taskId: task.id,
        newUrl: cleanUrl,
        headers: headersMap,
        proxy: proxy.trim() || null,
      });
      onSuccess?.();
      onClose();
    } catch (err: any) {
      setErrorMsg(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[120] flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div className="w-full max-w-lg rounded-2xl border border-zr bg-zr-surface shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-zr bg-zr-tableHead">
          <div className="flex items-center space-x-2">
            <RefreshCw className="w-4 h-4 text-zr-accent" />
            <h3 className="text-[14px] font-bold text-zr-primary">Change Download Link (Refresh Link)</h3>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-lg text-zr-secondary hover:text-zr-primary hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-5 space-y-4">
          <div className="text-[12px] text-zr-secondary">
            Update the URL for <b className="text-zr-primary">{task.filename}</b>. All previously downloaded bytes and segment positions will be preserved. The new link must support byte ranges and have the same file size.
          </div>

          <div>
            <label className="text-[12px] font-semibold text-zr-primary block mb-1">
              New Download URL
            </label>
            <input
              type="text"
              required
              value={newUrl}
              onChange={(e) => setNewUrl(e.target.value)}
              placeholder="https://example.com/file.zip"
              className="w-full px-3 py-2 rounded-lg border border-zr bg-zr-app text-zr-primary text-[12px] font-mono focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
          </div>

          <div>
            <label className="text-[12px] font-semibold text-zr-primary block mb-1">
              Custom Headers (Optional, one per line)
            </label>
            <textarea
              rows={3}
              value={headersText}
              onChange={(e) => setHeadersText(e.target.value)}
              placeholder="Cookie: session_id=xyz&#10;Authorization: Bearer token123&#10;Referer: https://example.com"
              className="w-full px-3 py-2 rounded-lg border border-zr bg-zr-app text-zr-primary text-[12px] font-mono focus:outline-none focus:ring-1 focus:ring-blue-500 resize-none"
            />
          </div>

          <div>
            <label className="text-[12px] font-semibold text-zr-primary block mb-1">
              Proxy Override (Optional)
            </label>
            <input
              type="text"
              value={proxy}
              onChange={(e) => setProxy(e.target.value)}
              placeholder="http://127.0.0.1:8080 or socks5://127.0.0.1:1080"
              className="w-full px-3 py-2 rounded-lg border border-zr bg-zr-app text-zr-primary text-[12px] font-mono focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
          </div>

          {errorMsg && (
            <div className="p-3 rounded-lg border border-red-500/30 bg-red-500/10 text-red-600 dark:text-red-400 text-[12px] flex items-start space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
              <span>{errorMsg}</span>
            </div>
          )}

          <div className="flex justify-end space-x-2 pt-2 border-t border-zr">
            <button
              type="button"
              onClick={onClose}
              disabled={loading}
              className="px-4 py-2 rounded-lg border border-zr text-zr-secondary hover:text-zr-primary hover:bg-slate-100 dark:hover:bg-slate-800 text-[12.5px] font-semibold transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading}
              className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-[12.5px] font-semibold transition-colors flex items-center space-x-1.5 disabled:opacity-50"
            >
              {loading && <RefreshCw className="w-3.5 h-3.5 animate-spin" />}
              <span>{loading ? "Validating link…" : "Apply & Refresh Link"}</span>
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
