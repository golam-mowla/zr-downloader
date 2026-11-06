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
      if (q.includes("60") || q.includes("50")) val += 0.5;
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

      let postTitle =
        caption || (author ? `${author} - Instagram Video` : "");
      if (!postTitle) {
        postTitle =
          document.title
            .replace(/ • Instagram photos and videos$/, "")
            .replace(/ \| Instagram$/, "")
            .trim() || "Instagram Video";
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
    if (activeDropdown) {
      activeDropdown.remove();
      activeDropdown = null;
    }
  }

  async function showFormatDropdown(anchorBtn, mediaTarget) {
    if (activeDropdown) {
      hideDropdown();
      return;
    }

    const activePlayingVid = Array.from(document.querySelectorAll("video")).find(
      (v) => !v.paused && v.currentTime > 0 && v.readyState >= 2
    );
    if (activePlayingVid) {
      currentTargetVideo = activePlayingVid;
      mediaTarget = extractMediaTarget(activePlayingVid);
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

    // 1. YouTube - Query REAL available qualities from player API, subtitles, and thumbnail
    if (hostname.includes("youtube.com") || hostname.includes("youtu.be")) {
      requestYouTubeData();
      let ytData = lastYouTubeData;
      if (!ytData || !ytData.qualities || ytData.qualities.length === 0) {
        ytData = parseYouTubeFromDom() || ytData;
      }

      let availableQualities = [];
      if (ytData && ytData.qualities && ytData.qualities.length > 0) {
        availableQualities = sortQualities(ytData.qualities);
      } else {
        const vH = vid && vid.videoHeight ? vid.videoHeight : 0;
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
        if (vH > 0) {
          availableQualities = allStandard.filter((q) => {
            const m = q.match(/(\d+)p/);
            return m && parseInt(m[1], 10) <= vH;
          });
        }
        if (availableQualities.length === 0) {
          availableQualities = ["360p", "240p", "144p"];
        }
      }

      // Add ONLY qualities that actually exist for this video
      availableQualities.forEach((q) => {
        options.push({
          label: `${title}, MP4 file, ${durStr}quality ${q}`,
          quality: q,
          url: mediaTarget.url || window.location.href,
          container: "MP4",
        });
      });

      // MP3 Audio
      options.push({
        label: `${title}, MP3 Audio, ${durStr}Highest Quality`,
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
          label: `${title}, Thumbnail (High Quality JPG Image)`,
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
            label: `${title}, Subtitle - ${sub.name} [${sub.languageCode.toUpperCase()}] (.vtt)`,
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
      // 2. Instagram (Reels & Feed)
      const cleanTitle = title.length > 55 ? title.slice(0, 55) + "..." : title;
      const targetInstagramUrl =
        mediaTarget.postUrl || mediaTarget.url || window.location.href;

      const vW = vid && vid.videoWidth ? vid.videoWidth : 0;
      const vH = vid && vid.videoHeight ? vid.videoHeight : 0;
      const maxDim = Math.max(vW, vH);

      let detectedQuality = "1080p HD";
      if (maxDim >= 1080) detectedQuality = "1080p HD";
      else if (maxDim >= 720) detectedQuality = "720p HD";
      else if (maxDim >= 480) detectedQuality = "480p";
      else if (maxDim > 0) detectedQuality = `${Math.min(vW, vH)}p`;

      options.push({
        label: `${cleanTitle}, MP4 file, ${durStr}quality ${detectedQuality} (Original HD Quality, with Sound)`,
        quality: detectedQuality,
        url: targetInstagramUrl,
        container: "MP4",
      });

      if (detectedQuality === "1080p HD") {
        options.push({
          label: `${cleanTitle}, MP4 file, ${durStr}quality 720p HD (with Sound)`,
          quality: "720p HD",
          url: targetInstagramUrl,
          container: "MP4",
        });
      }
      if (detectedQuality === "1080p HD" || detectedQuality === "720p HD") {
        options.push({
          label: `${cleanTitle}, MP4 file, ${durStr}quality 480p (with Sound)`,
          quality: "480p",
          url: targetInstagramUrl,
          container: "MP4",
        });
      }

      options.push({
        label: `${cleanTitle}, MP3 Audio, ${durStr}Highest Quality (Extracted Audio)`,
        quality: "Audio",
        url: targetInstagramUrl,
        container: "MP3",
      });

      const poster = mediaTarget.thumbnail || (vid && vid.poster);
      if (poster && isValidDownloadUrl(poster)) {
        options.push({
          label: `${cleanTitle}, Thumbnail / Cover Image (High Quality JPG)`,
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
        { res: "360p", kbps: "615 kbps", quality: "360p" },
        { res: "720p HD", kbps: "3402 kbps", quality: "720p HD" },
        { res: "1080p HD", kbps: "8089 kbps", quality: "1080p HD" },
        { res: "480p", kbps: "1454 kbps", quality: "480p" },
        { res: "160p", kbps: "224 kbps", quality: "160p" },
      ];

      kickRenditions.forEach((rend) => {
        options.push({
          label: `${cleanTitle}, TS file, ${durStr}quality ${rend.res}, ${rend.kbps}`,
          quality: rend.quality,
          container: "TS",
          url: kickUrl,
        });
        options.push({
          label: `${cleanTitle}, MP4 file, ${durStr}quality ${rend.res}, ${rend.kbps}`,
          quality: rend.quality,
          container: "MP4",
          url: kickUrl,
        });
      });
    } else {
      // 4. Any other website
      const cleanTitle = title.length > 40 ? title.slice(0, 40) + "..." : title;

      const validStreams = sniffedStreams
        .filter((st) => {
          if (st.isAudio) return false;
          if (st.contentLength > 0 && st.contentLength < 50000) return false;
          if (directSrc) {
            const cleanSt = stripRangeParams(st.url);
            if (cleanSt === directSrc) return true;
            if (directPath) {
              try {
                if (new URL(cleanSt).pathname === directPath) return true;
              } catch (e) {}
            }
            return false;
          }
          return true;
        })
        .map((st) => ({
          ...st,
          url: stripRangeParams(st.url),
        }));

      validStreams.sort(
        (a, b) =>
          (b.bitrate || b.contentLength || 0) -
          (a.bitrate || a.contentLength || 0)
      );

      const seen = new Set();
      validStreams.forEach((st) => {
        const bitStr = st.bitrate ? `${st.bitrate} kbps` : "";
        const key = `${st.quality}_${bitStr}_${st.url}`;
        if (seen.has(key)) return;
        seen.add(key);

        const detailsList = [bitStr, st.sizeFormatted].filter(Boolean);
        const detailStr =
          detailsList.length > 0 ? `, ${detailsList.join(", ")}` : "";

        options.push({
          label: `${cleanTitle}, MP4 file, ${durStr}quality ${st.quality}${detailStr}`,
          quality: st.quality,
          url: st.url,
        });
      });

      if (options.length === 0) {
        const cleanTargetUrl =
          directSrc || stripRangeParams(mediaTarget.url || window.location.href);
        options.push(
          {
            label: `${cleanTitle}, MP4 file, Best Available HD Quality`,
            quality: "Best",
            url: cleanTargetUrl,
          },
          {
            label: `${cleanTitle}, MP4 file, quality 1080p HD`,
            quality: "1080p HD",
            url: cleanTargetUrl,
          },
          {
            label: `${cleanTitle}, MP4 file, quality 720p HD`,
            quality: "720p HD",
            url: cleanTargetUrl,
          },
          {
            label: `${cleanTitle}, MP4 file, quality 480p`,
            quality: "480p",
            url: cleanTargetUrl,
          },
          {
            label: `${cleanTitle}, MP3 Audio`,
            quality: "Audio",
            url: cleanTargetUrl,
          }
        );
      }
    }

    // Create dropdown container
    const dropdown = document.createElement("div");
    dropdown.id = "zr-format-dropdown";
    Object.assign(dropdown.style, {
      position: "fixed",
      zIndex: "2147483647",
      background: "#ffffff",
      border: "1px solid #717171",
      boxShadow: "0 4px 16px rgba(0,0,0,0.35)",
      borderRadius: "2px",
      minWidth: "340px",
      maxWidth: "560px",
      fontFamily: 'Tahoma, "Segoe UI", Arial, sans-serif',
      fontSize: "12px",
      color: "#0f172a",
      direction: "ltr",
      userSelect: "none",
      overflow: "hidden",
    });

    // Top Header: "Download all"
    const header = document.createElement("div");
    header.innerText = "Download all";
    Object.assign(header.style, {
      padding: "6px 12px",
      fontWeight: "700",
      fontSize: "12px",
      color: "#1e293b",
      background: "#f1f5f9",
      borderBottom: "1px solid #cbd5e1",
      cursor: "pointer",
    });
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
      chrome.storage.local.get(["lowDiskSpace"], (st) => {
        const isLowSpace = Boolean(st && st.lowDiskSpace);
        chrome.runtime.sendMessage({
          action: "SEND_TO_ZR",
          url: stripRangeParams(options[0]?.url || mediaTarget.url),
          filename: mediaTarget.title,
          quality: "Best",
          autoStart: true,
          lowDiskSpace: isLowSpace,
        });
      });
    });
    dropdown.appendChild(header);

    // Format list
    const list = document.createElement("div");
    list.style.maxHeight = "360px";
    list.style.overflowY = "auto";

    options.forEach((opt, idx) => {
      const item = document.createElement("div");
      item.innerText = `${idx + 1}. ${opt.label}`;
      item.title = opt.label;
      Object.assign(item.style, {
        padding: "5px 12px",
        fontSize: "11.5px",
        color: "#0f172a",
        cursor: "pointer",
        whiteSpace: "nowrap",
        overflow: "hidden",
        textOverflow: "ellipsis",
        borderBottom: "1px solid #f1f5f9",
        transition: "background 0.08s ease, color 0.08s ease",
      });

      item.addEventListener("mouseenter", () => {
        item.style.background = "#2563eb";
        item.style.color = "#ffffff";
      });
      item.addEventListener("mouseleave", () => {
        item.style.background = "transparent";
        item.style.color = "#0f172a";
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
        chrome.storage.local.get(["lowDiskSpace"], (st) => {
          const isLowSpace = Boolean(st && st.lowDiskSpace);
          chrome.runtime.sendMessage({
            action: "SEND_TO_ZR",
            url: stripRangeParams(opt.url || mediaTarget.url),
            filename: opt.filename || mediaTarget.title,
            quality: opt.quality,
            autoStart: true,
            container:
              opt.container || (opt.quality === "Audio" ? "MP3" : "MP4"),
            referrer: window.location.href,
            isDirectFile: Boolean(opt.isDirectFile),
            lowDiskSpace: isLowSpace,
          });
        });
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

    const container =
      document.fullscreenElement ||
      document.webkitFullscreenElement ||
      document.body;
    container.appendChild(dropdown);
    activeDropdown = dropdown;
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
})();
