# ZR Downloader Assistant — Firefox Extension (`.xpi`)

Separate Firefox-compatible extension directory (`Manifest V2` with `browser_specific_settings.gecko` so `<all_urls>` and `http://127.0.0.1/*` bridge permissions are granted automatically on install).

## Files
- `zr-downloader-firefox.xpi` — Packaged Firefox `.xpi` bundle built from this folder.
- `manifest.json` — Firefox manifest (`zr-downloader@golammowla.dev`, `strict_min_version: 109.0`).

## How to Install in Firefox
1. **Standard Firefox (Temporary Add-on)**:
   - Open `about:debugging#/runtime/this-firefox`
   - Click **Load Temporary Add-on...**
   - Select `firefox-extension/manifest.json` (or `firefox-extension/zr-downloader-firefox.xpi`).
2. **Firefox Developer Edition / Nightly / Waterfox / Zen / LibreWolf (Permanent Unsigned `.xpi`)**:
   - Open `about:config` and set `xpinstall.signatures.required` to `false` (already supported in Developer/Nightly/forks).
   - Open `about:addons` -> gear icon -> **Install Add-on From File...** -> select `firefox-extension/zr-downloader-firefox.xpi`.
