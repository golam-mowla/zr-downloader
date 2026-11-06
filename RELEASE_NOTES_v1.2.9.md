# ZR Downloader v1.2.9 Release Notes

### 🚀 Highlights & Critical Fixes

#### 1. Browser Extension: "Download this video" Quality Click Fix
- **Instant Trigger**: Eliminated delay in format/quality click handlers by caching low-disk space settings in memory.
- **Fixed TDZ Reference Error**: Fixed JavaScript variable initialization in background service worker (`sendUrlToZr`) that previously failed on certain media requests.
- **Protocol Launch Fix**: Switched protocol invocation from iframes (which modern Chromium browsers block) to DOM element dispatch, ensuring the desktop app always triggers even if the background bridge server is restarting.
- **Dual-Action Support**: `background.js` now accepts both `SEND_TO_ZR` and `SEND_TO_GDM` actions to prevent any mismatch with cached browser sessions.

#### 2. Desktop Windows App: Instant Window Focus & Download Progress
- **Foreground Elevation**: Enhanced local bridge event handler in Rust with window unminimize and foreground elevation (`set_always_on_top` boost), ensuring ZR Downloader reliably pops up in front of the browser upon clicking any quality.
- **Immediate Task Creation**: The download task and IDM-style progress dialog now appear immediately upon click, without waiting for subprocess spawn latency.
- **Dual Protocol Support**: Registered both `zr://` and legacy `gdm://` protocol schemes in Windows Registry.

#### 3. Android Native App: Resolved HTTP 403 Forbidden on Media Downloads
- **Embedded yt-dlp & FFmpeg Merging**: Media URLs (YouTube, Instagram, Facebook, TikTok, Kick, etc.) are now routed through the bundled native `YoutubeDL` engine with automatic audio/video DASH stream merging.
- **Direct Link Sanitization**: `MediaGrabberDialog` passes the canonical page URL with resolution tag rather than ephemeral CDN stream tokens that expire or reject multi-threaded byte-range requests.
- **Segment Request Headers**: Enhanced OkHttp chunk engine with `Accept: */*` and `Accept-Encoding: identity`.
