// ZR Downloader - BitTorrent Engine
// High-performance asynchronous BitTorrent client powered by librqbit Api facade
// 100% clean-room Apache-2.0 implementation with qBittorrent-level features

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use librqbit::api::{Api, TorrentDetailsResponse, TorrentIdOrHash};
use librqbit::http_api_types::{PeerStatsFilter, PeerStatsSnapshot};
use librqbit::{
    AddTorrent, AddTorrentOptions, ConnectionOptions, DhtSessionConfig, ListenerMode,
    ListenerOptions, PeerConnectionOptions, Session, SessionOptions, TorrentStatsState,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTorrentRequest {
    pub source: String, // Magnet URI or local file path to .torrent
    pub save_folder: Option<String>,
    pub selected_files: Option<Vec<usize>>,
    pub paused: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentPiecesInfo {
    pub total_pieces: u32,
    pub pieces_bitfield: String, // hex of have pieces
    pub have_pieces: u32,
}

pub struct TorrentState {
    pub api: Arc<RwLock<Option<Api>>>,
}

impl Default for TorrentState {
    fn default() -> Self {
        Self {
            api: Arc::new(RwLock::new(None)),
        }
    }
}

pub const TOP_TRACKER_URLS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://explodie.org:6969/announce",
    "udp://tracker.openbittorrent.com:6969/announce",
    "udp://tracker.cyberia.is:6969/announce",
    "udp://p4p.arenabg.com:1337/announce",
    "http://tracker.openbittorrent.com:80/announce",
    "udp://open.demonii.com:1337/announce",
    "udp://tracker.dler.org:6969/announce",
    "udp://tracker.moeking.me:6969/announce",
    "udp://tracker.coppersurfer.tk:6969/announce",
    "udp://9.rarbg.to:2920/announce",
    "udp://tracker.pirateparty.gr:6969/announce",
    "udp://tracker.leechers-paradise.org:6969/announce",
    "udp://ipv4.tracker.harry.lu:80/announce",
    "udp://valakas.rollo.cc:6969/announce",
    "udp://tracker.zerobytes.xyz:1337/announce",
    "udp://tracker.fnix.net:6969/announce",
];

pub async fn get_or_init_torrent_api(
    state: &TorrentState,
    default_dir: Option<String>,
) -> Result<Api, String> {
    let mut guard = state.api.write().await;
    if let Some(ref api) = *guard {
        return Ok(api.clone());
    }

    let download_dir = if let Some(d) = default_dir {
        PathBuf::from(d)
    } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
        PathBuf::from(user_profile).join("Downloads")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join("Downloads")
    } else {
        std::env::temp_dir()
    };

    let listen_opts = ListenerOptions {
        mode: ListenerMode::TcpAndUtp,
        listen_addr: (std::net::Ipv4Addr::UNSPECIFIED, 0).into(),
        enable_upnp_port_forwarding: true,
        ..Default::default()
    };

    let mut default_trackers = std::collections::HashSet::new();
    for &t in TOP_TRACKER_URLS {
        if let Ok(u) = reqwest::Url::parse(t) {
            default_trackers.insert(u);
        }
    }

    let dht_config = DhtSessionConfig {
        bootstrap_addrs: Some(vec![
            "router.bittorrent.com:6881".to_string(),
            "router.utorrent.com:6881".to_string(),
            "dht.transmissionbt.com:6881".to_string(),
            "dht.libtorrent.org:25401".to_string(),
            "dht.aelitis.com:6881".to_string(),
        ]),
        port: None,
        persistence: None,
    };

    let peer_opts = PeerConnectionOptions {
        connect_timeout: Some(std::time::Duration::from_secs(4)),
        read_write_timeout: Some(std::time::Duration::from_secs(10)),
        keep_alive_interval: Some(std::time::Duration::from_secs(30)),
        max_request_window: Some(256),
    };

    let conn_opts = ConnectionOptions {
        enable_tcp: true,
        proxy_url: None,
        peer_opts: Some(peer_opts.clone()),
    };

    let session_opts = SessionOptions {
        fastresume: true,
        listen: Some(listen_opts),
        connect: Some(conn_opts),
        dht: Some(dht_config),
        trackers: default_trackers,
        peer_limit: Some(300),
        client_name_and_version: Some("ZR Downloader 1.2.12".to_string()),
        ..Default::default()
    };

    let session = Session::new_with_opts(download_dir, session_opts)
        .await
        .map_err(|e| format!("Failed to create BitTorrent session: {}", e))?;

    let api = Api::new(session, None);
    *guard = Some(api.clone());
    Ok(api)
}

// ---------------------------------------------------------------------------
// Command 1: Add Torrent (via Magnet link or .torrent file bytes)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn add_torrent(
    app: AppHandle,
    state: State<'_, TorrentState>,
    request: AddTorrentRequest,
) -> Result<TorrentDetailsResponse, String> {
    let api = get_or_init_torrent_api(&state, request.save_folder.clone()).await?;

    // Clean source string: strip @[, ], quotes, backticks, whitespace
    let mut clean_source = request.source.trim().to_string();
    if clean_source.starts_with("@[") && clean_source.ends_with(']') {
        clean_source = clean_source[2..clean_source.len() - 1].trim().to_string();
    }
    clean_source = clean_source
        .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '[' || c == ']')
        .trim()
        .to_string();

    let add_torrent = if clean_source.starts_with("magnet:") {
        AddTorrent::from_url(&clean_source)
    } else if clean_source.starts_with("http://") || clean_source.starts_with("https://") {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        let bytes = client
            .get(&clean_source)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch remote torrent file: {}", e))?
            .bytes()
            .await
            .map_err(|e| format!("Failed to read remote torrent file bytes: {}", e))?;
        AddTorrent::from_bytes(bytes.to_vec())
    } else {
        let p = PathBuf::from(&clean_source);
        if !p.exists() {
            return Err(format!("Torrent file does not exist: {}", clean_source));
        }
        let bytes = std::fs::read(&p).map_err(|e| format!("Failed to read torrent file: {}", e))?;
        AddTorrent::from_bytes(bytes)
    };

    let all_trackers: Vec<String> = TOP_TRACKER_URLS.iter().map(|&s| s.to_string()).collect();
    let add_opts = AddTorrentOptions {
        paused: request.paused.unwrap_or(false),
        output_folder: request.save_folder.clone(),
        only_files: request.selected_files.clone(),
        overwrite: true,
        peer_limit: Some(250),
        trackers: Some(all_trackers),
        peer_opts: Some(PeerConnectionOptions {
            connect_timeout: Some(std::time::Duration::from_secs(4)),
            read_write_timeout: Some(std::time::Duration::from_secs(10)),
            keep_alive_interval: Some(std::time::Duration::from_secs(30)),
            max_request_window: Some(256),
        }),
        ..Default::default()
    };

    let response = api
        .api_add_torrent(add_torrent, Some(add_opts))
        .await
        .map_err(|e| format!("Failed to add torrent: {}", e))?;

    let id = response.id.unwrap_or(0);
    let id_or_hash = TorrentIdOrHash::Id(id);
    let mut details = api
        .api_torrent_details(id_or_hash)
        .map_err(|e| format!("Failed to fetch torrent details: {}", e))?;
    if let Ok(st) = api.api_stats_v1(id_or_hash) {
        details.stats = Some(st);
    }

    // Background live progress ticker emitting to ZR Downloader UI
    let api_clone = api.clone();
    let app_clone = app.clone();
    let info_hash = details.info_hash.clone();

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

            let current_details = match api_clone.api_torrent_details(id_or_hash) {
                Ok(d) => d,
                Err(_) => break, // Torrent removed or closed
            };

            let stats_opt = api_clone.api_stats_v1(id_or_hash).ok();
            let stats = stats_opt.as_ref();
            let live = stats.and_then(|s| s.live.as_ref());

            let downloaded = stats.map(|s| s.progress_bytes).unwrap_or(0);
            let mut total = stats.map(|s| s.total_bytes).unwrap_or(0);
            if total == 0 {
                total = current_details
                    .files
                    .as_ref()
                    .map(|files| files.iter().map(|f| f.length).sum())
                    .unwrap_or(0);
            }

            let down_speed = live.map(|l| l.download_speed.mbps * 1024.0 * 1024.0 / 8.0).unwrap_or(0.0) as u64;
            let up_speed = live.map(|l| l.upload_speed.mbps * 1024.0 * 1024.0 / 8.0).unwrap_or(0.0) as u64;

            let live_peers = live.map(|l| l.snapshot.peer_stats.live).unwrap_or(0);
            let live_seeders = live.map(|l| l.snapshot.peer_stats.live_seeders).unwrap_or(0);
            let total_seen = live.map(|l| l.snapshot.peer_stats.seen).unwrap_or(0);

            let pct = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };

            let is_paused = matches!(
                stats.map(|s| &s.state),
                Some(TorrentStatsState::Paused | TorrentStatsState::Initializing { paused: true })
            );
            let status = if is_paused {
                "paused"
            } else if pct >= 100.0 {
                "seeding"
            } else if down_speed > 0 || live_peers > 0 {
                "downloading"
            } else {
                "connecting"
            };

            let eta_str = if down_speed > 0 && total > downloaded {
                format_eta((total - downloaded) / down_speed)
            } else {
                "--".to_string()
            };

            let _ = app_clone.emit(
                "download-progress",
                serde_json::json!({
                    "id": info_hash.clone(),
                    "percent": pct,
                    "text": format!("{:.1}% of {}", pct, format_bytes(total)),
                    "speed": format!("{}/s", format_bytes(down_speed)),
                    "speed_formatted": format!("{}/s", format_bytes(down_speed)),
                    "up_speed": format!("{}/s", format_bytes(up_speed)),
                    "up_speed_formatted": format!("{}/s", format_bytes(up_speed)),
                    "eta": eta_str.clone(),
                    "eta_formatted": eta_str,
                    "total_bytes": total,
                    "downloaded_bytes": downloaded,
                    "status": status,
                    "category": "torrents",
                    "peers": live_peers,
                    "seeders": live_seeders,
                    "total_peers": total_seen,
                }),
            );

            if pct >= 100.0 && status == "seeding" {
                // Keep polling for seeding speed until stopped
            }
        }
    });

    Ok(details)
}

// ---------------------------------------------------------------------------
// Command 2: Get Torrent Details (General, Files, Piece stats)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_torrent_details(
    state: State<'_, TorrentState>,
    id_or_hash: String,
) -> Result<TorrentDetailsResponse, String> {
    let api = get_or_init_torrent_api(&state, None).await?;
    let target = TorrentIdOrHash::parse(&id_or_hash)
        .map_err(|e| format!("Invalid torrent ID/hash: {}", e))?;
    let mut details = api.api_torrent_details(target)
        .map_err(|e| format!("Failed to get details: {}", e))?;
    if let Ok(st) = api.api_stats_v1(target) {
        details.stats = Some(st);
    }
    Ok(details)
}

// ---------------------------------------------------------------------------
// Command 3: Pause Torrent
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn pause_torrent(
    state: State<'_, TorrentState>,
    id_or_hash: String,
) -> Result<(), String> {
    let api = get_or_init_torrent_api(&state, None).await?;
    let target = TorrentIdOrHash::parse(&id_or_hash)
        .map_err(|e| format!("Invalid torrent ID/hash: {}", e))?;
    api.api_torrent_action_pause(target)
        .await
        .map(|_| ())
        .map_err(|e| format!("Failed to pause torrent: {}", e))
}

// ---------------------------------------------------------------------------
// Command 4: Resume Torrent
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn resume_torrent(
    state: State<'_, TorrentState>,
    id_or_hash: String,
) -> Result<(), String> {
    let api = get_or_init_torrent_api(&state, None).await?;
    let target = TorrentIdOrHash::parse(&id_or_hash)
        .map_err(|e| format!("Invalid torrent ID/hash: {}", e))?;
    api.api_torrent_action_start(target)
        .await
        .map(|_| ())
        .map_err(|e| format!("Failed to resume torrent: {}", e))
}

// ---------------------------------------------------------------------------
// Command 5: Remove Torrent (Optionally delete files)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn remove_torrent(
    app: AppHandle,
    state: State<'_, TorrentState>,
    engine_state: State<'_, crate::downloader::EngineState>,
    download_state: State<'_, crate::media::DownloadState>,
    id_or_hash: String,
    delete_files: bool,
) -> Result<(), String> {
    let api = get_or_init_torrent_api(&state, None).await?;
    let target = TorrentIdOrHash::parse(&id_or_hash)
        .map_err(|e| format!("Invalid torrent ID/hash: {}", e))?;
    if delete_files {
        api.api_torrent_action_delete(target)
            .await
            .map(|_| ())
            .map_err(|e| format!("Failed to delete torrent: {}", e))?;
    } else {
        api.api_torrent_action_forget(target)
            .await
            .map(|_| ())
            .map_err(|e| format!("Failed to forget torrent: {}", e))?;
    }
    {
        let mut tasks = engine_state.tasks.write().await;
        tasks.remove(&id_or_hash);
    }
    crate::media::forget_task(&download_state, &id_or_hash);
    crate::save_all_tasks(&app).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Command 6: Get Connected Peers
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_torrent_peers(
    state: State<'_, TorrentState>,
    id_or_hash: String,
) -> Result<PeerStatsSnapshot, String> {
    let api = get_or_init_torrent_api(&state, None).await?;
    let target = TorrentIdOrHash::parse(&id_or_hash)
        .map_err(|e| format!("Invalid torrent ID/hash: {}", e))?;
    api.api_peer_stats(target, PeerStatsFilter::default())
        .map_err(|e| format!("Failed to get peer stats: {}", e))
}

// ---------------------------------------------------------------------------
// Command 7: Get Pieces Info for Visual Bar
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_torrent_pieces(
    state: State<'_, TorrentState>,
    id_or_hash: String,
) -> Result<TorrentPiecesInfo, String> {
    let api = get_or_init_torrent_api(&state, None).await?;
    let target = TorrentIdOrHash::parse(&id_or_hash)
        .map_err(|e| format!("Invalid torrent ID/hash: {}", e))?;
    let (bf, total) = api
        .api_dump_haves(target)
        .map_err(|e| format!("Failed to dump pieces: {}", e))?;

    let have_count = bf.count_ones() as u32;
    let hex_bitfield: String = bf
        .as_raw_slice()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    Ok(TorrentPiecesInfo {
        total_pieces: total,
        pieces_bitfield: hex_bitfield,
        have_pieces: have_count,
    })
}

// ---------------------------------------------------------------------------
// Helper: Format bytes
// ---------------------------------------------------------------------------
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_eta(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}
