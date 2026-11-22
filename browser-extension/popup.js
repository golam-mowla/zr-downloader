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
const bridgeTokenInput = document.getElementById("bridge-token-input");
const btnSaveToken = document.getElementById("btn-save-token");
const bridgePortInput = document.getElementById("bridge-port-input");
const tokenStatusBadge = document.getElementById("token-status-badge");

let detectedUrl = "";

function updateTokenBadge(state) {
  if (!tokenStatusBadge) return;
  if (state === "paired") {
    tokenStatusBadge.textContent = "Paired ✓";
    tokenStatusBadge.style.background = "#065f46";
    tokenStatusBadge.style.color = "#34d399";
  } else if (state === "invalid") {
    tokenStatusBadge.textContent = "Token Required / Invalid";
    tokenStatusBadge.style.background = "#78350f";
    tokenStatusBadge.style.color = "#fcd34d";
  } else {
    tokenStatusBadge.textContent = "Offline";
    tokenStatusBadge.style.background = "#374151";
    tokenStatusBadge.style.color = "#9ca3af";
  }
}

// Check ZR Downloader Desktop App status on open (3 states: Offline, Token Invalid/Required, Paired)
function checkStatus() {
  chrome.runtime.sendMessage({ action: "CHECK_ZR_STATUS" }, (response) => {
    if (!response || !response.online) {
      statusPill.className = "status-pill offline";
      statusText.textContent = "Offline";
      updateTokenBadge("offline");
    } else if (!response.authenticated) {
      statusPill.className = "status-pill online";
      const port = response.port || (bridgePortInput ? bridgePortInput.value : 9666);
      statusText.textContent = `Online (:${port}) - Token Invalid`;
      updateTokenBadge("invalid");
    } else {
      statusPill.className = "status-pill online";
      const port = response.port || (bridgePortInput ? bridgePortInput.value : 9666);
      statusText.textContent = `Online (:${port}) - Paired ✓`;
      updateTokenBadge("paired");
      chrome.storage.local.get(["bridgeToken", "bridgePort"], (res) => {
        if (bridgeTokenInput && res.bridgeToken) {
          bridgeTokenInput.value = res.bridgeToken;
        }
        if (bridgePortInput && res.bridgePort) {
          bridgePortInput.value = res.bridgePort;
        }
      });
    }
  });
}

// Check stored preferences on popup load
chrome.storage.local.get(["interceptDownloads", "lowDiskSpace", "bridgeToken", "bridgePort"], (result) => {
  if (result.interceptDownloads !== undefined) {
    toggleInterception.checked = result.interceptDownloads;
  }
  if (result.lowDiskSpace !== undefined && toggleLowDiskSpace) {
    toggleLowDiskSpace.checked = result.lowDiskSpace;
  }
  if (bridgeTokenInput && result.bridgeToken) {
    bridgeTokenInput.value = result.bridgeToken;
  }
  if (bridgePortInput && result.bridgePort) {
    bridgePortInput.value = result.bridgePort;
  }
  checkStatus();
});

function saveBridgeConfig() {
  const token = (bridgeTokenInput ? bridgeTokenInput.value : "").trim();
  const port = parseInt(bridgePortInput ? bridgePortInput.value : "9666", 10) || 9666;
  chrome.storage.local.set({ bridgeToken: token, bridgePort: port }, () => {
    if (btnSaveToken) {
      btnSaveToken.textContent = "Saved!";
      setTimeout(() => {
        btnSaveToken.textContent = "Save";
      }, 1200);
    }
    checkStatus();
  });
}

if (btnSaveToken) {
  btnSaveToken.addEventListener("click", saveBridgeConfig);
}
if (bridgeTokenInput) {
  bridgeTokenInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") saveBridgeConfig();
  });
}
if (bridgePortInput) {
  bridgePortInput.addEventListener("change", saveBridgeConfig);
}

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
