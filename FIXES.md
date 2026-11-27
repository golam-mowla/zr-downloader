# Fixes in this build (resume / "restart from 0%" / low-disk mode)

## media.rs (yt-dlp / Kick / HLS path)
1. **Post-process failure no longer deletes the download.** When the merge / fix-up step fails,
   only the half-written ffmpeg `*.temp.*` output is removed. `.part`, `.ytdl`, `.part-Frag*` and
   `.f<id>.*` files are kept and the retry uses `--continue` + `--fixup warn` (no `--no-continue`,
   no `--force-overwrites`). Progress is no longer reset to 0%.
2. **Removed the stage 1 -> 2 escalation** (second delete-everything + full ffmpeg re-download).
   If the retry fails again, the files are kept (existing "completed with warning" path) or the
   task errors with data intact so Resume continues.
3. **HLS ffmpeg fallback only when nothing was downloaded yet.** Previously ANY failed HLS run
   (network blip, 403, ...) with no *finished* output file jumped to `--no-continue
   --force-overwrites` = restart at 0%, even at 99%.
4. **`postprocess_failed` requires a non-zero yt-dlp exit.** A harmless "Invalid data found"
   warning with exit code 0 used to trigger a restart.
5. **Stricter stem matching** (`name_matches_stem`): the next char after the stem must be a real
   boundary (`.`, end, or ` [id]`), so cleaning "Highlights" can't delete another task's
   "Highlights Day 2 [abc].mp4.part". `clean_partial_files` with an empty/short stem now deletes
   nothing (it used to wipe every partial file in the folder).
6. **Disk-space handling.** Detects "No space left on device" / low free space (Windows
   `GetDiskFreeSpaceExW`, Unix `df`), stops with a clear message, keeps all data, frees the ffmpeg
   temp file. Free space, press Resume.
7. **Removed `--skip-unavailable-fragments`** so missing fragments surface as an error / resume
   instead of silently producing a video with gaps.

## downloader.rs (direct HTTP, Low Disk Space mode)
8. Resume meta is written **atomically** (tmp file + rename).
9. Meta has a `clean` flag. After a crash / non-clean stop in low-disk mode, resume re-downloads the
   last 8 MB per segment, because progress counters run ahead of the 512 KB write buffer and
   could leave zero-filled gaps.

## Other
10. Android signing passwords can come from `ZR_KEYSTORE_PASSWORD` / `ZR_KEY_PASSWORD`
    (env or gradle property). The old default is kept as fallback so CI still builds.
    **Rotate the key and remove `release.keystore` from git history.**

## UI: Low Disk toggle greyed out where it can't work
11. In the Add Download dialog the Low Disk Space switch is now disabled (greyed, with an
    explanation) when the URL is HLS / m3u8 / DASH / Kick / Twitch / IVS, and the flag is never sent
    for them. Your saved preference is not changed. Backend (`is_hls_like_url` in media.rs) also never
    marks such tasks as low-disk. Preferences and the browser-extension popup now say it is for direct
    files only.

## Not changed (known limits)
* Low Disk Space mode still has no effect on HLS/Kick (yt-dlp always needs fragments + merge space).
* In low-disk mode the pre-allocated file still lives under the final name while downloading.
* Added unit tests: `cargo test resume_safety_tests` (in media.rs).

## Browser Extension → Desktop App Download Hand-off & Bridge Security (v3.7.0 / v1.2.44)

### A. Rust Backend (`src-tauri/src`)
1. **Settings Token Persistence (`settings.rs`)**:
   - `SettingsState::load_or_default()` now immediately writes the configuration to disk (atomic tmp + rename) whenever `settings.json` was missing, unparsable, or contained an empty `bridgePairToken`. This keeps the pairing token stable across app restarts.
2. **Single-Instance Forwarder Guard (`lib.rs`)**:
   - `run()` single-instance forwarders (for command-line arguments and `/focus`) now read the HTTP response with a 1.5s timeout and ONLY exit the process on HTTP 200. On HTTP 401 or network failure, the process does not exit, continuing normally to handle the link in-process.
3. **Bridge Security & Endpoints (`lib.rs`)**:
   - Path matching for `/ping` is now exact (`path == "/ping"` or `path.starts_with("/ping?")`). Non-ping endpoints cannot bypass token authentication.
   - Socket read loop reads until `\r\n\r\n` (capped at 64 KB, 3s timeout) to reliably parse complete headers and POST bodies.
   - Added CORS support for Tauri webview origins (`http://tauri.localhost`, `https://tauri.localhost`, `tauri://localhost`).
   - Added authenticated `GET /auth-check` endpoint returning `{"ok":true}`.
   - `parse_open_query` parses `cookies` and `userAgent` query params. Added POST `/open` JSON body parsing.
4. **Bridge Status Command (`lib.rs`)**:
   - Added `get_bridge_status` Tauri command returning `{port, listening: bool}`.
5. **Cookie & User-Agent Forwarding (`downloader.rs`, `media.rs`)**:
   - Direct HTTP downloads apply `Cookie` and `User-Agent` headers in worker requests.
   - yt-dlp media downloads forward cookies via `--add-header "Cookie:..."` and custom `--user-agent`.

### B. Frontend (`src`)
6. **Bridge Status Hook (`App.tsx`)**:
   - Replaced localhost fetch polling with `invoke("get_bridge_status")`, syncing `bridgeOnline` and `bridgePort` without CORS or webview fetch restrictions.
7. **Handoff Propagation (`App.tsx`, `AddDownloadModal.tsx`)**:
   - `handleOpenUrl` forwards `referrer`, `cookies`, and `userAgent` through both direct downloads and the Add Download modal.

### C. Browser Extension (`browser-extension`)
8. **Manifest Version**: Bumped to `3.7.0`.
9. **Startup Guard Removal (`background.js`)**: Removed arbitrary 4s startup timer while maintaining in-progress state and age checks.
10. **Non-Destructive Interception Order (`background.js`)**:
    - Browser download is NOT cancelled or erased until the desktop bridge responds successfully.
    - If desktop bridge is offline, attempts `zr://` deep link and retries bridge.
    - If handoff fails or is unauthorized, leaves browser download intact and displays a desktop notification explaining why.
11. **Cookie & Header Forwarding (`background.js`, `content.js`)**:
    - Collects cookies via `chrome.cookies.getAll` and `navigator.userAgent`.
    - Switches from GET to POST JSON when payload exceeds 7000 characters.
    - Best Quality grabber passes `referrer` and `isDirectFile`.
12. **Pairing Status & Badge (`popup.js`)**:
    - Verifies connectivity via `/ping` and authentication via `/auth-check`.
    - Distinct states: Offline, Online (Token Required / Token Invalid), Online (Paired ✓).
    - Sets extension badge (`KEY` with warning color) when token is invalid or unauthenticated.
13. **Expanded Interception Rules (`background.js`)**:
    - Intercepts documents, compressed archives (including 7z variants), packages (apk, aab, ipa), audio/video formats, and any file > 5 MB (excluding `text/html`).


## Universal Interception, Images Support & Android Release (v3.7.2 / v1.2.46)

### A. Universal Interception & Image Downloads (`browser-extension/`)
1. **Comprehensive File Type Coverage (`background.js`)**:
   - Expanded `INTERCEPT_EXTENSIONS` to include all image formats: `jpg`, `jpeg`, `png`, `webp`, `gif`, `bmp`, `svg`, `ico`, `tiff`, `tif`, `avif`, `heic`, `heif`, `psd`, `ai`, `raw`, `cr2`, `nef`.
   - Added missing archives (`lzma`, `cpio`, `arj`), packages (`xapk`, `apkm`), office documents, and 3D/AI models (`safetensors`, `ckpt`, `gguf`, `onnx`).
2. **MIME & Image Interception (`background.js`)**:
   - `handleDownloadInterception` detects `image/*` MIME types, document MIME types, and all image extensions.
   - Images and small direct downloads are no longer skipped.
3. **Context Menu Image Downloads (`background.js`, `content.js`)**:
   - When right-clicking on an image (`<img>` or `<picture>`), `info.srcUrl` is preserved and passed with `isDirectFile: true` and clean filename.
   - Bypasses yt-dlp to start high-speed accelerated HTTP downloads immediately in ZR Downloader.
4. **Resilient Video Grabber Overlay (`content.js`)**:
   - Fixed potential lockup where "fetching available qualities..." could stay on screen if player API parsing failed.
   - Wrapped format extraction in try/catch with immediate fallback to standard formats.
   - Added image element detection in `extractMediaTarget`.

### B. Desktop App Backend (`src-tauri/`)
5. **Image MIME Detection (`downloader.rs`)**:
   - Added mapping for all image MIME types in `extension_from_mime`.
   - Files without extensions in the URL now receive their proper image extension from the server's `Content-Type`.
6. **Images Categorization (`settings.rs`, `downloader.rs`)**:
   - Added `"Images"` category in `auto_category_subfolder` and `"images"` in `detect_category_with_mime`.
   - Automatically sorts downloaded images into the `Images` subfolder.

### C. Desktop UI (`src/`)
7. **Images Category & Sidebar (`Sidebar.tsx`, `App.tsx`)**:
   - Added `"images"` filter category with Lucide `Image` icon.
   - Added category counters for images.
8. **Settings Dialog Layout (`PreferencesModal.tsx`)**:
   - Optimized tab bar spacing and responsiveness so "Core Engines" and all 7 tabs fit on screen without being clipped.

### D. Android Native App (`android/`)
9. **Images Category Parity (`DownloadItem.kt`, `DownloadEngine.kt`)**:
   - Added `IMAGES("Images")` to `DownloadCategory`.
   - Added `extensionFromMime` in `DownloadEngine.kt` to infer extensions for images and documents.
10. **Version Bump**:
    - Bumped `versionCode = 19`, `versionName = "1.2.46"`.
