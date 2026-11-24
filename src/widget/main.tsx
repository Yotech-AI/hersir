// GA-343, GA-349: the view's entry (mcp-app.html), built into one HTML file by `vite build --mode mcp-app`
// (vite-plugin-singlefile): every script, style and font inside it, nothing fetched. Hersir embeds that file and serves it
// to Claude Desktop as ui://hersir/app.html (src-tauri/src/tools/apps.rs).
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { loadFonts } from "./fonts";
import "../styles/tokens.css";
import "../styles/components.css";
import "../styles/app.css";
import "./widget.css";

loadFonts();
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
