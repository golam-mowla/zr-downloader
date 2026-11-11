import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { StandaloneDownloadWindow } from "./components/StandaloneDownloadWindow";
import { CompletionNotice } from "./components/CompletionNotice";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./index.css";

const searchParams = typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
const isPopupWindow = searchParams?.get("popup") === "true";
const popupTaskId = searchParams?.get("taskId");
const isNoticeWindow = searchParams?.get("notify") === "true";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      {isNoticeWindow ? (
        <CompletionNotice
          name={searchParams?.get("name") || "Download"}
          folder={searchParams?.get("folder") || ""}
        />
      ) : isPopupWindow && popupTaskId ? (
        <StandaloneDownloadWindow taskId={popupTaskId} />
      ) : (
        <App />
      )}
    </ErrorBoundary>
  </React.StrictMode>
);
