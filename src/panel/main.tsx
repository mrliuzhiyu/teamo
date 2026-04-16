import React from "react";
import ReactDOM from "react-dom/client";
import PanelApp from "./PanelApp";
import "../index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <PanelApp />
  </React.StrictMode>,
);
