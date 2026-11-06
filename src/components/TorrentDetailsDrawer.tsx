import React, { useState, useEffect } from "react";
import {
  X,
  HardDrive,
  Users,
  Radio,
  FileCode,
  Grid,
  Info,
  ChevronDown,
  ArrowDown,
  ArrowUp,
  Folder,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { TorrentDetailsData, TorrentFileItem, TorrentPeerItem, TorrentTrackerItem } from "../types";

interface TorrentDetailsDrawerProps {
  torrentId: string | null;
  isOpen: boolean;
  onClose: () => void;
  theme?: "dark" | "light";
}

export const TorrentDetailsDrawer: React.FC<TorrentDetailsDrawerProps> = ({
  torrentId,
  isOpen,
  onClose,
  theme = "dark",
}) => {
  const [activeTab, setActiveTab] = useState<"general" | "trackers" | "peers" | "content" | "pieces">("general");
  const [details, setDetails] = useState<TorrentDetailsData | null>(null);
  const [piecesBitfield, setPiecesBitfield] = useState<number[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    if (!isOpen || !torrentId) return;

    let isMounted = true;

    const fetchDetails = async () => {
      try {
        const raw: any = await invoke("get_torrent_details", { idOrHash: torrentId });
        if (!isMounted || !raw) return;

        const filesTotal: number = (raw.files || []).reduce((acc: number, f: any) => acc + (f.length || 0), 0);
        const live = raw.stats?.live;
        const total = raw.stats?.total_bytes || filesTotal || live?.snapshot?.total_bytes || 0;
        const downloaded = raw.stats?.progress_bytes || live?.snapshot?.downloaded_and_checked_bytes || 0;
        const uploaded = raw.stats?.uploaded_bytes || live?.snapshot?.uploaded_bytes || 0;
        const downSpeedMbps = live?.download_speed?.mbps || 0;
        const upSpeedMbps = live?.upload_speed?.mbps || 0;
        const downBytes = downSpeedMbps * 1024 * 1024 / 8;
        const upBytes = upSpeedMbps * 1024 * 1024 / 8;

        const files: TorrentFileItem[] = (raw.files || []).map((f: any, idx: number) => ({
          index: idx,
          path: f.name || `file_${idx}`,
          filename: (f.name || "").split(/[/\\]/).pop() || `file_${idx}`,
          length: f.length || 0,
          length_formatted: formatBytes(f.length || 0),
          selected: true,
          priority: "normal",
        }));

        const numPieces = raw.total_pieces || raw.info?.pieces?.length || 100;
        const ratio = downloaded > 0 ? (uploaded / downloaded) : 0;
        const progress = total > 0 ? (downloaded / total) * 100 : 0;
        const peersConnected = live?.snapshot?.peer_stats?.live ?? 0;
        const seedsConnected = live?.snapshot?.peer_stats?.live_seeders ?? 0;

        setDetails({
          id: torrentId,
          name: raw.name || "Torrent",
          info_hash: raw.info_hash || torrentId,
          state: raw.stats?.state?.is_paused?.() ? "paused" : progress >= 100 ? "seeding" : "downloading",
          total_bytes: total,
          downloaded_bytes: downloaded,
          uploaded_bytes: uploaded,
          progress,
          download_speed: `${formatBytes(downBytes)}/s`,
          upload_speed: `${formatBytes(upBytes)}/s`,
          eta: downBytes > 0 ? formatEta((total - downloaded) / downBytes) : "--",
          ratio,
          peers_connected: peersConnected,
          seeds_connected: seedsConnected,
          save_path: raw.output_folder || "Downloads",
          piece_length: raw.info?.piece_length || 0,
          num_pieces: numPieces,
          pieces_downloaded: Math.round((progress / 100) * numPieces),
          files,
          peers: [],
          trackers: (raw.trackers || []).map((t: string) => ({
            url: t,
            status: "Working",
            peers: 0,
            seeds: 0,
            interval: 1800,
          })),
        });

        // Also fetch peers
        try {
          const peerStats: any = await invoke("get_torrent_peers", { idOrHash: torrentId });
          const peersMap = peerStats?.peers || {};
          const mappedPeers: TorrentPeerItem[] = Object.entries(peersMap).map(([addr, p]: [string, any]) => {
            const fetched = p?.counters?.fetched_bytes || 0;
            const up = p?.counters?.uploaded_bytes || 0;
            const clientName = p?.client_name || "BitTorrent Peer";
            const kind = p?.conn_kind ? String(p.conn_kind).toUpperCase() : "TCP";
            const isLive = p?.state === "live";
            return {
              addr: addr || "Unknown",
              client: clientName,
              flags: `${kind} ${isLive ? "D" : ""}`.trim(),
              down_speed: fetched > 0 ? formatBytes(fetched) : "0 B/s",
              up_speed: up > 0 ? formatBytes(up) : "0 B/s",
              progress: 100,
            };
          });
          if (mappedPeers.length > 0) {
            setDetails((prev) => prev ? { ...prev, peers: mappedPeers } : null);
          }
        } catch {}

        // Fetch pieces
        try {
          const piecesData: any = await invoke("get_torrent_pieces", { idOrHash: torrentId });
          if (piecesData && piecesData.pieces_bitfield) {
            const hex = String(piecesData.pieces_bitfield);
            const totalP = piecesData.total_pieces || numPieces;
            const bits: number[] = [];
            for (let i = 0; i < hex.length; i += 2) {
              const byteVal = parseInt(hex.substring(i, i + 2), 16) || 0;
              for (let bit = 7; bit >= 0; bit--) {
                if (bits.length < totalP) {
                  bits.push((byteVal >> bit) & 1);
                }
              }
            }
            setPiecesBitfield(bits);
          } else if (Array.isArray(piecesData)) {
            setPiecesBitfield(piecesData);
          }
        } catch {}
      } catch (e) {
        console.error("Fetch torrent details error:", e);
      }
    };

    fetchDetails();
    const interval = setInterval(fetchDetails, 1500);
    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, [isOpen, torrentId]);

  if (!isOpen || !torrentId) return null;

  return (
    <div className="border-t border-zr bg-zr-surface text-zr-primary transition-colors flex flex-col h-72 shadow-lg z-30">
      {/* Header & Tabs */}
      <div className="flex items-center justify-between border-b border-zr bg-zr-tableHead px-4 py-2">
        <div className="flex items-center space-x-1">
          <button
            onClick={() => setActiveTab("general")}
            className={`px-3 py-1.5 rounded-md text-[13px] font-semibold flex items-center space-x-1.5 transition-colors ${
              activeTab === "general"
                ? "bg-blue-600 text-white shadow-xs"
                : "text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700/60"
            }`}
          >
            <Info className="w-3.5 h-3.5" />
            <span>General</span>
          </button>
          <button
            onClick={() => setActiveTab("trackers")}
            className={`px-3 py-1.5 rounded-md text-[13px] font-semibold flex items-center space-x-1.5 transition-colors ${
              activeTab === "trackers"
                ? "bg-blue-600 text-white shadow-xs"
                : "text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700/60"
            }`}
          >
            <Radio className="w-3.5 h-3.5" />
            <span>Trackers ({details?.trackers.length || 0})</span>
          </button>
          <button
            onClick={() => setActiveTab("peers")}
            className={`px-3 py-1.5 rounded-md text-[13px] font-semibold flex items-center space-x-1.5 transition-colors ${
              activeTab === "peers"
                ? "bg-blue-600 text-white shadow-xs"
                : "text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700/60"
            }`}
          >
            <Users className="w-3.5 h-3.5" />
            <span>Peers ({details?.peers.length || details?.peers_connected || 0})</span>
          </button>
          <button
            onClick={() => setActiveTab("content")}
            className={`px-3 py-1.5 rounded-md text-[13px] font-semibold flex items-center space-x-1.5 transition-colors ${
              activeTab === "content"
                ? "bg-blue-600 text-white shadow-xs"
                : "text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700/60"
            }`}
          >
            <FileCode className="w-3.5 h-3.5" />
            <span>Files ({details?.files.length || 0})</span>
          </button>
          <button
            onClick={() => setActiveTab("pieces")}
            className={`px-3 py-1.5 rounded-md text-[13px] font-semibold flex items-center space-x-1.5 transition-colors ${
              activeTab === "pieces"
                ? "bg-blue-600 text-white shadow-xs"
                : "text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700/60"
            }`}
          >
            <Grid className="w-3.5 h-3.5" />
            <span>Pieces Bar</span>
          </button>
        </div>

        <button
          onClick={onClose}
          className="p-1 rounded-md text-zr-secondary hover:text-zr-primary hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
          title="Close details"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Tab Contents */}
      <div className="flex-1 overflow-y-auto p-4 text-[13px]">
        {activeTab === "general" && (
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="space-y-2 p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40">
              <span className="text-[12px] font-medium text-zr-secondary uppercase tracking-wider block">Transfer</span>
              <div className="space-y-1">
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Download Speed:</span>
                  <span className="font-semibold text-emerald-600 dark:text-emerald-400 flex items-center space-x-1">
                    <ArrowDown className="w-3 h-3" />
                    <span>{details?.download_speed || "0 B/s"}</span>
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Upload Speed:</span>
                  <span className="font-semibold text-blue-600 dark:text-blue-400 flex items-center space-x-1">
                    <ArrowUp className="w-3 h-3" />
                    <span>{details?.upload_speed || "0 B/s"}</span>
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Share Ratio:</span>
                  <span className="font-semibold text-zr-primary">{(details?.ratio || 0).toFixed(2)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">ETA:</span>
                  <span className="font-semibold text-zr-primary">{details?.eta || "--"}</span>
                </div>
              </div>
            </div>

            <div className="space-y-2 p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40">
              <span className="text-[12px] font-medium text-zr-secondary uppercase tracking-wider block">Swarm</span>
              <div className="space-y-1">
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Connected Peers:</span>
                  <span className="font-semibold text-zr-primary">{details?.peers_connected || 0}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Connected Seeds:</span>
                  <span className="font-semibold text-zr-primary">{details?.seeds_connected || 0}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Pieces:</span>
                  <span className="font-semibold text-zr-primary">{details?.pieces_downloaded} / {details?.num_pieces}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Piece Size:</span>
                  <span className="font-semibold text-zr-primary">{formatBytes(details?.piece_length || 0)}</span>
                </div>
              </div>
            </div>

            <div className="col-span-2 space-y-2 p-3 rounded-lg border border-zr bg-slate-50 dark:bg-slate-900/40">
              <span className="text-[12px] font-medium text-zr-secondary uppercase tracking-wider block">Information</span>
              <div className="space-y-1">
                <div className="flex justify-between">
                  <span className="text-zr-secondary">InfoHash:</span>
                  <span className="font-mono text-[12px] text-zr-primary select-all truncate max-w-xs">{details?.info_hash || "--"}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Save Path:</span>
                  <span className="font-mono text-[12px] text-zr-primary truncate max-w-xs">{details?.save_path || "--"}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Total Size:</span>
                  <span className="font-semibold text-zr-primary">{formatBytes(details?.total_bytes || 0)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-zr-secondary">Status:</span>
                  <span className="font-semibold capitalize text-zr-accent">{details?.state || "Unknown"}</span>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeTab === "trackers" && (
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-zr text-[12px] text-zr-secondary">
                  <th className="pb-2 font-medium">#</th>
                  <th className="pb-2 font-medium">Tracker URL</th>
                  <th className="pb-2 font-medium">Status</th>
                  <th className="pb-2 font-medium">Peers</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-zr/50 text-[12.5px]">
                {(details?.trackers || []).map((tr, idx) => (
                  <tr key={idx} className="hover:bg-slate-100 dark:hover:bg-slate-800/40">
                    <td className="py-2 text-zr-secondary">{idx + 1}</td>
                    <td className="py-2 font-mono text-[12px] text-zr-primary">{tr.url}</td>
                    <td className="py-2 text-emerald-600 dark:text-emerald-400 font-semibold">{tr.status}</td>
                    <td className="py-2 text-zr-secondary">{tr.peers}</td>
                  </tr>
                ))}
                {(!details?.trackers || details.trackers.length === 0) && (
                  <tr>
                    <td colSpan={4} className="py-4 text-center text-zr-muted">
                      DHT / PEX Swarm active (no manual trackers added)
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}

        {activeTab === "peers" && (
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-zr text-[12px] text-zr-secondary">
                  <th className="pb-2 font-medium">IP : Port</th>
                  <th className="pb-2 font-medium">Client</th>
                  <th className="pb-2 font-medium">Flags</th>
                  <th className="pb-2 font-medium">Progress</th>
                  <th className="pb-2 font-medium">Down Speed</th>
                  <th className="pb-2 font-medium">Up Speed</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-zr/50 text-[12.5px]">
                {(details?.peers || []).map((peer, idx) => (
                  <tr key={idx} className="hover:bg-slate-100 dark:hover:bg-slate-800/40">
                    <td className="py-2 font-mono text-[12px] text-zr-primary">{peer.addr}</td>
                    <td className="py-2 text-zr-primary">{peer.client}</td>
                    <td className="py-2 font-mono text-[11px] text-zr-secondary">{peer.flags}</td>
                    <td className="py-2">
                      <div className="flex items-center space-x-2">
                        <div className="w-16 h-1.5 bg-slate-200 dark:bg-slate-700 rounded-full overflow-hidden">
                          <div className="h-full bg-blue-600 rounded-full" style={{ width: `${peer.progress}%` }} />
                        </div>
                        <span className="text-[11px] text-zr-secondary">{peer.progress}%</span>
                      </div>
                    </td>
                    <td className="py-2 text-emerald-600 dark:text-emerald-400 font-semibold">{peer.down_speed}</td>
                    <td className="py-2 text-blue-600 dark:text-blue-400 font-semibold">{peer.up_speed}</td>
                  </tr>
                ))}
                {(!details?.peers || details.peers.length === 0) && (
                  <tr>
                    <td colSpan={6} className="py-4 text-center text-zr-muted">
                      Searching for swarm peers via Mainline DHT and UDP trackers...
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}

        {activeTab === "content" && (
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-zr text-[12px] text-zr-secondary">
                  <th className="pb-2 font-medium">#</th>
                  <th className="pb-2 font-medium">File Name</th>
                  <th className="pb-2 font-medium">Size</th>
                  <th className="pb-2 font-medium">Priority</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-zr/50 text-[12.5px]">
                {(details?.files || []).map((file, idx) => (
                  <tr key={idx} className="hover:bg-slate-100 dark:hover:bg-slate-800/40">
                    <td className="py-2 text-zr-secondary">{idx + 1}</td>
                    <td className="py-2 font-medium text-zr-primary">{file.filename}</td>
                    <td className="py-2 text-zr-secondary">{file.length_formatted}</td>
                    <td className="py-2">
                      <span className="px-2 py-0.5 rounded text-[11px] font-semibold bg-blue-100 dark:bg-blue-950/60 text-blue-700 dark:text-blue-300 border border-blue-300 dark:border-blue-800">
                        Normal
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {activeTab === "pieces" && (
          <div className="space-y-3">
            <div className="flex items-center justify-between text-[12px] text-zr-secondary">
              <span>Downloaded: {details?.pieces_downloaded} of {details?.num_pieces} pieces ({(details?.progress || 0).toFixed(1)}%)</span>
              <div className="flex items-center space-x-3 text-[11.5px]">
                <div className="flex items-center space-x-1">
                  <span className="w-3 h-3 rounded-xs bg-emerald-500 inline-block" />
                  <span>Downloaded</span>
                </div>
                <div className="flex items-center space-x-1">
                  <span className="w-3 h-3 rounded-xs bg-slate-300 dark:bg-slate-700 inline-block" />
                  <span>Missing</span>
                </div>
              </div>
            </div>
            {/* Visual Piece Block Grid */}
            <div className="p-3 rounded-lg border border-zr bg-slate-100 dark:bg-slate-900 flex flex-wrap gap-1 max-h-48 overflow-y-auto">
              {Array.from({ length: Math.min(200, details?.num_pieces || 100) }).map((_, i) => {
                const isDownloaded = details ? (i < (details.pieces_downloaded / details.num_pieces) * 200) : false;
                return (
                  <span
                    key={i}
                    className={`w-3 h-3 rounded-xs transition-colors ${
                      isDownloaded
                        ? "bg-emerald-500 shadow-xs"
                        : "bg-slate-300 dark:bg-slate-700 hover:bg-slate-400"
                    }`}
                    title={`Piece #${i + 1}: ${isDownloaded ? "Verified" : "Pending"}`}
                  />
                );
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return "0 B";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
  if (mb >= 1) return `${mb.toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

function formatEta(seconds: number): string {
  if (!isFinite(seconds) || seconds <= 0) return "--";
  const s = Math.round(seconds);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}

