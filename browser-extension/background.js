// ZR Downloader - Background Service Worker (Manifest V3)
// IDM-Style High-Speed Interception & Passive Media Stream Sniffer

const DEFAULT_ZR_BRIDGE_PORT = 9666;

async function autoPairBridge(config) {
  try {
    const controller = new AbortController();
    const tId = setTimeout(() => controller.abort(), 1500);
    const res = await fetch(`${config.url}/pair`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ client: "ZR-Extension" }),
      signal: controller.signal,
    });
    clearTimeout(tId);
    if (res.ok) {
      const data = await res.json();
      if (data && data.token) {
        await chrome.storage.local.set({
          bridgeToken: data.token,
          bridgePort: data.port || config.port,
        });
        config.token = data.token;
        return data.token;
      }
    }
  } catch (e) {}
  return config.token || "";
}

async function getBridgeConfig() {
  const cfg = await new Promise((resolve) => {
    chrome.storage.local.get(["bridgePort", "bridgeToken"], (res) => {
      const port = parseInt(res.bridgePort, 10) || DEFAULT_ZR_BRIDGE_PORT;
      const token = res.bridgeToken ? String(res.bridgeToken).trim() : "";
      resolve({
        port,
        token,
        url: `http://127.0.0.1:${port}`,
      });
    });
  });
  if (!cfg.token) {
    await autoPairBridge(cfg);
  }
  return cfg;
}

// Interceptable file extensions for IDM-style capture
const INTERCEPT_EXTENSIONS = new Set([
  // Torrent files
  "torrent",
  // Archives & 7zip variants
  "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "tgz",
  "cab", "lzh", "zst", "tbz2", "txz",
  // Executables, Installers, Packages & Mobile
  "exe", "msi", "dmg", "pkg", "deb", "rpm", "apk", "aab", "ipa", "appx", "msix", "jar",
  // Media files & variants
  "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "mp3", "flac", "wav", "m4a", "aac", "ogg",
  "ts", "m4v", "3gp", "mpg", "mpeg", "opus", "wma",
  // Documents & Data
  "pdf", "epub", "mobi", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "csv", "txt",
  // Fonts
  "ttf", "otf", "woff2",
  // Disk / Data
  "img", "vhd", "vmdk", "bin"
]);

// Keep track of downloads we are actively intercepting to avoid loops
const interceptedDownloadIds = new Set();

// Helper to get cookies for a URL as "name=value; ..." header string
async function getCookiesHeader(url) {
  try {
    if (!chrome.cookies || !chrome.cookies.getAll) return "";
    const cookies = await chrome.cookies.getAll({ url });
    if (!cookies || cookies.length === 0) return "";
    return cookies.map((c) => `${c.name}=${c.value}`).join("; ");
  } catch (e) {
    return "";
  }
}

// Sends an open request to the local bridge (timeout 3.5s)
// Returns { ok: true, status: 200 } on HTTP 200 with token,
// { ok: false, reason: "unauthorized" } on 401 or missing token,
// or { ok: false, reason: "offline" | "error" } otherwise.
async function sendBridgeOpen(payload) {
  let config = await getBridgeConfig();
  if (!config.token) {
    config.token = await autoPairBridge(config);
  }
  if (!config.token) {
    return { ok: false, reason: "unauthorized", message: "Token missing" };
  }

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 3500);

  try {
    const filenameParam = payload.filename ? `&filename=${encodeURIComponent(payload.filename)}` : "";
    const qualityParam = payload.quality ? `&quality=${encodeURIComponent(payload.quality)}` : "";
    const autoParam = payload.autoStart ? `&autostart=true` : "";
    const torrentParam = payload.isTorrent ? `&isTorrent=true` : "";
    const referrerParam = payload.referrer ? `&referrer=${encodeURIComponent(payload.referrer)}` : "";
    const containerParam = payload.container ? `&container=${encodeURIComponent(payload.container)}` : "";
    const directParam = payload.isDirectFile ? `&isDirectFile=true` : "";
    const lowSpaceParam = payload.lowDiskSpace ? `&lowDiskSpace=true` : "";
    const formatParam = payload.formatId ? `&formatid=${encodeURIComponent(payload.formatId)}` : "";
    const tokenParam = `&token=${encodeURIComponent(config.token)}`;
    const cookiesParam = payload.cookies ? `&cookies=${encodeURIComponent(payload.cookies)}` : "";
    const uaParam = payload.userAgent ? `&userAgent=${encodeURIComponent(payload.userAgent)}` : "";

    const fullGetUrl = `${config.url}/open?url=${encodeURIComponent(payload.url)}${filenameParam}${qualityParam}${autoParam}${torrentParam}${referrerParam}${containerParam}${directParam}${lowSpaceParam}${formatParam}${tokenParam}${cookiesParam}${uaParam}`;

    const headers = {
      "X-ZR-Token": config.token,
    };

    let resp;
    if (fullGetUrl.length > 7000) {
      resp = await fetch(`${config.url}/open`, {
        method: "POST",
        headers: {
          ...headers,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          url: payload.url,
          filename: payload.filename,
          quality: payload.quality,
          autoStart: Boolean(payload.autoStart),
          isTorrent: Boolean(payload.isTorrent),
          referrer: payload.referrer,
          container: payload.container,
          formatId: payload.formatId,
          isDirectFile: Boolean(payload.isDirectFile),
          lowDiskSpace: Boolean(payload.lowDiskSpace),
          cookies: payload.cookies,
          userAgent: payload.userAgent,
          token: config.token,
        }),
        signal: controller.signal,
      });
    } else {
      resp = await fetch(fullGetUrl, {
        method: "GET",
        headers,
        signal: controller.signal,
      });
    }

    clearTimeout(timeoutId);

    if (resp.status === 200) {
      return { ok: true, status: 200 };
    } else if (resp.status === 401) {
      const freshToken = await autoPairBridge(config);
      if (freshToken && freshToken !== config.token) {
        config.token = freshToken;
        headers["X-ZR-Token"] = freshToken;
        const retryUrl = `${config.url}/open?url=${encodeURIComponent(payload.url)}${filenameParam}${qualityParam}${autoParam}${torrentParam}${referrerParam}${containerParam}${directParam}${lowSpaceParam}${formatParam}&token=${encodeURIComponent(freshToken)}${cookiesParam}${uaParam}`;
        const retryResp = await fetch(retryUrl, { method: "GET", headers });
        if (retryResp.status === 200) {
          return { ok: true, status: 200 };
        }
      }
      return { ok: false, reason: "unauthorized", status: 401 };
    } else {
      return { ok: false, reason: "error", status: resp.status };
    }
  } catch (err) {
    clearTimeout(timeoutId);
    return { ok: false, reason: "offline", error: err?.message };
  }
}

// Passive media stream sniffer cache: tabId -> Map(streamUrl -> streamInfo)
const tabMediaStreams = new Map();

// Helper to format human byte size
function formatBytes(bytes) {
  if (!bytes || bytes <= 0) return "";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
  return `${mb.toFixed(1)} MB`;
}

// ---------------------------------------------------------------------------
// 1. Passive Media Stream Sniffing (IDM method: listens to HTTP response headers)
// ---------------------------------------------------------------------------
function normalizeMediaUrl(rawUrl) {
  if (!rawUrl || typeof rawUrl !== "string") return rawUrl;
  try {
    const parsed = new URL(rawUrl);
    let changed = false;
    if (parsed.searchParams.has("bytestart") || parsed.searchParams.has("byteend")) {
      parsed.searchParams.delete("bytestart");
      parsed.searchParams.delete("byteend");
      changed = true;
    }
    if (parsed.searchParams.has("range")) {
      parsed.searchParams.delete("range");
      changed = true;
    }
    if (parsed.hostname.includes("stream.kick.com") && parsed.pathname.includes("/ivs/v1/")) {
      if (parsed.pathname.endsWith("/")) {
        parsed.pathname += "master.m3u8";
        changed = true;
      }
    }
    return changed ? parsed.toString() : rawUrl;
  } catch (e) {
    return rawUrl;
  }
}

if (chrome.webRequest && chrome.webRequest.onHeadersReceived) {
  chrome.webRequest.onHeadersReceived.addListener(
    (details) => {
      try {
        if (!details.url || !details.responseHeaders) return;
        const tabId = details.tabId;
        if (tabId < 0) return;

        let contentType = "";
        let contentLength = 0;
        let totalLength = 0;

        for (const h of details.responseHeaders) {
          const name = h.name.toLowerCase();
          if (name === "content-type") contentType = h.value.toLowerCase();
          if (name === "content-length") contentLength = parseInt(h.value, 10) || 0;
          if (name === "content-range") {
            const m = h.value.match(/\/(\d+)/);
            if (m) {
              totalLength = parseInt(m[1], 10) || 0;
            }
          }
        }
        if (totalLength === 0) totalLength = contentLength;

        const cleanUrl = normalizeMediaUrl(details.url);
        const urlLower = cleanUrl.toLowerCase();

        const isMediaMime =
          contentType.startsWith("video/") ||
          contentType.startsWith("audio/") ||
          contentType.includes("application/x-mpegurl") ||
          contentType.includes("application/vnd.apple.mpegurl") ||
          contentType.includes("application/dash+xml");

        const isMediaExt =
          urlLower.includes(".mp4") ||
          urlLower.includes(".mkv") ||
          urlLower.includes(".webm") ||
          urlLower.includes(".m3u8") ||
          urlLower.includes(".m4s") ||
          urlLower.includes("videoplayback");

        if (isMediaMime || isMediaExt) {
          if (!tabMediaStreams.has(tabId)) {
            tabMediaStreams.set(tabId, new Map());
          }
          const streams = tabMediaStreams.get(tabId);

          const isAudio =
            contentType.startsWith("audio/") ||
            urlLower.includes("mime=audio") ||
            urlLower.includes("&audio=1") ||
            urlLower.includes("itag=140");

          let quality = "";
          let bitrateKbps = 0;
          let durationS = 0;

          // Decode Instagram/Facebook efg metadata parameter if present
          try {
            const parsed = new URL(cleanUrl);
            const efg = parsed.searchParams.get("efg");
            if (efg) {
              const meta = JSON.parse(atob(efg));
              if (meta.bitrate) {
                bitrateKbps = Math.round(meta.bitrate / 1000);
              }
              if (meta.duration_s) {
                durationS = meta.duration_s;
              }
              if (meta.vencode_tag) {
                const tag = meta.vencode_tag.toLowerCase();
                const m = tag.match(/(\d+)p/);
                if (m) {
                  const h = parseInt(m[1], 10);
                  quality = h >= 720 ? `${h}p HD` : `${h}p`;
                } else if (tag.includes("1080") || tag.includes("1920")) quality = "1080p HD";
                else if (tag.includes("720") || tag.includes("1280")) quality = "720p HD";
                else if (tag.includes("480")) quality = "480p";
                else if (tag.includes("360")) quality = "360p";
              }
            }
          } catch (e) {}

          // Fallback quality based on bitrate
          if (!quality && bitrateKbps > 0) {
            if (bitrateKbps >= 1500) quality = "1080p HD";
            else if (bitrateKbps >= 600) quality = "720p HD";
            else if (bitrateKbps >= 350) quality = "480p";
            else quality = "360p";
          }

          // YouTube videoplayback itags or url params
          if (!quality) {
            if (urlLower.includes("itag=137") || urlLower.includes("itag=399")) quality = "1080p HD";
            else if (urlLower.includes("itag=22") || urlLower.includes("itag=136") || urlLower.includes("itag=298")) quality = "720p HD";
            else if (urlLower.includes("itag=135") || urlLower.includes("itag=244")) quality = "480p";
            else if (urlLower.includes("itag=18") || urlLower.includes("itag=134")) quality = "360p";
            else if (isAudio) quality = "Audio Only";
            else if (urlLower.includes("1080")) quality = "1080p HD";
            else if (urlLower.includes("720")) quality = "720p HD";
            else if (urlLower.includes("480")) quality = "480p";
          }

          let effectiveLength = totalLength;
          if ((!effectiveLength || effectiveLength < 100000) && bitrateKbps > 0 && durationS > 0) {
            effectiveLength = Math.round((bitrateKbps * 1000 * durationS) / 8);
          }

          if (streams.has(cleanUrl)) {
            const existing = streams.get(cleanUrl);
            if (effectiveLength > existing.contentLength) {
              existing.contentLength = effectiveLength;
              existing.sizeFormatted = formatBytes(effectiveLength);
            }
            if (!existing.quality && quality) existing.quality = quality;
            if (!existing.bitrate && bitrateKbps) existing.bitrate = bitrateKbps;
          } else {
            // Skip tiny sub-100KB fragments if not a playlist or audio
            if (effectiveLength > 0 && effectiveLength < 100000 && !urlLower.includes(".m3u8") && !isAudio && details.url !== cleanUrl) {
              return;
            }

            const streamInfo = {
              url: cleanUrl,
              contentType,
              contentLength: effectiveLength,
              sizeFormatted: formatBytes(effectiveLength),
              quality: quality || (isAudio ? "Audio Only" : "HD Video"),
              bitrate: bitrateKbps,
              duration: durationS,
              isAudio,
              timestamp: Date.now(),
            };
            streams.set(cleanUrl, streamInfo);

            // Notify content script on the tab
            chrome.tabs.sendMessage(tabId, {
              action: "MEDIA_SNIFFED",
              stream: streamInfo,
            }).catch(() => {});
          }
        }
      } catch (e) {}
    },
    { urls: ["<all_urls>"] },
    ["responseHeaders"]
  );
}

// Clean up tab streams when tab is closed
chrome.tabs.onRemoved.addListener((tabId) => {
  tabMediaStreams.delete(tabId);
});

// Clean up on tab navigation / reload
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.status === "loading") {
    tabMediaStreams.delete(tabId);
  }
});

// Setup Context Menu reliably across install, startup, and background wake-up
function setupContextMenu() {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create(
      {
        id: "download-with-zr",
        title: "Download with ZR Downloader 🚀",
        contexts: ["all", "page", "video", "audio", "link", "selection", "image"],
      },
      () => {
        if (chrome.runtime.lastError) {
          // Ignore duplicate creation
        }
      }
    );
  });
}
chrome.runtime.onInstalled.addListener(setupContextMenu);
chrome.runtime.onStartup.addListener(setupContextMenu);
setupContextMenu();

// Intercept standard HTTP/HTTPS file downloads (IDM style)
async function handleDownloadInterception(downloadItem, suggest) {
  try {
    if (!downloadItem) return false;

    // 1. Ignore non-active downloads (completed, interrupted, paused from previous sessions)
    if (downloadItem.state && downloadItem.state !== "in_progress") {
      return false;
    }

    // 2. Ignore stale downloads whose start time was before this session or > 12 seconds ago
    if (downloadItem.startTime) {
      const ageMs = Date.now() - new Date(downloadItem.startTime).getTime();
      if (ageMs > 12000) {
        return false;
      }
    }

    const rawUrl = downloadItem.finalUrl || downloadItem.url;
    if (!rawUrl || !rawUrl.startsWith("http")) return false;

    // Check if filename or url matches intercept extensions
    let ext = "";
    if (downloadItem.filename) {
      const match = downloadItem.filename.match(/\.([a-zA-Z0-9]+)$/);
      if (match) ext = match[1].toLowerCase();
    }
    if (!ext) {
      try {
        const pathname = new URL(rawUrl).pathname;
        const match = pathname.match(/\.([a-zA-Z0-9]+)$/);
        if (match) ext = match[1].toLowerCase();
      } catch (e) {}
    }

    const mime = (downloadItem.mime || "").toLowerCase();
    const isTorrent =
      ext === "torrent" ||
      mime.includes("bittorrent") ||
      rawUrl.toLowerCase().includes(".torrent");

    const totalBytes = Math.max(
      downloadItem.fileSize || 0,
      downloadItem.totalBytes || 0
    );
    const isBigFile = totalBytes > 5 * 1024 * 1024;
    const isMediaMime =
      mime.startsWith("video/") ||
      mime.startsWith("audio/") ||
      mime.includes("matroska") ||
      mime.includes("mp4") ||
      mime.includes("webm") ||
      mime.includes("avi");
    const isBinaryMime =
      mime === "application/octet-stream" ||
      mime === "application/x-zip-compressed" ||
      mime === "application/x-rar-compressed" ||
      mime === "application/x-7z-compressed" ||
      mime === "application/zip";

    const isInterceptable =
      INTERCEPT_EXTENSIONS.has(ext) ||
      isTorrent ||
      isBigFile ||
      isMediaMime ||
      (isBinaryMime && ext && ext !== "html" && ext !== "htm");

    if (isInterceptable) {
      const storage = await chrome.storage.local.get(["interceptDownloads", "showNotifications"]);
      if (storage.interceptDownloads === false) {
        return false; // User disabled interception in popup/settings
      }

      if (interceptedDownloadIds.has(downloadItem.id)) return true;
      interceptedDownloadIds.add(downloadItem.id);

      const cleanFilename = downloadItem.filename ? downloadItem.filename.replace(/^.*[\\\/]/, "") : "";
      const referrer = downloadItem.referrer || "";
      const cookies = await getCookiesHeader(rawUrl);
      const userAgent = navigator.userAgent || "";

      const payload = {
        url: rawUrl,
        filename: cleanFilename,
        quality: null,
        autoStart: true,
        isTorrent,
        referrer,
        container: null,
        isDirectFile: !isTorrent,
        lowDiskSpace: false,
        formatId: "",
        cookies,
        userAgent,
      };

      let bridgeRes = await sendBridgeOpen(payload);

      // If app is not running (offline), optionally try zr:// launch once, then retry the bridge up to 3 times over ~4s
      if (!bridgeRes.ok && bridgeRes.reason === "offline") {
        const config = await getBridgeConfig();
        const tokenProtoParam = config.token ? `&token=${encodeURIComponent(config.token)}` : "";
        const fnParam = cleanFilename ? `&filename=${encodeURIComponent(cleanFilename)}` : "";
        const refParam = referrer ? `&referrer=${encodeURIComponent(referrer)}` : "";
        const protoUrl = `zr://open?url=${encodeURIComponent(rawUrl)}${fnParam}&autostart=true${isTorrent ? "&isTorrent=true" : "&isDirectFile=true"}${refParam}${tokenProtoParam}`;
        launchProtocolUrl(protoUrl);

        for (let i = 0; i < 3; i++) {
          await new Promise((r) => setTimeout(r, 1200));
          bridgeRes = await sendBridgeOpen(payload);
          if (bridgeRes.ok) break;
        }
      }

      if (bridgeRes.ok) {
        // Success! Cancel browser download only now
        try {
          chrome.downloads.cancel(downloadItem.id, () => {
            chrome.downloads.erase({ id: downloadItem.id }, () => {});
          });
        } catch (e) {}

        if (storage.showNotifications === true) {
          chrome.notifications.create({
            type: "basic",
            iconUrl: "icons/icon128.png",
            title: "ZR Downloader 🚀",
            message: isTorrent
              ? `Intercepted BitTorrent download:\n${cleanFilename || "Torrent file"}`
              : `Intercepted high-speed download:\n${cleanFilename || (ext ? ext.toUpperCase() + " file" : "File")}`,
            priority: 1,
          });
        }
        return true;
      } else {
        // Bridge failed (offline, 401, timeout) -> do NOT cancel native download! Let browser download proceed.
        interceptedDownloadIds.delete(downloadItem.id);
        if (suggest) {
          suggest();
        }
        const failMessage =
          bridgeRes.reason === "unauthorized"
            ? "Token missing or mismatch: open ZR Downloader → Preferences → copy pair token into the extension popup."
            : "ZR Downloader is not running. Download will proceed in browser.";
        chrome.notifications.create({
          type: "basic",
          iconUrl: "icons/icon128.png",
          title: "ZR Downloader - Capture Skipped",
          message: failMessage,
          priority: 2,
        });
        return false;
      }
    }
  } catch (err) {
    console.error("Error intercepting download:", err);
  }
  return false;
}

// Attach to onDeterminingFilename to suppress Brave's native file-picker prompt
if (chrome.downloads && chrome.downloads.onDeterminingFilename) {
  chrome.downloads.onDeterminingFilename.addListener((downloadItem, suggest) => {
    handleDownloadInterception(downloadItem, suggest).then((intercepted) => {
      if (!intercepted && suggest) {
        suggest();
      }
    });
    return true; // Keep callback channel open for async determination
  });
}

// Fallback listener for downloads created directly
chrome.downloads.onCreated.addListener((downloadItem) => {
  handleDownloadInterception(downloadItem);
});

// Handle Context Menu clicks
chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === "download-with-zr") {
    let candidateUrl = "";

    // 1. Ask content script for detected element URL
    if (tab && tab.id) {
      try {
        const resp = await chrome.tabs.sendMessage(tab.id, {
          action: "GET_LAST_CLICKED_MEDIA",
        });
        if (resp && resp.url) {
          candidateUrl = resp.url;
        }
      } catch (e) {}
    }

    // 2. Fallbacks from contextMenus info
    const isClean = (u) =>
      u &&
      typeof u === "string" &&
      !u.startsWith("blob:") &&
      !u.startsWith("data:") &&
      (u.startsWith("http://") || u.startsWith("https://"));

    if (!isClean(candidateUrl)) {
      if (isClean(info.linkUrl)) {
        candidateUrl = info.linkUrl;
      } else if (isClean(info.srcUrl)) {
        candidateUrl = info.srcUrl;
      } else if (info.selectionText && isClean(info.selectionText.trim())) {
        candidateUrl = info.selectionText.trim();
      } else if (isClean(info.pageUrl)) {
        candidateUrl = info.pageUrl;
      } else if (tab && isClean(tab.url)) {
        candidateUrl = tab.url;
      }
    }

    if (!isClean(candidateUrl)) {
      if (tab && isClean(tab.url)) {
        candidateUrl = tab.url;
      } else {
        return;
      }
    }

    await sendUrlToZr(candidateUrl, tab ? tab.id : null);
  }
});

// Send URL to ZR Downloader Desktop App
const formatsCache = new Map(); // canonicalUrl -> { t, promise }
function canonicalizeFormatUrl(rawUrl) {
  try {
    const u = new URL(rawUrl);
    const host = u.hostname.toLowerCase();
    if (host.includes("youtube.com") && u.pathname === "/watch" && u.searchParams.get("v")) {
      return `https://www.youtube.com/watch?v=${u.searchParams.get("v")}`;
    }
    if (host === "youtu.be") {
      const vid = u.pathname.replace(/^\/+/, "").split("/")[0];
      if (vid) return `https://www.youtube.com/watch?v=${vid}`;
    }
  } catch {}
  return rawUrl;
}
function getRealFormats(url) {
  if (!url) return Promise.resolve(null);
  const key = canonicalizeFormatUrl(url);
  const hit = formatsCache.get(key);
  if (hit && Date.now() - hit.t < 10 * 60 * 1000) return hit.promise;
  const promise = (async () => {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 15000);
    try {
      let config = await getBridgeConfig();
      if (!config.token) {
        config.token = await autoPairBridge(config);
      }
      const headers = {};
      if (config.token) {
        headers["X-ZR-Token"] = config.token;
      }
      const tokenParam = config.token ? `&token=${encodeURIComponent(config.token)}` : "";
      let res = await fetch(`${config.url}/formats?url=${encodeURIComponent(key)}${tokenParam}`, {
        headers,
        signal: controller.signal,
      });
      if (res.status === 401) {
        const freshToken = await autoPairBridge(config);
        if (freshToken) {
          headers["X-ZR-Token"] = freshToken;
          res = await fetch(`${config.url}/formats?url=${encodeURIComponent(key)}&token=${encodeURIComponent(freshToken)}`, {
            headers,
            signal: controller.signal,
          });
        }
      }
      if (!res.ok) throw new Error("formats " + res.status);
      return await res.json();
    } catch (e) {
      formatsCache.delete(key);
      return null;
    } finally {
      clearTimeout(timer);
    }
  })();
  formatsCache.set(key, { t: Date.now(), promise });
  return promise;
}

function launchProtocolUrl(protocolUrl) {
  try {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      if (tabs && tabs[0] && tabs[0].id) {
        chrome.scripting.executeScript({
          target: { tabId: tabs[0].id },
          func: (proto) => {
            const a = document.createElement("a");
            a.href = proto;
            a.style.display = "none";
            document.body.appendChild(a);
            a.click();
            setTimeout(() => a.remove(), 1500);
          },
          args: [protocolUrl],
        }).catch(() => {
          chrome.tabs.create({ url: protocolUrl, active: false }, (t) => {
            setTimeout(() => chrome.tabs.remove(t.id), 2500);
          });
        });
      } else {
        chrome.tabs.create({ url: protocolUrl, active: false }, (t) => {
          setTimeout(() => chrome.tabs.remove(t.id), 2500);
        });
      }
    });
  } catch (e) {}
}

async function sendUrlToZr(mediaUrl, tabId, customFilename, quality, autoStart = false, isTorrent = false, referrer = "", container = "", isDirectFile = false, lowDiskSpace = false, formatId = "") {
  let cleanUrl = normalizeMediaUrl((mediaUrl || "").trim());
  if (!cleanUrl) return { success: false, reason: "error", message: "Missing URL" };

  // Resolve active tab if not passed
  let effectiveTabId = tabId;
  if (!effectiveTabId) {
    try {
      const [activeTab] = await chrome.tabs.query({ active: true, currentWindow: true });
      if (activeTab && activeTab.id) {
        effectiveTabId = activeTab.id;
      }
    } catch {}
  }

  // Resolve Instagram reel/post page URL if direct video-only CDN URL was intercepted
  if (
    cleanUrl.includes("cdninstagram.com") ||
    cleanUrl.includes("fbcdn.net") ||
    (cleanUrl.includes("instagram.com") && cleanUrl.includes(".mp4"))
  ) {
    if (referrer && (referrer.includes("/reel/") || referrer.includes("/p/"))) {
      cleanUrl = referrer;
    } else if (effectiveTabId) {
      try {
        const tabInfo = await chrome.tabs.get(effectiveTabId);
        if (tabInfo && tabInfo.url && (tabInfo.url.includes("/reel/") || tabInfo.url.includes("/p/"))) {
          cleanUrl = tabInfo.url;
        }
      } catch {}
    }
  }

  // Resolve Kick channel page URL or referrer if raw stream.kick.com URL was intercepted
  if (cleanUrl.includes("stream.kick.com") && effectiveTabId) {
    try {
      const tabInfo = await chrome.tabs.get(effectiveTabId);
      if (tabInfo && tabInfo.url && tabInfo.url.includes("kick.com/") && !tabInfo.url.includes("stream.kick.com")) {
        if (!referrer) referrer = tabInfo.url;
      }
    } catch {}
  }

  // Notify tab that request is in progress
  if (effectiveTabId) {
    try {
      await chrome.tabs.sendMessage(effectiveTabId, {
        action: "SHOW_TOAST",
        title: "ZR Downloader 🚀",
        message: isTorrent
          ? "Adding torrent to ZR Downloader engine..."
          : quality
          ? `Sending ${quality} stream to ZR Downloader Engine...`
          : "Sending link to ZR Downloader engine...",
        status: "info",
      });
    } catch {}
  }

  const cookies = await getCookiesHeader(cleanUrl);
  const userAgent = navigator.userAgent || "";

  const payload = {
    url: cleanUrl,
    filename: customFilename,
    quality,
    autoStart: Boolean(autoStart),
    isTorrent: Boolean(isTorrent),
    referrer: referrer || "",
    container: container || "",
    isDirectFile: Boolean(isDirectFile),
    lowDiskSpace: Boolean(lowDiskSpace),
    formatId: formatId || "",
    cookies,
    userAgent,
  };

  const bridgeRes = await sendBridgeOpen(payload);

  if (bridgeRes.ok) {
    if (effectiveTabId) {
      try {
        await chrome.tabs.sendMessage(effectiveTabId, {
          action: "SHOW_TOAST",
          title: "ZR Downloader 🚀",
          message: isTorrent
            ? "Torrent added in ZR Downloader!"
            : quality
            ? `${quality} Download started in ZR Downloader!`
            : "Link sent to ZR Downloader! Download starting...",
          status: "success",
        });
      } catch {}
    }
    return { success: true, method: "bridge" };
  }

  if (bridgeRes.reason === "unauthorized") {
    return { success: false, reason: "unauthorized" };
  }

  // Local bridge is offline -> launch via protocol fallback without overwriting clipboard
  const config = await getBridgeConfig();
  const filenameParam = customFilename ? `&filename=${encodeURIComponent(customFilename)}` : "";
  const qualityParam = quality ? `&quality=${encodeURIComponent(quality)}` : "";
  const autoParam = autoStart ? `&autostart=true` : "";
  const torrentParam = isTorrent ? `&isTorrent=true` : "";
  const directParam = isDirectFile ? `&isDirectFile=true` : "";
  const lowSpaceParam = lowDiskSpace ? `&lowDiskSpace=true` : "";
  const referrerProtoParam = referrer ? `&referrer=${encodeURIComponent(referrer)}` : "";
  const containerProtoParam = container ? `&container=${encodeURIComponent(container)}` : "";
  const formatProtoParam = formatId ? `&formatid=${encodeURIComponent(formatId)}` : "";
  const tokenProtoParam = config.token ? `&token=${encodeURIComponent(config.token)}` : "";
  const protocolUrl = `zr://open?url=${encodeURIComponent(cleanUrl)}${filenameParam}${qualityParam}${autoParam}${torrentParam}${directParam}${lowSpaceParam}${referrerProtoParam}${containerProtoParam}${formatProtoParam}${tokenProtoParam}`;

  launchProtocolUrl(protocolUrl);

  return { success: false, reason: "offline" };
}

// Listen to messages from popup or content script
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "SEND_TO_ZR" || request.action === "SEND_TO_GDM") {
    const tabId = sender && sender.tab ? sender.tab.id : null;
    sendUrlToZr(
      request.url,
      tabId,
      request.filename,
      request.quality,
      Boolean(request.autoStart),
      Boolean(request.isTorrent),
      request.referrer || "",
      request.container || "",
      Boolean(request.isDirectFile),
      Boolean(request.lowDiskSpace),
      request.formatId || ""
    ).then((res) => sendResponse(res));
    return true;
  }

  if (request.action === "GET_FORMATS") {
    getRealFormats(request.url)
      .then((info) => sendResponse({ ok: Boolean(info), info }))
      .catch(() => sendResponse({ ok: false }));
    return true;
  }

  if (request.action === "GET_SNIFFED_MEDIA") {
    const tabId = sender && sender.tab ? sender.tab.id : null;
    const streams = tabId && tabMediaStreams.has(tabId)
      ? Array.from(tabMediaStreams.get(tabId).values())
      : [];
    sendResponse({ streams });
    return true;
  }

  if (request.action === "CHECK_ZR_STATUS" || request.action === "CHECK_GDM_STATUS") {
    (async () => {
      let isOnline = false;
      let isAuthenticated = false;
      let pingData = null;
      let config = { port: DEFAULT_ZR_BRIDGE_PORT, token: "", url: `http://127.0.0.1:${DEFAULT_ZR_BRIDGE_PORT}` };

      try {
        config = await getBridgeConfig();
      } catch (e) {}

      try {
        const controller = new AbortController();
        const tId = setTimeout(() => controller.abort(), 1500);
        const pingRes = await fetch(`${config.url}/ping`, { method: "GET", signal: controller.signal });
        clearTimeout(tId);
        if (pingRes.ok) {
          pingData = await pingRes.json();
          isOnline = true;
          if (pingData?.port && pingData.port !== config.port) {
            chrome.storage.local.set({ bridgePort: pingData.port });
            config.port = pingData.port;
            config.url = `http://127.0.0.1:${pingData.port}`;
          }
        }
      } catch {}

      if (isOnline) {
        if (!config.token) {
          config.token = await autoPairBridge(config);
        }
        if (config.token) {
          try {
            const controller = new AbortController();
            const tId = setTimeout(() => controller.abort(), 1500);
            let authRes = await fetch(`${config.url}/auth-check`, {
              method: "GET",
              headers: { "X-ZR-Token": config.token },
              signal: controller.signal,
            });
            clearTimeout(tId);
            if (authRes.ok) {
              isAuthenticated = true;
            } else if (authRes.status === 401) {
              const freshToken = await autoPairBridge(config);
              if (freshToken) {
                config.token = freshToken;
                const c2 = new AbortController();
                const t2 = setTimeout(() => c2.abort(), 1500);
                authRes = await fetch(`${config.url}/auth-check`, {
                  method: "GET",
                  headers: { "X-ZR-Token": freshToken },
                  signal: c2.signal,
                });
                clearTimeout(t2);
                if (authRes.ok) {
                  isAuthenticated = true;
                }
              }
            }
          } catch {}
        }
      }

      // Update badge
      if (!isOnline) {
        chrome.action.setBadgeText({ text: "" });
      } else if (!isAuthenticated) {
        chrome.action.setBadgeText({ text: "KEY" });
        chrome.action.setBadgeBackgroundColor({ color: "#f59e0b" }); // amber
      } else {
        chrome.action.setBadgeText({ text: "" });
      }

      sendResponse({
        online: isOnline,
        authenticated: isAuthenticated,
        port: (pingData && pingData.port) || config.port,
        hasToken: Boolean(config.token),
        data: pingData,
      });
    })().catch(() => {
      sendResponse({
        online: false,
        authenticated: false,
        port: DEFAULT_ZR_BRIDGE_PORT,
        hasToken: false,
      });
    });
    return true;
  }
});
