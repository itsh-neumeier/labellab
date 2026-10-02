// App settings, kept in the encrypted settings file next to the program
// (`ll_core::settings`, ADR-036) instead of the webview's local storage.
// Values are strings, like the local storage they replace; saves are
// batched.

import * as api from "./api";

const SAVE_DELAY_MS = 400;

let store: Record<string, unknown> = {};
let timer: number | undefined;

/** Loads the settings file; takes over values from the old local storage once. */
export async function initSettings(): Promise<void> {
  try {
    store = await api.loadSettings();
  } catch {
    store = {};
  }
  if (Object.keys(store).length) return;
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key?.startsWith("labellab.")) store[key] = localStorage.getItem(key);
    }
    if (Object.keys(store).length) scheduleSave();
  } catch {
    // no old values
  }
}

export function getSetting(key: string): string | null {
  const v = store[key];
  return typeof v === "string" ? v : v == null ? null : String(v);
}

export function setSetting(key: string, value: string | null): void {
  if (value === null) delete store[key];
  else store[key] = value;
  scheduleSave();
}

function scheduleSave(): void {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    api.saveSettings(store).catch((e) => console.warn("settings not saved:", e));
  }, SAVE_DELAY_MS);
}
