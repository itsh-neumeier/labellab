// Minimal i18n: flat key -> string dictionaries, `{name}` placeholders.
// German is the default (`AGENTS.md`), English the second language.
import de from "./i18n/de.json";
import en from "./i18n/en.json";

export type Lang = "de" | "en";
type Dict = Record<string, string>;

const dicts: Record<Lang, Dict> = { de, en };
const STORAGE_KEY = "labellab.lang";

function initialLang(): Lang {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "de" || stored === "en") return stored;
  } catch {
    // storage unavailable: use the default
  }
  // German is the project default; the webview's `navigator.language`
  // isn't a reliable system-language signal on Linux (WebKitGTK).
  return "de";
}

let lang: Lang = initialLang();

export function currentLang(): Lang {
  return lang;
}

/** Switches the UI language and remembers the user's choice. */
export function setLang(next: Lang): void {
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // not persisted, still switched for this session
  }
  applyLang(next);
}

/** Switches the UI language without persisting it (startup detection). */
export function applyLang(next: Lang): void {
  lang = next;
  document.documentElement.lang = next;
  applyStatic();
}

/** Looks up `key` (falls back to German, then to the key itself). */
export function t(key: string, vars: Record<string, string | number> = {}): string {
  const text = dicts[lang][key] ?? dicts.de[key] ?? key;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => String(vars[name] ?? `{${name}}`));
}

/** Fills elements marked with `data-i18n`, `data-i18n-title`, `data-i18n-placeholder`. */
export function applyStatic(root: ParentNode = document): void {
  root.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    el.textContent = t(el.dataset.i18n!);
  });
  root.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => {
    el.title = t(el.dataset.i18nTitle!);
  });
  root.querySelectorAll<HTMLInputElement>("[data-i18n-placeholder]").forEach((el) => {
    el.placeholder = t(el.dataset.i18nPlaceholder!);
  });
}

/**
 * User-facing text for an error from the backend. Backend commands reject
 * with `{ code, detail }`; known codes get a translated explanation
 * (`error.<code>`), the raw detail is kept in parentheses for diagnosis.
 */
export function errorText(e: unknown): string {
  if (e && typeof e === "object" && "code" in e) {
    const { code, detail } = e as { code: string; detail?: string };
    const key = `error.${code}`;
    const text = dicts[lang][key] ?? dicts.de[key];
    if (text) return detail ? `${text} (${detail})` : text;
    return detail ?? code;
  }
  return String(e);
}
