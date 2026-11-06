# ZR Downloader BitTorrent Integration Plan (qBittorrent-Grade Torrent Engine)

## Overview
Integration of a high-performance, asynchronous BitTorrent client into ZR Downloader with full qBittorrent-level capabilities.

## Engine Selection & Verdict
- **libtorrent-rasterbar Rust bindings status:**
  - Evaluated `libtorrent-sys` (v1.0.0) and `libtorrent` (v0.1.1). Both are unmaintained, require external C++ MSVC vcpkg/Boost toolchains, and fail to compile in standard Rust environments on Windows.
  - Per project instructions (*"if it won't build, tell me and fall back to librqbit"*), falling back to **`librqbit`** (v9.1.0).
  - **`librqbit` Advantages:** Actively maintained 100% safe async Rust (Tokio-native), built-in DHT, PEX, LSD, uTP (BEP 29), UPnP port forwarding, sequential streaming, piece-level prioritization, multi-file selection, and zero external C++ dependencies. Clean Apache-2.0 license (100% clean-room, zero GPL code).

---

## Roadmap & Phases

### Phase 1: Core Torrent Engine & UI (Completed & Integrated)
- [x] Engine initialization: Session manager, torrent storage directory, state persistence via `librqbit` (pure Rust, Apache-2.0).
- [x] Socket Listener & Trackers: Enabled `ListenerMode::TcpAndUtp` with UPnP port forwarding and pre-populated 10 fast public trackers (`tracker.opentrackr.org`, `open.stealth.si`, `explodie.org`, `openbittorrent.com`, etc.).
- [x] Peer Stats & Swarm Monitoring: Fixed `api_stats_v1` background polling ticker; correctly emits `live_peers`, `live_seeders`, `total_seen`, and upload/download speeds to the UI.
- [x] Details Drawer Fixes:
  - Fixed `TorrentDetailsDrawer.tsx` to read `peer_stats.live` (number count) and `peerStats.peers` (HashMap table).
  - Implemented hex bitfield parser for `pieces_bitfield` into interactive color-coded block grid.
- [x] Table View Swarm Indicators: Displays `Seeds: X | Peers: Y` and `↓ X MB/s ↑ Y KB/s` for torrent items in `DownloadTableView.tsx`.
- [x] Add Torrent capabilities:
  - Magnet URI parsing (xt infohash, dn display name, tr tracker list).
  - `.torrent` file parsing (metainfo, file tree, piece length, infohash).
  - Native file picker for `.torrent` files via Tauri Dialog plugin.
  - Windows command-line args handler for `.torrent` files and `magnet:` links with percent-encoding.
  - Windows registry integration: `magnet://` URL protocol handler & `.torrent` file association (`ZRDownloader.Torrent`).
  - Browser extension interception of `.torrent` file downloads and 1-click `magnet:` link triggers.
  - Native window Drag & Drop support for `.torrent` files.
  - Robust input sanitization: handles `@[path]`, surrounding quotes, brackets, and remote `.torrent` HTTP/HTTPS URLs.
- [x] Torrent State Management:
  - States: Queued, Checking (hash verification), Downloading, Seeding, Paused, Stalled, Error.
  - Actions: Start, Pause, Resume, Remove (with or without files).
- [x] Details Panel (`TorrentDetailsDrawer.tsx` - qBittorrent-style tabs):
  - **General:** Infohash, piece count & size, total size, progress, elapsed time, share ratio, down/up speeds.
  - **Trackers:** Tracker URLs, status (Working, Updating, Error), peers.
  - **Peers:** Connected peer IP, port, client identity, flags, download/upload speeds, progress bar.
  - **Content (Files):** Per-file name, size, priority selector (Normal).
  - **Pieces Bar:** Visual color-coded piece block grid showing downloaded verified pieces vs missing pieces.
- [x] Paths:
  - Incomplete / complete download save directory selection.
  - Sidebar "Torrents" category filter.

### Phase 1.5: Professional Brand Polish & Android Mobile Integration (Completed)
- [x] **Brand Cleanliness**:
  - Removed "PRO" tag from next to "ZR Downloader" in `src/components/Sidebar.tsx` and `browser-extension/popup.html`.
  - Replaced all user-facing "IDM" and legacy names across entire app with professional titles ("Multi-Threaded Turbo", "Parallel Connections", "Download Details Dialog", "Table View").
  - Refactored `IdmDownloadDialog.tsx` into clean, professional `DownloadStatusDialog.tsx`.
  - Retained all existing multi-threaded HTTP/HTTPS engines, Kick HLS segmented native resume, Instagram stream grabber, and BitTorrent settings.
- [x] **Mobile App Universal Browser Interception (Android)**:
  - Added comprehensive `android.intent.action.VIEW` and `android.intent.action.SEND` intent filters in `AndroidManifest.xml` for all mobile browsers (Chrome, Samsung Internet, Firefox, Edge, Brave, Opera).
  - Registered handlers for magnet schemes (`magnet:*`), torrent MIME types (`application/x-bittorrent`), and file extension pathPatterns (`.zip`, `.rar`, `.7z`, `.tar`, `.gz`, `.iso`, `.apk`, `.exe`, `.msi`, `.bin`, `.pdf`, `.mp4`, `.mkv`, `.webm`, `.avi`, `.mov`, `.mp3`, `.m4a`, `.flac`, `.wav`, `.m3u8`, `.ts`, `.torrent`).
  - Implemented comprehensive URI/URL extraction in `MainActivity.java` supporting direct VIEW clicks, browser share sheet SEND events, and custom protocols (`zr://`).
  - Added Android JavaScript bridge callbacks (`onAndroidSharedUrl`, `onAndroidFolderSelected`, `onAndroidProgress`, `onAndroidComplete`, `fetchMediaInfo`) for real-time progress and folder selection.
  - Added automated GitHub Actions CI workflow to build Android APK (`ZR-Downloader.apk`) on push and attach to releases.

### Phase 2: Speed, Network & Swarm Optimization
- [ ] Swarm defaults: 200 global / 100 per-torrent connections, 4-8 upload slots, optimistic unchoke, rarest-first picker.
- [ ] Protocol support: DHT (Mainline), Peer Exchange (PEX), Local Peer Discovery (LSD), UDP trackers, uTP.
- [ ] Encryption policies: Prefer encryption, Require encryption, Plaintext allowed.
- [ ] UPnP & NAT-PMP router automatic port forwarding.
- [ ] Sequential downloading & First/Last piece priority (stream while downloading videos).
- [ ] Auto-updating public tracker lists.
- [ ] Global & per-torrent rate limits (download and upload limits).
- [ ] Network interface binding (bind to VPN network interface e.g. tun0 / WireGuard / OpenVPN for privacy).
- [ ] SOCKS5 / HTTP proxy support.

### Phase 3: Organization, Categories & Management
- [ ] Torrent categories, tags, and sidebar filter tree (All, Downloading, Seeding, Completed, Paused, Active, Inactive, Error).
- [ ] Seeding limits: Ratio limit, Seeding time limit, Inactive timeout with configurable actions (pause, remove).
- [ ] Queue management: Max active downloads, max active uploads, ignore slow torrents.
- [ ] Built-in Torrent Creator (`.torrent` builder from file or directory, set piece size, announce URLs, private flag).
- [ ] Export `.torrent` files and copy magnet links.

### Phase 4: Automation & Power Features
- [ ] RSS Auto-downloader with regex filter rules.
- [ ] Built-in legal torrent search engine integration.
- [ ] Post-download automated tasks (run custom command, system sleep/shutdown when queue completes).
- [ ] Watched directory (auto-add `.torrent` files dropped into folder).
- [ ] Windows toast notifications and sound alerts on completion.

### Phase 5: Web UI & Remote Integration
- [ ] Embedded authenticated Web UI (localhost by default, CSRF protection, secure session tokens).
- [ ] REST API for remote control.
- [ ] Browser extension bridge integration: 1-click send magnet / .torrent from Chrome/Brave/Firefox to ZR Downloader.
- [ ] All-time transfer statistics and share ratio tracking.

### Phase 6: Mobile / Android Adaptation
- [ ] Adaptive responsive layout for Android & mobile screens.
- [ ] Background service execution with persistent notification.
- [ ] Battery & Wi-Fi only rules.

---

## Security & Privacy Checklist
- [x] Path sanitization: Strictly strip directory traversal (`../`, `..\\`), absolute paths, and drive letters from torrent file trees.
- [x] Executable warning: Prompts confirmation before opening `.exe`, `.msi`, `.bat`, `.cmd`, `.vbs`, `.ps1`.
- [x] Memory & message validation: Restrict piece buffers and handshake message sizes.
- [x] Zero telemetry: Absolute local-only operation.
