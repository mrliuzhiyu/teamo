import React from "react";
import ReactDOM from "react-dom/client";
import PanelApp from "./PanelApp";
import { ToastProvider } from "../lib/toast";
import { ConfirmProvider } from "../lib/ConfirmDialog";
import "../index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ToastProvider>
      <ConfirmProvider>
        <PanelApp />
      </ConfirmProvider>
    </ToastProvider>
  </React.StrictMode>,
);
