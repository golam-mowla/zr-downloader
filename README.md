# ZR Downloader

<p align="center">
  <img src="public/icon.png" width="120" height="120" alt="ZR Downloader Logo" style="border-radius: 24px; box-shadow: 0 0 30px rgba(212, 2, 3, 0.5);">
  <br><br>
  <strong>Ultra-Fast Multi-Threaded Download Accelerator & Media Extractor for Windows and Android</strong>
  <br>
  <em>Engineered with Rust, Tauri 2, React, and 100% Native Kotlin Jetpack Compose. Maximum bandwidth saturation with zero bloat.</em>
</p>

<p align="center">
  <a href="https://github.com/golam-mowla/zr-downloader/actions/workflows/main.yml"><img src="https://github.com/golam-mowla/zr-downloader/actions/workflows/main.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/golam-mowla/zr-downloader/releases"><img src="https://img.shields.io/github/v/release/golam-mowla/zr-downloader?color=22c55e" alt="Release"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-22c55e.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Android-blue.svg" alt="Platforms">
  <img src="https://img.shields.io/badge/Rust-Tauri_2-orange.svg" alt="Rust Tauri 2">
  <img src="https://img.shields.io/badge/Android-Kotlin_2.1%20%7C%20Compose-green.svg" alt="Android Kotlin Compose">
  <img src="https://img.shields.io/badge/Concurrency-Up_to_32_Streams-brightgreen.svg" alt="32 Streams">
</p>

---

## ⚡ Key Highlights & Architecture

ZR Downloader is a unified cross-platform download ecosystem built for performance, resilience, and privacy.

| Capability | Windows (Desktop) | Android (Mobile) |
| :--- | :--- | :--- |
| **Core Engine** | Rust (Tokio async IO, Rustls) | Kotlin Coroutines + OkHttp + WorkManager |
| **UI Framework** | React 18 + TypeScript + Tailwind CSS | Jetpack Compose + Material 3 Design |
| **Segmentation** | 1 to 32 concurrent HTTP Range connections | 1 to 32 concurrent Range threads |
| **Media Extraction** | Embedded yt-dlp & FFmpeg pipeline | Stream format selector (1080p, 720p, 480p, MP3) |
| **Taskbar Progress** | Real-time dynamic Windows taskbar icon progress | Dynamic Foreground Notification + Live Progress |
| **BitTorrent** | Embedded `librqbit` engine (DHT & PEX) | Magnet link interception & torrent queue |
| **Browser Integration** | Chrome / Brave / Edge extension (:9666 bridge) | Universal Android Intent Filters & Share Sheet |

---

## 🚀 Key Features

### ⚡ Up to 32-Segment HTTP Acceleration
Splits incoming network requests into multiple parallel chunk streams via HTTP `Range` headers. Files are written directly to pre-allocated disk space, maxing out your available bandwidth and delivering up to 500% faster speeds.

### 📊 Real-Time Taskbar Progress
Windows taskbar app icon dynamically reflects the current overall download progress:
- **Green Progress Bar**: Active downloading with live percentage.
- **Amber / Yellow Bar**: Transfers paused.
- **Red Bar**: Error state requiring attention.
- **Auto-Clear**: Seamlessly clears when downloads are finished.

### 🎬 Universal Media Grabber
Download videos, music, and audio streams from YouTube, TikTok, Instagram Reels, Facebook, Twitter/X, Vimeo, and thousands of streaming sites with resolution and audio-only extraction choices.

### 📱 100% Native Android App
Built strictly following the **2026 Recommended Android Architecture**:
- Kotlin 2.1.0 + Jetpack Compose
- Clean Architecture (Domain, Data, Presentation)
- Hilt 2.54 Dependency Injection
- Room SQLite Database + DataStore Preferences
- WorkManager foreground background service
- Universal browser download interception

### 🧩 Browser Extension ("ZR Assistant")
Seamless 1-click download interception for Chrome, Brave, and Edge. Automatically captures `.zip`, `.rar`, `.7z`, `.iso`, `.exe`, `.msi`, `.apk`, `.pdf`, `.mp4`, `.mkv` and hands them over to the desktop acceleration engine.

---

## 📦 Downloads (v1.2.6)

Grab the latest pre-compiled installers directly from [GitHub Releases](https://github.com/golam-mowla/zr-downloader/releases):

| Platform | Format | Description |
| :--- | :--- | :--- |
| **Windows 10/11 (x64)** | `.exe` (Setup) | 1-Click NSIS desktop installer with shortcuts |
| **Windows Enterprise** | `.msi` | Enterprise Windows Installer package |
| **Android Phone / Tablet** | `.apk` (Release) | Signed production native Android APK |
| **Google Play** | `.aab` | Optimized Android App Bundle |
| **Browser Extension** | Directory | Manifest V3 Assistant for Chrome / Brave / Edge |

---

## 🛠️ Building From Source

### Prerequisites
- [Node.js](https://nodejs.org/) (v20+ recommended)
- [Rust](https://rustup.rs/) (stable toolchain)
- [JDK 21](https://adoptium.net/) & Android SDK (for Android builds)

### Desktop (Windows)
```powershell
# 1. Clone repository
git clone https://github.com/golam-mowla/zr-downloader.git
cd zr-downloader

# 2. Install frontend dependencies
npm install

# 3. Build frontend and verify TypeScript
npm run build

# 4. Run development desktop app
npm run tauri dev

# 5. Build production Windows installers
npm run tauri build
```

### Mobile (Android)
```powershell
cd android

# Compile Debug APK
./gradlew assembleDebug

# Compile Release APK & AAB
./gradlew assembleRelease bundleRelease
```

---

## 🛡️ Privacy & Security

- **Zero Telemetry**: No user tracking, no phone-home analytics, and no remote advertising.
- **Clean-Room Engineering**: 100% MIT licensed code with zero GPL dependencies.
- **Local Isolation**: The desktop bridge only listens on `127.0.0.1` and verifies local origins.

---

## 👤 Author & Maintainer

**ZR Downloader** is designed and maintained by **[Golam Mowla](https://github.com/golam-mowla)**.

- **GitHub Profile**: [@golam-mowla](https://github.com/golam-mowla)
- **Repository**: [https://github.com/golam-mowla/zr-downloader](https://github.com/golam-mowla/zr-downloader)

---

## 📄 License

Released under the [MIT License](LICENSE). Copyright (c) 2026 Golam Mowla.
