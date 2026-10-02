// UI languages with their flags. Flags are small inline SVGs: emoji flags
// are not shown on Windows (Segoe UI Emoji has no flag glyphs).

export interface LangInfo {
  code: string;
  /** Name in the language itself. */
  name: string;
  /** Locale for dates and times. */
  locale: string;
  /** SVG content in a 30 × 20 view box. */
  flag: string;
}

const stripes = (dir: "h" | "v", colors: string[]): string =>
  colors
    .map((c, i) =>
      dir === "h"
        ? `<rect y="${(20 / colors.length) * i}" width="30" height="${20 / colors.length + 0.1}" fill="${c}"/>`
        : `<rect x="${(30 / colors.length) * i}" width="${30 / colors.length + 0.1}" height="20" fill="${c}"/>`,
    )
    .join("");

export const LANGS = [
  { code: "de", name: "Deutsch", locale: "de-DE", flag: stripes("h", ["#000", "#DD0000", "#FFCE00"]) },
  {
    code: "en",
    name: "English",
    locale: "en-GB",
    flag:
      '<rect width="30" height="20" fill="#012169"/>' +
      '<path d="M0 0L30 20M30 0L0 20" stroke="#fff" stroke-width="4"/>' +
      '<path d="M0 0L30 20M30 0L0 20" stroke="#C8102E" stroke-width="1.5"/>' +
      '<path d="M15 0V20M0 10H30" stroke="#fff" stroke-width="6"/>' +
      '<path d="M15 0V20M0 10H30" stroke="#C8102E" stroke-width="3.5"/>',
  },
  { code: "fr", name: "Français", locale: "fr-FR", flag: stripes("v", ["#002395", "#fff", "#ED2939"]) },
  { code: "es", name: "Español", locale: "es-ES", flag: stripes("h", ["#AA151B", "#F1BF00", "#F1BF00", "#AA151B"]) },
  { code: "it", name: "Italiano", locale: "it-IT", flag: stripes("v", ["#009246", "#fff", "#CE2B37"]) },
  { code: "nl", name: "Nederlands", locale: "nl-NL", flag: stripes("h", ["#AE1C28", "#fff", "#21468B"]) },
  { code: "pl", name: "Polski", locale: "pl-PL", flag: stripes("h", ["#fff", "#DC143C"]) },
  {
    code: "cs",
    name: "Čeština",
    locale: "cs-CZ",
    flag: stripes("h", ["#fff", "#D7141A"]) + '<path d="M0 0L15 10L0 20Z" fill="#11457E"/>',
  },
] as const satisfies readonly LangInfo[];

export type Lang = (typeof LANGS)[number]["code"];

export function isLang(code: unknown): code is Lang {
  return LANGS.some((l) => l.code === code);
}

export function langInfo(code: Lang): LangInfo {
  return LANGS.find((l) => l.code === code) ?? LANGS[0];
}

/** The flag as an inline SVG element (decorative). */
export function flagSvg(code: Lang): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 30 20");
  svg.setAttribute("class", "flag");
  svg.setAttribute("aria-hidden", "true");
  svg.innerHTML = langInfo(code).flag;
  return svg;
}

/**
 * Dropdown with flags (a native `<select>` cannot show images): a button
 * with the current flag and a list of all languages, keyboard operable.
 */
export function langPicker(host: HTMLElement, current: () => Lang, onPick: (code: Lang) => void): () => void {
  host.classList.add("lang-picker");
  const button = document.createElement("button");
  button.type = "button";
  button.setAttribute("aria-haspopup", "listbox");
  const list = document.createElement("ul");
  list.setAttribute("role", "listbox");
  list.hidden = true;
  host.replaceChildren(button, list);

  const options = LANGS.map((l) => {
    const li = document.createElement("li");
    li.setAttribute("role", "option");
    li.tabIndex = -1;
    li.dataset.code = l.code;
    const name = document.createElement("span");
    name.textContent = l.name;
    li.append(flagSvg(l.code), name);
    li.addEventListener("click", () => pick(l.code));
    return li;
  });
  list.append(...options);

  const render = () => {
    const code = current();
    const code2 = document.createElement("span");
    code2.textContent = code.toUpperCase();
    button.replaceChildren(flagSvg(code), code2);
    button.setAttribute("aria-label", `${langInfo(code).name} – Sprache / Language`);
    for (const li of options) li.setAttribute("aria-selected", String(li.dataset.code === code));
  };
  const close = (focus = false) => {
    list.hidden = true;
    button.setAttribute("aria-expanded", "false");
    if (focus) button.focus();
  };
  const pick = (code: Lang) => {
    close(true);
    if (code !== current()) onPick(code);
    render();
  };
  button.addEventListener("click", () => {
    if (!list.hidden) return close();
    list.hidden = false;
    button.setAttribute("aria-expanded", "true");
    (options.find((li) => li.dataset.code === current()) ?? options[0]).focus();
  });
  list.addEventListener("keydown", (e) => {
    const i = options.indexOf(document.activeElement as HTMLLIElement);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = (i + (e.key === "ArrowDown" ? 1 : options.length - 1)) % options.length;
      options[next].focus();
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (i >= 0) pick(options[i].dataset.code as Lang);
    } else if (e.key === "Escape" || e.key === "Tab") {
      close(e.key === "Escape");
    }
  });
  document.addEventListener("click", (e) => {
    if (!host.contains(e.target as Node)) close();
  });
  render();
  return render;
}
