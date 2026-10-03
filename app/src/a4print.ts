// Printing labels on A4 paper with a normal (office) printer: the chosen
// sheets are rendered through the normal render path at a high
// resolution, coloured like the tape (or in custom colours), laid out on
// A4 pages and printed with the system print dialog. A test page with
// rulers checks the scale; per-axis correction factors fix printers that
// scale (ADR-037).

/** A4 in mm and the margin around the content. */
export const PAGE_W = 210;
export const PAGE_H = 297;
const MARGIN = 10;
const HEADER_H = 14;
/** Render resolution multiplier (180 dpi × 4 = 720 dpi). */
export const RENDER_SCALE = 4;
const DOTS_PER_MM = (180 * RENDER_SCALE) / 25.4;

export interface A4Label {
  name: string;
  /** Base64 PNG mask of the printable area (ink opaque). */
  png: string;
  /** Tape width and printable height across it, in mm. */
  tapeMm: number;
  printableMm: number;
  /** Own colours of this label (pipe markers), overriding the options. */
  background?: string | null;
  ink?: string;
  /** Second mask (e.g. arrow tips) drawn in `extra` over the ink. */
  extraPng?: string;
  extra?: string;
}

export interface A4Options {
  title: string;
  header: boolean;
  gray: boolean;
  /** Tape (background) and ink colours, CSS. `null` background = none. */
  background: string | null;
  ink: string;
  /** Thin cut line around every label. */
  outline: boolean;
  gapMm: number;
  /** Correction factors (1 = 100 %), see the test page. */
  scaleX: number;
  scaleY: number;
  /** Logo image URL for the header. */
  logo: string;
  headerText: (page: number, pages: number) => string;
}

interface Placed {
  img: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

async function loadImage(src: string): Promise<HTMLImageElement> {
  const img = new Image();
  img.src = src;
  await img.decode();
  return img;
}

/**
 * The label as it looks on tape: full tape width, background colour, ink
 * colour, turned by 90° if `rotate` (for labels longer than the page is wide).
 */
async function tapeImage(
  label: A4Label,
  options: Pick<A4Options, "background" | "ink">,
  rotate: boolean,
): Promise<{ url: string; lengthMm: number }> {
  const opts = {
    background: label.background !== undefined ? label.background : options.background,
    ink: label.ink ?? options.ink,
  };
  const mask = await loadImage(`data:image/png;base64,${label.png}`);
  const tapePx = Math.round(label.tapeMm * DOTS_PER_MM);
  const top = Math.round(((label.tapeMm - label.printableMm) / 2) * DOTS_PER_MM);
  const length = mask.naturalWidth;
  const canvas = document.createElement("canvas");
  canvas.width = rotate ? tapePx : length;
  canvas.height = rotate ? length : tapePx;
  const ctx = canvas.getContext("2d")!;
  if (rotate) {
    ctx.translate(tapePx, 0);
    ctx.rotate(Math.PI / 2);
  }
  if (opts.background) {
    ctx.fillStyle = opts.background;
    ctx.fillRect(0, 0, length, tapePx);
  }
  // Ink: the mask's opaque pixels in the ink colour.
  const ink = document.createElement("canvas");
  ink.width = length;
  ink.height = mask.naturalHeight;
  const ictx = ink.getContext("2d")!;
  ictx.drawImage(mask, 0, 0);
  ictx.globalCompositeOperation = "source-in";
  ictx.fillStyle = opts.ink;
  ictx.fillRect(0, 0, ink.width, ink.height);
  ctx.drawImage(ink, 0, top);
  if (label.extraPng && label.extra) {
    const extraMask = await loadImage(`data:image/png;base64,${label.extraPng}`);
    const extra = document.createElement("canvas");
    extra.width = extraMask.naturalWidth;
    extra.height = extraMask.naturalHeight;
    const ectx = extra.getContext("2d")!;
    ectx.drawImage(extraMask, 0, 0);
    ectx.globalCompositeOperation = "source-in";
    ectx.fillStyle = label.extra;
    ectx.fillRect(0, 0, extra.width, extra.height);
    ctx.drawImage(extra, 0, top);
  }
  return { url: canvas.toDataURL("image/png"), lengthMm: length / DOTS_PER_MM };
}

/** Shelf packing of `items` (w × h in mm) onto pages of `areaW` × `areaH`. */
export function packShelves(
  items: { w: number; h: number }[],
  areaW: number,
  areaH: number,
  gap: number,
): { index: number; page: number; x: number; y: number }[] {
  const out: { index: number; page: number; x: number; y: number }[] = [];
  let [page, x, y, shelf] = [0, 0, 0, 0];
  items.forEach((it, index) => {
    if (x > 0 && x + it.w > areaW) {
      x = 0;
      y += shelf + gap;
      shelf = 0;
    }
    if (y > 0 && y + it.h > areaH) {
      page++;
      x = 0;
      y = 0;
      shelf = 0;
    }
    out.push({ index, page, x, y });
    x += it.w + gap;
    shelf = Math.max(shelf, it.h);
  });
  return out;
}

function header(opts: A4Options, page: number, pages: number): HTMLElement {
  const head = document.createElement("div");
  head.className = "a4-header";
  const logo = document.createElement("img");
  logo.src = opts.logo;
  const name = document.createElement("strong");
  name.textContent = "LabelLab";
  const text = document.createElement("span");
  text.textContent = opts.headerText(page, pages);
  head.append(logo, name, text);
  return head;
}

function pageElement(opts: A4Options, page: number, pages: number): { page: HTMLElement; body: HTMLElement } {
  const el = document.createElement("div");
  el.className = `a4-page${opts.gray ? " gray" : ""}`;
  // Correction for printers that scale: the whole content is stretched.
  const inner = document.createElement("div");
  inner.className = "a4-inner";
  inner.style.transform = `scale(${opts.scaleX}, ${opts.scaleY})`;
  if (opts.header) inner.append(header(opts, page, pages));
  const body = document.createElement("div");
  body.className = "a4-body";
  body.style.top = `${MARGIN + (opts.header ? HEADER_H : 0)}mm`;
  inner.append(body);
  el.append(inner);
  return { page: el, body };
}

/** A4 pages (DOM) with the labels, ready for preview and printing. */
export async function buildPages(labels: A4Label[], opts: A4Options): Promise<HTMLElement[]> {
  const areaW = PAGE_W - 2 * MARGIN;
  const areaH = PAGE_H - 2 * MARGIN - (opts.header ? HEADER_H : 0);
  const items: Placed[] = [];
  // Copies share one rendered image (colouring at 720 dpi is the slow part).
  const cache = new Map<string, Placed>();
  for (const label of labels) {
    const key = `${label.tapeMm}|${label.png}|${label.background}|${label.ink}|${label.extra}|${label.extraPng}`;
    let item = cache.get(key);
    if (!item) {
      const length = (await loadImage(`data:image/png;base64,${label.png}`)).naturalWidth / DOTS_PER_MM;
      // Too long for the page width: print along the page height instead.
      const rotate = length > areaW && length <= areaH;
      const { url, lengthMm } = await tapeImage(label, opts, rotate);
      item = rotate ? { img: url, x: 0, y: 0, w: label.tapeMm, h: lengthMm } : { img: url, x: 0, y: 0, w: lengthMm, h: label.tapeMm };
      cache.set(key, item);
    }
    items.push(item);
  }
  const placed = packShelves(items, areaW, areaH, opts.gapMm);
  const pages = Math.max(1, ...placed.map((p) => p.page + 1));
  const out = Array.from({ length: pages }, (_, i) => pageElement(opts, i + 1, pages));
  for (const p of placed) {
    const it = items[p.index];
    const img = document.createElement("img");
    img.src = it.img;
    img.className = `a4-label${opts.outline ? " outline" : ""}`;
    Object.assign(img.style, { left: `${p.x}mm`, top: `${p.y}mm`, width: `${it.w}mm`, height: `${it.h}mm` });
    out[p.page].body.append(img);
  }
  return out.map((o) => o.page);
}

/** Test page: rulers and squares to measure how exactly the printer prints. */
export function testPage(opts: A4Options, labels: { hint: string; horizontal: string; vertical: string }): HTMLElement {
  const { page, body } = pageElement(opts, 1, 1);
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", "190mm");
  svg.setAttribute("height", "240mm");
  svg.setAttribute("viewBox", "0 0 190 240");
  const add = (tag: string, attrs: Record<string, string | number>, text?: string) => {
    const el = document.createElementNS(ns, tag);
    for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
    if (text) el.textContent = text;
    svg.append(el);
  };
  const line = { stroke: "#000", "stroke-width": 0.2 };
  // Horizontal 100 mm ruler.
  add("line", { x1: 20, y1: 30, x2: 120, y2: 30, ...line });
  for (let mm = 0; mm <= 100; mm++) {
    const len = mm % 10 === 0 ? 5 : mm % 5 === 0 ? 3.5 : 2;
    add("line", { x1: 20 + mm, y1: 30, x2: 20 + mm, y2: 30 - len, ...line });
    if (mm % 10 === 0) add("text", { x: 20 + mm, y: 23, "font-size": 3, "text-anchor": "middle" }, String(mm));
  }
  add("text", { x: 20, y: 38, "font-size": 4 }, labels.horizontal);
  // Vertical 100 mm ruler.
  add("line", { x1: 150, y1: 30, x2: 150, y2: 130, ...line });
  for (let mm = 0; mm <= 100; mm++) {
    const len = mm % 10 === 0 ? 5 : mm % 5 === 0 ? 3.5 : 2;
    add("line", { x1: 150, y1: 30 + mm, x2: 150 + len, y2: 30 + mm, ...line });
    if (mm % 10 === 0) add("text", { x: 158, y: 31 + mm, "font-size": 3 }, String(mm));
  }
  add("text", { x: 150, y: 140, "font-size": 4 }, labels.vertical);
  // 10 mm squares and a 50 mm square.
  for (let i = 0; i < 5; i++) add("rect", { x: 20 + i * 15, y: 60, width: 10, height: 10, fill: "none", ...line });
  add("rect", { x: 20, y: 80, width: 50, height: 50, fill: "none", ...line });
  add("text", { x: 20, y: 136, "font-size": 3.5 }, "10 mm / 50 mm");
  const hint = document.createElement("p");
  hint.className = "a4-hint";
  hint.textContent = labels.hint;
  body.append(svg, hint);
  return page;
}

/** Prints `pages` with the system print dialog (only they are visible). */
export async function printPages(pages: HTMLElement[]): Promise<void> {
  const host = document.getElementById("a4-print")!;
  host.replaceChildren(...pages);
  await Promise.all(
    Array.from(host.querySelectorAll("img")).map((img) => (img.complete ? Promise.resolve() : img.decode().catch(() => {}))),
  );
  document.body.classList.add("printing-a4");
  // Webviews differ in whether print() blocks; clean up when printing ends.
  // Leftovers are invisible on screen and replaced by the next print.
  window.addEventListener(
    "afterprint",
    () => {
      document.body.classList.remove("printing-a4");
      host.replaceChildren();
    },
    { once: true },
  );
  window.print();
}

/**
 * The label as a PNG data URL in tape colours (720 dpi), e.g. to save it
 * as an image for documentation.
 */
export async function labelPng(label: A4Label, background: string | null, ink: string): Promise<string> {
  // `label` may carry its own colours (pipe markers), which win.
  return (await tapeImage(label, { background, ink }, false)).url;
}
