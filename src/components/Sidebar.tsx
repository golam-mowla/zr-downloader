import React from "react";
import {
  Layers,
  Video,
  Music,
  FileText,
  Archive,
  Binary,
  Radio,
  Settings,
  Sparkles,
  CheckCircle2,
  Clock,
  PauseCircle,
  AlertCircle,
  HelpCircle,
} from "lucide-react";

export type CategoryType = "all" | "video" | "audio" | "docs" | "compressed" | "programs" | "torrents";
export type StatusFilterType = "all" | "downloading" | "completed" | "paused" | "error";

interface SidebarProps {
  selectedCategory: CategoryType;
  onSelectCategory: (cat: CategoryType) => void;
  selectedStatus: StatusFilterType;
  onSelectStatus: (status: StatusFilterType) => void;
  categoryCounts: Record<CategoryType, number>;
  statusCounts: Record<StatusFilterType, number>;
  bridgeOnline: boolean;
  onOpenSettings: () => void;
  onOpenHelp?: (tab?: "help" | "logs") => void;
  onOpenMediaModal: () => void;
  theme?: "dark" | "light";
}

export const Sidebar: React.FC<SidebarProps> = ({
  selectedCategory,
  onSelectCategory,
  selectedStatus,
  onSelectStatus,
  categoryCounts,
  statusCounts,
  bridgeOnline,
  onOpenSettings,
  onOpenHelp,
  onOpenMediaModal,
  theme = "dark",
}) => {
  const isDark = theme === "dark";

  const categories: { id: CategoryType; label: string; icon: React.ReactNode }[] = [
    { id: "all", label: "All Downloads", icon: <Layers className="w-[18px] h-[18px] text-blue-500 flex-shrink-0" /> },
    { id: "torrents", label: "Torrents", icon: <Radio className="w-[18px] h-[18px] text-orange-500 flex-shrink-0" /> },
    { id: "compressed", label: "Compressed", icon: <Archive className="w-[18px] h-[18px] text-amber-500 flex-shrink-0" /> },
    { id: "docs", label: "Documents", icon: <FileText className="w-[18px] h-[18px] text-emerald-500 flex-shrink-0" /> },
    { id: "audio", label: "Music & Audio", icon: <Music className="w-[18px] h-[18px] text-pink-500 flex-shrink-0" /> },
    { id: "programs", label: "Programs", icon: <Binary className="w-[18px] h-[18px] text-teal-500 flex-shrink-0" /> },
    { id: "video", label: "Videos", icon: <Video className="w-[18px] h-[18px] text-purple-500 flex-shrink-0" /> },
  ];

  const statuses: { id: StatusFilterType; label: string; icon: React.ReactNode }[] = [
    { id: "downloading", label: "Downloading", icon: <Clock className="w-[18px] h-[18px] text-blue-500 flex-shrink-0" /> },
    { id: "completed", label: "Finished", icon: <CheckCircle2 className="w-[18px] h-[18px] text-emerald-500 flex-shrink-0" /> },
    { id: "paused", label: "Paused", icon: <PauseCircle className="w-[18px] h-[18px] text-amber-500 flex-shrink-0" /> },
    { id: "error", label: "Incomplete / Error", icon: <AlertCircle className="w-[18px] h-[18px] text-red-500 flex-shrink-0" /> },
  ];

  return (
    <aside className="w-64 h-screen flex flex-col border-r border-zr bg-zr-surface text-zr-primary p-4 z-20 select-none overflow-hidden transition-colors shrink-0">
      {/* Top Section */}
      <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden space-y-4 pr-1 -mr-1">
        {/* ZR Downloader Branding */}
        <div className="flex items-center space-x-3 px-1 py-1">
          <div className="w-10 h-10 rounded-xl overflow-hidden flex-shrink-0 shadow-md">
            <img
              src="/icon.png"
              alt="ZR Downloader Logo"
              className="w-full h-full object-cover"
            />
          </div>

          <div className="flex flex-col">
            <div className="flex items-center space-x-1.5">
              <span className="font-bold text-[15px] tracking-tight text-zr-primary">
                ZR Downloader
              </span>
            </div>
            <span className="text-[11.5px] font-medium text-zr-secondary">
              by Golam Mowla
            </span>
          </div>
        </div>

        {/* Media Grabber Button (Mac Glass Gradient) */}
        <button
          onClick={onOpenMediaModal}
          className="w-full flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-gradient-to-b from-[#3b9bff] to-[#0a6cf0] hover:brightness-105 transition-all text-[13px] font-semibold text-white shadow-[0_4px_14px_rgba(10,108,240,0.4)] border-t border-white/30"
        >
          <div className="flex items-center space-x-2">
            <Sparkles className="w-4 h-4 text-yellow-300" />
            <span>Media Grabber</span>
          </div>
          <span className="text-[10.5px] bg-white/20 text-white px-2 py-0.5 rounded-md font-mono font-medium backdrop-blur-xs">
            yt-dlp
          </span>
        </button>

        {/* Categories Section */}
        <div className="space-y-1">
          <p className="text-[12px] font-semibold text-zr-secondary uppercase tracking-wider px-2.5 mb-1.5">
            Categories
          </p>
          {categories.map((cat) => {
            const isSelected = selectedCategory === cat.id && selectedStatus === "all";
            const count = categoryCounts[cat.id] || 0;
            return (
              <button
                key={cat.id}
                onClick={() => {
                  onSelectCategory(cat.id);
                  onSelectStatus("all");
                }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-[13px] font-medium transition-colors ${
                  isSelected
                    ? "bg-[#2563EB] text-white shadow-xs"
                    : "text-zr-secondary hover:bg-zr-tableHead hover:text-zr-primary"
                }`}
              >
                <div className="flex items-center space-x-2.5">
                  {cat.icon}
                  <span>{cat.label}</span>
                </div>
                <span
                  className={`text-[12px] font-semibold tabular-nums px-2 py-0.5 rounded-full ${
                    isSelected
                      ? "bg-black/25 text-white"
                      : isDark
                      ? "bg-[#1E293B] text-slate-200"
                      : "bg-slate-200 text-slate-800"
                  }`}
                >
                  {count}
                </span>
              </button>
            );
          })}
        </div>

        {/* Status Queue Section */}
        <div className="space-y-1 pt-3 border-t border-zr">
          <p className="text-[12px] font-semibold text-zr-secondary uppercase tracking-wider px-2.5 mb-1.5">
            Status Queue
          </p>
          {statuses.map((st) => {
            const isSelected = selectedStatus === st.id;
            const count = statusCounts[st.id] || 0;
            return (
              <button
                key={st.id}
                onClick={() => onSelectStatus(st.id)}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-[13px] font-medium transition-colors ${
                  isSelected
                    ? "bg-[#2563EB] text-white shadow-xs"
                    : "text-zr-secondary hover:bg-zr-tableHead hover:text-zr-primary"
                }`}
              >
                <div className="flex items-center space-x-2.5">
                  {st.icon}
                  <span>{st.label}</span>
                </div>
                {count > 0 && (
                  <span
                    className={`text-[12px] font-semibold tabular-nums px-2 py-0.5 rounded-full ${
                      isSelected
                        ? "bg-black/25 text-white"
                        : isDark
                        ? "bg-[#1E293B] text-slate-200"
                        : "bg-slate-200 text-slate-800"
                    }`}
                  >
                    {count}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {/* Bottom Section */}
      <div className="shrink-0 pt-3 mt-2 border-t border-zr space-y-2">
        {/* Browser Extension Bridge Status */}
        <div className="flex items-center justify-between px-3 py-2 rounded-lg border border-zr bg-zr-tableHead text-[12px]">
          <div className="flex items-center space-x-2">
            <span
              className={`w-2 h-2 rounded-full ${
                bridgeOnline ? "bg-emerald-500 shadow-[0_0_6px_#10B981]" : "bg-red-500"
              }`}
            />
            <span className="font-medium text-zr-secondary">Browser Bridge</span>
          </div>
          <span className="font-semibold text-zr-secondary text-[12px]">
            :9666
          </span>
        </div>

        {/* Options & Settings Button */}
        <button
          onClick={onOpenSettings}
          className="w-full flex items-center space-x-2 px-3 py-2 rounded-lg transition-colors text-[13px] font-medium text-zr-secondary hover:text-zr-primary hover:bg-zr-tableHead"
        >
          <Settings className="w-[18px] h-[18px] text-zr-secondary" />
          <span>Options & Settings</span>
        </button>

        {/* Help, GitHub Repo & Failure Logs */}
        {onOpenHelp && (
          <button
            onClick={() => onOpenHelp("help")}
            className="w-full flex items-center space-x-2 px-3 py-2 rounded-lg transition-colors text-[13px] font-medium text-zr-secondary hover:text-zr-primary hover:bg-zr-tableHead"
          >
            <HelpCircle className="w-[18px] h-[18px] text-blue-500" />
            <span>Help &amp; Logs</span>
          </button>
        )}
      </div>
    </aside>
  );
};

