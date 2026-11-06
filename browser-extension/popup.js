// ZR Downloader - Popup Script

const statusPill = document.getElementById("status-pill");
const statusDot = document.getElementById("status-dot");
const statusText = document.getElementById("status-text");
const currentUrlEl = document.getElementById("current-url");
const btnDownloadCurrent = document.getElementById("btn-download-current");
const customUrlInput = document.getElementById("custom-url-input");
const btnDownloadCustom = document.getElementById("btn-download-custom");
const toggleInterception = document.getElementById("toggle-interception");
const toggleLowDiskSpace = document.getElementById("toggle-low-disk-space");

let detectedUrl = "";

// Check ZR Downloader Desktop App status on open
function checkStatus() {
  chrome.runtime.sendMessage({ action: "CHECK_ZR_STATUS" }, (response) => {
    if (response && response.online) {
      statusPill.className = "status-pill online";
      statusText.textContent = "Engine Online (:9666)";
    } else {
      statusPill.className = "status-pill";
      statusText.textContent = "Offline / Closed";
    }
  });
}

// Check interception storage preference
chrome.storage.local.get(["interceptDownloads", "lowDiskSpace"], (result) => {
  if (result.interceptDownloads !== undefined) {
    toggleInterception.checked = result.interceptDownloads;
  }
  if (result.lowDiskSpace !== undefined && toggleLowDiskSpace) {
    toggleLowDiskSpace.checked = result.lowDiskSpace;
  }
});

toggleInterception.addEventListener("change", () => {
  chrome.storage.local.set({ interceptDownloads: toggleInterception.checked });
});

if (toggleLowDiskSpace) {
  toggleLowDiskSpace.addEventListener("change", () => {
    chrome.storage.local.set({ lowDiskSpace: toggleLowDiskSpace.checked });
  });
}

// Detect current active tab URL & media
chrome.tabs.query({ active: true, currentWindow: true }, async (tabs) => {
  if (!tabs || !tabs[0]) return;
  const activeTab = tabs[0];
  detectedUrl = activeTab.url || "";

  // Ask content script for detected media element URL
  try {
    const resp = await chrome.tabs.sendMessage(activeTab.id, { action: "GET_LAST_CLICKED_MEDIA" });
    if (resp && resp.url) {
      detectedUrl = resp.url;
    }
  } catch (e) {
    // Content script not ready or page restricted
  }

  currentUrlEl.textContent = detectedUrl || "No media detected on current tab";
  currentUrlEl.title = detectedUrl;
});

// Download current media button
btnDownloadCurrent.addEventListener("click", () => {
  if (!detectedUrl || detectedUrl.startsWith("chrome://")) return;
  btnDownloadCurrent.textContent = "Sending to ZR Engine...";
  const isLowSpace = toggleLowDiskSpace ? toggleLowDiskSpace.checked : false;
  chrome.runtime.sendMessage({ action: "SEND_TO_ZR", url: detectedUrl, lowDiskSpace: isLowSpace }, () => {
    btnDownloadCurrent.textContent = "✓ Sent to ZR Downloader!";
    setTimeout(() => {
      window.close();
    }, 800);
  });
});

// Send custom URL button
btnDownloadCustom.addEventListener("click", () => {
  const url = (customUrlInput.value || "").trim();
  if (!url) return;
  btnDownloadCustom.textContent = "...";
  const isLowSpace = toggleLowDiskSpace ? toggleLowDiskSpace.checked : false;
  chrome.runtime.sendMessage({ action: "SEND_TO_ZR", url, lowDiskSpace: isLowSpace }, () => {
    btnDownloadCustom.textContent = "Sent!";
    setTimeout(() => {
      window.close();
    }, 700);
  });
});

customUrlInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    btnDownloadCustom.click();
  }
});

checkStatus();
setInterval(checkStatus, 3000);
