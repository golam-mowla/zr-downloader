// ZR Downloader - Media Overlay Grabber
// Seamless video detection across YouTube, Instagram, Facebook, Kick, Twitch, TikTok, Twitter, and ANY HTML5 video website.

(() => {
  let lastRightClickedElement = null;
  let lastDetectedMediaUrl = null;
  let activeGrabberButton = null;
  let activeDropdown = null;
  let currentTargetVideo = null;
  let currentTargetContainer = null;
  let hideTimer = null;
  let isPermanentlyDismissed = false;
  let cachedLowDiskSpace = false;

  try {
    if (typeof chrome !== "undefined" && chrome.storage && chrome.storage.local) {
      chrome.storage.local.get(["lowDiskSpace"], (st) => {
        if (st && st.lowDiskSpace !== undefined) {
          cachedLowDiskSpace = Boolean(st.lowDiskSpace);
        }
      });
      if (chrome.storage.onChanged) {
        chrome.storage.onChanged.addListener((changes, area) => {
          if (area === "local" && changes && changes.lowDiskSpace) {
            cachedLowDiskSpace = Boolean(changes.lowDiskSpace.newValue);
          }
        });
      }
    }
  } catch (e) {}

  // Cached YouTube data from MAIN world helper (yt-main-world.js)
  let lastYouTubeData = null;

  window.addEventListener("ZR_RESPONSE_YT_DATA", (e) => {
    if (e && e.detail) {
      lastYouTubeData = e.detail;
    }
  });

  function requestYouTubeData() {
    window.dispatchEvent(new CustomEvent("ZR_REQUEST_YT_DATA"));
  }

  // Synchronous fallback: parse ytInitialPlayerResponse directly from DOM script tags
  function parseYouTubeFromDom() {
    try {
      for (const s of document.scripts) {
        if (s.textContent && s.textContent.includes("ytInitialPlayerResponse")) {
          const m = s.textContent.match(/ytInitialPlayerResponse\s*=\s*({.+?});/);
          if (m) {
            const resp = JSON.parse(m[1]);
            if (resp) {
              const qualities = new Set();
              const allStreams = [
                ...(resp.streamingData?.formats || []),
                ...(resp.streamingData?.adaptiveFormats || []),
              ];
              for (const fmt of allStreams) {
                if (fmt.qualityLabel) qualities.add(fmt.qualityLabel.trim());
              }
              const subtitles = [];
              const tracks =
                resp.captions?.playerCaptionsTracklistRenderer?.captionTracks || [];
              for (const tr of tracks) {
                if (!tr.baseUrl) continue;
                subtitles.push({
                  name:
                    tr.name?.simpleText ||
                    tr.name?.runs?.[0]?.text ||
                    tr.languageCode ||
                    "Subtitle",
                  languageCode: tr.languageCode || "en",
                  baseUrl: tr.baseUrl,
                });
              }
              const thumbs = resp.videoDetails?.thumbnail?.thumbnails || [];
              const bestThumbnail =
                thumbs.length > 0 ? thumbs[thumbs.length - 1].url : "";
              const title =
                resp.videoDetails?.title ||
                document.title.replace(/ - YouTube$/, "").trim();
              const duration =
                parseInt(resp.videoDetails?.lengthSeconds, 10) || 0;
              return {
                title,
                duration,
                qualities: Array.from(qualities),
                subtitles,
                thumbnail: bestThumbnail,
                videoId: resp.videoDetails?.videoId || "",
              };
            }
          }
        }
      }
    } catch (e) {}
    return null;
  }

  function stripRangeParams(rawUrl) {
    if (!rawUrl || typeof rawUrl !== "string") return rawUrl;
    try {
      const u = new URL(rawUrl);
      let changed = false;
      if (u.searchParams.has("bytestart") || u.searchParams.has("byteend")) {
        u.searchParams.delete("bytestart");
        u.searchParams.delete("byteend");
        changed = true;
      }
      if (u.searchParams.has("range")) {
        u.searchParams.delete("range");
        changed = true;
      }
      return changed ? u.toString() : rawUrl;
    } catch (e) {
      return rawUrl;
    }
  }

  function isValidDownloadUrl(url) {
    if (!url || typeof url !== "string") return false;
    const clean = url.trim();
    if (
      clean.startsWith("blob:") ||
      clean.startsWith("data:") ||
      clean.startsWith("javascript:")
    ) {
      return false;
    }
    return clean.startsWith("http://") || clean.startsWith("https://");
  }

  function getShortTitle(str, maxLen = 28) {
    if (!str) return "Video";
    const clean = str.replace(/\s+/g, " ").trim();
    if (clean.length <= maxLen) return clean;
    return clean.slice(0, maxLen).trim() + "...";
  }

  function formatBytes(bytes) {
    if (!bytes || bytes <= 0) return "";
    const mb = bytes / (1024 * 1024);
    if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
    return `${mb.toFixed(1)} MB`;
  }

  function sortQualities(qualitiesList) {
    function getQualityWeight(q) {
      if (!q) return 0;
      const m = q.match(/(\d+)p/);
      let val = m ? parseInt(m[1], 10) : 0;
      if (q.includes("2160") || q.includes("4K")) val = 2160;
      else if (q.includes("1440") || q.includes("2K")) val = 1440;
      else if (q.includes("1080")) val = 1080;
      else if (q.includes("720")) val = 720;
      else if (q.includes("480")) val = 480;
      else if (q.includes("360")) val = 360;
      else if (q.includes("240")) val = 240;
      else if (q.includes("144")) val = 144;
      if (/premium/i.test(q)) val += 0.8;
      else if (q.includes("60") || q.includes("50")) val += 0.5;
      return val;
    }
    return [...qualitiesList].sort((a, b) => getQualityWeight(b) - getQualityWeight(a));
  }

  // Right-click media URL tracker
  document.addEventListener(
    "contextmenu",
    (e) => {
      lastRightClickedElement = e.target;
      lastDetectedMediaUrl = extractMediaTarget(e.target).url;
    },
    true
  );

  document.addEventListener(
    "mousedown",
    (e) => {
      if (e.button === 2) {
        lastRightClickedElement = e.target;
        lastDetectedMediaUrl = extractMediaTarget(e.target).url;
      }
      // Dismiss dropdown when clicking outside
      if (
        activeDropdown &&
        !activeDropdown.contains(e.target) &&
        (!activeGrabberButton || !activeGrabberButton.contains(e.target))
      ) {
        hideDropdown();
      }
    },
    true
  );

  // Intercept click on magnet: and .torrent links to trigger ZR Downloader Torrent Engine
  document.addEventListener(
    "click",
    (e) => {
      const anchor = e.target.closest("a");
      if (!anchor || !anchor.href) return;
      const href = anchor.href.trim();
      if (href.startsWith("magnet:")) {
        e.preventDefault();
        e.stopPropagation();
        showToast(
          "ZR Downloader 🚀",
          "Adding magnet link to ZR Downloader Torrent engine...",
          "info"
        );
        chrome.runtime.sendMessage({
          action: "SEND_TO_ZR",
          url: href,
          isTorrent: true,
          autoStart: false,
        });
        return;
      }
      if (
        href.toLowerCase().endsWith(".torrent") ||
        href.toLowerCase().includes(".torrent?")
      ) {
        e.preventDefault();
        e.stopPropagation();
        showToast(
          "ZR Downloader 🚀",
          "Adding .torrent file to ZR Downloader Torrent engine...",
          "info"
        );
        chrome.runtime.sendMessage({
          action: "SEND_TO_ZR",
          url: href,
          isTorrent: true,
          autoStart: false,
        });
        return;
      }
    },
    true
  );

  // Find actively visible / playing video on screen (Instagram Reels, Shorts, etc.)
  function findActiveVideoOnScreen() {
    const vids = Array.from(document.querySelectorAll("video"));
    if (vids.length === 0) return null;

    // First: actively playing
    const playing = vids.find((v) => !v.paused && v.currentTime > 0 && v.readyState >= 2);
    if (playing) return playing;

    // Second: closest to viewport center
    const vpCenterY = window.innerHeight / 2;
    let closestVid = null;
    let minDistance = Infinity;

    for (const v of vids) {
      const rect = v.getBoundingClientRect();
      if (rect.width < 80 || rect.height < 60) continue;
      if (rect.bottom <= 0 || rect.top >= window.innerHeight) continue;
      const vidCenterY = rect.top + rect.height / 2;
      const dist = Math.abs(vidCenterY - vpCenterY);
      if (dist < minDistance) {
        minDistance = dist;
        closestVid = v;
      }
    }
    return closestVid || vids[0];
  }

  // Find Instagram Reel / Post link
  function findInstagramPostUrl(vid, element) {
    if (
      window.location.href.includes("/reel/") ||
      window.location.href.includes("/p/")
    ) {
      return window.location.href;
    }
    const container = vid
      ? vid.closest(
          "article, div[role='dialog'], div[data-pressable-container='true'], main, section"
        )
      : element?.closest("article, div[role='dialog']");

    if (container) {
      const link = container.querySelector(
        "a[href*='/reel/'], a[href*='/p/']"
      );
      if (link && link.href) return link.href;
      const timeLink = container.querySelector("time")?.closest("a");
      if (timeLink && timeLink.href) return timeLink.href;
    }

    let parent = vid ? vid.parentElement : element?.parentElement;
    let depth = 0;
    while (parent && depth < 16) {
      const link = parent.querySelector("a[href*='/reel/'], a[href*='/p/']");
      if (link && link.href) return link.href;
      const timeLink = parent.querySelector("time")?.closest("a");
      if (timeLink && timeLink.href) return timeLink.href;
      parent = parent.parentElement;
      depth++;
    }

    return window.location.href;
  }

  // Extract clean target URL and best human filename/title
  function extractMediaTarget(element) {
    const hostname = window.location.hostname;
    let title = document.title || "Video";

    // 1. YouTube
    if (hostname.includes("youtube.com") || hostname.includes("youtu.be")) {
      const card = element?.closest?.(
        "ytd-rich-item-renderer, ytd-video-renderer, ytd-compact-video-renderer, ytd-grid-video-renderer"
      );
      if (card) {
        const titleEl = card.querySelector("#video-title, #video-title-link");
        const linkEl = card.querySelector("a#thumbnail, a#video-title-link");
        const cardTitle =
          titleEl?.innerText?.trim() || titleEl?.getAttribute("title") || "";
        const cardUrl = linkEl?.href || window.location.href;
        if (cardTitle && cardUrl) {
          return {
            url: cardUrl,
            title: cardTitle,
            videoEl: card.querySelector("video"),
          };
        }
      }

      const ytTitle = document.querySelector(
        "h1.ytd-watch-metadata yt-formatted-string, #title h1, h1.title"
      );
      if (ytTitle && ytTitle.innerText) {
        title = ytTitle.innerText.trim();
      } else {
        title = document.title.replace(/ - YouTube$/, "").trim();
      }
      return {
        url: window.location.href,
        title,
        videoEl:
          element?.tagName === "VIDEO"
            ? element
            : element?.querySelector?.("video") || document.querySelector("video"),
      };
    }

    // 2. Instagram (Reels, Stories, and Home Feed Articles)
    if (hostname.includes("instagram.com")) {
      const activePlaying = Array.from(document.querySelectorAll("video")).find(
        (v) => !v.paused && v.currentTime > 0 && v.readyState >= 2
      );
      const vid =
        activePlaying ||
        (element?.tagName === "VIDEO"
          ? element
          : element?.querySelector?.("video")) ||
        currentTargetVideo ||
        document.querySelector("video");

      const postUrl = findInstagramPostUrl(vid, element);

      let author = "";
      let caption = "";

      const postContainer = vid
        ? vid.closest(
            "article, div[role='dialog'], div[data-pressable-container='true'], main, section"
          )
        : element?.closest("article, div[role='dialog']");

      if (postContainer) {
        const authorEl = postContainer.querySelector(
          "header h2, header a, a[role='link']:not([href*='/p/']):not([href*='/reel/'])"
        );
        if (authorEl && authorEl.innerText) {
          author = authorEl.innerText.replace(/[\r\n\t]+/g, " ").trim();
        }

        const captionNodes = postContainer.querySelectorAll(
          "h1, div._a9zs, span._aade, span._aaco, span[dir='auto'], div[dir='auto']"
        );
        for (const node of captionNodes) {
          if (
            node.closest(
              "time, a[href*='/audio/'], a[href*='/music/'], button, header"
            )
          )
            continue;
          const t = node.innerText?.trim();
          if (t && t.length > 3) {
            caption = t
              .replace(/[\r\n\t]+/g, " ")
              .replace(/\s*\.\.\.\s*more$/i, "")
              .trim();
            if (caption.length > 60) caption = caption.slice(0, 60).trim() + "...";
            break;
          }
        }
      }

      let cleanAuthor = author ? author.replace(/[\r\n\t]+/g, " ").trim() : "";
      let cleanCaption = caption ? caption.replace(/[\r\n\t]+/g, " ").trim() : "";
      let postTitle = "";
      if (cleanAuthor && cleanCaption) {
        postTitle = `${cleanAuthor} - ${cleanCaption}`;
      } else if (cleanAuthor) {
        postTitle = `${cleanAuthor} - Instagram Video`;
      } else if (cleanCaption) {
        postTitle = cleanCaption;
      } else {
        postTitle =
          document.title
            .replace(/ • Instagram photos and videos$/, "")
            .replace(/ \| Instagram$/, "")
            .trim() || "Instagram Video";
      }
      postTitle = postTitle.replace(/[\\/:*?"<>|]/g, "_").trim();
      if (!/\.[a-zA-Z0-9]{2,5}$/.test(postTitle)) {
        postTitle = `${postTitle}.mp4`;
      }

      // Find poster thumbnail image
      let posterUrl = "";
      if (vid && vid.poster) {
        posterUrl = vid.poster;
      } else if (postContainer) {
        const img = postContainer.querySelector(
          "img[srcset], img[src*='cdninstagram']"
        );
        if (img && img.src) posterUrl = img.src;
      }

      return {
        url: postUrl,
        pageUrl: postUrl,
        title: postTitle,
        postUrl,
        videoEl: vid,
        thumbnail: posterUrl,
      };
    }

    // 3. Kick.com (Streams & VODs)
    if (hostname.includes("kick.com")) {
      let kickTitle = "";
      const titleEl = document.querySelector(
        ".stream-title, h1, h2, span[title]"
      );
      if (titleEl && titleEl.innerText) {
        kickTitle = titleEl.innerText.trim();
      }
      if (!kickTitle) {
        kickTitle = document.title
          .replace(/ - Kick$/, "")
          .replace(/ \| Kick$/, "")
          .replace(/ - Watch Live on Kick$/, "")
          .trim();
      }
      const vid =
        (element?.tagName === "VIDEO"
          ? element
          : element?.querySelector?.("video")) || currentTargetVideo;
      return {
        url: window.location.href,
        title: kickTitle || "Kick Video",
        videoEl: vid,
      };
    }

    // 4. Popular Social & Video platforms natively supported by yt-dlp
    if (
      hostname.includes("twitch.tv") ||
      hostname.includes("tiktok.com") ||
      hostname.includes("facebook.com") ||
      hostname.includes("fb.watch") ||
      hostname.includes("twitter.com") ||
      hostname.includes("x.com") ||
      hostname.includes("vimeo.com") ||
      hostname.includes("dailymotion.com") ||
      hostname.includes("reddit.com") ||
      hostname.includes("bilibili.com") ||
      hostname.includes("soundcloud.com")
    ) {
      title = document.title
        .replace(/ - Kick$/, "")
        .replace(/ \| Instagram$/, "")
        .replace(/ \/ X$/, "")
        .replace(/ \| Facebook$/, "")
        .trim();
      return {
        url: window.location.href,
        title,
        videoEl:
          element?.tagName === "VIDEO"
            ? element
            : element?.querySelector?.("video"),
      };
    }

    // 5. Direct HTML5 Video Element check
    if (element) {
      let vid = null;
      if (element.tagName === "VIDEO") {
        vid = element;
      } else {
        vid = element.querySelector?.("video") || element.closest?.("video");
      }

      if (vid) {
        if (vid.currentSrc && isValidDownloadUrl(vid.currentSrc)) {
          return { url: vid.currentSrc, title, videoEl: vid };
        }
        if (vid.src && isValidDownloadUrl(vid.src)) {
          return { url: vid.src, title, videoEl: vid };
        }
        const source = vid.querySelector("source");
        if (source && source.src && isValidDownloadUrl(source.src)) {
          return { url: source.src, title, videoEl: vid };
        }
      }
    }

    // 6. Anchor links
    const anchor = element?.closest?.("a");
    if (anchor && anchor.href && isValidDownloadUrl(anchor.href)) {
      return {
        url: anchor.href,
        title: anchor.innerText?.trim() || title,
        videoEl: null,
      };
    }

    // 7. Fallback to current Page URL
    return {
      url: window.location.href,
      title,
      videoEl:
        element?.tagName === "VIDEO"
          ? element
          : element?.querySelector?.("video"),
    };
  }

  // ---------------------------------------------------------------------------
  // IDM-Style Format Dropdown Menu
  // ---------------------------------------------------------------------------
  function hideDropdown() {
    dropdownSeq++; // invalidates any async build still in flight
    if (activeDropdown) {
      activeDropdown.remove();
      activeDropdown = null;
    }
    document.querySelectorAll("#zr-format-dropdown").forEach((n) => n.remove());
    try {
      scheduleHide();
    } catch (e) {}
  }

  // Close the quality list on ANY outside interaction (clicks, other tabs/iframes, scroll, Esc)
  (function installDropdownDismissers() {
    const dismissIfOutside = (e) => {
      if (!activeDropdown) return;
      const path = e.composedPath ? e.composedPath() : [e.target];
      if (path.includes(activeDropdown)) return;
      if (activeGrabberButton && path.includes(activeGrabberButton)) return;
      hideDropdown();
    };
    ["pointerdown", "mousedown", "touchstart", "click", "auxclick", "contextmenu"].forEach((t) =>
      window.addEventListener(t, dismissIfOutside, true)
    );
    window.addEventListener(
      "keydown",
      (e) => {
        if (e.key === "Escape" && activeDropdown) hideDropdown();
      },
      true
    );
    // Clicking inside an iframe (video players) never reaches this document, but it blurs the window.
    window.addEventListener("blur", () => {
      if (activeDropdown) hideDropdown();
    });
    window.addEventListener("resize", () => {
      if (activeDropdown) hideDropdown();
    });
    window.addEventListener(
      "scroll",
      (e) => {
        if (activeDropdown && !(e.target && activeDropdown.contains && activeDropdown.contains(e.target))) {
          hideDropdown();
        }
      },
      true
    );
    document.addEventListener("visibilitychange", () => {
      if (document.hidden && activeDropdown) hideDropdown();
    });
  })();

  // --- Real quality list (IDM-style) from ZR Downloader's yt-dlp engine -------------------------
  let dropdownSeq = 0;
  const zrFormatsCache = new Map(); // pageUrl -> { t, promise }
  const ZR_FORMAT_HOSTS =
    /(youtube\.com|youtu\.be|instagram\.com|facebook\.com|fb\.watch|tiktok\.com|twitter\.com|x\.com|vimeo\.com|dailymotion\.com|reddit\.com|twitch\.tv|kick\.com)/i;

  function canonicalizePageUrl(rawUrl) {
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

  function isStreamPremium(st) {
    if (!st) return false;
    return Boolean(st.isPremium) || /premium/i.test(st.formatNote || st.label || "") || String(st.id) === "616" || String(st.id) === "356";
  }

  function fetchRealFormats(pageUrl) {
    const cacheKey = canonicalizePageUrl(pageUrl);
    const hit = zrFormatsCache.get(cacheKey);
    if (hit && Date.now() - hit.t < 10 * 60 * 1000) return hit.promise;
    const promise = new Promise((resolve) => {
      try {
        chrome.runtime.sendMessage({ action: "GET_FORMATS", url: cacheKey }, (resp) => {
          if (chrome.runtime.lastError || !resp || !resp.ok || !resp.info) {
            zrFormatsCache.delete(cacheKey);
            resolve(null);
            return;
          }
          resolve(resp.info);
        });
      } catch (e) {
        zrFormatsCache.delete(cacheKey);
        resolve(null);
      }
    });
    zrFormatsCache.set(cacheKey, { t: Date.now(), promise });
    return promise;
  }

  function prefetchFormatsForPage() {
    try {
      if (ZR_FORMAT_HOSTS.test(location.hostname)) {
        if (location.hostname.includes("youtube.com") && !location.pathname.startsWith("/watch") && !location.pathname.startsWith("/shorts")) {
          return;
        }
        fetchRealFormats(location.href);
      }
    } catch (e) {}
  }

  function buildRealFormatOptions(info, pageUrl, fallbackTitle, durStr) {
    const streams = (info.videoStreams || []).filter((s) => s && s.height);
    if (!streams.length) return [];
    const title = (info.title || fallbackTitle || "Video").replace(/\.mp4$/i, "");
    const cleanTitle = title.length > 55 ? title.slice(0, 55) + "..." : title;
    const shortTitle = getShortTitle(cleanTitle, 28);
    const dur = durStr || formatDurationForList(info.duration);

    // One row per (height, fps, isPremium): prefer MP4, then largest stream, keeping Premium distinct
    const best = new Map();
    const score = (x) => (x.ext === "mp4" ? 1e13 : 0) + (x.filesize || 0);
    streams.forEach((st) => {
      const fps = st.fps && st.fps > 30 ? st.fps : 30;
      const premSuffix = isStreamPremium(st) ? "_prem" : "";
      const key = `${st.height}_${fps}${premSuffix}`;
      const cur = best.get(key);
      if (!cur || score(st) > score(cur)) best.set(key, st);
    });
    const rows = Array.from(best.values()).sort(
      (a, b) =>
        b.height - a.height ||
        (isStreamPremium(b) ? 1 : 0) - (isStreamPremium(a) ? 1 : 0) ||
        (b.fps || 0) - (a.fps || 0)
    );

    const options = [];
    rows.forEach((st) => {
      const prem = isStreamPremium(st);
      const hd = prem
        ? `${st.height}p Premium HD`
        : st.height >= 720
        ? `${st.height}p HD`
        : `${st.height}p`;
      const fpsInfo = st.fps && st.fps > 30 ? ` ${st.fps}fps` : "";
      const sizeInfo = st.filesize ? `, ${formatBytes(st.filesize)}` : "";
      options.push({
        label: `${shortTitle}, MP4 file, ${dur}quality ${hd}${fpsInfo}${sizeInfo}`,
        fullLabel: `${cleanTitle}, MP4 file, ${dur}quality ${hd}${fpsInfo}${sizeInfo}`,
        shortTitle,
        formatDisplay: `${hd}${fpsInfo} • ${(st.ext || "mp4").toUpperCase()}${st.filesize ? " • " + formatBytes(st.filesize) : ""}`,
        quality: prem ? `${st.height}p Premium` : `${st.height}p`,
        isPremium: prem,
        formatId: st.id,
        url: pageUrl,
        container: "MP4",
        filename: `${cleanTitle}.mp4`,
      });
    });

    options.push({
      label: `${shortTitle}, MP3 Audio, ${dur}Highest Quality`,
      fullLabel: `${cleanTitle}, MP3 Audio, ${dur}Highest Quality`,
      shortTitle,
      formatDisplay: "MP3 Audio",
      quality: "Audio",
      url: pageUrl,
      container: "MP3",
      filename: `${cleanTitle}.mp3`,
    });

    if (info.thumbnail && /^https?:/i.test(info.thumbnail)) {
      options.push({
        label: `${shortTitle}, Thumbnail (JPG)`,
        fullLabel: `${cleanTitle}, Thumbnail (JPG)`,
        shortTitle,
        formatDisplay: "Thumbnail JPG",
        quality: "Thumbnail",
        url: info.thumbnail,
        container: "JPG",
        isDirectFile: true,
        filename: `${cleanTitle.slice(0, 45)}_thumbnail.jpg`,
      });
    }
    return options;
  }

  function formatDurationForList(totalSec) {
    if (!totalSec || isNaN(totalSec) || totalSec <= 0) return "";
    const s = Math.round(totalSec);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    if (h > 0) return m > 0 ? `${h} hours ${m} min, ` : `${h} hours, `;
    if (m > 0 && s % 60 === 0) return `${m} min, `;
    return `${s} sec, `;
  }

  function createDropdownShell(anchorBtn) {
    document.querySelectorAll("#zr-format-dropdown").forEach((n) => n.remove());
    const shell = document.createElement("div");
    shell.id = "zr-format-dropdown";
    Object.assign(shell.style, {
      position: "fixed",
      zIndex: "2147483647",
      background: "#ffffff",
      border: "1px solid #717171",
      boxShadow: "0 4px 16px rgba(0,0,0,0.35)",
      borderRadius: "2px",
      minWidth: "380px",
      maxWidth: "640px",
      fontFamily: 'Tahoma, "Segoe UI", Arial, sans-serif',
      fontSize: "12px",
      color: "#0f172a",
      direction: "ltr",
      userSelect: "none",
      overflow: "hidden",
    });
    const loading = document.createElement("div");
    loading.innerText = "ZR Downloader — fetching available qualities…";
    Object.assign(loading.style, { padding: "10px 14px", color: "#475569" });
    shell.appendChild(loading);
    const btnRect = anchorBtn.getBoundingClientRect();
    shell.style.top = `${Math.min(window.innerHeight - 120, btnRect.bottom + 2)}px`;
    if (btnRect.left < window.innerWidth / 2) {
      shell.style.left = `${Math.max(8, btnRect.left)}px`;
    } else {
      shell.style.right = `${Math.max(8, window.innerWidth - btnRect.right)}px`;
    }
    (document.fullscreenElement || document.webkitFullscreenElement || document.body).appendChild(shell);
    activeDropdown = shell;
    return shell;
  }

  async function showFormatDropdown(anchorBtn, mediaTarget) {
    if (activeDropdown) {
      hideDropdown();
      return;
    }
    // Show the panel immediately (IDM-like) and fill it when the data arrives.
    const shell = createDropdownShell(anchorBtn);
    const mySeq = ++dropdownSeq;
    const stillOpen = () => activeDropdown === shell && dropdownSeq === mySeq && shell.isConnected;

    // Preserve specific mediaTarget attached to this button
    if (!mediaTarget || !mediaTarget.videoEl) {
      const allVids = Array.from(document.querySelectorAll("video"));
      let targetActiveVid = allVids.find((v) => !v.paused && v.currentTime > 0 && v.readyState >= 2);
      if (!targetActiveVid) {
        targetActiveVid = allVids.find((v) => {
          const r = v.getBoundingClientRect();
          return r.width > 120 && r.height > 120 && r.top < window.innerHeight && r.bottom > 0;
        });
      }
      if (!targetActiveVid && currentTargetVideo) {
        targetActiveVid = currentTargetVideo;
      }
      if (targetActiveVid) {
        currentTargetVideo = targetActiveVid;
        mediaTarget = extractMediaTarget(targetActiveVid);
      }
    } else {
      currentTargetVideo = mediaTarget.videoEl;
    }

    let sniffedStreams = [];
    try {
      const resp = await chrome.runtime.sendMessage({
        action: "GET_SNIFFED_MEDIA",
      });
      if (resp && Array.isArray(resp.streams)) {
        sniffedStreams = resp.streams;
      }
    } catch (e) {}

    if (!stillOpen()) return;

    const title = mediaTarget.title || "Video";
    const hostname = window.location.hostname;
    const vid = mediaTarget.videoEl || currentTargetVideo;
    const directSrc =
      vid && vid.currentSrc && isValidDownloadUrl(vid.currentSrc)
        ? stripRangeParams(vid.currentSrc)
        : vid && vid.src && isValidDownloadUrl(vid.src)
        ? stripRangeParams(vid.src)
        : "";

    let directPath = "";
    if (directSrc) {
      try {
        directPath = new URL(directSrc).pathname;
      } catch (e) {}
    }

    function formatDurationHuman(totalSec) {
      if (!totalSec || isNaN(totalSec) || totalSec <= 0) return "";
      const s = Math.round(totalSec);
      const hours = Math.floor(s / 3600);
      const mins = Math.floor((s % 3600) / 60);
      const secs = s % 60;
      if (hours > 0) {
        return mins > 0 ? `${hours} hours ${mins} min, ` : `${hours} hours, `;
      }
      if (mins > 0 && secs === 0) {
        return `${mins} min, `;
      }
      return `${s} sec, `;
    }

    let durationSec = 0;
    if (vid && vid.duration && !isNaN(vid.duration) && isFinite(vid.duration)) {
      durationSec = Math.round(vid.duration);
    }
    const durStr = formatDurationHuman(durationSec);

    const options = [];

    // 0. REAL formats straight from yt-dlp via the ZR app (fast 250ms cache hit check; live-upgrades open menu)
    let realOptions = null;
    let pendingRealFormats = null;
    {
      const hasDirectSniffed = sniffedStreams.some((st) => !st.isAudio && st.contentLength > 50000);
      const knownHost = ZR_FORMAT_HOSTS.test(hostname);
      if (knownHost || !hasDirectSniffed) {
        let pageUrl = mediaTarget.postUrl || mediaTarget.url || window.location.href;
        if (/^(blob:|data:)/i.test(pageUrl)) pageUrl = window.location.href;
        if (hostname.includes("youtube.com") && !/[?&]v=/.test(pageUrl) && !pageUrl.includes("youtu.be/")) {
          pageUrl = window.location.href;
        }
        const formatsPromise = fetchRealFormats(pageUrl);
        const info = await Promise.race([
          formatsPromise,
          new Promise((r) => setTimeout(() => r(undefined), 250)),
        ]);
        if (!stillOpen()) return;
        if (info === undefined) {
          pendingRealFormats = { promise: formatsPromise, pageUrl, title, durStr };
        } else if (info) {
          const built = buildRealFormatOptions(info, pageUrl, title, durStr);
          if (built.length) realOptions = built;
        }
      }
    }

    // 1. YouTube - Query REAL available qualities from player API, subtitles, and thumbnail
    if (realOptions && realOptions.length) {
      options.push(...realOptions);
    } else if (hostname.includes("youtube.com") || hostname.includes("youtu.be")) {
      requestYouTubeData();
      let ytData = lastYouTubeData;
      if (!ytData || !ytData.qualities || ytData.qualities.length === 0) {
        ytData = parseYouTubeFromDom() || ytData;
      }

      const allStandard = [
        "2160p (4K)",
        "1440p (2K)",
        "1080p HD",
        "720p HD",
        "480p",
        "360p",
        "240p",
        "144p",
      ];
      let availableQualities = [];
      if (ytData && ytData.qualities && ytData.qualities.length > 0) {
        // Show ONLY real available qualities detected from player API
        availableQualities = sortQualities(ytData.qualities);
      } else {
        availableQualities = allStandard;
      }

      // Add ONLY qualities that actually exist for this video with real metadata
      const shortTitle = getShortTitle(title, 28);
      availableQualities.forEach((q) => {
        const qTag = q.toLowerCase().includes("hd") || parseInt(q, 10) < 720 ? q : `${q} HD`;
        const meta = ytData?.streamMeta?.[q] || ytData?.streamMeta?.[qTag];
        const sizeInfo = meta?.contentLength ? ` (${formatBytes(meta.contentLength)})` : "";
        const fpsInfo = meta?.fps && meta.fps > 30 ? ` ${meta.fps}fps` : "";
        options.push({
          label: `${shortTitle}, MP4 file, quality ${qTag}${fpsInfo}${sizeInfo}`,
          fullLabel: `${title}, MP4 file, ${durStr}quality ${qTag}${fpsInfo}${sizeInfo}`,
          shortTitle: shortTitle,
          formatDisplay: `${qTag}${fpsInfo} • MP4${sizeInfo}`,
          quality: qTag,
          url: mediaTarget.url || window.location.href,
          container: "MP4",
        });
      });

      // MP3 Audio
      options.push({
        label: `${shortTitle}, MP3 Audio, Highest Quality`,
        fullLabel: `${title}, MP3 Audio, ${durStr}Highest Quality`,
        shortTitle: shortTitle,
        formatDisplay: "MP3 Audio (320k)",
        quality: "Audio",
        url: mediaTarget.url || window.location.href,
        container: "MP3",
      });

      // Thumbnail Image
      const thumbUrl =
        ytData?.thumbnail ||
        (ytData?.videoId
          ? `https://i.ytimg.com/vi/${ytData.videoId}/maxresdefault.jpg`
          : "");
      if (thumbUrl) {
        options.push({
          label: `${shortTitle}, Thumbnail (High Quality JPG Image)`,
          fullLabel: `${title}, Thumbnail (High Quality JPG Image)`,
          shortTitle: shortTitle,
          formatDisplay: "Thumbnail JPG",
          quality: "Thumbnail",
          url: thumbUrl,
          container: "JPG",
          isDirectFile: true,
          filename: `${title.slice(0, 50)}_thumbnail.jpg`,
        });
      }

      // Subtitles / Captions if available
      if (ytData && Array.isArray(ytData.subtitles) && ytData.subtitles.length > 0) {
        ytData.subtitles.forEach((sub) => {
          options.push({
            label: `${shortTitle}, Subtitle - ${sub.name} [${sub.languageCode.toUpperCase()}] (.vtt)`,
            fullLabel: `${title}, Subtitle - ${sub.name} [${sub.languageCode.toUpperCase()}] (.vtt)`,
            shortTitle: shortTitle,
            formatDisplay: `Sub [${sub.languageCode.toUpperCase()}]`,
            quality: "Subtitle",
            url: sub.baseUrl.includes("fmt=")
              ? sub.baseUrl
              : `${sub.baseUrl}&fmt=vtt`,
            container: "VTT",
            isDirectFile: true,
            filename: `${title.slice(0, 50)}_${sub.languageCode}.vtt`,
          });
        });
      }
    } else if (hostname.includes("instagram.com")) {
      // 2. Instagram (Reels & Feed) - IDM-style All Available Qualities
      const baseTitle = title.replace(/\.mp4$/i, "");
      const cleanTitle = baseTitle.length > 55 ? baseTitle.slice(0, 55) + "..." : baseTitle;
      const shortTitle = getShortTitle(cleanTitle, 28);
      const targetInstagramUrl =
        mediaTarget.postUrl || mediaTarget.url || window.location.href;

      const vW = vid && vid.videoWidth ? vid.videoWidth : 0;
      const vH = vid && vid.videoHeight ? vid.videoHeight : 0;
      const maxDim = Math.max(vW, vH);
      const minDim = Math.min(vW, vH);
      const resTag = maxDim >= 1080 ? "1080p HD" : maxDim >= 720 ? "720p HD" : minDim >= 480 ? "480p" : minDim > 0 ? `${minDim}p` : "HD";

      // Sniffed direct stream matched
      const matchedStream = sniffedStreams.find(
        (st) =>
          !st.isAudio &&
          ((directSrc && st.url && st.url.split("?")[0] === directSrc.split("?")[0]) ||
            st.url === directSrc)
      );
      const sizeStr = matchedStream?.sizeFormatted ? ` (${matchedStream.sizeFormatted})` : "";
      const mp4Name = `${cleanTitle}.mp4`;
      const mp3Name = `${cleanTitle}.mp3`;

      // 1. Full HD 1080p (Highest Available Quality)
      options.push({
        label: `${shortTitle}, Full HD 1080p MP4 (Highest Quality)`,
        fullLabel: `${cleanTitle}, Full HD 1080p MP4, ${durStr}Highest Available Quality`,
        shortTitle: shortTitle,
        formatDisplay: "1080p • Full HD",
        quality: "1080p",
        url: targetInstagramUrl,
        filename: mp4Name,
        container: "MP4",
        isDirectFile: false,
      });

      // 2. HD 720p
      options.push({
        label: `${shortTitle}, HD 720p MP4`,
        fullLabel: `${cleanTitle}, HD 720p MP4, ${durStr}High Definition`,
        shortTitle: shortTitle,
        formatDisplay: "720p • HD",
        quality: "720p",
        url: targetInstagramUrl,
        filename: mp4Name,
        container: "MP4",
        isDirectFile: false,
      });

      // 3. SD 480p
      options.push({
        label: `${shortTitle}, SD 480p MP4`,
        fullLabel: `${cleanTitle}, SD 480p MP4, ${durStr}Standard Definition`,
        shortTitle: shortTitle,
        formatDisplay: "480p • SD",
        quality: "480p",
        url: targetInstagramUrl,
        filename: mp4Name,
        container: "MP4",
        isDirectFile: false,
      });

      // 4. Mobile 360p
      options.push({
        label: `${shortTitle}, 360p MP4`,
        fullLabel: `${cleanTitle}, 360p MP4, ${durStr}Mobile Resolution`,
        shortTitle: shortTitle,
        formatDisplay: "360p • MP4",
        quality: "360p",
        url: targetInstagramUrl,
        filename: mp4Name,
        container: "MP4",
        isDirectFile: false,
      });

      // 5. Playing Direct Video Stream (Direct File)
      if (directSrc) {
        options.push({
          label: `${shortTitle}, Playing Stream MP4, ${resTag}${sizeStr}`,
          fullLabel: `${cleanTitle}, Playing Stream MP4, ${durStr}quality ${resTag}${sizeStr} (Direct Video Stream)`,
          shortTitle: shortTitle,
          formatDisplay: `${resTag} • Direct${sizeStr}`,
          quality: resTag,
          url: directSrc,
          filename: mp4Name,
          container: "MP4",
          isDirectFile: true,
        });
      }

      // 6. MP3 Audio
      options.push({
        label: `${shortTitle}, MP3 Audio, Highest Quality`,
        fullLabel: `${cleanTitle}, MP3 Audio, ${durStr}Highest Quality (Extracted Audio)`,
        shortTitle: shortTitle,
        formatDisplay: "MP3 Audio",
        quality: "Audio",
        url: directSrc || targetInstagramUrl,
        filename: mp3Name,
        container: "MP3",
        isDirectFile: false,
      });

      // 7. Thumbnail / Cover Image
      const poster = mediaTarget.thumbnail || (vid && vid.poster);
      if (poster && isValidDownloadUrl(poster)) {
        options.push({
          label: `${shortTitle}, Thumbnail / Cover Image (High Quality JPG)`,
          fullLabel: `${cleanTitle}, Thumbnail / Cover Image (High Quality JPG)`,
          shortTitle: shortTitle,
          formatDisplay: "Cover JPG",
          quality: "Thumbnail",
          url: poster,
          container: "JPG",
          isDirectFile: true,
          filename: `${cleanTitle.slice(0, 45)}_cover.jpg`,
        });
      }
    } else if (hostname.includes("kick.com")) {
      // 3. Kick.com (VODs & Live Streams)
      const cleanTitle = title.length > 55 ? title.slice(0, 55) + "..." : title;
      const kickUrl = window.location.href;

      const kickRenditions = [
        { res: "1080p HD", kbps: "8089 kbps", quality: "1080p HD" },
        { res: "720p HD", kbps: "3402 kbps", quality: "720p HD" },
        { res: "480p", kbps: "1454 kbps", quality: "480p" },
        { res: "360p", kbps: "615 kbps", quality: "360p" },
        { res: "160p", kbps: "224 kbps", quality: "160p" },
      ];

      kickRenditions.forEach((rend) => {
        options.push({
          label: `${cleanTitle}, MP4 file, ${durStr}quality ${rend.res}, ${rend.kbps}`,
          quality: rend.quality,
          formatDisplay: `${rend.res} • MP4`,
          container: "MP4",
          url: kickUrl,
        });
        options.push({
          label: `${cleanTitle}, TS file, ${durStr}quality ${rend.res}, ${rend.kbps}`,
          quality: rend.quality,
          formatDisplay: `${rend.res} • TS`,
          container: "TS",
          url: kickUrl,
        });
      });
    } else {
      // 4. Any other website (Facebook, TikTok, Twitter/X, Vimeo, News & HTML5 video)
      const cleanTitle = title.length > 40 ? title.slice(0, 40) + "..." : title;
      const cleanTargetUrl = directSrc || stripRangeParams(mediaTarget.url || window.location.href);

      const vW = vid && vid.videoWidth ? vid.videoWidth : 0;
      const vH = vid && vid.videoHeight ? vid.videoHeight : 0;
      const maxDim = Math.max(vW, vH);
      const realQuality = maxDim >= 1080 ? "1080p HD" : maxDim >= 720 ? "720p HD" : maxDim >= 480 ? "480p" : maxDim > 0 ? `${maxDim}p` : "HD";
      const dimStr = vW > 0 && vH > 0 ? ` (${vW}x${vH})` : "";

      // Add all sniffed media streams from background webRequest
      const seen = new Set();
      sniffedStreams.forEach((st) => {
        if (st.isAudio) return;
        if (st.contentLength > 0 && st.contentLength < 50000) return;
        const sUrl = stripRangeParams(st.url);
        const bitStr = st.bitrate ? `${st.bitrate} kbps` : "";
        const key = `${st.quality}_${bitStr}_${sUrl.split("?")[0]}`;
        if (seen.has(key)) return;
        seen.add(key);

        const detailsList = [bitStr, st.sizeFormatted].filter(Boolean);
        const detailStr = detailsList.length > 0 ? `, ${detailsList.join(", ")}` : "";

        options.push({
          label: `${cleanTitle}, MP4 file, ${durStr}quality ${st.quality}${detailStr}`,
          quality: st.quality || "HD",
          formatDisplay: `${st.quality || 'HD'} • MP4${st.sizeFormatted ? ' (' + st.sizeFormatted + ')' : ''}`,
          url: sUrl,
          container: "MP4",
          isDirectFile: true,
          filename: `${cleanTitle}.mp4`,
        });
      });

      // Standard selectable resolutions (IDM style)
      options.push({
        label: `${cleanTitle}, Full HD 1080p MP4`,
        quality: "1080p",
        formatDisplay: "1080p • Full HD",
        url: cleanTargetUrl,
        container: "MP4",
        isDirectFile: false,
      });

      options.push({
        label: `${cleanTitle}, HD 720p MP4`,
        quality: "720p",
        formatDisplay: "720p • HD",
        url: cleanTargetUrl,
        container: "MP4",
        isDirectFile: false,
      });

      options.push({
        label: `${cleanTitle}, SD 480p MP4`,
        quality: "480p",
        formatDisplay: "480p • SD",
        url: cleanTargetUrl,
        container: "MP4",
        isDirectFile: false,
      });

      // Direct playing video
      if (directSrc) {
        options.push({
          label: `${cleanTitle}, Playing Video${dimStr} (Direct File)`,
          quality: realQuality,
          formatDisplay: `${realQuality}${dimStr} • Direct`,
          url: directSrc,
          container: "MP4",
          isDirectFile: true,
          filename: `${cleanTitle}.mp4`,
        });
      }

      // Audio only option
      options.push({
        label: `${cleanTitle}, MP3 Audio (Highest Quality)`,
        quality: "Audio",
        formatDisplay: "MP3 Audio",
        url: cleanTargetUrl,
        container: "MP3",
        isDirectFile: false,
      });

      // Cover thumbnail if present
      const poster = mediaTarget.thumbnail || (vid && vid.poster);
      if (poster && isValidDownloadUrl(poster)) {
        options.push({
          label: `${cleanTitle}, Thumbnail Cover Image (JPG)`,
          quality: "Thumbnail",
          formatDisplay: "Cover JPG",
          url: poster,
          container: "JPG",
          isDirectFile: true,
          filename: `${cleanTitle.slice(0, 45)}_cover.jpg`,
        });
      }
    }

    if (!stillOpen()) return;

    const renderDropdownPanel = (optionsToRender, isRefreshing) => {
      if (!stillOpen()) return;
      // Fill the already-open panel
      const dropdown = shell;
      dropdown.innerHTML = "";
      Object.assign(dropdown.style, {
        position: "fixed",
        zIndex: "2147483647",
        background: "#ffffff",
        border: "1px solid #717171",
        boxShadow: "0 4px 16px rgba(0,0,0,0.35)",
        borderRadius: "2px",
        minWidth: "380px",
        maxWidth: "640px",
        fontFamily: 'Tahoma, "Segoe UI", Arial, sans-serif',
        fontSize: "12px",
        color: "#0f172a",
        direction: "ltr",
        userSelect: "none",
        overflow: "hidden",
      });

      // Top Header: "Download all"
      const header = document.createElement("div");
      Object.assign(header.style, {
        padding: "6px 12px",
        fontWeight: "700",
        fontSize: "12px",
        color: "#1e293b",
        background: "#f1f5f9",
        borderBottom: "1px solid #cbd5e1",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "8px",
      });
      const headerTitle = document.createElement("span");
      headerTitle.innerText = "Download all";
      header.appendChild(headerTitle);
      if (isRefreshing) {
        const syncBadge = document.createElement("span");
        syncBadge.innerText = "Checking exact sizes…";
        Object.assign(syncBadge.style, {
          fontSize: "10px",
          fontWeight: "500",
          color: "#64748b",
        });
        header.appendChild(syncBadge);
      }
      header.addEventListener("mouseenter", () => {
        header.style.background = "#e2e8f0";
      });
      header.addEventListener("mouseleave", () => {
        header.style.background = "#f1f5f9";
      });
      header.addEventListener("click", (e) => {
        e.stopPropagation();
        hideDropdown();
        showToast(
          "ZR Downloader 🚀",
          `Starting Best Quality download in ZR Downloader...`,
          "success"
        );
        const isLowSpace = Boolean(cachedLowDiskSpace);
        const payload = {
          action: "SEND_TO_ZR",
          url: stripRangeParams(optionsToRender[0]?.url || mediaTarget.url),
          filename: mediaTarget.title,
          quality: "Best",
          autoStart: true,
          lowDiskSpace: isLowSpace,
        };
        try {
          chrome.runtime.sendMessage(payload, () => {
            if (chrome.runtime.lastError) {
              chrome.runtime.sendMessage({ ...payload, action: "SEND_TO_GDM" });
            }
          });
        } catch (err) {
          try {
            chrome.runtime.sendMessage({ ...payload, action: "SEND_TO_GDM" });
          } catch {}
        }
      });
      dropdown.appendChild(header);

      // Format list
      const list = document.createElement("div");
      list.style.maxHeight = "360px";
      list.style.overflowY = "auto";

      optionsToRender.forEach((opt, idx) => {
        const item = document.createElement("div");
        item.title = opt.fullLabel || opt.label;
        Object.assign(item.style, {
          padding: "6px 14px",
          fontSize: "12px",
          color: "#0f172a",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "10px",
          borderBottom: "1px solid #f1f5f9",
          transition: "background 0.08s ease, color 0.08s ease",
        });

        const titleSpan = document.createElement("span");
        titleSpan.innerText = `${idx + 1}. ${opt.shortTitle || getShortTitle(opt.filename || mediaTarget.title, 24)}`;
        Object.assign(titleSpan.style, {
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
          flex: "1",
          minWidth: "0",
          maxWidth: "250px",
        });

        const isPremBadge = Boolean(opt.isPremium || /premium/i.test(opt.quality || opt.formatDisplay || ""));
        const formatSpan = document.createElement("span");
        formatSpan.innerText = opt.formatDisplay || opt.quality;
        Object.assign(formatSpan.style, {
          whiteSpace: "nowrap",
          fontWeight: "700",
          fontSize: "11px",
          padding: "2px 8px",
          borderRadius: "4px",
          background: isPremBadge ? "rgba(217, 119, 6, 0.14)" : "rgba(37, 99, 235, 0.08)",
          color: isPremBadge ? "#b45309" : "#2563eb",
          flexShrink: "0",
        });

        item.appendChild(titleSpan);
        item.appendChild(formatSpan);

        item.addEventListener("mouseenter", () => {
          item.style.background = "#2563eb";
          item.style.color = "#ffffff";
          formatSpan.style.background = "rgba(255, 255, 255, 0.25)";
          formatSpan.style.color = "#ffffff";
        });
        item.addEventListener("mouseleave", () => {
          item.style.background = "transparent";
          item.style.color = "#0f172a";
          formatSpan.style.background = isPremBadge
            ? "rgba(217, 119, 6, 0.14)"
            : "rgba(37, 99, 235, 0.08)";
          formatSpan.style.color = isPremBadge ? "#b45309" : "#2563eb";
        });

        item.addEventListener("click", (e) => {
          e.stopPropagation();
          e.preventDefault();
          hideDropdown();
          if (activeGrabberButton) {
            activeGrabberButton.style.display = "none";
          }
          showToast(
            "ZR Downloader 🚀",
            `Starting ${opt.quality} download in ZR Downloader...`,
            "success"
          );
          const isLowSpace = Boolean(cachedLowDiskSpace);
          const payload = {
            action: "SEND_TO_ZR",
            url: stripRangeParams(opt.url || mediaTarget.url),
            filename: opt.filename || mediaTarget.title,
            quality: opt.quality,
            formatId: opt.formatId || "",
            autoStart: true,
            container:
              opt.container || (opt.quality === "Audio" ? "MP3" : "MP4"),
            referrer: window.location.href,
            isDirectFile: Boolean(opt.isDirectFile),
            lowDiskSpace: isLowSpace,
          };
          try {
            chrome.runtime.sendMessage(payload, () => {
              if (chrome.runtime.lastError) {
                chrome.runtime.sendMessage({ ...payload, action: "SEND_TO_GDM" });
              }
            });
          } catch (err) {
            try {
              chrome.runtime.sendMessage({ ...payload, action: "SEND_TO_GDM" });
            } catch {}
          }
        });

        list.appendChild(item);
      });

      dropdown.appendChild(list);

      // Position directly beneath the grabber button
      const btnRect = anchorBtn.getBoundingClientRect();
      const top = Math.min(window.innerHeight - 200, btnRect.bottom + 2);
      if (btnRect.left < window.innerWidth / 2) {
        dropdown.style.left = `${Math.max(8, btnRect.left)}px`;
        dropdown.style.right = "auto";
      } else {
        dropdown.style.right = `${Math.max(
          8,
          window.innerWidth - btnRect.right
        )}px`;
        dropdown.style.left = "auto";
      }
      dropdown.style.top = `${top}px`;

      if (!dropdown.isConnected) {
        (document.fullscreenElement || document.webkitFullscreenElement || document.body).appendChild(dropdown);
      }
      activeDropdown = dropdown;
    };

    renderDropdownPanel(options, Boolean(pendingRealFormats));

    if (pendingRealFormats) {
      pendingRealFormats.promise.then((lateInfo) => {
        if (!stillOpen()) return;
        if (lateInfo) {
          const lateBuilt = buildRealFormatOptions(
            lateInfo,
            pendingRealFormats.pageUrl,
            pendingRealFormats.title,
            pendingRealFormats.durStr
          );
          if (lateBuilt.length) {
            renderDropdownPanel(lateBuilt, false);
            return;
          }
        }
        renderDropdownPanel(options, false);
      });
    }
  }

  // ---------------------------------------------------------------------------
  // IDM-Style Floating Video Grabber Overlay (Draggable & Repositionable)
  // ---------------------------------------------------------------------------
  let isDraggingGrabber = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragStartBtnLeft = 0;
  let dragStartBtnTop = 0;
  let hasDragged = false;
  let userCustomOffset = null;

  function ensureGrabberContainer() {
    const fsEl =
      document.fullscreenElement || document.webkitFullscreenElement;
    if (fsEl) {
      if (activeGrabberButton && activeGrabberButton.parentElement !== fsEl) {
        fsEl.appendChild(activeGrabberButton);
      }
    } else {
      if (
        activeGrabberButton &&
        activeGrabberButton.parentElement !== document.body
      ) {
        document.body.appendChild(activeGrabberButton);
      }
    }
  }

  function createVideoGrabber() {
    if (activeGrabberButton && document.contains(activeGrabberButton)) return;

    const existing = document.getElementById("zr-floating-grabber");
    if (existing) {
      activeGrabberButton = existing;
      return;
    }

    const btn = document.createElement("div");
    btn.id = "zr-floating-grabber";
    btn.innerHTML = `
      <div style="display:flex;align-items:center;gap:5px;padding:0 2px;">
        <span style="display:inline-flex;align-items:center;color:#64748b;cursor:grab;font-size:12px;padding:0 2px;user-select:none;" title="Drag to move" id="zr-drag-handle">⠿</span>
        <span style="display:inline-block;color:#16a34a;font-size:12px;font-weight:900;text-shadow:0 1px 1px rgba(0,0,0,0.25);" id="zr-play-icon">▶</span>
        <span style="font-size:12px;font-weight:800;letter-spacing:0.2px;font-family:Arial,Tahoma,sans-serif;color:#dc2626;text-shadow:0 1px 0 #ffffff;white-space:nowrap;cursor:pointer;" id="zr-download-text">Download this video</span>
        <span style="font-size:10px;padding:0 4px;cursor:pointer;color:#334155;border-left:1px solid #cbd5e1;" title="Choose quality & format" id="zr-arrow-btn">▼</span>
        <span style="font-size:11px;color:#475569;font-weight:bold;margin-left:4px;padding:0 4px;border-left:1px solid #94a3b8;cursor:pointer;" title="ZR Downloader" id="zr-help-btn">?</span>
        <span style="font-size:11px;color:#475569;font-weight:bold;padding:0 2px;cursor:pointer;" title="Hide overlay" id="zr-close-btn">✕</span>
      </div>
    `;

    Object.assign(btn.style, {
      position: "fixed",
      zIndex: "2147483647",
      display: "none",
      alignItems: "center",
      padding: "4px 8px",
      borderRadius: "4px",
      background:
        "linear-gradient(180deg, #ffffff 0%, #f1f5f9 50%, #e2e8f0 100%)",
      border: "1px solid #64748b",
      boxShadow:
        "0 2px 8px rgba(0,0,0,0.4), inset 0 1px 0 rgba(255,255,255,0.9)",
      cursor: "pointer",
      userSelect: "none",
      transition: "opacity 0.15s ease, transform 0.1s ease",
      opacity: "0",
      pointerEvents: "none",
      direction: "ltr",
    });

    btn.addEventListener("mouseenter", () => {
      clearTimeout(hideTimer);
      btn.style.transform = "scale(1.02)";
      btn.style.borderColor = "#2563eb";
      btn.style.boxShadow =
        "0 3px 12px rgba(37,99,235,0.45), inset 0 1px 0 #ffffff";
    });

    btn.addEventListener("mouseleave", () => {
      btn.style.transform = "scale(1)";
      btn.style.borderColor = "#64748b";
      btn.style.boxShadow =
        "0 2px 8px rgba(0,0,0,0.4), inset 0 1px 0 rgba(255,255,255,0.9)";
      scheduleHide();
    });

    btn.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      if (
        e.target &&
        (e.target.id === "zr-close-btn" ||
          e.target.id === "zr-help-btn" ||
          e.target.id === "zr-arrow-btn")
      )
        return;
      isDraggingGrabber = true;
      hasDragged = false;
      dragStartX = e.clientX;
      dragStartY = e.clientY;
      const r = btn.getBoundingClientRect();
      dragStartBtnLeft = r.left;
      dragStartBtnTop = r.top;
      btn.style.cursor = "grabbing";
      clearTimeout(hideTimer);
    });

    btn.addEventListener("click", (e) => {
      if (hasDragged) {
        e.stopPropagation();
        e.preventDefault();
        return;
      }
      const target = e.target;
      if (target && target.id === "zr-close-btn") {
        e.stopPropagation();
        e.preventDefault();
        hideDropdown();
        btn.style.display = "none";
        isPermanentlyDismissed = true;
        setTimeout(() => {
          isPermanentlyDismissed = false;
        }, 8000);
        return;
      }
      if (target && target.id === "zr-help-btn") {
        e.stopPropagation();
        e.preventDefault();
        showToast(
          "ZR Downloader 🚀",
          "High-Speed HTTP & Media Extractor",
          "info"
        );
        return;
      }

      e.stopPropagation();
      e.preventDefault();

      const mediaTarget = extractMediaTarget(
        currentTargetVideo || currentTargetContainer
      );

      // Open format selector dropdown
      showFormatDropdown(btn, mediaTarget);
    });

    document.body.appendChild(btn);
    activeGrabberButton = btn;
  }

  document.addEventListener(
    "mousemove",
    (e) => {
      if (!isDraggingGrabber || !activeGrabberButton) return;
      const dx = e.clientX - dragStartX;
      const dy = e.clientY - dragStartY;
      if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
        hasDragged = true;
        clearTimeout(hideTimer);
        const newLeft = Math.max(
          8,
          Math.min(
            window.innerWidth - activeGrabberButton.offsetWidth - 8,
            dragStartBtnLeft + dx
          )
        );
        const newTop = Math.max(
          8,
          Math.min(
            window.innerHeight - activeGrabberButton.offsetHeight - 8,
            dragStartBtnTop + dy
          )
        );
        activeGrabberButton.style.left = `${newLeft}px`;
        activeGrabberButton.style.top = `${newTop}px`;
        activeGrabberButton.style.right = "auto";
        userCustomOffset = { relX: newLeft, relY: newTop, isAbsolute: true };
      }
    },
    { passive: true }
  );

  document.addEventListener("mouseup", () => {
    if (isDraggingGrabber && activeGrabberButton) {
      isDraggingGrabber = false;
      activeGrabberButton.style.cursor = "pointer";
    }
  });

  function scheduleHide() {
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (activeDropdown) return;
      if (activeGrabberButton && !activeGrabberButton.matches(":hover")) {
        activeGrabberButton.style.opacity = "0";
        activeGrabberButton.style.pointerEvents = "none";
        setTimeout(() => {
          if (
            activeGrabberButton &&
            activeGrabberButton.style.opacity === "0"
          ) {
            activeGrabberButton.style.display = "none";
          }
        }, 160);
      }
    }, 1800);
  }

  function positionGrabber(video, container) {
    if (isPermanentlyDismissed || isDraggingGrabber) return;
    createVideoGrabber();
    ensureGrabberContainer();

    currentTargetVideo = video;
    currentTargetContainer = container || (video ? video.parentElement : null);
    clearTimeout(hideTimer);

    // ALWAYS use the video element's bounding rect so the button stays anchored to the actual video
    const rect = video
      ? video.getBoundingClientRect()
      : container
      ? container.getBoundingClientRect()
      : null;

    if (!rect || rect.width < 100 || rect.height < 60) return;

    const fsEl =
      document.fullscreenElement || document.webkitFullscreenElement;

    if (fsEl && fsEl.contains(activeGrabberButton)) {
      activeGrabberButton.style.position = "absolute";
      activeGrabberButton.style.top = "18px";
      activeGrabberButton.style.right = "18px";
      activeGrabberButton.style.left = "auto";
    } else if (userCustomOffset) {
      activeGrabberButton.style.position = "fixed";
      let top = 12;
      let left = 12;
      if (userCustomOffset.isAbsolute) {
        top = Math.max(
          8,
          Math.min(window.innerHeight - 50, userCustomOffset.relY)
        );
        left = Math.max(
          8,
          Math.min(window.innerWidth - 180, userCustomOffset.relX)
        );
      } else {
        top = Math.max(
          8,
          Math.min(window.innerHeight - 50, rect.top + userCustomOffset.relY)
        );
        left = Math.max(
          8,
          Math.min(window.innerWidth - 180, rect.left + userCustomOffset.relX)
        );
      }
      activeGrabberButton.style.top = `${top}px`;
      activeGrabberButton.style.left = `${left}px`;
      activeGrabberButton.style.right = "auto";
    } else {
      activeGrabberButton.style.position = "fixed";
      const top = Math.max(
        12,
        Math.min(window.innerHeight - 50, rect.top + 10)
      );
      const right = Math.max(
        12,
        Math.min(window.innerWidth - 180, window.innerWidth - rect.right + 10)
      );

      activeGrabberButton.style.top = `${top}px`;
      activeGrabberButton.style.right = `${right}px`;
      activeGrabberButton.style.left = "auto";
    }

    activeGrabberButton.style.display = "flex";
    activeGrabberButton.style.opacity = "1";
    activeGrabberButton.style.pointerEvents = "auto";
    prefetchFormatsForPage();
  }

  // ---------------------------------------------------------------------------
  // Global Detection & Hover Logic
  // ---------------------------------------------------------------------------
  let lastCheckTime = 0;
  function handleGlobalPointerMove(e) {
    const now = Date.now();
    if (now - lastCheckTime < 60) return;
    lastCheckTime = now;

    if (isDraggingGrabber || isPermanentlyDismissed) return;

    const x = e.clientX;
    const y = e.clientY;

    // 1. Keep open if over button or dropdown
    if (activeGrabberButton && activeGrabberButton.style.display !== "none") {
      const btnRect = activeGrabberButton.getBoundingClientRect();
      const dropRect = activeDropdown
        ? activeDropdown.getBoundingClientRect()
        : null;

      const overBtn =
        x >= btnRect.left - 20 &&
        x <= btnRect.right + 20 &&
        y >= btnRect.top - 20 &&
        y <= btnRect.bottom + 20;

      const overDrop =
        dropRect &&
        x >= dropRect.left - 10 &&
        x <= dropRect.right + 10 &&
        y >= dropRect.top - 10 &&
        y <= dropRect.bottom + 10;

      if (overBtn || overDrop) {
        clearTimeout(hideTimer);
        return;
      }
    }

    const hostname = window.location.hostname;

    // 2. YouTube Special Fast-Path
    if (hostname.includes("youtube.com") || hostname.includes("youtu.be")) {
      const ytPlayer = document.querySelector(
        "#movie_player, .html5-video-player"
      );
      if (ytPlayer) {
        const ytRect = ytPlayer.getBoundingClientRect();
        if (
          x >= ytRect.left &&
          x <= ytRect.right &&
          y >= ytRect.top &&
          y <= ytRect.bottom &&
          ytRect.width > 200 &&
          ytRect.height > 120
        ) {
          const vid = ytPlayer.querySelector("video");
          if (vid) {
            positionGrabber(vid, ytPlayer);
            return;
          }
        }
      }
    }

    // 3. Instagram Special Fast-Path (covers Reels, Feed, Stories)
    if (hostname.includes("instagram.com")) {
      const igVideos = document.querySelectorAll("video");
      for (let i = 0; i < igVideos.length; i++) {
        const v = igVideos[i];
        const r = v.getBoundingClientRect();
        if (r.width < 100 || r.height < 60) continue;
        if (
          x >= r.left - 10 &&
          x <= r.right + 10 &&
          y >= r.top - 10 &&
          y <= r.bottom + 10
        ) {
          positionGrabber(v, v);
          return;
        }
      }
    }

    // 4. Scan all HTML5 video elements across any website
    const videos = document.querySelectorAll("video");
    let foundHovered = false;

    for (let i = 0; i < videos.length; i++) {
      const vid = videos[i];
      const rect = vid.getBoundingClientRect();
      if (rect.width < 120 || rect.height < 80) continue;

      if (
        x >= rect.left &&
        x <= rect.right &&
        y >= rect.top &&
        y <= rect.bottom
      ) {
        positionGrabber(vid, vid);
        foundHovered = true;
        break;
      }
    }

    if (!foundHovered) {
      scheduleHide();
    }
  }

  // Bind global pointer listeners
  window.addEventListener("mousemove", handleGlobalPointerMove, {
    passive: true,
  });
  window.addEventListener("pointermove", handleGlobalPointerMove, {
    passive: true,
  });

  // When ANY video starts playing, show the grabber
  window.addEventListener(
    "play",
    (e) => {
      if (e.target && e.target.tagName === "VIDEO") {
        const vid = e.target;
        positionGrabber(vid, vid);
        setTimeout(scheduleHide, 4000);
      }
    },
    true
  );

  // Fullscreen support
  document.addEventListener("fullscreenchange", ensureGrabberContainer);
  document.addEventListener("webkitfullscreenchange", ensureGrabberContainer);

  // Periodic check to attach to playing video on Instagram / social feeds
  setInterval(() => {
    if (window.location.hostname.includes("instagram.com")) {
      const playing = Array.from(document.querySelectorAll("video")).find(
        (v) => !v.paused && v.currentTime > 0
      );
      if (playing && (!activeGrabberButton || activeGrabberButton.style.display === "none")) {
        const r = playing.getBoundingClientRect();
        if (r.top >= 0 && r.bottom <= window.innerHeight) {
          positionGrabber(playing, playing);
          setTimeout(scheduleHide, 4000);
        }
      }
    }
    createVideoGrabber();
  }, 2500);

  // Message listener from background & popup
  chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.action === "GET_LAST_CLICKED_MEDIA") {
      const target = extractMediaTarget(
        currentTargetVideo || lastRightClickedElement
      );
      sendResponse({ url: target.url, title: target.title });
      return true;
    }

    if (request.action === "SHOW_TOAST") {
      showToast(request.title, request.message, request.status);
    }
  });

  // Modern In-Page Toast Notification
  function showToast(title, message, status = "info") {
    let container = document.getElementById("zr-toast-container");
    if (!container) {
      container = document.createElement("div");
      container.id = "zr-toast-container";
      Object.assign(container.style, {
        position: "fixed",
        bottom: "24px",
        right: "24px",
        zIndex: "2147483647",
        display: "flex",
        flexDirection: "column",
        gap: "8px",
        pointerEvents: "none",
        direction: "ltr",
      });
      document.body.appendChild(container);
    }

    const toast = document.createElement("div");
    Object.assign(toast.style, {
      display: "flex",
      alignItems: "center",
      gap: "10px",
      padding: "10px 14px",
      borderRadius: "8px",
      background: "#0f172a",
      color: "#f8fafc",
      boxShadow:
        "0 8px 24px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.1)",
      borderLeft:
        status === "success" ? "4px solid #10b981" : "4px solid #3b82f6",
      fontFamily:
        "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif",
      fontSize: "12px",
      transition: "all 0.25s ease",
      opacity: "0",
      transform: "translateY(12px)",
      pointerEvents: "auto",
    });

    toast.innerHTML = `
      <span style="font-size:18px;">${status === "success" ? "✅" : "🚀"}</span>
      <div>
        <div style="font-weight:700;font-size:12px;color:#ffffff;">${title}</div>
        <div style="font-size:11px;color:#cbd5e1;">${message}</div>
      </div>
    `;

    container.appendChild(toast);

    requestAnimationFrame(() => {
      toast.style.opacity = "1";
      toast.style.transform = "translateY(0)";
    });

    setTimeout(() => {
      toast.style.opacity = "0";
      toast.style.transform = "translateY(8px)";
      setTimeout(() => toast.remove(), 300);
    }, 3500);
  }

  // Proactively warm format cache on page load and YouTube SPA navigation
  setTimeout(prefetchFormatsForPage, 300);
  window.addEventListener("yt-navigate-finish", () => setTimeout(prefetchFormatsForPage, 150));
  window.addEventListener("yt-page-data-updated", () => setTimeout(prefetchFormatsForPage, 150));
  window.addEventListener("popstate", () => setTimeout(prefetchFormatsForPage, 150));
})();
