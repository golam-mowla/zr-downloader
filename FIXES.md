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
