# ZR Downloader v1.2.8 - Release Notes

## 🚀 What's New in v1.2.8

### 1. 💽 Universal Low Disk Space Mode (Zero-Temp In-Place)
- **Available Everywhere**: The Low Disk Space switch is now prominently featured in both **Direct File (Multi-Threaded)** and **Social & Video (yt-dlp)** tabs when adding downloads.
- **Zero-Temp Streaming (`--no-part`)**: Media downloads stream directly into the target file without duplicate temporary `.part` files.
- **Zero Assembly Time**: Eliminates multi-minute disk assembly lags on large downloads (e.g. 20 GB file uses strictly 20 GB instead of 40 GB on SSD).
- **Full Speed Retained**: Download speed remains at maximum saturation with 16 parallel threads while cutting disk footprint in half.

### 2. 📱 Android App Startup Crash Fix
- **Fixed `AdaptiveIconDrawable` Crash**: Resolved the fatal `IllegalArgumentException: Only VectorDrawables and rasterized asset types are supported in Compose` by migrating `painterResource` to rasterized `ic_zr_logo.png`.
- **Rock-Solid Stability**: The Android APK now launches instantly and smoothly without stopping or crashing.

### 3. 🛡️ Google Play Protect Warning Elimination
- **Package Installer Heuristic Cleared**: Removed `.apk` and `application/vnd.android.package-archive` scheme handlers from `AndroidManifest.xml` that caused Google Play Protect to flag the app as an installer/harmful package.

### 4. 🎬 Dynamic YouTube & Social Media Inspection on Android
- **Embedded yt-dlp & FFmpeg Engine**: Bundled native `youtubedl-android` and `ffmpeg` runtimes directly inside the APK.
- **Real-Time Stream Inspection**: The "Inspect" button dynamically fetches available resolutions (1080p, 720p, 480p, 360p, audio m4a/mp3), codecs, and estimated file sizes.
- **Rich Media Card**: Displays real video thumbnail, title, channel name, and duration before starting the download.
- **16-Stream Multi-Threaded Engine**: Direct stream URLs are routed through the high-performance accelerator engine.
