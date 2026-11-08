import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { StandaloneDownloadWindow } from "./components/StandaloneDownloadWindow";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./index.css";

const searchParams = typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
const isPopupWindow = searchParams?.get("popup") === "true";
const popupTaskId = searchParams?.get("taskId");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      {isPopupWindow && popupTaskId ? (
        <StandaloneDownloadWindow taskId={popupTaskId} />
      ) : (
        <App />
      )}
    </ErrorBoundary>
  </React.StrictMode>
);
