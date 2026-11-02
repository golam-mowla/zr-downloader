// Goat Download Manager (GDM) - Background Service Worker (Manifest V3)
// IDM-Style High-Speed Interception & Passive Media Stream Sniffer

const GDM_BRIDGE_PORT = 9666;
const GDM_BRIDGE_URL = `http://127.0.0.1:${GDM_BRIDGE_PORT}`;

// Interceptable file extensions for IDM-style capture
const INTERCEPT_EXTENSIONS = new Set([
  // Torrent files
  "torrent",
  // Archives
  "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "tgz",
  // Executables & Installers
  "exe", "msi", "dmg", "pkg", "deb", "rpm", "apk", "appx", "msix",
  // Media files
  "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "mp3", "flac", "wav", "m4a", "aac", "ogg",
  // Documents
  "pdf", "epub", "mobi",
  // Disk / Data
  "img", "vhd", "vmdk", "bin"
]);

// Keep track of downloads we are actively intercepting to avoid loops
const interceptedDownloadIds = new Set();
const SERVICE_WORKER_START_TIME = Date.now();

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

// Create Context Menu on installation
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: "download-with-gdm",
      title: "Download with ZR Downloader 🚀",
      contexts: ["all"],
    });
  });
});

// Intercept standard HTTP/HTTPS file downloads (IDM style)
async function handleDownloadInterception(downloadItem, suggest) {
  try {
    if (!downloadItem) return false;

    // 1. Prevent startup notification/interception storms:
    // If browser just launched or restored tabs, ignore historical items
    if (Date.now() - SERVICE_WORKER_START_TIME < 4000) {
      return false;
    }

    // 2. Ignore non-active downloads (completed, interrupted, paused from previous sessions)
    if (downloadItem.state && downloadItem.state !== "in_progress") {
      return false;
    }

    // 3. Ignore stale downloads whose start time was before this session or > 12 seconds ago
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

    const isTorrent =
      ext === "torrent" ||
      (downloadItem.mime && downloadItem.mime.toLowerCase().includes("bittorrent")) ||
      rawUrl.toLowerCase().includes(".torrent");

    if (INTERCEPT_EXTENSIONS.has(ext) || isTorrent) {
      const storage = await chrome.storage.local.get(["interceptDownloads", "showNotifications"]);
      if (storage.interceptDownloads === false) {
        return false; // User disabled interception in popup/settings
      }

      if (interceptedDownloadIds.has(downloadItem.id)) return true;
      interceptedDownloadIds.add(downloadItem.id);

      // Cancel the browser's native download immediately before Brave/Chrome shows Save As dialog
      try {
        chrome.downloads.cancel(downloadItem.id, () => {
          chrome.downloads.erase({ id: downloadItem.id }, () => {});
        });
      } catch (e) {}

      // Only show desktop notification if user explicitly enabled it (default: silent like IDM)
      if (storage.showNotifications === true) {
        chrome.notifications.create({
          type: "basic",
          iconUrl: "icons/icon128.png",
          title: "Goat Download Manager 🐐",
          message: isTorrent
            ? `Intercepted BitTorrent download:\n${downloadItem.filename || "Torrent file"}`
            : `Intercepted high-speed download:\n${downloadItem.filename || ext.toUpperCase() + " file"}`,
          priority: 1,
        });
      }

      // Forward to GDM Desktop Engine
      const cleanFilename = downloadItem.filename ? downloadItem.filename.replace(/^.*[\\\/]/, "") : "";
      const referrer = downloadItem.referrer || "";
      await sendUrlToGdm(rawUrl, null, cleanFilename, null, false, isTorrent, referrer);
      return true;
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
  if (info.menuItemId === "download-with-gdm") {
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

    await sendUrlToGdm(candidateUrl, tab ? tab.id : null);
  }
});

// Send URL to GDM Desktop App
async function sendUrlToGdm(mediaUrl, tabId, customFilename, quality, autoStart = false, isTorrent = false, referrer = "", container = "") {
  let cleanUrl = normalizeMediaUrl((mediaUrl || "").trim());
  if (!cleanUrl) return;

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

  // Notify tab that request is in progress
  if (effectiveTabId) {
    try {
      await chrome.tabs.sendMessage(effectiveTabId, {
        action: "SHOW_TOAST",
        title: "Goat Download Manager 🐐",
        message: isTorrent
          ? "Adding torrent to GDM engine..."
          : quality
          ? `Sending ${quality} stream to GDM Engine...`
          : "Sending link to GDM engine...",
        status: "info",
      });
    } catch {}
  }

  // 1. Try sending to local bridge HTTP server (if app is running)
  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 1400);

    const filenameParam = customFilename ? `&filename=${encodeURIComponent(customFilename)}` : "";
    const qualityParam = quality ? `&quality=${encodeURIComponent(quality)}` : "";
    const autoParam = autoStart ? `&autostart=true` : "";
    const torrentParam = isTorrent ? `&isTorrent=true` : "";
    const referrerParam = referrer ? `&referrer=${encodeURIComponent(referrer)}` : "";
    const containerParam = container ? `&container=${encodeURIComponent(container)}` : "";
    const bridgeEndpoint = `${GDM_BRIDGE_URL}/open?url=${encodeURIComponent(cleanUrl)}${filenameParam}${qualityParam}${autoParam}${torrentParam}${referrerParam}${containerParam}`;
    const resp = await fetch(bridgeEndpoint, {
      method: "GET",
      signal: controller.signal,
    });
    clearTimeout(timeoutId);

    if (resp.ok) {
      if (effectiveTabId) {
        try {
          await chrome.tabs.sendMessage(effectiveTabId, {
            action: "SHOW_TOAST",
            title: "Goat Download Manager 🐐",
            message: isTorrent
              ? "Torrent added in GDM!"
              : quality
              ? `${quality} Download started in GDM!`
              : "Link sent to GDM! Download starting...",
            status: "success",
          });
        } catch {}
      }
      return { success: true, method: "bridge" };
    }
  } catch (err) {
    // Local bridge not active (app might be closed or starting)
  }

  // 2. Launch GDM via custom Windows protocol
  const filenameParam = customFilename ? `&filename=${encodeURIComponent(customFilename)}` : "";
  const qualityParam = quality ? `&quality=${encodeURIComponent(quality)}` : "";
  const autoParam = autoStart ? `&autostart=true` : "";
  const torrentParam = isTorrent ? `&isTorrent=true` : "";
  const protocolUrl = `gdm://open?url=${encodeURIComponent(cleanUrl)}${filenameParam}${qualityParam}${autoParam}${torrentParam}`;

  if (effectiveTabId) {
    try {
      await chrome.scripting.executeScript({
        target: { tabId: effectiveTabId },
        func: (proto, raw) => {
          navigator.clipboard.writeText(raw).catch(() => {});
          const iframe = document.createElement("iframe");
          iframe.style.display = "none";
          iframe.src = proto;
          document.body.appendChild(iframe);
          setTimeout(() => iframe.remove(), 2500);
        },
        args: [protocolUrl, cleanUrl],
      });

      await chrome.tabs.sendMessage(effectiveTabId, {
        action: "SHOW_TOAST",
        title: "Goat Download Manager 🐐",
        message: "Opening GDM desktop app...",
        status: "info",
      });

      return { success: true, method: "protocol" };
    } catch (e) {
      chrome.tabs.create({ url: protocolUrl, active: false }, (t) => {
        setTimeout(() => chrome.tabs.remove(t.id), 2000);
      });
    }
  } else {
    chrome.tabs.create({ url: protocolUrl, active: false }, (t) => {
      setTimeout(() => chrome.tabs.remove(t.id), 2000);
    });
  }

  return { success: true, method: "protocol" };
}

// Listen to messages from popup or content script
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "SEND_TO_GDM") {
    const tabId = sender && sender.tab ? sender.tab.id : null;
    sendUrlToGdm(
      request.url,
      tabId,
      request.filename,
      request.quality,
      Boolean(request.autoStart),
      Boolean(request.isTorrent),
      request.referrer || "",
      request.container || ""
    ).then((res) => sendResponse(res));
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

  if (request.action === "CHECK_GDM_STATUS") {
    fetch(`${GDM_BRIDGE_URL}/ping`, { method: "GET" })
      .then((res) => res.json())
      .then((data) => sendResponse({ online: true, data }))
      .catch(() => sendResponse({ online: false }));
    return true;
  }
});
