import type { ThemePref } from "./ipc";

const media = window.matchMedia("(prefers-color-scheme: dark)");
let current: ThemePref = "system";

function apply() {
  const dark = current === "dark" || (current === "system" && media.matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

media.addEventListener("change", apply);

/** Applies the user's theme preference; "system" tracks the OS live. */
export function setTheme(pref: ThemePref) {
  current = pref;
  apply();
}

apply();
