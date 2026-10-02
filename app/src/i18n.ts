// Minimal i18n: flat key -> string dictionaries, `{name}` placeholders.
// German is the default (`AGENTS.md`); missing texts in other languages
// fall back to English, then German.
import cs from "./i18n/cs.json";
import de from "./i18n/de.json";
import en from "./i18n/en.json";
import es from "./i18n/es.json";
import fr from "./i18n/fr.json";
import it from "./i18n/it.json";
import nl from "./i18n/nl.json";
import pl from "./i18n/pl.json";
import { isLang, type Lang } from "./langs";
import { getSetting, setSetting } from "./settings";

export type { Lang } from "./langs";
type Dict = Record<string, string>;

const dicts: Record<Lang, Dict> = { de, en, fr, es, it, nl, pl, cs };
const STORAGE_KEY = "labellab.lang";

// German is the project default; the webview's `navigator.language`
// isn't a reliable system-language signal on Linux (WebKitGTK).
let lang: Lang = "de";

/** Takes the saved language (call after the settings are loaded). */
export function loadLang(): void {
  const stored = getSetting(STORAGE_KEY);
  if (isLang(stored)) lang = stored;
}

export function currentLang(): Lang {
  return lang;
}

/** Switches the UI language and remembers the user's choice. */
export function setLang(next: Lang): void {
  setSetting(STORAGE_KEY, next);
  applyLang(next);
}

/** Switches the UI language without persisting it (startup detection). */
export function applyLang(next: Lang): void {
  lang = next;
  document.documentElement.lang = next;
  applyStatic();
}

/** Text for `key` in the current language, else English, else German. */
function lookup(key: string): string | undefined {
  return dicts[lang][key] ?? dicts.en[key] ?? dicts.de[key];
}

/** Looks up `key` (falls back to English, German, then the key itself). */
export function t(key: string, vars: Record<string, string | number> = {}): string {
  const text = lookup(key) ?? key;
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
    const text = lookup(key);
    if (text) return detail ? `${text} (${detail})` : text;
    return detail ?? code;
  }
  return String(e);
}
