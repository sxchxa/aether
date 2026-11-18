// Thème de l'interface (sombre, clair ou celui de Windows) et contraste renforcé.
// Appliqué sur <html> avant le premier rendu (aucun flash), mémorisé localement.

export type ThemePref = "dark" | "light" | "system";

const THEME_KEY = "aether.theme";
const CONTRAST_KEY = "aether.contrast";

function read(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    /* préférence non mémorisée */
  }
}

export function getThemePref(): ThemePref {
  const v = read(THEME_KEY);
  return v === "light" || v === "system" ? v : "dark";
}

export function getHighContrast(): boolean {
  return read(CONTRAST_KEY) === "1";
}

const media = () => window.matchMedia("(prefers-color-scheme: light)");

/** Applique le thème et le contraste à la page. */
export function applyTheme(pref: ThemePref = getThemePref(), contrast: boolean = getHighContrast()): void {
  const light = pref === "light" || (pref === "system" && media().matches);
  const root = document.documentElement;
  root.dataset.theme = light ? "light" : "dark";
  root.dataset.contrast = contrast ? "high" : "normal";
  root.style.colorScheme = light ? "light" : "dark";
}

export function setThemePref(pref: ThemePref): void {
  write(THEME_KEY, pref);
  applyTheme(pref);
}

export function setHighContrast(on: boolean): void {
  write(CONTRAST_KEY, on ? "1" : "0");
  applyTheme(undefined, on);
}

/** Suit le thème de Windows en direct lorsque « Système » est choisi. */
export function watchSystemTheme(): void {
  media().addEventListener("change", () => {
    if (getThemePref() === "system") applyTheme();
  });
}
