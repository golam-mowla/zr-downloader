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
    <aside className="w-64 h-screen flex flex-col justify-between border-r border-gdm bg-gdm-surface text-gdm-primary p-4 z-20 select-none overflow-y-auto transition-colors">
      {/* Top Section */}
      <div className="space-y-5">
        {/* Goat Branding */}
        <div className="flex items-center space-x-3 px-1 pt-1">
          <div className="w-10 h-10 rounded-full overflow-hidden flex-shrink-0 bg-transparent">
            <img
              src="/icon.png"
              alt="Goat Logo"
              className="w-full h-full object-contain"
            />
          </div>

          <div className="flex flex-col">
            <div className="flex items-center space-x-1.5">
              <span className="font-bold text-[15px] tracking-tight text-gdm-primary">
                ZR Downloader
              </span>
            </div>
            <span className="text-[12px] font-medium text-gdm-secondary">
              High-Speed Download Accelerator
            </span>
          </div>
        </div>

        {/* Media Grabber Button */}
        <button
          onClick={onOpenMediaModal}
          className="w-full flex items-center justify-between px-3.5 py-2.5 rounded-lg bg-[#2563EB] hover:bg-blue-600 transition-colors text-[13px] font-semibold text-white shadow-xs"
        >
          <div className="flex items-center space-x-2">
            <Sparkles className="w-4 h-4 text-yellow-300" />
            <span>Media Grabber</span>
          </div>
          <span className="text-[12px] bg-black/25 text-white px-2 py-0.5 rounded font-mono font-medium">
            yt-dlp
          </span>
        </button>

        {/* Categories Section */}
        <div className="space-y-1">
          <p className="text-[12px] font-semibold text-gdm-secondary uppercase tracking-wider px-2.5 mb-1.5">
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
                    : "text-gdm-secondary hover:bg-gdm-tableHead hover:text-gdm-primary"
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
        <div className="space-y-1 pt-3 border-t border-gdm">
          <p className="text-[12px] font-semibold text-gdm-secondary uppercase tracking-wider px-2.5 mb-1.5">
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
                    : "text-gdm-secondary hover:bg-gdm-tableHead hover:text-gdm-primary"
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
      <div className="pt-3 border-t border-gdm space-y-2">
        {/* Browser Extension Bridge Status */}
        <div className="flex items-center justify-between px-3 py-2 rounded-lg border border-gdm bg-gdm-tableHead text-[12px]">
          <div className="flex items-center space-x-2">
            <span
              className={`w-2 h-2 rounded-full ${
                bridgeOnline ? "bg-emerald-500 shadow-[0_0_6px_#10B981]" : "bg-red-500"
              }`}
            />
            <span className="font-medium text-gdm-secondary">Browser Bridge</span>
          </div>
          <span className="font-semibold text-gdm-secondary text-[12px]">
            :9666
          </span>
        </div>

        {/* Options & Settings Button */}
        <button
          onClick={onOpenSettings}
          className="w-full flex items-center space-x-2 px-3 py-2 rounded-lg transition-colors text-[13px] font-medium text-gdm-secondary hover:text-gdm-primary hover:bg-gdm-tableHead"
        >
          <Settings className="w-[18px] h-[18px] text-gdm-secondary" />
          <span>Options & Settings</span>
        </button>
      </div>
    </aside>
  );
};
