// ZR Downloader - YouTube Main World Helper Script
// Runs in the MAIN world to access movie_player API and window.ytInitialPlayerResponse directly without CSP restrictions.

(() => {
  function getPlayer() {
    return document.getElementById("movie_player") || document.querySelector(".html5-video-player");
  }

  function extractYouTubeInfo() {
    const player = getPlayer();
    let resp = null;
    if (player && typeof player.getPlayerResponse === "function") {
      try {
        resp = player.getPlayerResponse();
      } catch (e) {}
    }
    if (!resp && window.ytInitialPlayerResponse) {
      resp = window.ytInitialPlayerResponse;
    }

    let qualityLevels = [];
    if (player && typeof player.getAvailableQualityLevels === "function") {
      try {
        qualityLevels = player.getAvailableQualityLevels();
      } catch (e) {}
    }

    let qualityData = [];
    if (player && typeof player.getAvailableQualityData === "function") {
      try {
        const qd = player.getAvailableQualityData();
        if (Array.isArray(qd)) qualityData = qd;
      } catch (e) {}
    }

    if (!resp && (!qualityLevels || qualityLevels.length === 0) && qualityData.length === 0) {
      return null;
    }

    // Extract actual available video qualities from adaptiveFormats & formats
    const qualities = new Set();
    const streamMeta = {}; // label -> { height, width, bitrate, size, fps }

    if (qualityData.length > 0) {
      for (const item of qualityData) {
        if (!item) continue;
        const lbl = item.qualityLabel || "";
        if (/premium/i.test(lbl)) {
          qualities.add(lbl.trim());
        }
      }
    }

    if (resp && resp.streamingData) {
      const allStreams = [
        ...(resp.streamingData.formats || []),
        ...(resp.streamingData.adaptiveFormats || []),
      ];

      for (const fmt of allStreams) {
        if (!fmt.qualityLabel) continue;
        const qLabel = fmt.qualityLabel.trim();
        qualities.add(qLabel);

        const currentMeta = streamMeta[qLabel] || {};
        if (fmt.height && (!currentMeta.height || fmt.height > currentMeta.height)) {
          currentMeta.height = fmt.height;
        }
        if (fmt.width) currentMeta.width = fmt.width;
        if (fmt.fps) currentMeta.fps = fmt.fps;
        if (fmt.bitrate && (!currentMeta.bitrate || fmt.bitrate > currentMeta.bitrate)) {
          currentMeta.bitrate = Math.round(fmt.bitrate / 1000);
        }
        if (fmt.contentLength) {
          currentMeta.contentLength = parseInt(fmt.contentLength, 10);
        }
        streamMeta[qLabel] = currentMeta;
      }
    }

    // Fallback: If streamingData wasn't present, map getAvailableQualityData / availableQualityLevels
    if (qualities.size === 0 && qualityData.length > 0) {
      for (const item of qualityData) {
        if (item && item.qualityLabel && item.quality !== "auto") {
          qualities.add(String(item.qualityLabel).trim());
        }
      }
    }
    if ((qualities.size === 0 || qualities.size <= 2) && Array.isArray(qualityLevels) && qualityLevels.length > 0) {
      const qMap = {
        highres: "4K 2160p",
        hd2880: "2880p",
        hd2160: "2160p (4K)",
        hd1440: "1440p (2K)",
        hd1080: "1080p HD",
        hd720: "720p HD",
        large: "480p",
        medium: "360p",
        small: "240p",
        tiny: "144p",
      };
      for (const q of qualityLevels) {
        if (q && q !== "auto" && qMap[q]) {
          qualities.add(qMap[q]);
        }
      }
    }

    // Subtitles / Captions
    const subtitles = [];
    try {
      const tracks =
        resp?.captions?.playerCaptionsTracklistRenderer?.captionTracks || [];
      for (const tr of tracks) {
        if (!tr || !tr.baseUrl) continue;
        const name =
          tr.name?.simpleText ||
          tr.name?.runs?.[0]?.text ||
          tr.languageCode ||
          "Subtitle";
        subtitles.push({
          name,
          languageCode: tr.languageCode || "en",
          baseUrl: tr.baseUrl,
          isAuto: tr.kind === "asr",
        });
      }
    } catch (e) {}

    // Thumbnails
    let bestThumbnail = "";
    try {
      const thumbs = resp?.videoDetails?.thumbnail?.thumbnails || [];
      if (thumbs.length > 0) {
        bestThumbnail = thumbs[thumbs.length - 1].url;
      }
    } catch (e) {}

    const videoId =
      resp?.videoDetails?.videoId ||
      new URLSearchParams(window.location.search).get("v") ||
      "";

    if (!bestThumbnail && videoId) {
      bestThumbnail = `https://i.ytimg.com/vi/${videoId}/maxresdefault.jpg`;
    }

    // Title & duration
    let title = resp?.videoDetails?.title || "";
    if (!title) {
      title = document.title.replace(/ - YouTube$/, "").trim();
    }
    const duration = parseInt(resp?.videoDetails?.lengthSeconds, 10) || 0;

    return {
      title,
      videoId,
      duration,
      qualities: Array.from(qualities),
      streamMeta,
      subtitles,
      thumbnail: bestThumbnail,
    };
  }

  function broadcastData() {
    const data = extractYouTubeInfo();
    if (data) {
      window.dispatchEvent(
        new CustomEvent("ZR_RESPONSE_YT_DATA", { detail: data })
      );
    }
  }

  // Answer requests from content script
  window.addEventListener("ZR_REQUEST_YT_DATA", () => {
    broadcastData();
  });

  // Watch for SPA YouTube player changes
  window.addEventListener("yt-navigate-finish", () => {
    setTimeout(broadcastData, 300);
    setTimeout(broadcastData, 1000);
  });
  window.addEventListener("yt-player-updated", broadcastData);
  document.addEventListener("playback-quality-change", broadcastData);

  // Initial broadcast after DOM ready
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => {
      setTimeout(broadcastData, 600);
    });
  } else {
    setTimeout(broadcastData, 600);
  }
})();
