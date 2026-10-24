export type Theme = "system" | "light" | "dark";
export type Accent = "ember" | "ocean" | "forest" | "plum";
export type Mode = "light" | "dark";

export const ACCENTS: { value: Accent; label: string; swatch: string }[] = [
  { value: "ember", label: "Ember", swatch: "#c2410c" },
  { value: "ocean", label: "Ocean", swatch: "#0e7490" },
  { value: "forest", label: "Forest", swatch: "#15803d" },
  { value: "plum", label: "Plum", swatch: "#7c3aed" },
];

const CACHE_KEY = "parley.appearance";
const DARK_QUERY = "(prefers-color-scheme: dark)";

export const resolveMode = (theme: Theme, systemDark: boolean): Mode => (theme === "system" ? (systemDark ? "dark" : "light") : theme);

let current: { theme: Theme; accent: Accent } = { theme: "system", accent: "ember" };
let listening = false;

function paint() {
  const root = document.documentElement;
  root.dataset.mode = resolveMode(current.theme, window.matchMedia(DARK_QUERY).matches);
  root.dataset.accent = current.accent;
}

/** Applies the theme and accent to the page, follows the system when asked, and remembers them. */
export function applyAppearance(theme: Theme, accent: Accent) {
  current = { theme, accent };
  paint();
  if (!listening) {
    window.matchMedia(DARK_QUERY).addEventListener("change", paint);
    listening = true;
  }
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(current));
  } catch {
    // Only the first paint of the next launch is affected.
  }
}

/** Paints the last-used appearance before React renders, so a dark-mode launch never flashes light. */
export function initAppearance() {
  let saved: Partial<typeof current> = {};
  try {
    saved = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "{}");
  } catch {
    saved = {};
  }
  applyAppearance(saved.theme ?? "system", saved.accent ?? "ember");
}
