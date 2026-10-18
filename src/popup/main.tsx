import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/inter/opsz.css";
import "../index.css";
import "../lib/theme";
import { Popup } from "./Popup";

document.documentElement.classList.add("popup-page");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Popup />
  </StrictMode>,
);
