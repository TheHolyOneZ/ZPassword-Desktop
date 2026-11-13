import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./lib/motion";
import "./styles/base.css";
import { initTheme } from "./lib/theme";

initTheme();

async function boot() {
  const { App } = await import("./app/App");
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

boot();
