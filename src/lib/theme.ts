export type Theme = "steel" | "aluminium" | "system";

const KEY = "zp.theme";
const light = typeof window !== "undefined" && "matchMedia" in window ? window.matchMedia("(prefers-color-scheme: light)") : null;

export function loadTheme(): Theme {
  try {
    const t = localStorage.getItem(KEY);
    return t === "aluminium" || t === "system" ? t : "steel";
  } catch {
    return "steel";
  }
}

function apply(theme: Theme) {
  const aluminium = theme === "aluminium" || (theme === "system" && !!light?.matches);
  if (aluminium) document.documentElement.dataset.theme = "aluminium";
  else delete document.documentElement.dataset.theme;
}

export function saveTheme(theme: Theme) {
  try {
    localStorage.setItem(KEY, theme);
  } catch {
  }
  apply(theme);
}

export function initTheme() {
  apply(loadTheme());
  light?.addEventListener("change", () => apply(loadTheme()));
}
