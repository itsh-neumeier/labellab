// LabelLab editor: elements are boxes freely placed on the label (drag to
// move, handles to resize, magnetic snapping to edges of the tape and of
// other boxes). Underneath the boxes sits the live preview: the exact
// 1-bit raster that gets printed, rendered by the Rust backend through the
// same path as the print job.
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import appIcon from "../src-tauri/icons/128x128.png";
import { ask, message, open, save } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { Connection, Device, Element, Item, Label, Rect } from "./api";
import { bindImageEditor, openImageEditor } from "./imageEditor";
import { bindPaint, loadPaint, newPaint, segmentSvg } from "./framePaint";
import { BOLD_MARK, ITALIC_MARK, stripMarkup, toggleMark } from "./richtext";
import { buildCode, emptyFields, parseCode, type CodeFields, type CodeKind } from "./codes";
import { applyLang, applyStatic, currentLang, errorText, loadLang, setLang, t, type Lang } from "./i18n";
import { langInfo, langPicker } from "./langs";
import { PIPE_CSS, PIPE_GROUPS, pipeGroup, pipeLength, type PipeGroup } from "./pipes";
import { getSetting, initSettings, setSetting } from "./settings";
import { buildPages, labelPng, printPages, RENDER_SCALE, testPage, type A4Label, type A4Options } from "./a4print";
import { handleEdges, resizeRect, roundRect, snapMove, snapResize, targets, type Guides } from "./snap";
import { INK_CSS, TAPE_CSS, TAPE_STYLES, parseStyleKey, styleKey, type TapeStyle } from "./tapes";

const DOTS_PER_MM = 180 / 25.4;
const PREVIEW_DEBOUNCE_MS = 40;
/** Width of the vertical ruler (matches `.vruler` in styles.css). */
const VRULER_PX = 26;
/** Base color of clear tape (matches `.stage.clear-tape`). */
const CLEAR_TAPE_CSS = "#e9edf1";
const HISTORY_DEBOUNCE_MS = 400;
const HISTORY_LIMIT = 200;
/** On-screen tape height the zoom is fitted to when the tape changes. */
const FIT_TAPE_PX = 220;
/** Snap distance in screen pixels. */
const SNAP_PX = 8;
/** Smallest box edge in mm. */
const MIN_BOX_MM = 1;
/** Arrow-key nudge in mm (Shift: coarse). */
const NUDGE_MM = 0.5;
const NUDGE_COARSE_MM = 5;
/** Gap in mm when a new element is placed after the existing ones. */
const NEW_ITEM_GAP_MM = 1;

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function newLabel(): Label {
  return {
    version: 2,
    elements: [{ type: "text", text: "LabelLab", size_pt: null, align: "center" }],
    gap_mm: 2,
    padding_mm: 1,
    min_length_mm: null,
    frame: false,
  };
}

const state = {
  label: newLabel(),
  /** All sheets of the open document; `state.label` is the current one's label. */
  sheets: [] as api.Sheet[],
  sheet: 0,
  /** Document JSON at the last open/save/new, to detect unsaved changes. */
  savedSnapshot: "",
  filePath: null as string | null,
  models: [] as api.Model[],
  devices: [] as Device[],
  selected: -1,
  fonts: [] as string[],
  iconsets: [] as api.IconSet[],
  csv: null as (api.Csv & { name: string }) | null,
  /** Last focused text field in an element card (CSV column insertion). */
  lastField: null as { el: HTMLInputElement | HTMLTextAreaElement; apply: (v: string) => void } | null,
  printing: false,
  history: [] as string[],
  historyIndex: -1,
};

// ---------------------------------------------------------------- tape / geometry

function selectedModel(): string {
  return $<HTMLSelectElement>("model").value;
}

function selectedWidth(): number {
  return Number($<HTMLSelectElement>("width").value);
}

/** Printable tape height in mm for the selected model/tape. */
/** Printable height of one tape strip in mm. */
function tapeMm(): number {
  const model = state.models.find((m) => m.name === selectedModel());
  return model?.tapes.find((tp) => tp.width_mm === selectedWidth())?.printable_mm ?? 10;
}

function strips(): number {
  return Math.max(1, state.label.strips ?? 1);
}

/** Editor height in mm: all stacked strips. */
/** Non-printable strip at each tape edge in mm (tape width minus printable area, halved). */
function tapeMarginMm(): number {
  const model = state.models.find((m) => m.name === selectedModel());
  const tape = model?.tapes.find((tp) => tp.width_mm === selectedWidth());
  return tape ? Math.max(0, (tape.width_mm - tape.printable_mm) / 2) : 0;
}

/** Left margin of the label in mm (falls back to the right one). */
function startPad(): number {
  return state.label.padding_start_mm ?? state.label.padding_mm;
}

function endPad(): number {
  return state.label.padding_mm;
}

/** Elements (1-based) reaching into the left margin, or the right one of a fixed-length label. */
/** Portrait editing: tape vertical, the label length runs downwards. */
function portrait(): boolean {
  return state.label.orientation === "portrait";
}

/** Start and size of a box along the label length. */
function alongStart(r: Rect): number {
  return portrait() ? r.y_mm : r.x_mm;
}

function alongSize(r: Rect): number {
  return portrait() ? r.h_mm : r.w_mm;
}

/**
 * Switches the editor orientation. Boxes are turned with the view (portrait
 * = landscape turned 90° clockwise), so the printed label stays the same.
 */
async function setOrientation(next: api.Orientation): Promise<void> {
  if ((state.label.orientation ?? "landscape") === next) return;
  await ensureRects();
  const tape = labelHeightMm();
  for (const item of state.label.elements) {
    const r = item.rect;
    if (!r) continue;
    if (next === "portrait") {
      item.rect = roundRect({ x_mm: tape - r.y_mm - r.h_mm, y_mm: r.x_mm, w_mm: r.h_mm, h_mm: r.w_mm });
      item.rotation = ((item.rotation ?? 0) + 90) % 360;
    } else {
      item.rect = roundRect({ x_mm: r.y_mm, y_mm: tape - r.x_mm - r.w_mm, w_mm: r.h_mm, h_mm: r.w_mm });
      item.rotation = ((item.rotation ?? 0) + 270) % 360;
    }
    if (!item.rotation) delete item.rotation;
  }
  state.label.orientation = next === "portrait" ? "portrait" : undefined;
  changed(true);
}

function marginViolations(): number[] {
  const l = state.label;
  const fixed = l.fixed_length && l.min_length_mm ? l.min_length_mm : null;
  const eps = 0.05;
  return l.elements.flatMap((item, i) => {
    const r = item.rect;
    if (!r || item.type === "fill") return [];
    const left = alongStart(r) < startPad() - eps;
    const right = fixed !== null && alongStart(r) + alongSize(r) > fixed - endPad() + eps;
    return left || right ? [i + 1] : [];
  });
}

function labelHeightMm(): number {
  return tapeMm() * strips();
}

/** Preview resolution multiplier (1 = exact print raster). */
function previewScale(): number {
  return Number($<HTMLSelectElement>("quality").value) || 1;
}

function zoom(): number {
  return Number($<HTMLInputElement>("zoom").value);
}

/** Screen pixels per mm (one preview pixel = one print dot). */
function pxPerMm(): number {
  return zoom() * DOTS_PER_MM;
}

// ---------------------------------------------------------------- history

let historyTimer: number | undefined;

/** Records the current label as an undo step (if it changed). */
function commit(): void {
  window.clearTimeout(historyTimer);
  const snap = JSON.stringify(state.label);
  if (state.history[state.historyIndex] === snap) return;
  state.history = state.history.slice(0, state.historyIndex + 1);
  state.history.push(snap);
  if (state.history.length > HISTORY_LIMIT) state.history.shift();
  state.historyIndex = state.history.length - 1;
  updateHistoryButtons();
  updateFileName();
}

function commitSoon(): void {
  window.clearTimeout(historyTimer);
  historyTimer = window.setTimeout(commit, HISTORY_DEBOUNCE_MS);
}

function resetHistory(): void {
  state.history = [];
  state.historyIndex = -1;
  commit();
}

function stepHistory(delta: number): void {
  commit(); // flush a pending text edit first
  const next = state.historyIndex + delta;
  if (next < 0 || next >= state.history.length) return;
  state.historyIndex = next;
  state.label = JSON.parse(state.history[next]);
  multi.clear();
  if (state.selected >= state.label.elements.length) state.selected = -1;
  renderAll();
  updateHistoryButtons();
}

function updateHistoryButtons(): void {
  $<HTMLButtonElement>("btn-undo").disabled = state.historyIndex <= 0;
  $<HTMLButtonElement>("btn-redo").disabled = state.historyIndex >= state.history.length - 1;
}

/** Applies an edit: preview now, undo step after typing pauses. */
function changed(structural = false): void {
  if (structural) {
    renderElements();
    renderBoxes();
    commit();
  } else {
    commitSoon();
  }
  schedulePreview();
}

// ---------------------------------------------------------------- preview

let previewTimer: number | undefined;
let previewSeq = 0;
let fitZoomPending = true;

function schedulePreview(): void {
  updateSeriesButton();
  window.clearTimeout(previewTimer);
  previewTimer = window.setTimeout(runPreview, PREVIEW_DEBOUNCE_MS);
}

let previewBusy = false;
let previewAgain = false;

/** At most one render in flight; edits meanwhile cause exactly one more. */
async function runPreview(): Promise<void> {
  if (previewBusy) {
    previewAgain = true;
    return;
  }
  previewBusy = true;
  try {
    await updatePreview();
  } finally {
    previewBusy = false;
    if (previewAgain) {
      previewAgain = false;
      void runPreview();
    }
  }
}

/** Elements whose text was clipped in the last preview. */
let overflowing = new Set<number>();

function markOverflow(): void {
  document
    .querySelectorAll<HTMLElement>(".box")
    .forEach((b) => b.classList.toggle("overflow", overflowing.has(Number(b.dataset.index))));
}

async function updatePreview(): Promise<void> {
  const seq = ++previewSeq;
  const img = $<HTMLImageElement>("preview");
  const msg = $("preview-msg");
  const model = selectedModel();
  const width = selectedWidth();
  if (!model || !width) return;

  try {
    const scale = previewScale();
    const preview = await api.renderPreview(state.label, model, width, previewRow() ?? 1, numbering(), scale);
    if (seq !== previewSeq) return; // a newer render is on its way
    const url = `data:image/png;base64,${preview.png}`;
    img.onload = () => {
      img.dataset.scale = String(scale);
      const ink = $("ink");
      ink.style.maskImage = `url("${url}")`;
      ink.style.setProperty("-webkit-mask-image", `url("${url}")`);
      ink.classList.remove("stale");
      layoutStage();
      const lengthMm = ((portrait() ? img.naturalHeight : img.naturalWidth) / scale / DOTS_PER_MM).toFixed(1);
      const height = strips() > 1 ? `${strips()}×${width}` : `${width}`;
      $("dims").textContent = t("preview.dims", { length: lengthMm, width: height });
    };
    img.onerror = () => {
      msg.textContent = t("preview.error", { error: "PNG" });
    };
    img.src = url;
    overflowing = new Set(preview.overflowing);
    markOverflow();
    const inMargin = marginViolations();
    msg.textContent = [
      preview.overflowing.length ? t("preview.overflow", { items: preview.overflowing.map((i) => i + 1).join(", ") }) : "",
      inMargin.length ? t("preview.inMargin", { items: inMargin.join(", ") }) : "",
    ]
      .filter(Boolean)
      .join(" ");
  } catch (e) {
    if (seq !== previewSeq) return;
    $("ink").classList.add("stale");
    $("dims").textContent = "";
    msg.textContent = t("preview.error", { error: errorText(e) });
  }
}

/** Sizes the stage (preview + box overlay) for the current zoom. */
function layoutStage(): void {
  const img = $<HTMLImageElement>("preview");
  const ppm = pxPerMm();
  const scale = Number(img.dataset.scale) || 1;
  const upright = portrait();
  // Rendered label length (fixed length cuts boxes beyond it); a portrait
  // preview comes turned, the length is then its height.
  const lengthPx = ((upright ? img.naturalHeight : img.naturalWidth) / scale) * zoom();
  const boxesEnd = Math.max(0, ...state.label.elements.map((i) => (i.rect ? alongStart(i.rect) + alongSize(i.rect) : 0)));
  const along = Math.max(lengthPx, boxesEnd * ppm);
  const across = labelHeightMm() * ppm;
  const [width, height] = upright ? [across, along] : [along, across];
  const ink = $("ink");
  ink.style.width = `${upright ? across : lengthPx}px`;
  ink.style.height = `${upright ? lengthPx : across}px`;
  const stage = $("stage");
  stage.style.width = `${width}px`;
  stage.style.height = `${height}px`;
  // Start/end margins as marked zones at both label ends.
  const zone = (id: string, start: number, size: number) => {
    const z = $(id).style;
    z.left = upright ? "0" : `${start}px`;
    z.right = upright ? "0" : "";
    z.top = upright ? `${start}px` : "0";
    z.bottom = upright ? "" : "0";
    z.width = upright ? "" : `${Math.max(0, size)}px`;
    z.height = upright ? `${Math.max(0, size)}px` : "";
    z.display = size > 0 ? "" : "none";
  };
  zone("margin-start", 0, startPad() * ppm);
  zone("margin-end", lengthPx - endPad() * ppm, endPad() * ppm);
  $("stage-row").classList.toggle("portrait", upright);
  // Show the whole tape: grey bands for what the print head can't reach.
  const margin = `${tapeMarginMm() * ppm}px`;
  const frame = $("tape-frame").style;
  frame.paddingBlock = upright ? "0" : margin;
  frame.paddingInline = upright ? margin : "0";
  const lines = $("strip-lines");
  lines.replaceChildren();
  for (let k = 1; k < strips(); k++) {
    const line = document.createElement("div");
    line.className = upright ? "strip-line vertical" : "strip-line";
    if (upright) line.style.left = `${k * tapeMm() * ppm}px`;
    else line.style.top = `${k * tapeMm() * ppm}px`;
    lines.append(line);
  }
  const tapePx = across + 2 * tapeMarginMm() * ppm;
  drawRuler(upright ? tapePx : width, ppm);
  drawVRuler(upright ? height : tapePx, ppm);
  repositionBoxes();
}

/** mm ruler left of the tape, across the full tape width (0 = top tape edge). */
function drawVRuler(heightPx: number, ppm: number): void {
  const ruler = $("vruler");
  const widthMm = heightPx / ppm;
  const every = ppm >= 4 ? 1 : 5;
  const parts: string[] = [];
  for (let mm = 0; mm <= widthMm + 0.01; mm += every) {
    const y = (mm * ppm).toFixed(1);
    const major = mm % 5 === 0;
    const w = major ? 7 : 3;
    parts.push(`<line x1="${VRULER_PX - w}" x2="${VRULER_PX}" y1="${y}" y2="${y}" />`);
    if (major && mm > 0) parts.push(`<text x="${VRULER_PX - 9}" y="${y}" stroke="none" dominant-baseline="middle">${mm}</text>`);
  }
  ruler.style.height = `${heightPx}px`;
  ruler.innerHTML =
    `<svg width="${VRULER_PX}" height="${heightPx}" stroke="currentColor" fill="currentColor" font-size="9" ` +
    `text-anchor="end" font-family="system-ui, sans-serif">${parts.join("")}</svg>`;
}

/** mm ruler above the stage: small ticks per mm (if wide enough), numbers every 10 mm. */
function drawRuler(widthPx: number, ppm: number): void {
  const ruler = $("ruler");
  const lengthMm = Math.ceil(widthPx / ppm);
  const every = ppm >= 4 ? 1 : ppm >= 1.5 ? 5 : 10;
  const parts: string[] = [];
  for (let mm = 0; mm <= lengthMm; mm += every) {
    const x = (mm * ppm).toFixed(1);
    const major = mm % 10 === 0;
    const h = major ? 9 : mm % 5 === 0 ? 6 : 3;
    parts.push(`<line x1="${x}" x2="${x}" y1="${18 - h}" y2="18" />`);
    if (major) parts.push(`<text x="${x}" y="8" stroke="none">${mm}</text>`);
  }
  ruler.style.width = `${widthPx}px`;
  ruler.innerHTML =
    `<svg width="${widthPx}" height="18" stroke="currentColor" fill="currentColor" font-size="9" ` +
    `text-anchor="middle" font-family="system-ui, sans-serif">${parts.join("")}</svg>`;
}

/**
 * Updates box positions for the current zoom without rebuilding them: a
 * rebuild in the middle of a drag would drop the element holding the
 * pointer capture (the preview reloads while dragging).
 */
function repositionBoxes(): void {
  document.querySelectorAll<HTMLElement>(".box").forEach((box) => {
    const rect = state.label.elements[Number(box.dataset.index)]?.rect;
    if (rect) placeBox(box, rect);
  });
}

const ZOOM_MIN = 0.5;
const ZOOM_MAX = 12;
const ZOOM_WHEEL_FACTOR = 1.15;

/** Mouse wheel over the preview zooms around the pointer; Shift+wheel scrolls. */
function bindWheelZoom(): void {
  const wrap = $("tape-wrap");
  wrap.addEventListener(
    "wheel",
    (e) => {
      if (e.shiftKey || e.deltaY === 0) return;
      e.preventDefault();
      const input = $<HTMLInputElement>("zoom");
      const before = zoom();
      const raw = e.deltaY < 0 ? before * ZOOM_WHEEL_FACTOR : before / ZOOM_WHEEL_FACTOR;
      const next = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(raw * 4) / 4 || ZOOM_MIN));
      const step = next === before ? (e.deltaY < 0 ? 0.25 : -0.25) : 0;
      const target = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, next + step));
      if (target === before) return;
      // Keep the point under the pointer where it is.
      const r = wrap.getBoundingClientRect();
      const px = e.clientX - r.left;
      const py = e.clientY - r.top;
      const cx = (wrap.scrollLeft + px) / before;
      const cy = (wrap.scrollTop + py) / before;
      input.value = String(target);
      input.dispatchEvent(new Event("input", { bubbles: true }));
      wrap.scrollLeft = cx * target - px;
      wrap.scrollTop = cy * target - py;
    },
    { passive: false },
  );
}

function fitZoom(): void {
  const z = Math.min(ZOOM_MAX, Math.max(1, Math.round(FIT_TAPE_PX / (labelHeightMm() * DOTS_PER_MM))));
  $<HTMLInputElement>("zoom").value = String(z);
}

/** Running number settings for `{{n}}`/`{{A}}`. */
function numbering(): api.Numbering {
  return {
    start: Math.trunc(Number($<HTMLInputElement>("num-start").value) || 0),
    step: Math.trunc(Number($<HTMLInputElement>("num-step").value) || 0),
  };
}

/** Labels in a numbered series without CSV (0 = single label). */
function numberedCount(): number {
  return Math.max(0, Math.trunc(Number($<HTMLInputElement>("num-count").value) || 0));
}

/** CSV record shown in the preview, or null without CSV. */
function previewRow(): number | null {
  if (!state.csv) return null;
  const n = Number($<HTMLInputElement>("preview-row").value) || 1;
  return Math.min(Math.max(1, n), Math.max(1, state.csv.rows.length));
}

// ---------------------------------------------------------------- tape colors

const TAPE_STYLE_KEY = "labellab.tapeStyle";
/**
 * Colours the cassette reported at the last status read. Cassettes may
 * report other colours than they have, and differently coloured cassettes
 * may report the same codes (seen with compatible 24 mm tapes), so a status
 * read only sets the tape style when the reported codes change; a style the
 * user picked stays until another coding is inserted.
 */
const TAPE_REPORTED_KEY = "labellab.tapeReported";

/** Applies the reported colours unless the same codes were reported before. */
function applyReportedTape(reported: TapeStyle): void {
  let last = "";
  try {
    last = getSetting(TAPE_REPORTED_KEY) ?? "";
    setSetting(TAPE_REPORTED_KEY, styleKey(reported));
  } catch {
    // not remembered: apply every time
  }
  if (last !== styleKey(reported)) fillTapeStyles(reported);
}

function styleName(st: TapeStyle): string {
  return t("preview.inkOn", { ink: t(`color.${st.ink}`), tape: t(`color.${st.tape}`) });
}

function fillTapeStyles(detected?: TapeStyle): void {
  const select = $<HTMLSelectElement>("tape-style");
  const current = detected ? styleKey(detected) : select.value || loadTapeStyle();
  select.replaceChildren();
  const all = [...TAPE_STYLES];
  if (detected && !all.some((st) => styleKey(st) === styleKey(detected))) {
    select.add(new Option(t("preview.custom", { ink: t(`color.${detected.ink}`), tape: t(`color.${detected.tape}`) }), styleKey(detected)));
  }
  for (const st of all) select.add(new Option(styleName(st), styleKey(st)));
  select.value = current;
  if (!select.value) select.value = styleKey(TAPE_STYLES[0]);
  applyTapeStyle();
}

function loadTapeStyle(): string {
  try {
    return getSetting(TAPE_STYLE_KEY) ?? "";
  } catch {
    return "";
  }
}

function applyTapeStyle(): void {
  const key = $<HTMLSelectElement>("tape-style").value;
  try {
    setSetting(TAPE_STYLE_KEY, key);
  } catch {
    // not remembered, still applied
  }
  const st = parseStyleKey(key) ?? TAPE_STYLES[0];
  const bg = TAPE_CSS[st.tape];
  const stage = $("stage");
  stage.classList.toggle("clear-tape", bg === null);
  stage.style.backgroundColor = bg ?? "";
  $("ink").style.backgroundColor = INK_CSS[st.ink] ?? INK_CSS.black;
  stage.classList.toggle("dark-tape", st.tape === "black");
  $("tape-frame").style.setProperty("--tape-bg", bg ?? CLEAR_TAPE_CSS);
}

// ---------------------------------------------------------------- boxes (canvas)

function elementTitle(el: Element): string {
  if (el.type === "fuse_box" && el.fixed) return t("elements.patch_panel");
  return t(`elements.${el.type}`);
}

function boxCaption(item: Item): string {
  switch (item.type) {
    case "text":
      return stripMarkup(item.text).split("\n")[0] || elementTitle(item);
    case "qr":
    case "barcode":
      return `${elementTitle(item)}: ${item.data}`;
    case "image":
      return item.path.split(/[\\/]/).pop() || elementTitle(item);
    case "symbol":
      return `${elementTitle(item)}: ${symbolLabel(item.name)}`;
    case "fill":
      return elementTitle(item);
    case "shape":
      return `${elementTitle(item)}: ${t(`shape.${item.shape}`)}`;
    case "fuse_box":
      return item.fields.map((f) => stripMarkup(f.text).replace(/\n/g, " ")).join(" | ") || elementTitle(item);
    case "table":
      return `${elementTitle(item)}: ${(item.cells[0] ?? []).map((c) => stripMarkup(c).replace(/\n/g, " ")).join(" | ")}`;
    case "pipe_marker":
      return `${elementTitle(item)}: ${item.text}`;
  }
}

function placeBox(el: HTMLElement, r: Rect): void {
  const ppm = pxPerMm();
  el.style.left = `${r.x_mm * ppm}px`;
  el.style.top = `${r.y_mm * ppm}px`;
  el.style.width = `${r.w_mm * ppm}px`;
  el.style.height = `${r.h_mm * ppm}px`;
}

function renderBoxes(): void {
  const layer = $("boxes");
  layer.replaceChildren();
  state.label.elements.forEach((item, index) => {
    if (!item.rect) return;
    const box = document.createElement("div");
    box.className = `box${isSelected(index) ? " selected" : ""}${overflowing.has(index) ? " overflow" : ""}${item.locked ? " locked" : ""}${item.hidden ? " is-hidden" : ""}`;
    box.dataset.index = String(index);
    box.title = boxCaption(item);
    placeBox(box, item.rect);
    const tag = document.createElement("span");
    tag.className = "tag";
    tag.textContent = `${index + 1}`;
    box.append(tag);
    for (const h of ["n", "s", "e", "w", "ne", "nw", "se", "sw"]) {
      const handle = document.createElement("div");
      handle.className = `handle h-${h}`;
      handle.dataset.handle = h;
      box.append(handle);
    }
    box.addEventListener("pointerdown", (e) => startDrag(e, index));
    layer.append(box);
  });
}

function showGuides(g: Guides): void {
  const layer = $("boxes");
  layer.querySelectorAll(".guide").forEach((n) => n.remove());
  const ppm = pxPerMm();
  for (const x of g.x) {
    const line = document.createElement("div");
    line.className = "guide guide-x";
    line.style.left = `${x * ppm}px`;
    layer.append(line);
  }
  for (const y of g.y) {
    const line = document.createElement("div");
    line.className = "guide guide-y";
    line.style.top = `${y * ppm}px`;
    layer.append(line);
  }
}

function startDrag(e: PointerEvent, index: number): void {
  if (e.button !== 0) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.shiftKey || e.ctrlKey || e.metaKey) {
    toggleSelect(index);
    return;
  }
  const handle = (e.target as HTMLElement).dataset.handle ?? null;
  // Dragging one of several selected boxes moves them all; else select just this one.
  if (!isSelected(index) || handle) select(index);
  else if (state.selected !== index) {
    multi.add(state.selected);
    multi.delete(index);
    state.selected = index;
    showSelection();
  }
  const item = state.label.elements[index];
  if (!item.rect || item.locked) return;
  const box = (e.currentTarget as HTMLElement);
  const start = { ...item.rect };
  // The other selected boxes follow the dragged one by the same distance.
  const group = handle
    ? []
    : selection()
        .slice(1)
        .map((i) => ({ i, item: state.label.elements[i] }))
        .filter((g) => g.item.rect && !g.item.locked)
        .map((g) => ({ ...g, start: { ...g.item.rect! } }));
  const startX = e.clientX;
  const startY = e.clientY;
  const others = state.label.elements.filter((_, i) => !isSelected(i) && state.label.elements[i].rect).map((i) => i.rect!);
  const snapTargets = targets(others, labelHeightMm());
  const fixedLength = state.label.fixed_length ? state.label.min_length_mm : null;
  if (portrait()) {
    // Tape edges/centre are vertical lines, label margins horizontal ones.
    const tape = labelHeightMm();
    snapTargets.y = snapTargets.y.filter((v) => v !== tape && v !== tape / 2);
    snapTargets.x.push(tape, tape / 2, ...Array.from({ length: strips() - 1 }, (_, k) => (k + 1) * tapeMm()));
    snapTargets.y.push(startPad());
    if (fixedLength) snapTargets.y.push(fixedLength - endPad());
  } else {
    snapTargets.x.push(startPad());
    if (fixedLength) snapTargets.x.push(fixedLength - endPad());
    for (let k = 1; k < strips(); k++) snapTargets.y.push(k * tapeMm());
  }
  box.setPointerCapture(e.pointerId);

  const onMove = (ev: PointerEvent) => {
    const ppm = pxPerMm();
    const dx = (ev.clientX - startX) / ppm;
    const dy = (ev.clientY - startY) / ppm;
    const threshold = ev.altKey ? 0 : SNAP_PX / ppm;
    let next: Rect;
    let guides: Guides = { x: [], y: [] };
    if (!handle) {
      ({ rect: next, guides } = snapMove({ ...start, x_mm: start.x_mm + dx, y_mm: start.y_mm + dy }, snapTargets, threshold));
    } else {
      const edges = handleEdges(handle);
      const resized = resizeRect(start, dx, dy, edges, MIN_BOX_MM, ev.shiftKey);
      // No snapping while keeping the ratio: snapping one edge would break it.
      ({ rect: next, guides } = ev.shiftKey
        ? { rect: resized, guides: { x: [], y: [] } }
        : snapResize(resized, snapTargets, threshold, edges));
    }
    item.rect = roundRect(next);
    placeBox(box, item.rect);
    if (group.length) {
      const mx = item.rect.x_mm - start.x_mm;
      const my = item.rect.y_mm - start.y_mm;
      for (const g of group) g.item.rect = roundRect({ ...g.start, x_mm: g.start.x_mm + mx, y_mm: g.start.y_mm + my });
      repositionBoxes();
    }
    showGuides(guides);
    updateRectInputs(index);
    schedulePreview();
  };
  const onUp = () => {
    box.removeEventListener("pointermove", onMove);
    box.removeEventListener("pointerup", onUp);
    box.removeEventListener("pointercancel", onUp);
    showGuides({ x: [], y: [] });
    layoutStage();
    commit();
  };
  box.addEventListener("pointermove", onMove);
  box.addEventListener("pointerup", onUp);
  box.addEventListener("pointercancel", onUp);
}

/** Further selected elements besides `state.selected` (Shift/Ctrl+click). */
const multi = new Set<number>();

/** Every selected element index, the primary one first. */
function selection(): number[] {
  if (state.selected < 0) return [];
  const n = state.label.elements.length;
  return [state.selected, ...[...multi].filter((i) => i !== state.selected && i >= 0 && i < n)];
}

function isSelected(index: number): boolean {
  return index === state.selected || multi.has(index);
}

/** Marks the selection in boxes and layer list, shows the properties. */
function showSelection(scrollTo = state.selected): void {
  document.querySelectorAll<HTMLElement>(".box").forEach((b) => b.classList.toggle("selected", isSelected(Number(b.dataset.index))));
  document.querySelectorAll<HTMLElement>(".layer").forEach((c) => {
    const i = Number(c.dataset.index);
    c.classList.toggle("selected", isSelected(i));
    if (i === scrollTo) c.scrollIntoView({ block: "nearest" });
  });
  renderProps();
}

function select(index: number): void {
  if (state.selected === index && multi.size === 0) return;
  multi.clear();
  state.selected = index;
  showSelection();
}

/** Shift/Ctrl+click: adds `index` to the selection or takes it out. */
function toggleSelect(index: number): void {
  if (state.selected < 0) {
    state.selected = index;
  } else if (isSelected(index)) {
    multi.delete(index);
    if (index === state.selected) {
      const [next] = selection().slice(1);
      state.selected = next ?? -1;
      if (next !== undefined) multi.delete(next);
    }
  } else {
    multi.add(state.selected);
    state.selected = index;
  }
  showSelection(index);
}

/** Box for a new element: after the rightmost box, full tape height. */
function newRect(type: Element["type"]): Rect {
  const h = labelHeightMm();
  const end = Math.max(0, ...state.label.elements.map((i) => (i.rect ? alongStart(i.rect) + alongSize(i.rect) : 0)));
  const start = state.label.elements.length ? end + NEW_ITEM_GAP_MM : startPad();
  if (portrait()) {
    // Tape-wide boxes stacked down the label.
    const fuse = FUSE_DEFAULT_COUNT * FUSE_DEFAULT_PITCH_MM;
    const len = { text: 8, qr: h, barcode: 12, image: h, symbol: h, fill: 0.5, shape: h, fuse_box: fuse, table: 20, pipe_marker: pipeLength(h) }[type];
    return roundRect({ x_mm: 0, y_mm: start, w_mm: h, h_mm: len });
  }
  const fuse = FUSE_DEFAULT_COUNT * FUSE_DEFAULT_PITCH_MM;
  const w = { text: 25, qr: h, barcode: 30, image: h * 1.5, symbol: h, fill: 0.5, shape: h * 1.5, fuse_box: fuse, table: 40, pipe_marker: pipeLength(h) }[type];
  return roundRect({ x_mm: start, y_mm: 0, w_mm: w, h_mm: h });
}

/** Gives every element without a box the box the flow layout uses. */
async function ensureRects(): Promise<void> {
  // The label of this call: the user may switch sheets while it resolves.
  const label = state.label;
  if (label.elements.every((i) => i.rect)) return;
  try {
    const rects = await api.resolveRects(label, selectedModel(), selectedWidth());
    label.elements.forEach((item, i) => {
      if (!item.rect && rects[i]) item.rect = roundRect(rects[i]);
    });
  } catch {
    // e.g. no font or invalid content: fall back to simple placement
    for (const item of label.elements) {
      if (!item.rect) item.rect = newRect(item.type);
    }
  }
}

// ---------------------------------------------------------------- element cards

function makeButton(text: string, title: string, onClick: () => void): HTMLButtonElement {
  const b = document.createElement("button");
  b.textContent = text;
  b.title = title;
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onClick();
  });
  return b;
}

function field(labelKey: string, control: HTMLElement): HTMLLabelElement {
  const label = document.createElement("label");
  label.className = "field";
  const span = document.createElement("span");
  span.textContent = t(labelKey);
  label.append(span, control);
  return label;
}

function textInput(value: string, onInput: (v: string) => void): HTMLInputElement {
  const input = document.createElement("input");
  input.type = "text";
  input.value = value;
  input.addEventListener("input", () => {
    onInput(input.value);
    syncBoxCaption();
    changed();
  });
  trackField(input, onInput);
  return input;
}

/** Remembers `el` as the target for inserting CSV placeholders. */
function trackField(el: HTMLInputElement | HTMLTextAreaElement, apply: (v: string) => void): void {
  el.addEventListener("focus", () => (state.lastField = { el, apply }));
}

function numberInput(value: number | null | undefined, step: number, placeholder: string, onInput: (v: number | null) => void): HTMLInputElement {
  const input = document.createElement("input");
  input.type = "number";
  input.step = String(step);
  input.value = value == null ? "" : String(value);
  input.placeholder = placeholder;
  input.addEventListener("input", () => {
    const v = input.value === "" ? null : Number(input.value);
    onInput(v === null || Number.isNaN(v) ? null : v);
    changed();
  });
  return input;
}

function syncBoxCaption(): void {
  document.querySelectorAll<HTMLElement>(".box").forEach((b) => {
    const item = state.label.elements[Number(b.dataset.index)];
    if (item) b.title = boxCaption(item);
  });
  document.querySelectorAll<HTMLElement>(".layer").forEach((row) => {
    const item = state.label.elements[Number(row.dataset.index)];
    const caption = row.querySelector(".layer-caption");
    if (item && caption) caption.textContent = layerCaption(item);
  });
}

const RECT_KEYS: (keyof Rect)[] = ["x_mm", "y_mm", "w_mm", "h_mm"];

function rectFields(item: Item, index: number): HTMLElement {
  const row = document.createElement("div");
  row.className = "rect-fields";
  for (const key of RECT_KEYS) {
    const input = numberInput(item.rect?.[key], 0.1, "", (v) => {
      if (!item.rect || v === null) return;
      item.rect[key] = key === "w_mm" || key === "h_mm" ? Math.max(MIN_BOX_MM, v) : v;
      const box = document.querySelector<HTMLElement>(`.box[data-index="${index}"]`);
      if (box) placeBox(box, item.rect);
    });
    input.dataset.rect = key;
    row.append(field(`rect.${key}`, input));
  }
  return row;
}

function updateRectInputs(index: number): void {
  const item = state.label.elements[index];
  const card = document.querySelector<HTMLElement>(`.element[data-index="${index}"]`);
  if (!item?.rect || !card) return;
  for (const key of RECT_KEYS) {
    const input = card.querySelector<HTMLInputElement>(`input[data-rect="${key}"]`);
    if (input && document.activeElement !== input) input.value = String(item.rect[key]);
  }
}

function codeWizardButton(item: Item): HTMLButtonElement {
  const button = makeButton(t("code.edit"), t("code.editHint"), () => openCodeWizard(item));
  button.className = "code-edit";
  return button;
}

type FuseBoxItem = Extract<Item, { type: "fuse_box" }>;

const FUSE_DEFAULT_COUNT = 12;
const FUSE_DEFAULT_PITCH_MM = 17.5;
const FUSE_RATIOS = [0.5, 1, 1.5, 2, 2.5, 3, 4, 5, 6, 8];

const fuseRatio = (f: api.FuseField) => f.ratio ?? 1;

/** Length of a fuse box in mm: its ratios in modules of `pitch_mm`. */
function fuseLength(item: FuseBoxItem): number {
  return item.fields.reduce((sum, f) => sum + fuseRatio(f), 0) * item.pitch_mm;
}

/** Sizes the box along the tape to the fields (like the module grid on the rail). */
function fitFuseBox(item: FuseBoxItem): void {
  if (!item.rect) return;
  const len = Math.round(fuseLength(item) * 10) / 10;
  item.rect = portrait() ? { ...item.rect, h_mm: len } : { ...item.rect, w_mm: len };
  const index = state.label.elements.indexOf(item);
  if (index >= 0) updateRectInputs(index);
}

/** Next automatic field text after the last one ("F12" → "F13"). */
function nextFuseText(fields: api.FuseField[]): string {
  const last = fields[fields.length - 1]?.text ?? "F0";
  const m = /^(.*?)(\d+)$/.exec(last);
  if (!m) return `F${fields.length + 1}`;
  return `${m[1]}${String(Number(m[2]) + 1).padStart(m[2].length, "0")}`;
}

/** Properties of a fuse box: layout, text style and the list of fields. */
function fuseBoxFields(item: FuseBoxItem): HTMLElement[] {
  const list = document.createElement("div");
  list.className = "fuse-list";
  const count = numberInput(item.fields.length, 1, "", (v) => {
    const n = Math.max(1, Math.min(99, Math.round(v ?? 1)));
    while (item.fields.length < n) item.fields.push({ text: nextFuseText(item.fields) });
    item.fields.length = n;
    fitFuseBox(item);
    renderList();
  });
  count.min = "1";
  const pitch = numberInput(item.pitch_mm, 0.5, String(FUSE_DEFAULT_PITCH_MM), (v) => {
    item.pitch_mm = v && v > 0 ? v : FUSE_DEFAULT_PITCH_MM;
    fitFuseBox(item);
  });
  pitch.min = "1";
  pitch.title = t("fuse.pitchHint");
  const sizeRow = document.createElement("div");
  sizeRow.className = "row";
  sizeRow.append(field("fuse.count", count), field("fuse.pitch", pitch));

  const separator = document.createElement("select");
  for (const s of api.FUSE_SEPARATORS) {
    separator.add(new Option(t(`fuse.sep.${s}`), s, false, s === (item.separator ?? "frame")));
  }
  separator.addEventListener("change", () => {
    item.separator = separator.value as api.FuseSeparator;
    changed();
  });
  const check = (key: "vertical" | "reverse", label: string) => {
    const wrap = document.createElement("label");
    wrap.className = "check";
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = !!item[key];
    box.addEventListener("change", () => {
      item[key] = box.checked;
      changed();
    });
    wrap.append(box, ` ${t(label)}`);
    return wrap;
  };
  const optRow = document.createElement("div");
  optRow.className = "row";
  optRow.append(check("vertical", "fuse.vertical"), check("reverse", "fuse.reverse"));

  const size = numberInput(item.size_pt, 0.5, t("layout.auto"), (v) => {
    item.size_pt = v && v > 0 ? v : null;
  });
  const font = fontPicker(item.font ?? null, (family) => {
    item.font = family;
    changed(true);
  });
  // Field text last edited: with a selection there, F/K style just that part.
  let lastArea: HTMLTextAreaElement | null = null;
  const toggle = (label: string, title: string, key: "bold" | "italic") => {
    const b = makeButton(label, title, () => {
      if (lastArea?.isConnected && toggleMark(lastArea, key === "bold" ? BOLD_MARK : ITALIC_MARK)) return;
      item[key] = !item[key];
      b.classList.toggle("on", !!item[key]);
      changed();
    });
    b.className = `toggle${item[key] ? " on" : ""}`;
    b.style.fontWeight = key === "bold" ? "700" : "";
    b.style.fontStyle = key === "italic" ? "italic" : "";
    return b;
  };
  const style = document.createElement("div");
  style.className = "row font-row";
  const boldButton = toggle("F", t("elements.boldHint"), "bold");
  const italicButton = toggle("K", t("elements.italicHint"), "italic");
  style.append(field("elements.font", font), boldButton, italicButton);
  const spacing = numberInput(item.line_spacing, 0.1, "1.0", (v) => {
    item.line_spacing = v && v > 0 ? Math.min(3, Math.max(0.5, v)) : null;
  });
  spacing.min = "0.5";
  spacing.max = "3";
  spacing.title = t("elements.lineSpacingHint");
  const sizeField = document.createElement("div");
  sizeField.className = "row";
  sizeField.append(field("elements.size", size), field("elements.lineSpacing", spacing));
  const sepRow = document.createElement("div");
  sepRow.className = "row";
  sepRow.append(field("fuse.separator", separator));

  function renderList(): void {
    list.replaceChildren();
    count.value = String(item.fields.length);
    item.fields.forEach((f, i) => {
      const row = document.createElement("div");
      row.className = "fuse-row";
      const num = document.createElement("span");
      num.className = "muted";
      num.textContent = String(i + 1);
      // Several lines (Enter); **bold** / __italic__ for parts, like text elements.
      const text = document.createElement("textarea");
      text.rows = Math.min(4, Math.max(1, f.text.split("\n").length));
      text.value = f.text;
      text.addEventListener("input", () => {
        f.text = text.value;
        text.rows = Math.min(4, Math.max(1, text.value.split("\n").length));
        syncBoxCaption();
        changed();
      });
      text.addEventListener("focus", () => (lastArea = text));
      text.addEventListener("keydown", (e) => {
        if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
        const key = e.key.toLowerCase();
        if (key !== "b" && key !== "i") return;
        e.preventDefault();
        (key === "b" ? boldButton : italicButton).click();
      });
      trackField(text, (v) => (f.text = v));
      const ratio = document.createElement("select");
      ratio.title = t("fuse.ratio");
      const ratios = FUSE_RATIOS.includes(fuseRatio(f)) ? FUSE_RATIOS : [...FUSE_RATIOS, fuseRatio(f)].sort((a, b) => a - b);
      for (const r of ratios) ratio.add(new Option(`${r.toLocaleString()}×`, String(r), false, r === fuseRatio(f)));
      ratio.addEventListener("change", () => {
        f.ratio = Number(ratio.value);
        if (f.ratio === 1) delete f.ratio;
        fitFuseBox(item);
        changed();
      });
      const dir = document.createElement("select");
      dir.title = t("fuse.direction");
      dir.add(new Option("·", "", false, f.vertical == null));
      dir.add(new Option("→", "h", false, f.vertical === false));
      dir.add(new Option("↑", "v", false, f.vertical === true));
      dir.addEventListener("change", () => {
        f.vertical = dir.value === "" ? null : dir.value === "v";
        if (f.vertical == null) delete f.vertical;
        changed();
      });
      row.append(num, text);
      if (!item.fixed) row.append(ratio);
      row.append(dir);
      if (item.fixed) {
        row.classList.add("fixed");
      } else if (i < item.fields.length - 1) {
        const merge = makeButton("⇔", t("fuse.merge"), () => {
          const next = item.fields[i + 1];
          f.ratio = fuseRatio(f) + fuseRatio(next);
          if (!f.text.trim()) f.text = next.text;
          item.fields.splice(i + 1, 1);
          renderList();
          changed();
        });
        merge.type = "button";
        row.append(merge);
      } else {
        row.append(document.createElement("span"));
      }
      list.append(row);
    });
  }
  renderList();
  return [sizeRow, sepRow, optRow, style, sizeField, list];
}

type PipeItem = Extract<Item, { type: "pipe_marker" }>;

/** Recommended cassette of a DIN 2403 group, e.g. "Weiß auf Grün". */
function pipeTapeText(g: PipeGroup): string {
  const tape = t("pipe.tapeHint", { ink: t(`color.${g.ink}`), tape: t(`color.${g.tape}`) });
  const extra = g.extra ? ` ${t(`pipe.extra.${g.extra}`)}` : "";
  return g.style ? tape + extra : `${tape} ${t("pipe.noCassette")}${extra}`;
}

/**
 * Controls shared by the element properties and the wizard: substance
 * group, text, second line, direction, tips, frame and hazard pictograms.
 * `onChange(structural)` is called after every edit.
 */
function pipeControls(m: api.PipeMarkerFields, onChange: (structural: boolean) => void): HTMLElement[] {
  const group = document.createElement("select");
  group.add(new Option(t("pipe.groupNone"), ""));
  for (const g of PIPE_GROUPS) group.add(new Option(`${g.id} – ${t(g.name)}`, String(g.id), false, g.id === m.group));
  const info = document.createElement("p");
  info.className = "muted hint";
  const showInfo = () => {
    const g = pipeGroup(m.group);
    info.textContent = g ? pipeTapeText(g) : "";
    info.hidden = !g;
  };
  showInfo();
  const media = document.createElement("datalist");
  media.id = `pipe-media-${Math.random().toString(36).slice(2)}`;
  const fillMedia = () => media.replaceChildren(...(pipeGroup(m.group)?.media ?? PIPE_GROUPS.flatMap((g) => g.media)).map((v) => new Option(v)));
  fillMedia();
  const text = document.createElement("input");
  text.type = "text";
  text.value = m.text;
  text.setAttribute("list", media.id);
  text.addEventListener("input", () => {
    m.text = text.value;
    onChange(false);
  });
  const sub = document.createElement("input");
  sub.type = "text";
  sub.value = m.sub_text ?? "";
  sub.placeholder = t("pipe.subHint");
  sub.addEventListener("input", () => {
    m.sub_text = sub.value || undefined;
    onChange(false);
  });
  const tips = document.createElement("select");
  for (const v of ["none", "solid", "hatched"] as const) tips.add(new Option(t(`pipe.tips.${v}`), v, false, v === (m.tips ?? "none")));
  tips.addEventListener("change", () => {
    m.tips = tips.value as api.PipeMarkerFields["tips"];
    onChange(false);
  });
  group.addEventListener("change", () => {
    m.group = group.value === "" ? null : Number(group.value);
    const g = pipeGroup(m.group);
    if (g) {
      m.tips = g.tips;
      tips.value = g.tips;
      if (!m.symbols?.length && g.symbols.length) m.symbols = [...g.symbols];
      if (!m.text.trim() || PIPE_GROUPS.some((x) => x.media.includes(m.text))) {
        m.text = g.media[0];
        text.value = m.text;
      }
    }
    showInfo();
    fillMedia();
    renderSymbols();
    onChange(true);
  });
  // Direction: three toggle buttons, one always on.
  const dirRow = document.createElement("div");
  dirRow.className = "row pipe-dir";
  const dirLabel = document.createElement("span");
  dirLabel.textContent = t("pipe.direction");
  dirRow.append(dirLabel);
  const dirButtons = (["left", "both", "right"] as const).map((d) => {
    const b = makeButton({ left: "◀", both: "◀▶", right: "▶" }[d], t(`pipe.dir.${d}`), () => {
      m.direction = d;
      dirButtons.forEach((x) => x.classList.toggle("on", x === b));
      onChange(false);
    });
    b.type = "button";
    b.classList.add("toggle");
    b.classList.toggle("on", (m.direction ?? "right") === d);
    return b;
  });
  dirRow.append(...dirButtons);
  const frame = document.createElement("select");
  for (const v of ["filled", "outline"] as const) frame.add(new Option(t(`pipe.frame.${v}`), v, false, v === (m.frame ?? "filled")));
  frame.addEventListener("change", () => {
    m.frame = frame.value as api.PipeMarkerFields["frame"];
    onChange(false);
  });
  // Hazard pictograms: up to three, chosen in the symbol dialog.
  const symbols = document.createElement("div");
  symbols.className = "row pipe-symbols";
  function renderSymbols(): void {
    symbols.replaceChildren();
    const label = document.createElement("span");
    label.textContent = t("pipe.symbols");
    symbols.append(label);
    (m.symbols ?? []).forEach((name, i) => {
      const found = findIcon(name);
      const chip = makeButton("", `${symbolLabel(name)} – ${t("pipe.symbolRemove")}`, () => {
        m.symbols = (m.symbols ?? []).filter((_, k) => k !== i);
        renderSymbols();
        onChange(true);
      });
      chip.type = "button";
      chip.className = "symbol-chip";
      if (found) {
        const img = document.createElement("img");
        img.src = svgUrl(found.icon.svg);
        img.alt = "";
        chip.append(img);
      } else {
        chip.textContent = name;
      }
      symbols.append(chip);
    });
    if ((m.symbols ?? []).length < 3) {
      const add = makeButton("＋", t("pipe.symbolAdd"), () => {
        openSymbolPicker((m.symbols ?? []).at(-1) ?? "ghs:GHS07", (name) => {
          m.symbols = [...(m.symbols ?? []), name];
          renderSymbols();
          onChange(true);
        });
      });
      add.type = "button";
      symbols.append(add);
    }
  }
  renderSymbols();
  const opts = document.createElement("div");
  opts.className = "row";
  opts.append(field("pipe.tips", tips), field("pipe.frame", frame));
  return [field("pipe.group", group), info, field("pipe.text", text), media, field("pipe.sub", sub), dirRow, opts, symbols];
}

/** Properties of a pipe marker element. */
function pipeFields(item: PipeItem): HTMLElement[] {
  const size = numberInput(item.size_pt, 0.5, t("layout.auto"), (v) => {
    item.size_pt = v && v > 0 ? v : null;
  });
  const font = fontPicker(item.font ?? null, (family) => {
    item.font = family;
    changed(true);
  });
  const row = document.createElement("div");
  row.className = "row";
  row.append(field("elements.size", size), field("elements.font", font));
  return [...pipeControls(item, (structural) => {
    syncBoxCaption();
    changed(structural);
  }), row];
}

type TableItem = Extract<Item, { type: "table" }>;

const TABLE_MAX = 50;

/** Properties of a table: size, grid lines, text style and the cells. */
function tableFields(item: TableItem): HTMLElement[] {
  const cols = () => Math.max(1, ...item.cells.map((r) => r.length));
  /** Every row as long as the widest one. */
  const normalize = () => {
    const n = cols();
    for (const r of item.cells) while (r.length < n) r.push("");
  };
  normalize();
  const grid = document.createElement("div");
  grid.className = "table-grid";
  const rowsInput = numberInput(item.cells.length, 1, "", (v) => {
    const n = Math.max(1, Math.min(TABLE_MAX, Math.round(v ?? 1)));
    while (item.cells.length < n) item.cells.push(Array.from({ length: cols() }, () => ""));
    item.cells.length = n;
    if (item.row_ratios) item.row_ratios.length = Math.min(item.row_ratios.length, n);
    renderGrid();
  });
  rowsInput.min = "1";
  const colsInput = numberInput(cols(), 1, "", (v) => {
    const n = Math.max(1, Math.min(TABLE_MAX, Math.round(v ?? 1)));
    for (const r of item.cells) {
      while (r.length < n) r.push("");
      r.length = n;
    }
    if (item.col_ratios) item.col_ratios.length = Math.min(item.col_ratios.length, n);
    renderGrid();
  });
  colsInput.min = "1";
  const sizeRow = document.createElement("div");
  sizeRow.className = "row";
  sizeRow.append(field("table.rows", rowsInput), field("table.cols", colsInput));

  const check = (label: string, get: () => boolean, set: (v: boolean) => void) => {
    const wrap = document.createElement("label");
    wrap.className = "check";
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = get();
    box.addEventListener("change", () => {
      set(box.checked);
      changed();
    });
    wrap.append(box, ` ${t(label)}`);
    return wrap;
  };
  const line = numberInput(item.line_mm ?? 0.2, 0.05, "0.2", (v) => {
    item.line_mm = v != null && v >= 0 ? Math.min(3, v) : 0.2;
  });
  line.min = "0";
  line.title = t("table.lineHint");
  sizeRow.append(field("table.line", line));
  const lineRow = document.createElement("div");
  lineRow.className = "row";
  lineRow.append(
    check("table.frame", () => item.frame !== false, (v) => (item.frame = v ? undefined : false)),
    check("table.header", () => !!item.header, (v) => (item.header = v || undefined)),
  );

  const size = numberInput(item.size_pt, 0.5, t("layout.auto"), (v) => {
    item.size_pt = v && v > 0 ? v : null;
  });
  const spacing = numberInput(item.line_spacing, 0.1, "1.0", (v) => {
    item.line_spacing = v && v > 0 ? Math.min(3, Math.max(0.5, v)) : null;
  });
  spacing.min = "0.5";
  spacing.max = "3";
  spacing.title = t("elements.lineSpacingHint");
  const sizeField = document.createElement("div");
  sizeField.className = "row";
  sizeField.append(field("elements.size", size), field("elements.lineSpacing", spacing));
  const font = fontPicker(item.font ?? null, (family) => {
    item.font = family;
    changed(true);
  });
  // Cell last edited: with a selection there, F/K style just that part.
  let lastArea: HTMLTextAreaElement | null = null;
  const toggle = (label: string, title: string, key: "bold" | "italic") => {
    const b = makeButton(label, title, () => {
      if (lastArea?.isConnected && toggleMark(lastArea, key === "bold" ? BOLD_MARK : ITALIC_MARK)) return;
      item[key] = !item[key];
      b.classList.toggle("on", !!item[key]);
      changed();
    });
    b.className = `toggle${item[key] ? " on" : ""}`;
    b.style.fontWeight = key === "bold" ? "700" : "";
    b.style.fontStyle = key === "italic" ? "italic" : "";
    return b;
  };
  const boldButton = toggle("F", t("elements.boldHint"), "bold");
  const italicButton = toggle("K", t("elements.italicHint"), "italic");
  const style = document.createElement("div");
  style.className = "row font-row";
  style.append(field("elements.font", font), boldButton, italicButton);

  /** Relative size input for column/row `i` (empty = 1). */
  const ratioInput = (list: "col_ratios" | "row_ratios", i: number, title: string) => {
    const input = numberInput(item[list]?.[i] ?? null, 0.5, "1", (v) => {
      const ratios = item[list] ?? [];
      while (ratios.length <= i) ratios.push(1);
      ratios[i] = v && v > 0 ? Math.min(20, Math.max(0.1, v)) : 1;
      item[list] = ratios.every((r) => r === 1) ? undefined : ratios;
    });
    input.className = "ratio";
    input.min = "0.1";
    input.title = title;
    return input;
  };
  function renderGrid(): void {
    normalize();
    const n = cols();
    rowsInput.value = String(item.cells.length);
    colsInput.value = String(n);
    grid.replaceChildren();
    grid.style.gridTemplateColumns = `3.6em repeat(${n}, minmax(4.5em, 1fr))`;
    grid.append(document.createElement("span"));
    for (let c = 0; c < n; c++) grid.append(ratioInput("col_ratios", c, t("table.colRatio", { n: c + 1 })));
    item.cells.forEach((row, r) => {
      grid.append(ratioInput("row_ratios", r, t("table.rowRatio", { n: r + 1 })));
      row.forEach((cell, c) => {
        const area = document.createElement("textarea");
        area.rows = Math.min(3, Math.max(1, cell.split("\n").length));
        area.value = cell;
        area.addEventListener("input", () => {
          row[c] = area.value;
          area.rows = Math.min(3, Math.max(1, area.value.split("\n").length));
          syncBoxCaption();
          changed();
        });
        area.addEventListener("focus", () => (lastArea = area));
        area.addEventListener("keydown", (e) => {
          if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
          const key = e.key.toLowerCase();
          if (key !== "b" && key !== "i") return;
          e.preventDefault();
          (key === "b" ? boldButton : italicButton).click();
        });
        trackField(area, (v) => (row[c] = v));
        grid.append(area);
      });
    });
  }
  renderGrid();
  const hint = document.createElement("p");
  hint.className = "muted hint";
  hint.textContent = t("table.hint");
  return [sizeRow, lineRow, style, sizeField, grid, hint];
}

function contentFields(item: Item): HTMLElement[] {
  switch (item.type) {
    case "fuse_box":
      return fuseBoxFields(item);
    case "table":
      return tableFields(item);
    case "pipe_marker":
      return pipeFields(item);
    case "text": {
      const area = document.createElement("textarea");
      area.rows = Math.min(4, Math.max(2, item.text.split("\n").length));
      area.value = item.text;
      area.addEventListener("input", () => {
        item.text = area.value;
        syncBoxCaption();
        changed();
      });
      trackField(area, (v) => (item.text = v));
      const size = numberInput(item.size_pt, 0.5, t("layout.auto"), (v) => {
        item.size_pt = v && v > 0 ? v : null;
      });
      const spacing = numberInput(item.line_spacing, 0.1, "1.0", (v) => {
        item.line_spacing = v && v > 0 ? Math.min(3, Math.max(0.5, v)) : null;
      });
      spacing.min = "0.5";
      spacing.max = "3";
      spacing.title = t("elements.lineSpacingHint");
      const row = document.createElement("div");
      row.className = "row";
      row.append(field("elements.size", size), field("elements.lineSpacing", spacing));

      const font = fontPicker(item.font ?? null, (family) => {
        item.font = family;
        changed(true);
      });
      // With text selected in the field: style just that part (inline
      // markers); otherwise the whole element.
      const toggle = (label: string, title: string, key: "bold" | "italic") => {
        const b = makeButton(label, title, () => {
          if (toggleMark(area, key === "bold" ? BOLD_MARK : ITALIC_MARK)) return;
          item[key] = !item[key];
          b.classList.toggle("on", !!item[key]);
          changed(true);
        });
        b.className = `toggle${item[key] ? " on" : ""}`;
        b.style.fontWeight = key === "bold" ? "700" : "";
        b.style.fontStyle = key === "italic" ? "italic" : "";
        return b;
      };
      const boldButton = toggle("F", t("elements.boldHint"), "bold");
      const italicButton = toggle("K", t("elements.italicHint"), "italic");
      area.addEventListener("keydown", (e) => {
        if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
        const key = e.key.toLowerCase();
        if (key !== "b" && key !== "i") return;
        e.preventDefault();
        (key === "b" ? boldButton : italicButton).click();
      });
      const style = document.createElement("div");
      style.className = "row font-row";
      style.append(field("elements.font", font), boldButton, italicButton);
      return [area, style, row];
    }
    case "qr":
      return [textInput(item.data, (v) => (item.data = v)), codeWizardButton(item)];
    case "barcode": {
      const select = document.createElement("select");
      for (const s of api.SYMBOLOGIES) {
        select.add(new Option(api.SYMBOLOGY_NAMES[s], s, false, s === item.symbology));
      }
      select.title = t("elements.symbology");
      select.addEventListener("change", () => {
        item.symbology = select.value as api.Symbology;
        changed(true);
      });
      return [select, textInput(item.data, (v) => (item.data = v)), codeWizardButton(item)];
    }
    case "image": {
      const row = document.createElement("div");
      row.className = "row";
      const name = document.createElement("span");
      name.className = "muted filename";
      name.textContent = item.path ? item.path.split(/[\\/]/).pop()! : t("elements.noImage");
      name.title = item.path;
      row.append(
        makeButton(t("elements.chooseImage"), "", async () => {
          const path = await open({
            multiple: false,
            filters: [{ name: t("file.imageFilter"), extensions: ["png", "jpg", "jpeg", "bmp", "svg"] }],
          });
          if (typeof path === "string") {
            item.path = path;
            item.edit = undefined; // crop/colors belong to the old image
            changed(true);
          }
        }),
        name,
      );
      const invert = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = item.invert;
      box.addEventListener("change", () => {
        item.invert = box.checked;
        changed(true);
      });
      invert.append(box, ` ${t("elements.invert")}`);
      const slider = (key: "brightness" | "contrast") => {
        const wrap = document.createElement("label");
        wrap.className = "range-row";
        const name = document.createElement("span");
        name.textContent = t(`elements.${key}`);
        const range = document.createElement("input");
        range.type = "range";
        range.min = "-100";
        range.max = "100";
        range.step = "5";
        range.value = String(item[key] ?? 0);
        const out = document.createElement("output");
        out.textContent = range.value;
        range.addEventListener("input", () => {
          item[key] = Number(range.value);
          out.textContent = range.value;
          changed();
        });
        range.addEventListener("dblclick", () => {
          range.value = "0";
          range.dispatchEvent(new Event("input"));
        });
        wrap.append(name, range, out);
        return wrap;
      };
      const edit = makeButton(t("imgedit.open"), "", async () => {
        if (!item.path) return;
        // Preview with only this element, on the current label and tape.
        const preview = async (e: api.ImageEdit) =>
          (await api.renderPreview({ ...state.label, elements: [{ ...item, edit: e }] }, selectedModel(), selectedWidth(), null, null, 2)).png;
        const result = await openImageEditor(item.path, item.edit, preview);
        if (result === null) return;
        item.edit = Object.keys(result).length ? result : undefined;
        changed(true);
      });
      // Source: a fixed file or a CSV column holding a path per record.
      const column = /^\{\{\s*(.+?)\s*\}\}$/.exec(item.path)?.[1] ?? null;
      edit.disabled = !item.path || column !== null;
      edit.classList.toggle("on", !!item.edit);
      const source = document.createElement("select");
      source.add(new Option(t("elements.imageFile"), ""));
      const columns = [...(state.csv?.headers ?? [])];
      if (column && !columns.includes(column)) columns.push(column);
      for (const c of columns) source.add(new Option(t("elements.imageColumn", { column: c }), c, false, c === column));
      source.value = column ?? "";
      source.addEventListener("change", () => {
        item.path = source.value ? `{{${source.value}}}` : "";
        item.edit = undefined;
        changed(true);
        renderProps();
      });
      const sourceRow = field("elements.imageSource", source);
      sourceRow.title = t("elements.imageSourceHint");
      if (column !== null) {
        const hint = document.createElement("p");
        hint.className = "muted hint";
        hint.textContent = t("elements.imageSourceHint");
        return [sourceRow, hint, invert, slider("brightness"), slider("contrast")];
      }
      return [sourceRow, row, edit, invert, slider("brightness"), slider("contrast")];
    }
    case "symbol": {
      const select = document.createElement("button");
      select.type = "button";
      select.className = "symbol-choose";
      select.title = t("symbol.choose");
      const found = findIcon(item.name);
      if (found) {
        const img = document.createElement("img");
        img.src = svgUrl(found.icon.svg);
        img.alt = "";
        select.append(img);
      }
      const caption = document.createElement("span");
      caption.textContent = found ? symbolLabel(item.name) : `${item.name} (${t("symbol.unknown")})`;
      select.append(caption);
      select.addEventListener("click", (e) => {
        e.preventDefault();
        openSymbolPicker(item.name, (name) => {
          item.name = name;
          changed(true);
        });
      });
      const invert = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = item.invert;
      box.addEventListener("change", () => {
        item.invert = box.checked;
        changed(true);
      });
      invert.append(box, ` ${t("elements.invert")}`);
      return [select, invert];
    }
    case "fill":
      return [];
    case "shape": {
      const kind = document.createElement("select");
      for (const k of ["line", "rectangle", "rounded_rectangle", "ellipse"] as const) {
        kind.add(new Option(t(`shape.${k}`), k, false, k === item.shape));
      }
      kind.addEventListener("change", () => {
        item.shape = kind.value as api.ShapeKind;
        changed(true);
      });
      const stroke = numberInput(item.stroke_mm, 0.1, "", (v) => {
        if (v !== null && v > 0) item.stroke_mm = v;
      });
      stroke.min = "0.1";
      const filled = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = !!item.filled;
      box.disabled = item.shape === "line";
      box.addEventListener("change", () => {
        item.filled = box.checked;
        changed(true);
      });
      filled.append(box, ` ${t("shape.filled")}`);
      const row = document.createElement("div");
      row.className = "row";
      row.append(field("shape.stroke", stroke), filled);
      return [kind, row];
    }
  }
}

/** Moves/sizes the element's box relative to the label and tape. */
function alignItem(index: number, how: "left" | "hcenter" | "right" | "top" | "vcenter" | "bottom" | "fill"): void {
  const item = state.label.elements[index];
  if (!item?.rect) return;
  const r = { ...item.rect };
  const height = labelHeightMm();
  if (portrait()) {
    // Across the tape = horizontal, along the label = vertical.
    const length = labelLengthWithout(index, r.h_mm);
    if (how === "left") r.x_mm = 0;
    if (how === "hcenter") r.x_mm = (height - r.w_mm) / 2;
    if (how === "right") r.x_mm = height - r.w_mm;
    if (how === "top") r.y_mm = startPad();
    if (how === "vcenter") r.y_mm = (length - r.h_mm) / 2;
    if (how === "bottom") r.y_mm = Math.max(0, length - endPad() - r.h_mm);
    if (how === "fill") {
      r.x_mm = 0;
      r.w_mm = height;
    }
    item.rect = roundRect(r);
    changed(true);
    return;
  }
  const length = labelLengthWithout(index, r.w_mm);
  switch (how) {
    case "left":
      r.x_mm = startPad();
      break;
    case "hcenter":
      r.x_mm = (length - r.w_mm) / 2;
      break;
    case "right":
      r.x_mm = Math.max(0, length - endPad() - r.w_mm);
      break;
    case "top":
      r.y_mm = 0;
      break;
    case "vcenter":
      r.y_mm = (height - r.h_mm) / 2;
      break;
    case "bottom":
      r.y_mm = height - r.h_mm;
      break;
    case "fill":
      r.y_mm = 0;
      r.h_mm = height;
      break;
  }
  item.rect = roundRect(r);
  changed(true);
}

/**
 * Label length in mm the element at `index` is aligned to: the other boxes
 * plus padding, at least the minimum length (exactly it when fixed), and
 * never shorter than the element itself.
 */
function labelLengthWithout(index: number, ownWidth: number): number {
  const l = state.label;
  const others = Math.max(
    0,
    ...l.elements.filter((it, i) => i !== index && it.rect).map((it) => alongStart(it.rect!) + alongSize(it.rect!)),
  );
  const min = l.min_length_mm ?? 0;
  if (l.fixed_length && min > 0) return min;
  return Math.max(others + endPad(), min, ownWidth + startPad() + endPad());
}

function alignRow(index: number): HTMLElement {
  const row = document.createElement("div");
  row.className = "align-row";
  const label = document.createElement("span");
  label.textContent = t("align.title");
  row.append(label);
  const buttons: [string, string, Parameters<typeof alignItem>[1]][] = [
    ["⇤", "align.toLeft", "left"],
    ["↔", "align.toHCenter", "hcenter"],
    ["⇥", "align.toRight", "right"],
    ["⤒", "align.toTop", "top"],
    ["↕", "align.toVCenter", "vcenter"],
    ["⤓", "align.toBottom", "bottom"],
    ["▯", "align.fillHeight", "fill"],
  ];
  for (const [icon, key, how] of buttons) row.append(makeButton(icon, t(key), () => alignItem(index, how)));
  return row;
}

/** Content alignment inside the box: horizontal and vertical, three each. */
function contentAlignRow(item: Item): HTMLElement | null {
  const aligned = ["text", "qr", "barcode", "image", "symbol", "fuse_box", "table"];
  if (!aligned.includes(item.type)) return null;
  const row = document.createElement("div");
  row.className = "align-row";
  const label = document.createElement("span");
  label.textContent = t("content.title");
  row.append(label);
  const h = (): api.TextAlign =>
    item.type === "text"
      ? item.align
      : item.type === "fuse_box" || item.type === "table"
        ? (item.align ?? "center")
        : (item.halign ?? "center");
  const v = (): api.VAlign => item.valign ?? "middle";
  const buttons: [string, string, () => boolean, () => void][] = [
    ["⇤", "content.left", () => h() === "left", () => setH("left")],
    ["↔", "content.hcenter", () => h() === "center", () => setH("center")],
    ["⇥", "content.right", () => h() === "right", () => setH("right")],
    ["⤒", "content.top", () => v() === "top", () => (item.valign = "top")],
    ["↕", "content.vmiddle", () => v() === "middle", () => (item.valign = null)],
    ["⤓", "content.bottom", () => v() === "bottom", () => (item.valign = "bottom")],
  ];
  function setH(a: api.TextAlign): void {
    if (item.type === "text" || item.type === "fuse_box" || item.type === "table") item.align = a;
    else item.halign = a === "center" ? null : a;
  }
  for (const [icon, key, on, apply] of buttons) {
    const b = makeButton(icon, t(key), () => {
      apply();
      changed(true);
    });
    b.classList.toggle("on", on());
    row.append(b);
  }
  return row;
}

/** Removes several elements at once (one undo step). */
function removeItems(indices: number[]): void {
  if (indices.length <= 1) {
    if (indices.length) removeItem(indices[0]);
    return;
  }
  for (const i of [...indices].sort((a, b) => b - a)) state.label.elements.splice(i, 1);
  multi.clear();
  state.selected = -1;
  changed(true);
}

function removeItem(index: number): void {
  multi.clear();
  state.label.elements.splice(index, 1);
  state.selected = Math.min(state.selected, state.label.elements.length - 1);
  changed(true);
}

function duplicateItem(index: number): void {
  multi.clear();
  const copy: Item = JSON.parse(JSON.stringify(state.label.elements[index]));
  // Next to the original along the label length (y in portrait).
  if (copy.rect) {
    const r = copy.rect;
    copy.rect = roundRect(portrait() ? { ...r, y_mm: r.y_mm + r.h_mm } : { ...r, x_mm: r.x_mm + r.w_mm });
  }
  state.label.elements.splice(index + 1, 0, copy);
  state.selected = index + 1;
  changed(true);
}

function lockButton(item: Item): HTMLButtonElement {
  const b = makeButton(item.locked ? "🔒" : "🔓", t(item.locked ? "elements.unlock" : "elements.lock"), () => {
    item.locked = !item.locked || undefined;
    changed(true);
  });
  if (item.locked) b.classList.add("on");
  return b;
}

function elementCard(item: Item, index: number): HTMLLIElement {
  const li = document.createElement("li");
  li.className = `element${index === state.selected ? " selected" : ""}`;
  li.dataset.index = String(index);
  li.addEventListener("pointerdown", (e) => (e.shiftKey || e.ctrlKey || e.metaKey ? toggleSelect(index) : select(index)));

  const header = document.createElement("header");
  const title = document.createElement("strong");
  title.textContent = `${index + 1}. ${elementTitle(item)}`;
  if (item.rotation) title.textContent += ` · ${item.rotation}°`;
  header.append(
    title,
    makeButton("⟳", t("elements.rotate"), () => {
      item.rotation = ((item.rotation ?? 0) + 90) % 360;
      changed(true);
    }),
  );
  const nameInput = document.createElement("input");
  nameInput.type = "text";
  nameInput.value = item.title ?? "";
  nameInput.placeholder = t("elements.namePlaceholder");
  nameInput.addEventListener("input", () => {
    item.title = nameInput.value.trim() || undefined;
    syncBoxCaption();
    commitSoon();
    updateFileName();
  });
  li.append(header, field("elements.name", nameInput), ...contentFields(item));
  if (item.rect && !item.locked) li.append(alignRow(index));
  const contentRow = item.rect ? contentAlignRow(item) : null;
  if (contentRow) li.append(contentRow);
  if (item.rect) li.append(rectFields(item, index));
  return li;
}

/** Layer list text: the user's name, else the content. */
function layerCaption(item: Item): string {
  if (item.title) return item.title;
  return item.type === "fill" || item.type === "shape" ? "" : boxCaption(item);
}

/** Layer list row: name plus show/hide, lock, duplicate, delete. */
function layerRow(item: Item, index: number): HTMLLIElement {
  const li = document.createElement("li");
  li.className = `layer${isSelected(index) ? " selected" : ""}${item.hidden ? " is-hidden" : ""}`;
  li.dataset.index = String(index);
  li.addEventListener("pointerdown", () => select(index));
  const name = document.createElement("span");
  name.className = "layer-name";
  const kind = document.createElement("small");
  kind.textContent = `${index + 1}. ${elementTitle(item)}`;
  const caption = document.createElement("span");
  caption.className = "layer-caption";
  caption.textContent = layerCaption(item);
  caption.title = t("elements.renameHint");
  // Double-click: rename in place (Enter/leaving keeps, Esc cancels).
  caption.addEventListener("dblclick", (e) => {
    e.stopPropagation();
    const input = document.createElement("input");
    input.type = "text";
    input.value = item.title ?? "";
    input.placeholder = boxCaption(item);
    caption.replaceWith(input);
    input.focus();
    input.select();
    let done = false;
    const finish = (keep: boolean) => {
      if (done) return;
      done = true;
      if (keep) item.title = input.value.trim() || undefined;
      changed(true);
    };
    input.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") finish(true);
      if (ev.key === "Escape") finish(false);
    });
    input.addEventListener("blur", () => finish(true));
  });
  name.append(kind, caption);
  // Grip: drag to change the drawing order (lower in the list = in front).
  const grip = document.createElement("span");
  grip.className = "layer-grip";
  grip.textContent = "⠿";
  grip.title = t("elements.reorderHint");
  grip.addEventListener("pointerdown", (e) => startLayerDrag(e, li, index));
  li.prepend(grip);
  const eye = makeButton(item.hidden ? "◌" : "👁", t(item.hidden ? "elements.show" : "elements.hide"), () => {
    item.hidden = !item.hidden || undefined;
    changed(true);
  });
  li.append(
    name,
    eye,
    lockButton(item),
    makeButton("⧉", t("elements.duplicate"), () => duplicateItem(index)),
    makeButton("✕", t("elements.remove"), () => removeItem(index)),
  );
  return li;
}

/** Moves element `from` to position `to` (drawing order), keeping it selected. */
function moveItem(from: number, to: number): void {
  const items = state.label.elements;
  to = Math.max(0, Math.min(items.length - 1, to));
  if (from === to || !items[from]) return;
  multi.clear();
  const [item] = items.splice(from, 1);
  items.splice(to, 0, item);
  state.selected = to;
  changed(true);
}

/** Pointer drag of a layer row by its grip; drops between the rows. */
function startLayerDrag(e: PointerEvent, li: HTMLLIElement, from: number): void {
  e.preventDefault();
  e.stopPropagation();
  const list = $("elements");
  const rows = Array.from(list.querySelectorAll<HTMLLIElement>("li.layer"));
  li.classList.add("dragging");
  let target = from;
  const marker = document.createElement("li");
  marker.className = "layer-drop";
  const move = (ev: PointerEvent) => {
    // Index of the first row whose middle is below the pointer.
    const i = rows.findIndex((r) => {
      const box = r.getBoundingClientRect();
      return ev.clientY < box.top + box.height / 2;
    });
    const slot = i < 0 ? rows.length : i;
    target = slot > from ? slot - 1 : slot;
    if (slot < rows.length) rows[slot].before(marker);
    else rows[rows.length - 1]?.after(marker);
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", up);
    marker.remove();
    li.classList.remove("dragging");
    if (target !== from) moveItem(from, target);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", up);
}

function renderElements(): void {
  const list = $("elements");
  list.replaceChildren();
  if (state.label.elements.length === 0) {
    const empty = document.createElement("li");
    empty.className = "muted";
    empty.textContent = t("elements.empty");
    list.append(empty);
  }
  state.label.elements.forEach((item, i) => list.append(layerRow(item, i)));
  renderProps();
}

type GroupAlign = "left" | "hcenter" | "right" | "top" | "vcenter" | "bottom" | "hspread" | "vspread";

/** Aligns the selected boxes to each other (their common bounding box) or spreads them evenly. */
function alignGroup(how: GroupAlign): void {
  const items = selection()
    .map((i) => state.label.elements[i])
    .filter((it) => it?.rect && !it.locked);
  if (items.length < 2) return;
  const rs = items.map((it) => it.rect!);
  const left = Math.min(...rs.map((r) => r.x_mm));
  const right = Math.max(...rs.map((r) => r.x_mm + r.w_mm));
  const top = Math.min(...rs.map((r) => r.y_mm));
  const bottom = Math.max(...rs.map((r) => r.y_mm + r.h_mm));
  const spread = (axis: "x" | "y") => {
    const pos = axis === "x" ? "x_mm" : "y_mm";
    const size = axis === "x" ? "w_mm" : "h_mm";
    const sorted = [...items].sort((a, b) => a.rect![pos] - b.rect![pos]);
    const total = sorted.reduce((sum, it) => sum + it.rect![size], 0);
    const span = (axis === "x" ? right - left : bottom - top) - total;
    const gap = span / (sorted.length - 1);
    let at = axis === "x" ? left : top;
    for (const it of sorted) {
      it.rect = { ...it.rect!, [pos]: at };
      at += it.rect[size] + gap;
    }
  };
  for (const it of items) {
    const r = { ...it.rect! };
    if (how === "left") r.x_mm = left;
    if (how === "hcenter") r.x_mm = (left + right - r.w_mm) / 2;
    if (how === "right") r.x_mm = right - r.w_mm;
    if (how === "top") r.y_mm = top;
    if (how === "vcenter") r.y_mm = (top + bottom - r.h_mm) / 2;
    if (how === "bottom") r.y_mm = bottom - r.h_mm;
    it.rect = r;
  }
  if (how === "hspread") spread("x");
  if (how === "vspread") spread("y");
  for (const it of items) it.rect = roundRect(it.rect!);
  changed(true);
  renderProps();
}

/** Properties panel head for several selected elements: count and alignment to each other. */
function groupCard(): HTMLElement {
  const card = document.createElement("div");
  card.className = "group-card";
  const head = document.createElement("p");
  head.className = "group-head";
  head.textContent = t("group.count", { n: selection().length });
  const row = document.createElement("div");
  row.className = "align-row";
  const label = document.createElement("span");
  label.textContent = t("group.align");
  row.append(label);
  const buttons: [string, string, GroupAlign][] = [
    ["⇤", "group.left", "left"],
    ["↔", "group.hcenter", "hcenter"],
    ["⇥", "group.right", "right"],
    ["⤒", "group.top", "top"],
    ["↕", "group.vcenter", "vcenter"],
    ["⤓", "group.bottom", "bottom"],
    ["⋯", "group.hspread", "hspread"],
    ["⋮", "group.vspread", "vspread"],
  ];
  for (const [icon, key, how] of buttons) row.append(makeButton(icon, t(key), () => alignGroup(how)));
  const hint = document.createElement("p");
  hint.className = "muted hint";
  hint.textContent = t("group.hint");
  card.append(head, row, hint);
  return card;
}

/** Right panel: every setting of the selected element. */
function renderProps(): void {
  const body = $("props-body");
  body.replaceChildren();
  if (selection().length > 1) body.append(groupCard());
  const item = state.label.elements[state.selected];
  if (!item) {
    const hint = document.createElement("p");
    hint.className = "muted hint";
    hint.textContent = t("props.none");
    body.append(hint);
    return;
  }
  body.append(elementCard(item, state.selected));
}

function defaultElement(type: Element["type"]): Element {
  switch (type) {
    case "text":
      return { type, text: "Text", size_pt: null, align: "center" };
    case "qr":
      return { type, data: "https://" };
    case "barcode":
      return { type, symbology: "code128", data: "12345" };
    case "image":
      return { type, path: "", invert: false };
    case "symbol":
      return { type, name: "material:warning", invert: false };
    case "fill":
      return { type };
    case "shape":
      return { type, shape: "rectangle", stroke_mm: 0.3, filled: false };
    case "fuse_box":
      return {
        type,
        fields: Array.from({ length: FUSE_DEFAULT_COUNT }, (_, i) => ({ text: `F${i + 1}` })),
        pitch_mm: FUSE_DEFAULT_PITCH_MM,
        separator: "frame",
      };
    case "pipe_marker":
      return { type, text: "Wasser", group: 1 };
    case "table":
      return {
        type,
        cells: [
          ["A1", "B1", "C1"],
          ["A2", "B2", "C2"],
        ],
      };
  }
}

// ---------------------------------------------------------------- label settings

function renderLayout(): void {
  const l = state.label;
  $<HTMLInputElement>("padding").value = String(l.padding_mm);
  $<HTMLInputElement>("padding-start").value = String(l.padding_start_mm ?? l.padding_mm);
  $<HTMLInputElement>("min-length").value = l.min_length_mm ? String(l.min_length_mm) : "";
  $<HTMLInputElement>("fixed-length").checked = !!l.fixed_length;
  $<HTMLSelectElement>("orientation").value = l.orientation ?? "landscape";
  $("edit-template-row").hidden = !l.source;
  renderDecorButton();
  renderBorder();
  $<HTMLSelectElement>("strips").value = String(strips());
  updateStripsHint();
}

function updateStripsHint(): void {
  const select = $<HTMLSelectElement>("strips");
  const overlap = Math.max(0, selectedWidth() - tapeMm()).toFixed(1);
  select.parentElement!.title = strips() > 1
    ? t("preview.stripsHint", { n: strips(), width: selectedWidth(), overlap })
    : t("device.stripsHint");
}

function bindLayout(): void {
  const num = (id: string, apply: (v: number | null) => void) => {
    const input = $<HTMLInputElement>(id);
    input.addEventListener("input", () => {
      const v = input.value === "" ? null : Math.max(0, Number(input.value));
      apply(v === null || Number.isNaN(v) ? null : v);
      changed();
    });
  };
  // Changing the left margin moves the boxes along, so the content keeps
  // its distance to the margin instead of sliding into it.
  $<HTMLSelectElement>("orientation").addEventListener("change", (e) => {
    void setOrientation((e.target as HTMLSelectElement).value as api.Orientation);
  });
  num("padding-start", (v) => {
    const before = startPad();
    state.label.padding_start_mm = v;
    // Not clamped at 0: typing 1 → 3 → 1 must give the original positions back.
    const delta = startPad() - before;
    for (const item of state.label.elements) {
      if (!item.rect) continue;
      item.rect = roundRect(
        portrait()
          ? { ...item.rect, y_mm: item.rect.y_mm + delta }
          : { ...item.rect, x_mm: item.rect.x_mm + delta },
      );
    }
    renderBoxes();
    state.label.elements.forEach((_, i) => updateRectInputs(i));
  });
  num("padding", (v) => (state.label.padding_mm = v ?? 0));
  num("min-length", (v) => (state.label.min_length_mm = v && v > 0 ? v : null));
  $<HTMLInputElement>("fixed-length").addEventListener("change", (e) => {
    state.label.fixed_length = (e.target as HTMLInputElement).checked || undefined;
    changed(true);
  });
  bindBorder();
}

// ---------------------------------------------------------------- border

/** Minimum gap between border and content, mm (`BORDER_CLEARANCE_MM` in `ll-core`). */
const BORDER_CLEARANCE_MM = 0.3;

/** Default border (matches `ll_core::label::LabelBorder::default`). */
function defaultBorder(): api.Border {
  return {
    style: "solid",
    width_mm: Math.round((2 / DOTS_PER_MM) * 100) / 100,
    sides: { top: true, bottom: true, left: true, right: true },
    pattern_mm: 1.5,
    inset_mm: 0,
  };
}

/** The border in effect: `border`, or the default one for the older `frame` flag. */
function currentBorder(): api.Border | null {
  return state.label.border ?? (state.label.frame ? defaultBorder() : null);
}

function renderBorder(): void {
  const b = currentBorder();
  $<HTMLSelectElement>("border-style").value = b?.style ?? "";
  const d = b ?? defaultBorder();
  $<HTMLInputElement>("border-width").value = String(d.width_mm);
  $<HTMLInputElement>("border-pattern").value = String(d.pattern_mm);
  const insets = borderInsets(d);
  document.querySelectorAll<HTMLInputElement>("[data-inset]").forEach((input) => {
    input.value = String(insets[input.dataset.inset as BorderSide]);
  });
  document.querySelectorAll<HTMLInputElement>("[data-side]").forEach((c) => {
    c.checked = d.sides[c.dataset.side as keyof api.Border["sides"]];
  });
  document.querySelectorAll<HTMLElement>(".border-opt").forEach((el) => (el.hidden = !b));
  const patterned = b?.style === "dashed" || b?.style === "striped";
  document.querySelectorAll<HTMLElement>(".border-pattern").forEach((el) => (el.hidden = !patterned));
}

type BorderSide = keyof api.Border["sides"];

/** Border distance per side (matches `LabelBorder::insets` in `ll-core`). */
function borderInsets(b: api.Border): Record<BorderSide, number> {
  const i = b.insets_mm ?? { top: b.inset_mm, bottom: b.inset_mm, left: b.inset_mm, right: b.inset_mm };
  return { top: Math.max(0, i.top), bottom: Math.max(0, i.bottom), left: Math.max(0, i.left), right: Math.max(0, i.right) };
}

/** Space the border takes from each edge, in mm (matches `border_reserve` in `ll-core`). */
function borderReserveMm(b: api.Border, side: BorderSide): number {
  const width = Math.max(0, b.width_mm);
  return borderInsets(b)[side] + width + Math.max(width, BORDER_CLEARANCE_MM);
}

/**
 * Moves/shrinks boxes so they don't overlap the border on the top, bottom
 * and left edges (the label end grows by itself). Returns whether a box
 * changed.
 */
function fitBoxesInsideBorder(b: api.Border): boolean {
  if (portrait()) return false; // border sides refer to the landscape tape
  const top = b.sides.top ? borderReserveMm(b, "top") : 0;
  const bottom = labelHeightMm() - (b.sides.bottom ? borderReserveMm(b, "bottom") : 0);
  // The border's left edge sits inside the label margin.
  const left = b.sides.left ? startPad() + borderReserveMm(b, "left") : 0;
  let moved = false;
  for (const item of state.label.elements) {
    const rect = item.rect;
    if (!rect) continue;
    const y = Math.max(rect.y_mm, top);
    const end = Math.min(rect.y_mm + rect.h_mm, bottom);
    const x = Math.max(rect.x_mm, left);
    if (end - y < 1) continue; // box lies (almost) entirely in the border area: leave it
    const round = (v: number) => Math.round(v * 100) / 100;
    const next = { x_mm: round(x), y_mm: round(y), w_mm: rect.w_mm, h_mm: round(end - y) };
    if (next.x_mm !== rect.x_mm || next.y_mm !== rect.y_mm || next.h_mm !== rect.h_mm) {
      item.rect = next;
      moved = true;
    }
  }
  return moved;
}

function bindBorder(): void {
  /** Applies `edit` to a copy of the current border and stores it. */
  const update = (edit: (b: api.Border) => void, rerender = false) => {
    const b = { ...(currentBorder() ?? defaultBorder()) };
    b.sides = { ...b.sides };
    edit(b);
    state.label.border = b;
    state.label.frame = false;
    const moved = fitBoxesInsideBorder(b);
    if (rerender) renderBorder();
    changed(rerender || moved);
  };
  $<HTMLSelectElement>("border-style").addEventListener("change", (e) => {
    const style = (e.target as HTMLSelectElement).value as api.BorderStyle | "";
    if (!style) {
      state.label.border = null;
      state.label.frame = false;
      renderBorder();
      changed(true);
      return;
    }
    update((b) => (b.style = style), true);
  });
  const num = (id: string, min: number, apply: (b: api.Border, v: number) => void) => {
    $<HTMLInputElement>(id).addEventListener("input", (e) => {
      const v = Number((e.target as HTMLInputElement).value);
      if (Number.isFinite(v) && v >= min) update((b) => apply(b, v));
    });
  };
  num("border-width", 0.05, (b, v) => (b.width_mm = v));
  num("border-pattern", 0.1, (b, v) => (b.pattern_mm = v));
  document.querySelectorAll<HTMLInputElement>("[data-inset]").forEach((input) => {
    num(input.id, 0, (b, v) => {
      b.insets_mm = { ...borderInsets(b), [input.dataset.inset as BorderSide]: v };
    });
  });
  document.querySelectorAll<HTMLInputElement>("[data-side]").forEach((c) => {
    c.addEventListener("change", () =>
      update((b) => (b.sides[c.dataset.side as keyof api.Border["sides"]] = c.checked), true),
    );
  });
}

// ---------------------------------------------------------------- font picker

/** CSS `font-family` value for a system font family name. */
function cssFont(family: string): string {
  return `"${family.replace(/["\\]/g, "")}", system-ui, sans-serif`;
}

/**
 * Font dropdown that shows every family in its own font (a native
 * `<select>` can't style its options reliably in the webviews), with a
 * search field and keyboard navigation.
 */
function fontPicker(current: string | null, onPick: (family: string | null) => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "fontpick-btn";
  button.textContent = current ?? t("elements.defaultFont");
  if (current) button.style.fontFamily = cssFont(current);
  button.addEventListener("click", (e) => {
    e.preventDefault();
    e.stopPropagation();
    openFontPopup(button, current, onPick);
  });
  return button;
}

function openFontPopup(anchor: HTMLElement, current: string | null, onPick: (family: string | null) => void): void {
  document.querySelector(".fontpick-pop")?.remove();
  const pop = document.createElement("div");
  pop.className = "fontpick-pop";
  const search = document.createElement("input");
  search.type = "search";
  search.placeholder = t("font.search");
  const list = document.createElement("div");
  list.className = "fontpick-list";
  pop.append(search, list);

  const families = current && !state.fonts.includes(current) ? [current, ...state.fonts] : state.fonts;
  let shown: (string | null)[] = [];
  let active = 0;

  const close = () => {
    pop.remove();
    document.removeEventListener("pointerdown", outside, true);
  };
  const pick = (family: string | null) => {
    close();
    if (family !== current) onPick(family);
  };
  const outside = (e: Event) => {
    if (!pop.contains(e.target as Node) && e.target !== anchor) close();
  };
  const highlight = () => {
    list.querySelectorAll(".fontpick-item").forEach((el, i) => el.classList.toggle("active", i === active));
    list.children[active]?.scrollIntoView({ block: "nearest" });
  };
  const render = () => {
    const q = search.value.trim().toLowerCase();
    shown = [...(q ? [] : [null]), ...families.filter((f) => f.toLowerCase().includes(q))];
    list.replaceChildren();
    for (const family of shown) {
      const item = document.createElement("div");
      item.className = `fontpick-item${family === current ? " current" : ""}`;
      if (family) {
        item.style.fontFamily = cssFont(family);
        item.textContent = family;
      } else {
        item.textContent = t("elements.defaultFont");
      }
      item.title = family ?? t("elements.defaultFont");
      item.addEventListener("click", () => pick(family));
      list.append(item);
    }
    if (!shown.length) {
      const none = document.createElement("div");
      none.className = "muted hint";
      none.textContent = t("font.none");
      list.append(none);
    }
    active = Math.max(0, shown.indexOf(current));
    if (q) active = 0;
    highlight();
  };
  search.addEventListener("input", render);
  search.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.min(Math.max(0, active + (e.key === "ArrowDown" ? 1 : -1)), shown.length - 1);
      highlight();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (shown.length) pick(shown[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  });

  const r = anchor.getBoundingClientRect();
  const width = Math.max(r.width, 280);
  pop.style.left = `${Math.min(r.left, window.innerWidth - width - 8)}px`;
  pop.style.width = `${width}px`;
  const below = window.innerHeight - r.bottom;
  if (below < 300 && r.top > below) pop.style.bottom = `${window.innerHeight - r.top + 4}px`;
  else pop.style.top = `${r.bottom + 4}px`;
  document.body.append(pop);
  document.addEventListener("pointerdown", outside, true);
  render();
  search.focus();
}

function renderAll(): void {
  renderElements();
  renderLayout();
  renderBoxes();
  updateFileName();
  schedulePreview();
}

// ---------------------------------------------------------------- devices, status, print

function fillWidths(preferred?: number): void {
  const select = $<HTMLSelectElement>("width");
  const current = preferred ?? (selectedWidth() || 12);
  const model = state.models.find((m) => m.name === selectedModel());
  select.replaceChildren();
  for (const tape of model?.tapes ?? []) {
    select.add(new Option(`${tape.width_mm} mm`, String(tape.width_mm), false, tape.width_mm === current));
  }
  fitZoomPending = true;
}

function tapeChanged(): void {
  if (fitZoomPending) {
    fitZoomPending = false;
    fitZoom();
  }
  layoutStage();
  schedulePreview();
}

function selectedConnection(): Connection | null {
  const index = Number($<HTMLSelectElement>("device").value);
  return Number.isInteger(index) && state.devices[index] ? state.devices[index].connection : null;
}

function setStatus(text: string, kind: "" | "ok" | "error" = ""): void {
  const el = $("status");
  el.textContent = text;
  el.className = `status ${kind}`;
}

function setMessage(text: string, isError = false): void {
  const el = $("message");
  el.textContent = text;
  el.className = isError ? "message error" : "message";
}

const DEVICE_KEY = "labellab.device";
/** Device bar and print bar fields remembered in the settings file (model before tape). */
const PERSISTED_FIELDS = ["model", "width", "strips", "copies", "margin", "cut-marks", "mirror", "zoom", "quality"];

/** Restores the remembered fields and saves them on every change. */
function bindPersistedFields(): void {
  for (const id of PERSISTED_FIELDS) {
    const el = $<HTMLInputElement | HTMLSelectElement>(id);
    const key = `labellab.field.${id}`;
    const isCheck = el instanceof HTMLInputElement && el.type === "checkbox";
    const saved = getSetting(key);
    const known = !(el instanceof HTMLSelectElement) || Array.from(el.options).some((o) => o.value === saved);
    if (saved !== null && known) {
      if (isCheck) (el as HTMLInputElement).checked = saved === "1";
      else el.value = saved;
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
    }
    const save = () => setSetting(key, isCheck ? ((el as HTMLInputElement).checked ? "1" : "0") : el.value);
    el.addEventListener("input", save);
    el.addEventListener("change", save);
  }
  const device = $<HTMLSelectElement>("device");
  device.addEventListener("change", () => {
    const d = state.devices[Number(device.value)];
    if (d) setSetting(DEVICE_KEY, d.name);
  });
}

async function refreshDevices(autoStatus = true): Promise<void> {
  const select = $<HTMLSelectElement>("device");
  const { devices, warnings } = await api.listDevices();
  state.devices = devices;
  select.replaceChildren();
  if (devices.length === 0) select.add(new Option(t("device.none"), ""));
  devices.forEach((d, i) => select.add(new Option(d.model ? `${d.name} – ${d.model}` : d.name, String(i))));
  if (warnings.length) console.warn("device enumeration:", warnings);

  // Pick the printer used last time, else the first recognized one, switch
  // to its model and read the tape status right away.
  const last = getSetting(DEVICE_KEY);
  const lastIndex = last ? devices.findIndex((d) => d.name === last) : -1;
  const index = lastIndex >= 0 ? lastIndex : devices.findIndex((d) => d.model);
  if (index >= 0) {
    select.value = String(index);
    applyDeviceModel(devices[index]);
    if (autoStatus) {
      setStatus(t("device.autoStatus"));
      await readStatus();
    }
  }
}

function applyDeviceModel(device: Device | undefined): void {
  const modelSelect = $<HTMLSelectElement>("model");
  if (device?.model && device.model !== modelSelect.value) {
    modelSelect.value = device.model;
    fillWidths();
    tapeChanged();
  }
}

function hex(v: number): string {
  return `0x${v.toString(16).padStart(2, "0")}`;
}

async function readStatus(): Promise<boolean> {
  const connection = selectedConnection();
  if (!connection) {
    setStatus(t("print.noDevice"), "error");
    return false;
  }
  setStatus(t("device.reading"));
  try {
    const s = await api.queryStatus(connection);
    if (s.has_error) {
      // Known bits in words ("Akku schwach"), else the raw bytes.
      const known = s.errors.map((id) => t(`printerError.${id}`)).join(", ");
      setStatus(
        known ? t("device.statusErrors", { errors: known }) : t("device.statusError", { e1: hex(s.error1), e2: hex(s.error2) }),
        "error",
      );
      return false;
    }
    const model = state.models.find((m) => m.name === selectedModel());
    if (!model?.tapes.some((tp) => tp.width_mm === s.width_mm)) {
      setStatus(t("device.tapeUnknown", { width: s.width_mm }), "error");
      return false;
    }
    if (s.width_mm !== selectedWidth()) {
      fillWidths(s.width_mm);
      tapeChanged();
    }
    if (s.tape_color_id && s.text_color_id && s.tape_color_id in TAPE_CSS && s.text_color_id in INK_CSS) {
      applyReportedTape({ tape: s.tape_color_id, ink: s.text_color_id });
    }
    setStatus(t("device.statusOk", { width: s.width_mm }), "ok");
    return true;
  } catch (e) {
    setStatus(t("error.prefix", { error: errorText(e) }), "error");
    return false;
  }
}

// ---------------------------------------------------------------- printer info

const hexByte = (b: number) => b.toString(16).padStart(2, "0").toUpperCase();

/** Translation of `prefix.id`, or the raw code when unknown. */
function codeName(prefix: string, id: string | null, code: number): string {
  if (!id) return `${t("info.unknownCode")} (0x${hexByte(code)})`;
  const key = `${prefix}.${id}`;
  const text = t(key);
  return text === key ? id : text;
}

async function showPrinterInfo(): Promise<void> {
  const dialog = $<HTMLDialogElement>("info-dialog");
  if (!dialog.open) dialog.showModal();
  const msg = $("info-msg");
  const table = $("info-table");
  const raw = $("info-raw");
  const connection = selectedConnection();
  table.replaceChildren();
  raw.textContent = "";
  if (!connection) {
    msg.textContent = t("print.noDevice");
    return;
  }
  msg.textContent = t("device.reading");
  try {
    const i = await api.printerInfo(connection);
    msg.textContent = t("info.intro");
    const errors = i.errors.length ? i.errors.map((e) => codeName("printerError", e, 0)).join(", ") : t("info.noErrors");
    const rows: [string, string][] = [
      ["info.model", i.model ?? `${t("info.unknownCode")} (0x${hexByte(i.series_byte)} 0x${hexByte(i.model_byte)})`],
      ["info.width", `${i.width_mm} mm`],
      ["info.mediaType", codeName("media", i.media_type_id, i.media_type)],
      ["info.tapeColor", codeName("color", i.tape_color_id, i.raw[24])],
      ["info.textColor", codeName("color", i.text_color_id, i.raw[25])],
      ["info.errors", `${errors} (0x${hexByte(i.error1)} 0x${hexByte(i.error2)})`],
      ["info.statusType", `0x${hexByte(i.status_type)} · ${t("info.phase")} 0x${hexByte(i.phase)}`],
    ];
    for (const [key, value] of rows) {
      const tr = document.createElement("tr");
      const th = document.createElement("th");
      th.textContent = t(key);
      const td = document.createElement("td");
      td.textContent = value;
      tr.append(th, td);
      table.append(tr);
    }
    // The printer only knows the cassette's coding: let the user say what is really inserted.
    if (i.tape_color_id && i.text_color_id) {
      const tr = document.createElement("tr");
      const th = document.createElement("th");
      th.textContent = t("info.useAs");
      const td = document.createElement("td");
      const select = document.createElement("select");
      const current = $<HTMLSelectElement>("tape-style").value;
      const shown = parseStyleKey(current);
      const styles = TAPE_STYLES.some((st) => styleKey(st) === current) || !shown ? TAPE_STYLES : [shown, ...TAPE_STYLES];
      for (const st of styles) select.add(new Option(styleName(st), styleKey(st), false, styleKey(st) === current));
      select.addEventListener("change", () => {
        const tape = $<HTMLSelectElement>("tape-style");
        if (!Array.from(tape.options).some((o) => o.value === select.value)) fillTapeStyles(parseStyleKey(select.value) ?? undefined);
        tape.value = select.value;
        applyTapeStyle();
      });
      const hint = document.createElement("div");
      hint.className = "muted small";
      hint.textContent = t("info.useAsHint");
      td.append(select, hint);
      tr.append(th, td);
      table.append(tr);
    }
    raw.textContent = [0, 8, 16, 24]
      .map((o) => `${String(o).padStart(2, "0")}: ${i.raw.slice(o, o + 8).map(hexByte).join(" ")}`)
      .join("\n");
  } catch (e) {
    msg.textContent = t("error.prefix", { error: errorText(e) });
  }
}

function bindPrinterInfo(): void {
  $("btn-info").addEventListener("click", () => void showPrinterInfo());
  $("info-reload").addEventListener("click", () => void showPrinterInfo());
  $("info-copy").addEventListener("click", () => {
    const text = $("info-raw").textContent ?? "";
    void navigator.clipboard?.writeText(text).then(
      () => setStatus(t("info.copied"), "ok"),
      () => undefined,
    );
  });
}

// ---------------------------------------------------------------- decorative frames

let frameSets: api.FrameSet[] = [];

async function loadFrameSets(): Promise<void> {
  try {
    frameSets = await api.frameSets();
  } catch {
    frameSets = [];
  }
  renderDecorButton();
}

function findFrame(name: string | null | undefined): (api.FrameDef & { preview: string }) | null {
  if (!name) return null;
  const [setId, frameId] = name.split(":");
  return frameSets.find((s) => s.id === setId)?.frames.find((f) => f.id === frameId) ?? null;
}

function renderDecorButton(): void {
  const frame = findFrame(state.label.decor);
  const thumb = $<HTMLImageElement>("decor-thumb");
  thumb.hidden = !frame;
  if (frame) thumb.src = `data:image/png;base64,${frame.preview}`;
  $("decor-name").textContent = frame ? textOf(frame.name) : state.label.decor ? `${state.label.decor} (?)` : t("decor.none");
}

/** Width : height of an SVG segment (from its width/height or viewBox). */
function svgAspect(svg: string): number {
  const doc = new DOMParser().parseFromString(svg, "image/svg+xml").documentElement;
  const vb = (doc.getAttribute("viewBox") ?? "").split(/[\s,]+/).map(Number);
  const w = parseFloat(doc.getAttribute("width") ?? "") || vb[2];
  const h = parseFloat(doc.getAttribute("height") ?? "") || vb[3];
  return w > 0 && h > 0 ? w / h : 0;
}

/** Moves boxes that start inside the frame's start piece to just after it. */
function fitBoxesInsideDecor(frame: api.FrameDef): void {
  if (portrait()) return;
  const left = startPad() + svgAspect(frame.start) * labelHeightMm();
  for (const item of state.label.elements) {
    const r = item.rect;
    if (r && r.x_mm < left) item.rect = roundRect({ ...r, x_mm: left });
  }
}

function setDecor(name: string | null): void {
  state.label.decor = name ?? undefined;
  const frame = findFrame(name);
  if (frame) fitBoxesInsideDecor(frame);
  renderDecorButton();
  changed(true);
}

function renderDecorList(): void {
  const list = $("decor-list");
  list.replaceChildren();
  const tile = (label: string, preview: string | null, name: string | null) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = `decor-tile${(state.label.decor ?? null) === name ? " on" : ""}`;
    if (preview) {
      const img = document.createElement("img");
      img.src = `data:image/png;base64,${preview}`;
      img.alt = "";
      b.append(img);
    }
    const caption = document.createElement("span");
    caption.textContent = label;
    b.append(caption);
    b.addEventListener("click", () => {
      setDecor(name);
      $<HTMLDialogElement>("decor-dialog").close();
    });
    return b;
  };
  list.append(tile(t("decor.none"), null, null));
  for (const set of frameSets) {
    const h = document.createElement("h3");
    h.textContent = textOf(set.name);
    if (!set.builtin && set.id !== "eigene") {
      h.append(
        makeButton("✕", t("decor.removeSet"), async () => {
          if (!(await ask(t("decor.removeSetConfirm", { name: textOf(set.name) }), { kind: "warning" }))) return;
          await api.removeFrameSet(set.id);
          await loadFrameSets();
          renderDecorList();
        }),
      );
    }
    const grid = document.createElement("div");
    grid.className = "decor-grid";
    for (const f of set.frames) {
      const name = `${set.id}:${f.id}`;
      const cell = tile(textOf(f.name), f.preview, name);
      if (set.id === "eigene") {
        const tools = document.createElement("span");
        tools.className = "decor-tools";
        tools.append(
          makeButton("✎", t("decor.edit"), () => openFrameEditor(f)),
          makeButton("✕", t("decor.delete"), async () => {
            await api.deleteFrame(f.id);
            if (state.label.decor === name) setDecor(null);
            await loadFrameSets();
            renderDecorList();
          }),
        );
        cell.append(tools);
      }
      grid.append(cell);
    }
    list.append(h, grid);
  }
}

let editingFrameId: string | null = null;
let framePreviewTimer: number | undefined;

function editedFrame(): api.FrameDef {
  const name = $<HTMLInputElement>("fe-name").value.trim() || t("frameEditor.untitled");
  const slug = name.toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "rahmen";
  return {
    id: editingFrameId ?? `${slug}-${Date.now().toString(36)}`,
    name,
    start: segmentSvg("start"),
    middle: segmentSvg("middle"),
    end: segmentSvg("end"),
  };
}

function scheduleFramePreview(): void {
  window.clearTimeout(framePreviewTimer);
  framePreviewTimer = window.setTimeout(async () => {
    const msg = $("fe-msg");
    try {
      $<HTMLImageElement>("fe-preview").src = `data:image/png;base64,${await api.framePreview(editedFrame())}`;
      msg.textContent = "";
    } catch (e) {
      msg.textContent = t("error.prefix", { error: errorText(e) });
    }
  }, 250);
}

/** Printable height of the selected tape in print dots (1 drawing pixel = 1 dot). */
function tapeDots(): number {
  return Math.max(8, Math.round((tapeMm() * 180) / 25.4));
}

async function openFrameEditor(frame?: api.FrameDef): Promise<void> {
  editingFrameId = frame?.id ?? null;
  $<HTMLInputElement>("fe-name").value = frame ? textOf(frame.name) : "";
  $("fe-msg").textContent = "";
  if (frame) await loadPaint(frame, tapeDots());
  else newPaint(tapeDots());
  $<HTMLDialogElement>("frame-editor").showModal();
  scheduleFramePreview();
}

function bindDecor(): void {
  $("btn-decor").addEventListener("click", () => {
    renderDecorList();
    $<HTMLDialogElement>("decor-dialog").showModal();
  });
  $("decor-new").addEventListener("click", () => openFrameEditor());
  $("decor-import").addEventListener("click", async () => {
    const path = await open({ multiple: false, filters: [{ name: t("decor.fileFilter"), extensions: ["llabel-frames"] }] });
    if (typeof path !== "string") return;
    try {
      await api.importFrameSet(path);
      await loadFrameSets();
      renderDecorList();
    } catch (e) {
      setStatus(t("error.prefix", { error: errorText(e) }), "error");
    }
  });
  bindPaint(scheduleFramePreview);
  $<HTMLInputElement>("fe-name").addEventListener("input", scheduleFramePreview);
  $("fe-save").addEventListener("click", async () => {
    try {
      const name = await api.saveFrame(editedFrame());
      $<HTMLDialogElement>("frame-editor").close();
      await loadFrameSets();
      setDecor(name);
      if ($<HTMLDialogElement>("decor-dialog").open) renderDecorList();
    } catch (e) {
      $("fe-msg").textContent = t("error.prefix", { error: errorText(e) });
    }
  });
}

// ---------------------------------------------------------------- collapsible sidebar

const SECTIONS_KEY = "labellab.sections";
/** Sections open on first start (by title key); the rest starts collapsed. */
const SECTIONS_OPEN_BY_DEFAULT = ["elements.title", "layout.title"];

/**
 * Makes every sidebar section (an `h2` and what follows up to the next
 * one) collapsible by clicking its title. The open/closed state is
 * remembered per viewer. Runs before the UI is bound; it only moves nodes.
 */
function makeSectionsCollapsible(): void {
  let saved: Record<string, boolean> = {};
  try {
    saved = JSON.parse(getSetting(SECTIONS_KEY) ?? "{}");
  } catch {
    // defaults
  }
  const panel = document.querySelector<HTMLElement>("aside.panel");
  if (!panel) return;
  for (const h2 of Array.from(panel.querySelectorAll<HTMLElement>(":scope > h2"))) {
    const key = h2.dataset.i18n ?? "";
    const section = document.createElement("section");
    section.className = "side-section";
    const body = document.createElement("div");
    body.className = "side-body";
    h2.before(section);
    let next = h2.nextElementSibling;
    while (next && next.tagName !== "H2") {
      const after = next.nextElementSibling;
      body.append(next);
      next = after;
    }
    section.append(h2, body);
    const open = saved[key] ?? SECTIONS_OPEN_BY_DEFAULT.includes(key);
    section.classList.toggle("collapsed", !open);
    h2.tabIndex = 0;
    const toggle = () => {
      section.classList.toggle("collapsed");
      saved[key] = !section.classList.contains("collapsed");
      try {
        setSetting(SECTIONS_KEY, JSON.stringify(saved));
      } catch {
        // not remembered
      }
    };
    h2.addEventListener("click", toggle);
    h2.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        toggle();
      }
    });
  }
}

// ---------------------------------------------------------------- splash / about

/** Shortest time the start screen stays up, ms. */
const SPLASH_MIN_MS = 1800;

function showSplash(): void {
  $("splash").classList.remove("hidden");
}

function hideSplash(): void {
  $("splash").classList.add("hidden");
}

function bindSplash(): void {
  $<HTMLImageElement>("splash-icon").src = appIcon;
  void getVersion().then(
    (v) => ($("splash-version").textContent = v),
    () => undefined,
  );
  $("splash").addEventListener("click", hideSplash);
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") hideSplash();
  });
  $("btn-about").addEventListener("click", showSplash);
}

// ---------------------------------------------------------------- cut options

const CUT_MODE_KEY = "labellab.cutMode";

/** Print settings from the cut mode select (like the cut options of other label editors). */
function cutSettings(): Pick<api.PrintJob, "cut" | "chain" | "cutEvery" | "cutMarks" | "mirror"> {
  const mode = $<HTMLSelectElement>("cut-mode").value;
  const every = Math.max(2, Number($<HTMLInputElement>("cut-every").value) || 2);
  return {
    cut: mode === "each" || mode === "end" || mode === "every",
    chain: mode === "end" || mode === "every" || mode === "chain",
    cutEvery: mode === "every" ? every : 0,
    cutMarks: $<HTMLInputElement>("cut-marks").checked,
    mirror: $<HTMLInputElement>("mirror").checked,
  };
}

function bindCutOptions(): void {
  const select = $<HTMLSelectElement>("cut-mode");
  try {
    const saved = getSetting(CUT_MODE_KEY);
    if (saved && Array.from(select.options).some((o) => o.value === saved)) select.value = saved;
  } catch {
    // default
  }
  const update = () => {
    $("cut-every-wrap").hidden = select.value !== "every";
    try {
      setSetting(CUT_MODE_KEY, select.value);
    } catch {
      // not remembered
    }
  };
  select.addEventListener("change", update);
  update();
}

// ---------------------------------------------------------------- keep-alive

/**
 * Interval of the optional keep-alive status query. Meant to stop the
 * printer's automatic power-off; TODO(verify): whether a status request
 * resets the auto-off timer is unconfirmed (docs/PROTOCOL.md).
 */
const KEEPALIVE_MS = 2 * 60 * 1000;
let keepAliveTimer: number | undefined;

function setKeepAlive(on: boolean): void {
  window.clearInterval(keepAliveTimer);
  keepAliveTimer = on ? window.setInterval(() => void keepAlivePing(), KEEPALIVE_MS) : undefined;
  const button = $<HTMLButtonElement>("btn-keepalive");
  button.classList.toggle("on", on);
  button.title = t("device.keepAliveHint");
  if (on) void keepAlivePing();
}

/** Quiet status query; only failures show up in the status bar. */
async function keepAlivePing(): Promise<void> {
  const connection = selectedConnection();
  if (!connection || state.printing) return;
  try {
    await api.queryStatus(connection);
    const time = new Date().toLocaleTimeString(langInfo(currentLang()).locale, { hour: "2-digit", minute: "2-digit" });
    $("btn-keepalive").title = `${t("device.keepAliveHint")}\n${t("device.keepAliveLast", { time })}`;
  } catch (e) {
    setStatus(t("device.keepAliveFailed", { error: errorText(e) }), "error");
  }
}

function setPrinting(on: boolean, text?: string): void {
  state.printing = on;
  const button = $<HTMLButtonElement>("btn-print");
  button.disabled = on;
  button.classList.toggle("busy", on);
  button.textContent = on ? (text ?? t("print.printingBusy")) : t("print.print");
  $<HTMLButtonElement>("btn-feed-cut").disabled = on;
}

/** Feed and cut without printing (e.g. after "no cut" / chain printing). */
async function feedCut(): Promise<void> {
  if (state.printing) return;
  const connection = selectedConnection();
  if (!connection) {
    setMessage(t("print.noDevice"), true);
    return;
  }
  setPrinting(true);
  setMessage("");
  try {
    await api.feedCut({ connection, model: selectedModel() });
    setMessage(t("print.feedCutDone"));
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  } finally {
    setPrinting(false);
  }
}

/** Selected CSV record range for printing, null = all (or no CSV). */
function selectedRows(): [number, number] | null {
  if (!state.csv) return null;
  const mode = document.querySelector<HTMLInputElement>('input[name="rows"]:checked')?.value;
  if (mode !== "range") return null;
  const from = Math.max(1, Number($<HTMLInputElement>("row-from").value) || 1);
  const to = Math.max(from, Number($<HTMLInputElement>("row-to").value) || from);
  return [from, to];
}

async function print(): Promise<void> {
  if (state.printing) return;
  const connection = selectedConnection();
  if (!connection) {
    setMessage(t("print.noDevice"), true);
    return;
  }
  setPrinting(true);
  setMessage("");
  let unlisten = () => {};
  try {
    unlisten = await api.onPrintProgress(({ done, total }) => {
      if (state.printing) setPrinting(true, t("print.printingProgress", { done: Math.min(done + 1, total), total }));
      if (done === total) setMessage(t("print.doneCount", { total }));
    });
    await api.printLabel({
      label: state.label,
      connection,
      model: selectedModel(),
      job: {
        copies: Math.max(1, Number($<HTMLInputElement>("copies").value) || 1),
        ...cutSettings(),
        marginDots: Math.max(0, Number($<HTMLInputElement>("margin").value) || 0),
        rows: selectedRows(),
        count: state.csv ? null : numberedCount() || null,
        numbering: numbering(),
      },
    });
    // History is a convenience: a failure to record it must not look like a print error.
    api
      .recordHistory(state.label, selectedModel(), selectedWidth(), historyName(), seriesNumbers()?.length ?? 1)
      .catch(() => {});
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  } finally {
    unlisten();
    setPrinting(false);
  }
}

/** Name for the history: file name, else the first text, else a generic name. */
function historyName(): string {
  const file = state.filePath?.split(/[\\/]/).pop();
  if (file) return file;
  const text = state.label.elements.find((i) => i.type === "text");
  const first = text && text.type === "text" ? stripMarkup(text.text).split("\n")[0].trim() : "";
  return first.slice(0, 60) || t("history.untitled");
}

async function showHistory(): Promise<void> {
  const dialog = $<HTMLDialogElement>("history-dialog");
  const list = $("history-list");
  const msg = $("history-msg");
  list.replaceChildren();
  msg.textContent = "";
  dialog.showModal();
  try {
    const entries = await api.history();
    if (!entries.length) msg.textContent = t("history.empty");
    for (const e of entries) {
      const li = document.createElement("li");
      const meta = document.createElement("div");
      meta.className = "meta";
      const name = document.createElement("strong");
      name.textContent = e.name;
      const info = document.createElement("div");
      info.className = "muted";
      info.textContent = t("history.meta", { date: e.printed_at, width: e.width_mm, count: e.count });
      meta.append(name, info);
      const open = makeButton(t("history.reopen"), "", async () => {
        try {
          if (!(await confirmDiscard())) return;
          const label = await api.loadHistory(e.id);
          const width = e.model === selectedModel() ? e.width_mm : null;
          dialog.close();
          await setDocument({ version: 3, sheets: [{ name: e.name, width_mm: width, label }] }, null);
        } catch (err) {
          msg.textContent = t("error.prefix", { error: errorText(err) });
        }
      });
      open.type = "button";
      li.append(meta, open);
      if (e.preview) {
        const thumb = document.createElement("div");
        thumb.className = "thumb";
        const img = document.createElement("img");
        img.alt = "";
        img.src = `data:image/png;base64,${e.preview}`;
        thumb.append(img);
        li.append(thumb);
      }
      list.append(li);
    }
  } catch (err) {
    msg.textContent = t("error.prefix", { error: errorText(err) });
  }
}

// ---------------------------------------------------------------- CSV series

function renderCsv(): void {
  const csv = state.csv;
  $("csv-name").textContent = csv ? csv.name : t("data.none");
  $("btn-csv-clear").hidden = !csv;
  $("csv-details").hidden = !csv;
  if (!csv) return;
  const chips = $("csv-columns");
  chips.replaceChildren();
  for (const name of [...csv.headers, "#"]) {
    const chip = makeButton(name, `{{${name}}}`, () => insertPlaceholder(name));
    chip.className = "chip";
    chips.append(chip);
  }
  const count = csv.rows.length;
  for (const id of ["preview-row", "row-from", "row-to"]) $<HTMLInputElement>(id).max = String(count);
  if (Number($<HTMLInputElement>("row-to").value) > count || !$<HTMLInputElement>("row-to").value) {
    $<HTMLInputElement>("row-to").value = String(count);
  }
  updateCsvSummary();
}

function updateCsvSummary(): void {
  if (!state.csv) return;
  const count = state.csv.rows.length;
  const rows = selectedRows();
  const selected = rows ? Math.max(0, Math.min(rows[1], count) - rows[0] + 1) : count;
  $("csv-summary").textContent = t("data.summary", { count, selected });
}

/** Inserts `{{name}}` at the cursor of the last edited field (or appends it to the selected text element). */
function insertPlaceholder(name: string): void {
  const token = `{{${name}}}`;
  const target = state.lastField;
  if (target && target.el.isConnected) {
    const el = target.el;
    const start = el.selectionStart ?? el.value.length;
    const end = el.selectionEnd ?? start;
    el.value = el.value.slice(0, start) + token + el.value.slice(end);
    target.apply(el.value);
    el.focus({ preventScroll: true });
    el.setSelectionRange(start + token.length, start + token.length);
    syncBoxCaption();
    changed();
    return;
  }
  const item = state.label.elements[state.selected];
  if (item?.type === "text") item.text += token;
  else if (item?.type === "qr" || item?.type === "barcode") item.data += token;
  else return;
  changed(true);
}

async function loadCsvFile(): Promise<void> {
  const path = await open({ multiple: false, filters: [{ name: t("data.filter"), extensions: ["csv", "txt"] }] });
  if (typeof path !== "string") return;
  try {
    const csv = await api.loadCsv(path);
    state.csv = { ...csv, name: path.split(/[\\/]/).pop() ?? path };
    $<HTMLInputElement>("preview-row").value = "1";
    $<HTMLInputElement>("row-from").value = "1";
    $<HTMLInputElement>("row-to").value = String(csv.rows.length);
    renderCsv();
    schedulePreview();
    setMessage("");
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
}

/** Placeholders the series fills itself (not CSV columns). */
const BUILTIN_PLACEHOLDERS = new Set(["n", "a", "A", "datum", "date", "zeit", "time"]);

/** CSV columns the current label uses (`{{name}}`), in order of appearance. */
function labelColumns(): string[] {
  const seen: string[] = [];
  const texts = state.sheets.flatMap((s) => (s === state.sheets[state.sheet] ? state.label : s.label).elements);
  for (const item of texts) {
    const source = item.type === "text" ? item.text : item.type === "qr" || item.type === "barcode" ? item.data : "";
    for (const m of source.matchAll(/\{\{\s*([^}:\s]+)(?::[^}]*)?\s*\}\}/g)) {
      if (!BUILTIN_PLACEHOLDERS.has(m[1]) && !seen.includes(m[1])) seen.push(m[1]);
    }
  }
  return seen;
}

/** Writes a sample CSV (the label's columns, or example columns) and loads it. */
async function createCsvSample(): Promise<void> {
  const columns = labelColumns();
  const header = columns.length ? columns : t("data.sampleColumns").split(";");
  const quote = (v: string) => (/[;"\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v);
  const rows = [1, 2, 3].map((i) => header.map((c) => quote(`${c} ${i}`)).join(";"));
  const content = `${header.map(quote).join(";")}\r\n${rows.join("\r\n")}\r\n`;
  const path = await save({ defaultPath: t("data.sampleFile"), filters: [{ name: t("data.filter"), extensions: ["csv"] }] });
  if (!path) return;
  try {
    await api.saveTextFile(path, content);
    const csv = await api.loadCsv(path);
    state.csv = { ...csv, name: path.split(/[\\/]/).pop() ?? path };
    $<HTMLInputElement>("preview-row").value = "1";
    $<HTMLInputElement>("row-from").value = "1";
    $<HTMLInputElement>("row-to").value = String(csv.rows.length);
    renderCsv();
    schedulePreview();
    setMessage(t("data.sampleSaved", { path }));
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
}

async function clearCsvFile(): Promise<void> {
  await api.clearCsv();
  state.csv = null;
  renderCsv();
  schedulePreview();
}

// ---------------------------------------------------------------- symbols (icon sets)

/** Text of an icon set field in the UI language. */
function textOf(text: api.I18nText | null | undefined): string {
  if (!text) return "";
  if (typeof text === "string") return text;
  return text[currentLang()] ?? text.en ?? text.de ?? Object.values(text)[0] ?? "";
}

function svgUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** Looks up `set:icon` or a bare `icon` (first set that has it), like `ll_render::iconset::resolve`. */
function findIcon(name: string): { set: api.IconSet; icon: api.Icon } | null {
  const [setId, iconId] = name.includes(":") ? (name.split(":", 2) as [string, string]) : [null, name];
  for (const set of state.iconsets) {
    if (setId !== null && set.id !== setId) continue;
    const icon = set.icons.find((i) => i.id === iconId);
    if (icon) return { set, icon };
  }
  return null;
}

/** "W012 · Warnung vor elektrischer Spannung" style caption for a symbol name. */
function symbolLabel(name: string): string {
  const found = findIcon(name);
  if (!found) return name;
  const title = textOf(found.icon.name);
  return title && title !== found.icon.id ? `${found.icon.id} · ${title}` : found.icon.id;
}

async function loadIconsets(): Promise<void> {
  try {
    state.iconsets = await api.iconsets();
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
  renderElements();
  renderBoxes();
}

/** Picker state: selected set/category filter and the callback for a pick. */
const picker = {
  setId: "",
  category: "" as string | null, // "" = all, null = uncategorized
  current: "",
  onPick: (_name: string) => {},
};

function openSymbolPicker(current: string, onPick: (name: string) => void): void {
  const found = findIcon(current);
  picker.current = found ? `${found.set.id}:${found.icon.id}` : current;
  picker.onPick = onPick;
  picker.setId = found?.set.id ?? state.iconsets[0]?.id ?? "";
  picker.category = "";
  $<HTMLInputElement>("symbol-search").value = "";
  $("symbol-msg").textContent = "";
  renderSymbolTree();
  renderSymbolGrid();
  const dialog = $<HTMLDialogElement>("symbol-dialog");
  if (!dialog.open) dialog.showModal();
  $("symbol-search").focus();
  // Scroll the current symbol into view.
  dialog.querySelector(".symbol-tile.current")?.scrollIntoView({ block: "center" });
}

function renderSymbolTree(): void {
  const tree = $("symbol-tree");
  tree.replaceChildren();
  const searching = $<HTMLInputElement>("symbol-search").value.trim() !== "";
  const entry = (label: string, count: number, active: boolean, onClick: () => void) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = active ? "active" : "";
    const name = document.createElement("span");
    name.textContent = label;
    const n = document.createElement("span");
    n.className = "n";
    n.textContent = String(count);
    b.append(name, n);
    b.addEventListener("click", onClick);
    return b;
  };
  for (const set of state.iconsets) {
    const title = document.createElement("div");
    title.className = "set";
    title.textContent = textOf(set.name);
    title.title = [textOf(set.description), set.license ? t("symbol.license", { license: set.license }) : ""]
      .filter(Boolean)
      .join("\n");
    tree.append(title);
    const select = (category: string | null) => () => {
      picker.setId = set.id;
      picker.category = category;
      renderSymbolTree();
      renderSymbolGrid();
    };
    const isSet = !searching && picker.setId === set.id;
    tree.append(entry(t("symbol.all"), set.icons.length, isSet && picker.category === "", select("")));
    for (const cat of set.categories) {
      const count = set.icons.filter((i) => i.category === cat.id).length;
      if (count) tree.append(entry(textOf(cat.name), count, isSet && picker.category === cat.id, select(cat.id)));
    }
    const loose = set.icons.filter((i) => !i.category).length;
    if (loose && set.categories.length) {
      tree.append(entry(t("symbol.uncategorized"), loose, isSet && picker.category === null, select(null)));
    }
  }
  const set = state.iconsets.find((s) => s.id === picker.setId);
  $("symbol-remove").hidden = !set || set.builtin;
}

function renderSymbolGrid(): void {
  const grid = $("symbol-grid");
  grid.replaceChildren();
  const query = $<HTMLInputElement>("symbol-search").value.trim().toLowerCase();
  const hits: { set: api.IconSet; icon: api.Icon }[] = [];
  for (const set of state.iconsets) {
    if (!query && set.id !== picker.setId) continue;
    for (const icon of set.icons) {
      if (query) {
        const hay = [icon.id, textOf(icon.name), ...(icon.tags ?? [])].join(" ").toLowerCase();
        if (!hay.includes(query)) continue;
      } else if (picker.category === null ? icon.category : picker.category && icon.category !== picker.category) {
        continue;
      }
      hits.push({ set, icon });
    }
  }
  for (const { set, icon } of hits) {
    const full = `${set.id}:${icon.id}`;
    const tile = document.createElement("button");
    tile.type = "button";
    tile.className = `symbol-tile${full === picker.current ? " current" : ""}`;
    tile.title = [textOf(icon.name), full, icon.author ? `© ${icon.author}` : "", icon.license ?? ""]
      .filter(Boolean)
      .join("\n");
    const img = document.createElement("img");
    img.loading = "lazy";
    img.alt = "";
    img.src = svgUrl(icon.svg);
    const code = document.createElement("span");
    code.className = "code";
    code.textContent = icon.id;
    const label = document.createElement("span");
    label.className = "label";
    label.textContent = textOf(icon.name) === icon.id ? "" : textOf(icon.name);
    tile.append(img, code, label);
    tile.addEventListener("click", () => {
      $<HTMLDialogElement>("symbol-dialog").close();
      picker.onPick(full);
    });
    grid.append(tile);
  }
  const set = state.iconsets.find((s) => s.id === picker.setId);
  $("symbol-info").textContent = hits.length
    ? [t("symbol.count", { n: hits.length }), !query && set?.license ? t("symbol.license", { license: set.license }) : ""]
        .filter(Boolean)
        .join(" · ")
    : t("symbol.none");
}

async function importIconsetFile(): Promise<void> {
  const path = await open({ multiple: false, filters: [{ name: t("symbol.filter"), extensions: ["llabel-iconset"] }] });
  if (typeof path !== "string") return;
  try {
    const set = await api.importIconset(path);
    await loadIconsets();
    picker.setId = set.id;
    picker.category = "";
    $<HTMLInputElement>("symbol-search").value = "";
    renderSymbolTree();
    renderSymbolGrid();
    $("symbol-msg").textContent = t("symbol.imported", { name: textOf(set.name), n: set.icons.length });
  } catch (e) {
    $("symbol-msg").textContent = t("error.prefix", { error: errorText(e) });
  }
}

async function removeCurrentIconset(): Promise<void> {
  const set = state.iconsets.find((s) => s.id === picker.setId);
  if (!set || set.builtin) return;
  const ok = await ask(t("symbol.removeConfirm", { name: textOf(set.name) }), {
    title: t("symbol.remove"),
    kind: "warning",
    okLabel: t("file.yes"),
    cancelLabel: t("file.no"),
  });
  if (!ok) return;
  try {
    await api.removeIconset(set.id);
    await loadIconsets();
    picker.setId = state.iconsets[0]?.id ?? "";
    picker.category = "";
    renderSymbolTree();
    renderSymbolGrid();
    $("symbol-msg").textContent = t("symbol.removed", { name: textOf(set.name) });
  } catch (e) {
    $("symbol-msg").textContent = t("error.prefix", { error: errorText(e) });
  }
}

function bindSymbolPicker(): void {
  $("symbol-search").addEventListener("input", () => {
    renderSymbolTree();
    renderSymbolGrid();
  });
  $("symbol-import").addEventListener("click", () => void importIconsetFile());
  $("symbol-remove").addEventListener("click", () => void removeCurrentIconset());
}

// ---------------------------------------------------------------- series overview

/** At most this many labels are drawn in the series overview. */
const SERIES_PREVIEW_MAX = 100;

/** Record/running numbers of the series that would be printed, or null for a single label. */
function seriesNumbers(): number[] | null {
  if (state.csv) {
    const count = state.csv.rows.length;
    const [from, to] = selectedRows() ?? [1, count];
    const out: number[] = [];
    for (let n = from; n <= Math.min(to, count); n++) out.push(n);
    return out;
  }
  const count = numberedCount();
  return count > 0 ? Array.from({ length: count }, (_, i) => i + 1) : null;
}

function updateSeriesButton(): void {
  $("btn-series").hidden = seriesNumbers() === null;
}

/** Current series overview run (a reopened dialog stops the old one). */
let seriesRun = 0;

async function showSeries(): Promise<void> {
  const run = ++seriesRun;
  const numbers = seriesNumbers();
  const model = selectedModel();
  const width = selectedWidth();
  if (!numbers || !model || !width) return;
  const dialog = $<HTMLDialogElement>("series-dialog");
  const list = $("series-list");
  const msg = $("series-msg");
  list.replaceChildren();
  dialog.showModal();
  const shown = numbers.slice(0, SERIES_PREVIEW_MAX);
  const st = parseStyleKey($<HTMLSelectElement>("tape-style").value) ?? TAPE_STYLES[0];
  const bg = TAPE_CSS[st.tape];
  const heightPx = 48;
  const pxPerDot = heightPx / (labelHeightMm() * DOTS_PER_MM);
  for (const [i, n] of shown.entries()) {
    if (!dialog.open || run !== seriesRun) return; // closed or reopened while drawing
    msg.textContent = t("series.loading", { done: i, total: shown.length });
    try {
      const preview = await api.renderPreview(state.label, model, width, n, numbering(), 1);
      const url = `data:image/png;base64,${preview.png}`;
      const img = new Image();
      await new Promise((resolve, reject) => {
        img.onload = resolve;
        img.onerror = reject;
        img.src = url;
      });
      const item = document.createElement("div");
      item.className = "series-item";
      const num = document.createElement("span");
      num.className = "num muted";
      num.textContent = t("series.label", { n });
      const tape = document.createElement("div");
      tape.className = `tape${bg === null ? " clear-tape" : ""}`;
      tape.style.backgroundColor = bg ?? "";
      // Same scale both ways (a portrait preview comes turned upright).
      tape.style.width = `${img.naturalWidth * pxPerDot}px`;
      tape.style.height = `${img.naturalHeight * pxPerDot}px`;
      const ink = document.createElement("div");
      ink.className = "ink";
      ink.style.backgroundColor = INK_CSS[st.ink] ?? INK_CSS.black;
      ink.style.maskImage = `url("${url}")`;
      ink.style.setProperty("-webkit-mask-image", `url("${url}")`);
      tape.append(ink);
      item.append(num, tape);
      if (preview.overflowing.length) {
        const warn = document.createElement("span");
        warn.className = "warn";
        warn.textContent = "⚠";
        warn.title = t("preview.overflow", { items: preview.overflowing.map((x) => x + 1).join(", ") });
        item.append(warn);
      }
      list.append(item);
    } catch (e) {
      if (run === seriesRun) msg.textContent = t("preview.error", { error: errorText(e) });
      return;
    }
  }
  if (run !== seriesRun) return;
  msg.textContent =
    numbers.length > shown.length
      ? t("series.limited", { total: numbers.length, shown: shown.length })
      : t("series.count", { total: numbers.length });
}

// ---------------------------------------------------------------- files

function updateFileName(): void {
  const name = (state.filePath?.split(/[\\/]/).pop() ?? t("toolbar.untitled")) + (isDirty() ? " •" : "");
  $("file-name").textContent = name;
  document.title = `${name} – LabelLab`;
  updateAutosaveToggle();
  scheduleAutosave();
}

// ---------------------------------------------------------------- sheets (several labels per file)

/** The open document with the current sheet's latest state. */
function currentDocument(): api.LabelDocument {
  syncSheet();
  return { version: 3, sheets: state.sheets };
}

/** Writes the editor state back into the current sheet. */
function syncSheet(): void {
  const sheet = state.sheets[state.sheet];
  if (!sheet) return;
  sheet.label = state.label;
  sheet.width_mm = selectedWidth() || sheet.width_mm;
}

/**
 * Comparable form of `doc` for the unsaved-changes check. In an untitled
 * document the tape width (follows the inserted tape) is not a change.
 */
function snapshotOf(doc: api.LabelDocument, untitled = !state.filePath): string {
  return JSON.stringify(untitled ? { ...doc, sheets: doc.sheets.map((s) => ({ ...s, width_mm: undefined })) } : doc);
}

function isDirty(): boolean {
  return state.sheets.length > 0 && snapshotOf(currentDocument()) !== state.savedSnapshot;
}

/** Marks the document as saved; `snapshot` = the state that was written. */
function markSaved(snapshot = snapshotOf(currentDocument())): void {
  state.savedSnapshot = snapshot;
  updateFileName();
}

/** Asks before unsaved changes would be lost; true = go ahead. */
async function confirmDiscard(): Promise<boolean> {
  if (!isDirty()) return true;
  return ask(t("file.unsaved"), {
    title: t("file.unsavedTitle"),
    kind: "warning",
    okLabel: t("file.discard"),
    cancelLabel: t("file.cancel"),
  });
}

/** Replaces the whole document (open, new, history) and shows sheet `index`. */
/** Suggested file name for the first save of an imported document. */
let saveSuggestion: string | null = null;

async function setDocument(doc: api.LabelDocument, path: string | null): Promise<void> {
  saveSuggestion = null;
  state.sheets = doc.sheets.length ? doc.sheets : [{ name: t("sheet.default", { n: 1 }), label: newLabel() }];
  state.filePath = path;
  await showSheet(0, false);
  markSaved();
}

/** Switches the editor to sheet `index` (`sync`: keep the current one's edits first). */
async function showSheet(index: number, sync = true): Promise<void> {
  if (sync) syncSheet();
  state.sheet = Math.max(0, Math.min(index, state.sheets.length - 1));
  const sheet = state.sheets[state.sheet];
  state.label = sheet.label;
  for (const item of state.label.elements) if (item.rect) item.rect = roundRect(item.rect);
  state.selected = -1;
  multi.clear();
  const model = state.models.find((m) => m.name === selectedModel());
  if (sheet.width_mm && model?.tapes.some((tp) => tp.width_mm === sheet.width_mm) && sheet.width_mm !== selectedWidth()) {
    fillWidths(sheet.width_mm);
    tapeChanged();
  }
  await ensureRects();
  resetHistory();
  renderAll();
  renderSheetTabs();
}

function renderSheetTabs(): void {
  const bar = $("sheet-tabs");
  bar.replaceChildren();
  state.sheets.forEach((sheet, i) => {
    const tab = document.createElement("button");
    tab.type = "button";
    tab.className = `tab${i === state.sheet ? " active" : ""}`;
    tab.title = t("sheet.rename");
    const name = document.createElement("span");
    name.textContent = sheet.name;
    tab.append(name);
    if (state.sheets.length > 1 && i === state.sheet) {
      const x = document.createElement("span");
      x.className = "x";
      x.textContent = "×";
      x.title = t("sheet.remove");
      x.addEventListener("click", (e) => {
        e.stopPropagation();
        void removeSheet(i);
      });
      tab.append(x);
    }
    tab.addEventListener("click", () => {
      if (i !== state.sheet) void showSheet(i);
    });
    tab.addEventListener("dblclick", () => renameSheet(i, tab));
    bar.append(tab);
  });
  const add = document.createElement("button");
  add.type = "button";
  add.className = "add";
  add.textContent = "+";
  add.title = t("sheet.add");
  add.addEventListener("click", () => {
    syncSheet();
    state.sheets.push({ name: t("sheet.default", { n: state.sheets.length + 1 }), width_mm: selectedWidth(), label: newLabel() });
    void showSheet(state.sheets.length - 1, false).then(updateFileName);
  });
  bar.append(add);
}

function renameSheet(index: number, tab: HTMLElement): void {
  const sheet = state.sheets[index];
  const input = document.createElement("input");
  input.value = sheet.name;
  tab.replaceChildren(input);
  input.focus();
  input.select();
  const done = (keep: boolean) => {
    if (keep && input.value.trim()) sheet.name = input.value.trim();
    renderSheetTabs();
    updateFileName();
  };
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") done(true);
    if (e.key === "Escape") done(false);
    e.stopPropagation();
  });
  input.addEventListener("blur", () => done(true));
}

async function removeSheet(index: number): Promise<void> {
  const sheet = state.sheets[index];
  const ok = await ask(t("sheet.removeConfirm", { name: sheet.name }), {
    title: t("sheet.remove"),
    kind: "warning",
    okLabel: t("file.yes"),
    cancelLabel: t("file.no"),
  });
  if (!ok) return;
  state.sheets.splice(index, 1);
  await showSheet(Math.min(index, state.sheets.length - 1), false);
  updateFileName();
}

const LLABEL_FILTER = () => [{ name: t("file.filter"), extensions: ["llabel"] }];
const OPEN_FILTER = () => [
  { name: t("file.openFilter"), extensions: ["llabel", "lbx"] },
  ...LLABEL_FILTER(),
  { name: t("lbx.filter"), extensions: ["lbx"] },
];

async function openFile(): Promise<void> {
  if (!(await confirmDiscard())) return;
  const path = await open({ multiple: false, filters: OPEN_FILTER() });
  if (typeof path !== "string") return;
  await openPath(path);
}

/** Imports an `.lbx` file as a new, unsaved document and lists what was approximated. */
async function importLbx(path: string): Promise<void> {
  const { document, warnings } = await api.importLbx(path);
  await setDocument(document, null);
  saveSuggestion = path.replace(/\.lbx$/i, ".llabel");
  // Not saved yet: closing asks, saving asks for a `.llabel` name.
  state.savedSnapshot = "";
  updateFileName();
  if (!warnings.length) return;
  const lines = warnings.map((w) => `• ${t(`lbx.warn.${w.kind}`)}${w.detail ? ` – ${w.detail}` : ""}`);
  await message(`${t("lbx.warnIntro")}\n\n${lines.join("\n")}`, { title: t("lbx.title"), kind: "info" });
}

async function openPath(path: string): Promise<void> {
  try {
    if (/\.lbx$/i.test(path)) {
      await importLbx(path);
      setMessage(t("lbx.imported", { name: path.split(/[\\/]/).pop() ?? path }));
      return;
    }
    await setDocument(await api.loadDocument(path), path);
    rememberRecent(path);
    setMessage("");
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
}

// ---------------------------------------------------------------- recent files

const RECENT_KEY = "labellab.recent";
const RECENT_MAX = 8;

function recentFiles(): string[] {
  try {
    const list: unknown = JSON.parse(getSetting(RECENT_KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((p): p is string => typeof p === "string") : [];
  } catch {
    return [];
  }
}

function rememberRecent(path: string): void {
  const list = [path, ...recentFiles().filter((p) => p !== path)].slice(0, RECENT_MAX);
  try {
    setSetting(RECENT_KEY, JSON.stringify(list));
  } catch {
    // not remembered
  }
  renderRecent();
}

function renderRecent(): void {
  const select = $<HTMLSelectElement>("recent");
  const list = recentFiles();
  select.replaceChildren(new Option(t("toolbar.recent"), ""));
  for (const path of list) {
    const option = new Option(path.split(/[\\/]/).pop() ?? path, path);
    option.title = path;
    select.add(option);
  }
  select.value = "";
  select.hidden = list.length === 0;
}

/** Saves to the open file; asks for a path the first time or with `asNew`. */
async function saveFile(asNew = false): Promise<void> {
  const path =
    !asNew && state.filePath
      ? state.filePath
      : await save({ defaultPath: state.filePath ?? saveSuggestion ?? "label.llabel", filters: LLABEL_FILTER() });
  if (!path) return;
  try {
    // Snapshot what is written: edits made during the save stay unsaved.
    const doc = currentDocument();
    const snapshot = snapshotOf(doc, false);
    await api.saveDocument(path, doc);
    state.filePath = path;
    markSaved(snapshot);
    rememberRecent(path);
    setMessage(t("file.saved", { path }));
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
}

// ---------------------------------------------------------------- auto-save

const AUTOSAVE_KEY = "labellab.autosave";
/** Pause after the last change before an automatic save, ms. */
const AUTOSAVE_DELAY_MS = 1500;
let autosaveTimer: number | undefined;

/** Auto-save preference: on unless the user turned it off. */
function autosaveWanted(): boolean {
  try {
    return getSetting(AUTOSAVE_KEY) !== "off";
  } catch {
    return true;
  }
}

/** Auto-save works on a file that was saved once (it has a path). */
function updateAutosaveToggle(): void {
  const box = $<HTMLInputElement>("autosave");
  box.disabled = !state.filePath;
  box.checked = !!state.filePath && autosaveWanted();
}

function scheduleAutosave(): void {
  window.clearTimeout(autosaveTimer);
  if (!state.filePath || !autosaveWanted() || !isDirty()) return;
  autosaveTimer = window.setTimeout(() => void autosave(), AUTOSAVE_DELAY_MS);
}

async function autosave(): Promise<void> {
  const path = state.filePath;
  if (!path || !autosaveWanted() || !isDirty()) return;
  try {
    const doc = currentDocument();
    const snapshot = snapshotOf(doc);
    await api.saveDocument(path, doc);
    markSaved(snapshot);
    scheduleAutosave();
    const time = new Date().toLocaleTimeString(langInfo(currentLang()).locale, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    $("file-name").title = t("file.autosaved", { time });
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
}

function bindAutosave(): void {
  $<HTMLInputElement>("autosave").addEventListener("change", (e) => {
    const on = (e.target as HTMLInputElement).checked;
    try {
      setSetting(AUTOSAVE_KEY, on ? "on" : "off");
    } catch {
      // not remembered
    }
    if (on) scheduleAutosave();
  });
}

// ---------------------------------------------------------------- Bluetooth pairing

async function scanPairable(): Promise<void> {
  const list = $("pair-list");
  const msg = $("pair-msg");
  list.replaceChildren();
  msg.textContent = t("pair.searching");
  try {
    const found = await api.discoverBluetooth();
    msg.textContent = found.length ? "" : t("pair.none");
    for (const d of found) {
      const li = document.createElement("li");
      const label = document.createElement("span");
      label.textContent = d.model ? `${d.name} – ${d.model}` : d.name;
      const button = makeButton(t("pair.pair"), "", async () => {
        button.disabled = true;
        msg.textContent = t("pair.pairing", { name: d.name });
        try {
          await api.pairBluetooth(d.id);
          msg.textContent = t("pair.done", { name: d.name });
          await refreshDevices();
        } catch (e) {
          msg.textContent = t("error.prefix", { error: errorText(e) });
          button.disabled = false;
        }
      });
      li.append(label, button);
      list.append(li);
    }
  } catch (e) {
    msg.textContent = t("error.prefix", { error: errorText(e) });
  }
}

function bindPairing(): void {
  const dialog = $<HTMLDialogElement>("pair-dialog");
  $("btn-pair").addEventListener("click", () => {
    dialog.showModal();
    void scanPairable();
  });
  $("pair-rescan").addEventListener("click", () => void scanPairable());
}

// ---------------------------------------------------------------- wizard (M7 layouts)

const num = (id: string) => Number($<HTMLInputElement>(id).value) || 0;

/** Template tiles in the gallery: kind, i18n key, category key, mini drawing. */
/** Gallery tiles; `id` tells tiles of the same generator apart (field presets). */
const TEMPLATES: { id: string; kind: api.Layout["kind"]; name: string; cat: string; svg: string }[] = [
  { id: "cable_flag", kind: "cable_flag", name: "wizard.cableFlag", cat: "wizard.catCable",
    svg: '<rect x="2" y="8" width="26" height="16" rx="2"/><rect x="31" y="13" width="10" height="6"/><rect x="44" y="8" width="26" height="16" rx="2"/>' },
  { id: "single_flag", kind: "single_flag", name: "wizard.singleFlag", cat: "wizard.catCable",
    svg: '<rect x="4" y="13" width="14" height="6"/><rect x="20" y="8" width="48" height="16" rx="2"/>' },
  { id: "cable_wrap", kind: "cable_wrap", name: "wizard.cableWrap", cat: "wizard.catCable",
    svg: '<rect x="6" y="6" width="60" height="20" rx="10"/><path d="M20 6v20M36 6v20M52 6v20"/>' },
  { id: "terminal_block", kind: "terminal_block", name: "wizard.terminalBlock", cat: "wizard.catPanel",
    svg: '<rect x="2" y="4" width="68" height="24"/><path d="M2 16h68M19 4v24M36 4v24M53 4v24"/>' },
  { id: "patch_panel", kind: "patch_panel", name: "wizard.patchPanel", cat: "wizard.catSpecial",
    svg: '<rect x="2" y="8" width="68" height="16"/><path d="M19 8v16M36 8v16M53 8v16"/>' },
  { id: "fuse_box", kind: "fuse_box", name: "wizard.fuseBox", cat: "wizard.catSpecial",
    svg: '<rect x="2" y="6" width="68" height="20"/><path d="M22 6v20M34 6v20M46 6v20M58 6v20"/><path d="M28 10v12M40 10v12M52 10v12M64 10v12" stroke-width="2"/>' },
  { id: "terminal_strip", kind: "fuse_box", name: "wizard.terminalStrip", cat: "wizard.catSpecial",
    svg: '<rect x="2" y="8" width="68" height="16"/><path d="M8.8 8v16M15.6 8v16M22.4 8v16M29.2 8v16M36 8v16M42.8 8v16M49.6 8v16M56.4 8v16M63.2 8v16"/>' },
  { id: "asset_tag", kind: "asset_tag", name: "wizard.assetTag", cat: "wizard.catOffice",
    svg: '<rect x="2" y="4" width="68" height="24" rx="2"/><path d="M7 9h5v5H7zM15 9h3M7 17h3v6H7zM14 18h4v5M18 14v3"/><path d="M26 12h22" stroke-width="1"/><path d="M26 21h38" stroke-width="3.5"/>' },
  { id: "pipe_marker", kind: "pipe_marker", name: "wizard.pipeMarker", cat: "wizard.catPlant",
    svg: '<path d="M2 8h52l14 8-14 8H2z"/><path d="M54 8v16" /><path d="M10 16h36" stroke-width="3"/>' },
  { id: "lsa_strip", kind: "fuse_box", name: "wizard.lsaStrip", cat: "wizard.catSpecial",
    svg: '<rect x="2" y="8" width="68" height="16"/><path d="M15.6 8v3M29.2 8v3M42.8 8v3M56.4 8v3M15.6 21v3M29.2 21v3M42.8 21v3M56.4 21v3"/><path d="M8 13h3v6M21 13h4v3h-4v3h4M35 13h4v6h-4M48 13h4M50 13v6"/>' },
];

/** Tile chosen in the gallery (see `TEMPLATES`). */
let wizardTile = "cable_flag";

function renderGallery(): void {
  const gallery = $("wz-gallery");
  gallery.replaceChildren();
  const current = $<HTMLInputElement>("wz-kind").value;
  for (const cat of [...new Set(TEMPLATES.map((tp) => tp.cat))]) {
    const head = document.createElement("div");
    head.className = "cat";
    head.textContent = t(cat);
    const tiles = document.createElement("div");
    tiles.className = "tiles";
    for (const tp of TEMPLATES.filter((x) => x.cat === cat)) {
      const tile = document.createElement("button");
      tile.type = "button";
      tile.className = `wz-tile${tp.id === wizardTile && tp.kind === current ? " active" : ""}`;
      tile.innerHTML = `<svg viewBox="0 0 72 32" fill="none" stroke="currentColor" stroke-width="1.5">${tp.svg}</svg>`;
      const label = document.createElement("span");
      label.textContent = t(tp.name);
      tile.append(label);
      tile.addEventListener("click", () => {
        if (wizardTile !== tp.id || $<HTMLInputElement>("wz-kind").value !== tp.kind) {
          wizardTile = tp.id;
          applyFieldDefaults(tp.id);
          fbSpans = [];
          fbTexts = [];
        }
        $<HTMLInputElement>("wz-kind").value = tp.kind;
        if ($<HTMLSelectElement>("wz-target").value !== "replace") $<HTMLSelectElement>("wz-target").value = defaultWizardTarget(tp.kind, false);
        renderGallery();
        renderFbFields();
        updateWizard();
      });
      tiles.append(tile);
    }
    gallery.append(head, tiles);
  }
}

/**
 * Typical fields per tile: count, pitch (mm), prefix, digits and for the
 * field-row element main switch text and vertical text. Pitches are common
 * values (19" patch panel 12.7, DIN module 17.5, 2.5 mm² terminal 5.2,
 * LSA strip 10 pairs); every value stays editable.
 */
const FIELD_DEFAULTS: Record<string, { count: number; pitch: number; prefix: string; digits?: number; main?: string; vertical?: boolean }> = {
  patch_panel: { count: 24, pitch: 12.7, prefix: "" },
  terminal_block: { count: 6, pitch: 15, prefix: "1A-A", digits: 2 },
  fuse_box: { count: 12, pitch: 17.5, prefix: "F", main: "HAUPTSCHALTER", vertical: true },
  terminal_strip: { count: 20, pitch: 5.2, prefix: "", main: "", vertical: true },
  lsa_strip: { count: 10, pitch: 10, prefix: "", main: "", vertical: false },
};

function applyFieldDefaults(tile: string): void {
  if (tile === "pipe_marker") $<HTMLInputElement>("wz-pipe-length").value = String(pipeLength(labelHeightMm()));
  const d = FIELD_DEFAULTS[tile];
  if (!d) return;
  $<HTMLInputElement>("wz-count").value = String(d.count);
  $<HTMLInputElement>("wz-pitch").value = String(d.pitch);
  $<HTMLInputElement>("wz-prefix").value = d.prefix;
  $<HTMLInputElement>("wz-digits").value = String(d.digits ?? 0);
  $<HTMLInputElement>("wz-start").value = "1";
  $<HTMLInputElement>("wz-step").value = "1";
  if (d.main !== undefined) $<HTMLInputElement>("wz-main").value = d.main;
  if (d.vertical !== undefined) $<HTMLInputElement>("wz-fb-vertical").checked = d.vertical;
}

function fieldSpec(): api.FieldSpec {
  return {
    count: Math.min(500, Math.max(1, Math.trunc(num("wz-count")))),
    pitch_mm: Math.max(1, num("wz-pitch")),
    start: Math.trunc(num("wz-start")),
    step: Math.trunc(num("wz-step")),
    prefix: $<HTMLInputElement>("wz-prefix").value,
    digits: Math.min(12, Math.max(0, Math.trunc(num("wz-digits")))),
    separators: $<HTMLInputElement>("wz-separators").checked,
    margin_mm: Math.max(0, num("wz-margin")),
  };
}

// Fuse box fields: modules per field and custom texts, edited in the wizard.
let fbSpans: number[] = [];
let fbTexts: string[] = [];

/** Keeps the fields covering exactly `modules` modules. */
function normalizeFbSpans(modules: number): void {
  let total = fbSpans.reduce((a, b) => a + b, 0);
  while (total < modules) {
    fbSpans.push(1);
    total++;
  }
  while (total > modules && fbSpans.length) {
    const last = fbSpans.length - 1;
    if (fbSpans[last] > 1) fbSpans[last]--;
    else fbSpans.pop();
    total--;
  }
  fbTexts = fbTexts.slice(0, fbSpans.length);
}

/** Strip of fields: merge neighbours (⇔) or split a merged field (✂); text per field. */
function renderFbFields(): void {
  const box = $("wz-fb-fields");
  box.replaceChildren();
  const spec = fieldSpec();
  normalizeFbSpans(spec.count);
  fbSpans.forEach((span, i) => {
    if (i > 0) {
      const merge = makeButton("⇔", t("wizard.merge"), () => {
        fbSpans.splice(i - 1, 2, fbSpans[i - 1] + span);
        fbTexts.splice(i, 1);
        renderFbFields();
        updateWizard();
      });
      merge.type = "button";
      merge.className = "fb-merge";
      box.append(merge);
    }
    const field = document.createElement("div");
    field.className = "fb-field";
    field.style.setProperty("--span", String(span));
    const input = document.createElement("input");
    input.type = "text";
    input.value = fbTexts[i] ?? "";
    input.placeholder = `${spec.prefix}${String(spec.start + i * spec.step).padStart(spec.digits, "0")}`;
    input.title = t("wizard.fieldText");
    input.addEventListener("input", () => {
      fbTexts[i] = input.value;
    });
    field.append(input);
    if (span > 1) {
      const split = makeButton(`✂ ${span}`, t("wizard.split"), () => {
        fbSpans.splice(i, 1, ...Array<number>(span).fill(1));
        fbTexts.splice(i + 1, 0, ...Array<string>(span - 1).fill(""));
        renderFbFields();
        updateWizard();
      });
      split.type = "button";
      field.append(split);
    }
    box.append(field);
  });
}

/** Fills the wizard from a stored template (editing it again). */
function fillWizard(layout: api.Layout): void {
  $<HTMLInputElement>("wz-kind").value = layout.kind;
  wizardTile = layout.kind;
  const set = (id: string, v: string | number) => ($<HTMLInputElement>(id).value = String(v));
  const check = (id: string, v: boolean) => ($<HTMLInputElement>(id).checked = v);
  if ("text" in layout) set("wz-text", layout.text);
  if ("diameter_mm" in layout) set("wz-diameter", layout.diameter_mm);
  if ("flag_mm" in layout) set("wz-flag", layout.flag_mm);
  if ("flag_mm" in layout) $<HTMLInputElement>("wz-center-mark").checked = !!layout.center_mark;
  if (layout.kind === "cable_wrap") {
    set("wz-repeats", layout.repeats ?? 0);
    check("wz-vertical", layout.vertical);
  }
  if ("count" in layout) {
    set("wz-count", layout.count);
    set("wz-pitch", layout.pitch_mm);
    set("wz-prefix", layout.prefix);
    set("wz-start", layout.start);
    set("wz-step", layout.step);
    set("wz-digits", layout.digits);
    set("wz-margin", layout.margin_mm);
    check("wz-separators", layout.separators);
  }
  if (layout.kind === "terminal_block") $<HTMLSelectElement>("wz-rows").value = String(layout.rows);
  if (layout.kind === "pipe_marker") {
    const { kind: _kind, length_mm, ...fields } = layout;
    wizardPipe = { ...fields, symbols: [...(fields.symbols ?? [])] };
    set("wz-pipe-length", length_mm);
    renderWizardPipe();
  }
  if (layout.kind === "asset_tag") {
    set("wz-owner", layout.owner);
    set("wz-number", layout.number);
    $<HTMLSelectElement>("wz-code").value = layout.code;
    set("wz-code-data", layout.code_data);
    set("wz-length", layout.length_mm);
  }
  if (layout.kind === "fuse_box") {
    check("wz-fb-vertical", layout.vertical);
    set("wz-main", layout.main_switch);
    set("wz-main-width", layout.main_switch_mm);
    check("wz-main-right", layout.main_switch_right);
    fbSpans = layout.spans?.length ? [...layout.spans] : [];
    fbTexts = [...(layout.texts ?? [])];
    if (fbSpans.length) set("wz-count", fbSpans.reduce((a, b) => a + b, 0));
  }
}

function wizardLayout(): api.Layout {
  const kind = $<HTMLInputElement>("wz-kind").value as api.Layout["kind"];
  const text = $<HTMLInputElement>("wz-text").value;
  const diameter_mm = Math.max(0.5, num("wz-diameter"));
  switch (kind) {
    case "pipe_marker":
      return { kind, ...wizardPipe, length_mm: Math.min(1000, Math.max(20, num("wz-pipe-length"))) };
    case "asset_tag":
      return {
        kind,
        owner: $<HTMLInputElement>("wz-owner").value,
        number: $<HTMLInputElement>("wz-number").value,
        code: $<HTMLSelectElement>("wz-code").value as api.AssetCode,
        code_data: $<HTMLInputElement>("wz-code-data").value,
        length_mm: Math.min(500, Math.max(10, num("wz-length"))),
      };
    case "cable_flag":
    case "single_flag":
      return {
        kind,
        text,
        diameter_mm,
        flag_mm: Math.max(5, num("wz-flag")),
        center_mark: $<HTMLInputElement>("wz-center-mark").checked,
      };
    case "cable_wrap": {
      const repeats = Math.trunc(num("wz-repeats"));
      return { kind, text, diameter_mm, repeats: repeats > 0 ? repeats : null, vertical: $<HTMLInputElement>("wz-vertical").checked };
    }
    case "terminal_block":
      return { kind, rows: Number($<HTMLSelectElement>("wz-rows").value) || 2, ...fieldSpec() };
    case "fuse_box": {
      const spec = fieldSpec();
      normalizeFbSpans(spec.count);
      const merged = fbSpans.some((s) => s > 1);
      return {
        kind,
        vertical: $<HTMLInputElement>("wz-fb-vertical").checked,
        main_switch: $<HTMLInputElement>("wz-main").value,
        main_switch_mm: Math.max(1, num("wz-main-width")),
        main_switch_right: $<HTMLInputElement>("wz-main-right").checked,
        ...spec,
        spans: merged ? [...fbSpans] : [],
        texts: fbTexts.some((x) => x?.trim()) ? fbSpans.map((_, i) => fbTexts[i] ?? "") : [],
      };
    }
    default:
      return { kind: "patch_panel", ...fieldSpec() };
  }
}

/** Pipe marker being set up in the wizard. */
let wizardPipe: api.PipeMarkerFields = { text: "Trinkwasser", group: 1 };

function renderWizardPipe(): void {
  $("wz-pipe").replaceChildren(...pipeControls(wizardPipe, () => updateWizard()));
}

// ---------------------------------------------------------------- code wizard

type CodeType = "qr" | api.Symbology;

/** Element being edited by the code wizard; null = insert a new one. */
let codeTarget: Item | null = null;
let codeSeq = 0;
let codeValid = false;

const CODE_FIELD_IDS: [keyof CodeFields, string][] = [
  ["text", "cw-text"],
  ["url", "cw-url"],
  ["ssid", "cw-ssid"],
  ["password", "cw-pass"],
  ["name", "cw-vname"],
  ["org", "cw-vorg"],
  ["web", "cw-vurl"],
  ["subject", "cw-subject"],
];

function codeFields(): CodeFields {
  const f = emptyFields();
  for (const [key, id] of CODE_FIELD_IDS) (f[key] as string) = $<HTMLInputElement>(id).value;
  const kind = $<HTMLSelectElement>("cw-kind").value;
  // Phone and e-mail have one input per kind; use the visible one.
  f.phone = $<HTMLInputElement>(kind === "vcard" ? "cw-vtel" : "cw-tel").value;
  f.email = $<HTMLInputElement>(kind === "vcard" ? "cw-vmail" : "cw-mail").value;
  f.security = $<HTMLSelectElement>("cw-sec").value as CodeFields["security"];
  f.hidden = $<HTMLInputElement>("cw-hidden").checked;
  return f;
}

function fillCodeFields(f: CodeFields): void {
  for (const [key, id] of CODE_FIELD_IDS) $<HTMLInputElement>(id).value = f[key] as string;
  $<HTMLInputElement>("cw-vtel").value = $<HTMLInputElement>("cw-tel").value = f.phone;
  $<HTMLInputElement>("cw-vmail").value = $<HTMLInputElement>("cw-mail").value = f.email;
  $<HTMLSelectElement>("cw-sec").value = f.security;
  $<HTMLInputElement>("cw-hidden").checked = f.hidden;
}

type CodeElement = Extract<Element, { type: "qr" | "barcode" }>;

function codeElement(): CodeElement {
  const type = $<HTMLSelectElement>("cw-type").value as CodeType;
  if (type === "qr") {
    return { type: "qr", data: buildCode($<HTMLSelectElement>("cw-kind").value as CodeKind, codeFields()) };
  }
  return { type: "barcode", symbology: type, data: $<HTMLTextAreaElement>("cw-text").value.trim() };
}

/** The structured QR kinds need their main field. */
function missingCodeField(kind: CodeKind): boolean {
  const f = codeFields();
  const required = { text: f.text, url: f.url.replace(/^https?:\/\/$/, ""), wifi: f.ssid, vcard: f.name, email: f.email, tel: f.phone }[kind];
  return !required.trim();
}

function updateCodeWizard(): void {
  const type = $<HTMLSelectElement>("cw-type").value as CodeType;
  const kind = type === "qr" ? $<HTMLSelectElement>("cw-kind").value : "barcode";
  $("cw-kind-row").hidden = type !== "qr";
  document.querySelectorAll<HTMLElement>("#code-dialog .cw-group").forEach((g) => {
    g.hidden = !(g.dataset.kinds ?? "").split(" ").includes(kind);
  });
  const element = codeElement();
  const info = $("cw-info");
  const hint = type === "qr" ? "" : t(`code.hint.${type}`);
  const apply = $<HTMLButtonElement>("cw-apply");
  if (!element.data || (type === "qr" && missingCodeField(kind as CodeKind))) {
    codeValid = false;
    apply.disabled = true;
    info.textContent = hint;
    $<HTMLImageElement>("cw-preview").removeAttribute("src");
    $("cw-preview").parentElement!.hidden = true;
    return;
  }
  // Live preview through the print render path; errors (wrong length,
  // invalid characters) disable "insert".
  const seq = ++codeSeq;
  const height = labelHeightMm();
  const rect = { x_mm: 0, y_mm: 0, w_mm: type === "qr" ? height : Math.max(30, height * 2), h_mm: height };
  const label: Label = {
    version: state.label.version,
    gap_mm: 0,
    padding_mm: 1,
    padding_start_mm: 1,
    frame: false,
    elements: [{ ...element, rect: { ...rect, x_mm: 1 } }],
  };
  void (async () => {
    try {
      const preview = await api.renderPreview(label, selectedModel(), selectedWidth(), null, null, 2);
      if (seq !== codeSeq) return;
      codeValid = true;
      apply.disabled = false;
      $<HTMLImageElement>("cw-preview").src = `data:image/png;base64,${preview.png}`;
      $("cw-preview").parentElement!.hidden = false;
      info.textContent = [hint, type === "qr" ? t("code.chars", { n: element.data.length }) : ""].filter(Boolean).join(" ");
    } catch (err) {
      if (seq !== codeSeq) return;
      codeValid = false;
      apply.disabled = true;
      $<HTMLImageElement>("cw-preview").removeAttribute("src");
      $("cw-preview").parentElement!.hidden = true;
      info.textContent = [hint, t("error.prefix", { error: errorText(err) })].filter(Boolean).join(" ");
    }
  })();
}

/** Opens the code wizard for a new code or to edit `item` (QR or barcode). */
function openCodeWizard(item: Item | null): void {
  codeTarget = item;
  const typeSelect = $<HTMLSelectElement>("cw-type");
  typeSelect.replaceChildren(
    new Option(t("elements.qr"), "qr"),
    ...api.SYMBOLOGIES.map((s) => new Option(api.SYMBOLOGY_NAMES[s], s)),
  );
  if (item?.type === "qr") {
    const parsed = parseCode(item.data);
    typeSelect.value = "qr";
    $<HTMLSelectElement>("cw-kind").value = parsed.kind;
    fillCodeFields(parsed.fields);
  } else if (item?.type === "barcode") {
    typeSelect.value = item.symbology;
    fillCodeFields({ ...emptyFields(), text: item.data });
  } else {
    typeSelect.value = "qr";
    $<HTMLSelectElement>("cw-kind").value = "url";
    fillCodeFields(emptyFields());
  }
  $("cw-apply").textContent = t(item ? "code.apply" : "code.insert");
  updateCodeWizard();
  $<HTMLDialogElement>("code-dialog").showModal();
}

function bindCodeWizard(): void {
  const dialog = $<HTMLDialogElement>("code-dialog");
  $("btn-code").addEventListener("click", () => openCodeWizard(null));
  let lastType = "qr";
  $("cw-type").addEventListener("change", () => {
    // QR → barcode: keep the content as barcode data.
    const text = $<HTMLTextAreaElement>("cw-text");
    if (lastType === "qr" && $<HTMLSelectElement>("cw-type").value !== "qr" && !text.value) {
      text.value = buildCode($<HTMLSelectElement>("cw-kind").value as CodeKind, codeFields());
    }
    lastType = $<HTMLSelectElement>("cw-type").value;
  });
  $("cw-type").addEventListener("focus", () => (lastType = $<HTMLSelectElement>("cw-type").value));
  dialog.addEventListener("input", updateCodeWizard);
  dialog.addEventListener("change", updateCodeWizard);
  $("cw-apply").addEventListener("click", (e) => {
    if (!codeValid) {
      e.preventDefault();
      return;
    }
    const element = codeElement();
    if (codeTarget && state.label.elements.includes(codeTarget)) {
      // Switching QR ⇄ barcode keeps box, name and alignment.
      if (codeTarget.type !== element.type && codeTarget.rect) {
        // A barcode needs a longer box than a QR code and vice versa.
        const fresh = newRect(element.type);
        codeTarget.rect = portrait() ? { ...codeTarget.rect, h_mm: fresh.h_mm } : { ...codeTarget.rect, w_mm: fresh.w_mm };
      }
      const target = codeTarget as Record<string, unknown>;
      delete target.symbology;
      Object.assign(target, element);
    } else {
      state.label.elements.push({ ...element, rect: newRect(element.type) });
      state.selected = state.label.elements.length - 1;
    }
    codeTarget = null;
    changed(true);
  });
}

let wizardSeq = 0;

/**
 * Where a template goes by default: edited templates and empty labels are
 * replaced, special elements (fuse box) join the current label, other
 * templates get a new sheet.
 */
function defaultWizardTarget(kind: string, editing: boolean): string {
  if (editing || state.label.elements.length === 0) return "replace";
  return kind === "fuse_box" || kind === "patch_panel" ? "insert" : "sheet";
}

function updateWizard(): void {
  const kind = $<HTMLInputElement>("wz-kind").value;
  document.querySelectorAll<HTMLElement>("#wizard .wz-group").forEach((g) => {
    g.hidden = !(g.dataset.kind ?? "").split(" ").includes(kind);
  });
  const layout = wizardLayout();
  const info = $("wz-info");
  info.textContent = "diameter_mm" in layout ? t("wizard.wrapInfo", { wrap: (Math.PI * layout.diameter_mm).toFixed(1) }) : "";
  // Live preview of the generated label (same render path as printing).
  const seq = ++wizardSeq;
  void (async () => {
    try {
      const label = await api.generateLayout(layout, selectedModel(), selectedWidth());
      // First label of the series: placeholders like {{n:05}} show a real number.
      // Pipe markers are shown in colour, which needs the 720 dpi raster of the A4/PNG path.
      const scale = layout.kind === "pipe_marker" && pipeGroup(layout.group) ? RENDER_SCALE : 2;
      const preview = await api.renderPreview(label, selectedModel(), selectedWidth(), 1, numbering(), scale);
      if (seq !== wizardSeq) return;
      const group = layout.kind === "pipe_marker" ? pipeGroup(layout.group) : undefined;
      if (group) {
        // In the group's colours, as it looks on the matching cassette.
        const width = selectedWidth();
        const tape = state.models.find((m) => m.name === selectedModel())?.tapes.find((tp) => tp.width_mm === width);
        const url = await labelPng(
          { name: "", png: preview.png, tapeMm: width, printableMm: tape?.printable_mm ?? width },
          PIPE_CSS[group.tape],
          PIPE_CSS[group.ink],
        );
        if (seq !== wizardSeq) return;
        $<HTMLImageElement>("wz-preview-img").src = url;
      } else {
        $<HTMLImageElement>("wz-preview-img").src = `data:image/png;base64,${preview.png}`;
      }
      const length = label.min_length_mm ?? 0;
      info.textContent = [info.textContent, t("wizard.length", { length: length.toFixed(1) })].filter(Boolean).join(" · ");
    } catch (err) {
      if (seq === wizardSeq) info.textContent = t("error.prefix", { error: errorText(err) });
    }
  })();
}

function bindWizard(): void {
  const dialog = $<HTMLDialogElement>("wizard");
  const openWizard = (layout: api.Layout | null) => {
    if (layout) fillWizard(layout);
    else renderWizardPipe();
    $<HTMLSelectElement>("wz-target").value = defaultWizardTarget($<HTMLInputElement>("wz-kind").value, !!layout);
    renderGallery();
    renderFbFields();
    updateWizard();
    dialog.showModal();
  };
  $("btn-wizard").addEventListener("click", () => openWizard(null));
  $("btn-edit-template").addEventListener("click", () => {
    if (state.label.source) openWizard(state.label.source);
  });
  dialog.addEventListener("input", (e) => {
    const id = (e.target as HTMLElement).id;
    if (["wz-count", "wz-prefix", "wz-start", "wz-step", "wz-digits"].includes(id)) renderFbFields();
    updateWizard();
  });
  dialog.addEventListener("change", updateWizard);
  $("wz-create").addEventListener("click", async (e) => {
    e.preventDefault();
    try {
      const layout = wizardLayout();
      const label = await api.generateLayout(layout, selectedModel(), selectedWidth());
      // Pipe markers: show the preview in the recommended cassette's colours.
      const style = layout.kind === "pipe_marker" ? pipeGroup(layout.group)?.style : null;
      const st = style ? parseStyleKey(style) : null;
      if (st) {
        fillTapeStyles(st);
        $<HTMLSelectElement>("tape-style").value = style!;
        applyTapeStyle();
      }
      for (const item of label.elements) {
        if (item.rect) item.rect = roundRect(item.rect);
      }
      const target = $<HTMLSelectElement>("wz-target").value;
      const name = t(TEMPLATES.find((tp) => tp.id === wizardTile)?.name ?? "wizard.title");
      if (target === "file") {
        if (!(await confirmDiscard())) return;
        dialog.close();
        await setDocument({ version: 3, sheets: [{ name, width_mm: selectedWidth(), label }] }, null);
        renderAll();
        return;
      }
      if (target === "insert") {
        if (portrait()) {
          $("wz-info").textContent = t("wizard.insertLandscapeOnly");
          return;
        }
        // Append after the existing content, like a newly added element.
        await ensureRects();
        const end = Math.max(0, ...state.label.elements.map((i) => (i.rect ? i.rect.x_mm + i.rect.w_mm : 0)));
        const start = state.label.elements.length ? end + NEW_ITEM_GAP_MM : startPad();
        const first = Math.min(...label.elements.map((i) => i.rect?.x_mm ?? 0));
        for (const item of label.elements) {
          if (item.rect) item.rect = roundRect({ ...item.rect, x_mm: item.rect.x_mm - first + start });
        }
        state.label.elements.push(...label.elements);
        state.selected = state.label.elements.length - 1;
        dialog.close();
        changed(true);
        renderAll();
        return;
      }
      if (target === "sheet") {
        syncSheet();
        state.sheets.push({ name, width_mm: selectedWidth(), label });
        dialog.close();
        await showSheet(state.sheets.length - 1, false);
        changed(true);
        renderAll();
        return;
      }
      state.label = label;
      state.selected = -1;
      multi.clear();
      dialog.close();
      changed(true);
      renderAll();
    } catch (err) {
      $("wz-info").textContent = t("error.prefix", { error: errorText(err) });
    }
  });
}

// ---------------------------------------------------------------- A4 printing

/** Sheets chosen for A4 printing, with copies (by sheet index). */
let a4Copies: number[] = [];
let a4Seq = 0;
let a4Timer: number | undefined;

const A4_FIELDS: [string, string][] = [
  ["a4-header", "labellab.a4.header"],
  ["a4-gray", "labellab.a4.gray"],
  ["a4-outline", "labellab.a4.outline"],
  ["a4-colors", "labellab.a4.colors"],
  ["a4-bg", "labellab.a4.bg"],
  ["a4-ink", "labellab.a4.ink"],
  ["a4-gap", "labellab.a4.gap"],
  ["a4-sx", "labellab.a4.scaleX"],
  ["a4-sy", "labellab.a4.scaleY"],
];

function a4Options(): A4Options {
  const colors = $<HTMLSelectElement>("a4-colors").value;
  const st = parseStyleKey($<HTMLSelectElement>("tape-style").value) ?? TAPE_STYLES[0];
  const background =
    colors === "custom" ? $<HTMLInputElement>("a4-bg").value : colors === "tape" ? TAPE_CSS[st.tape] : null;
  const ink = colors === "custom" ? $<HTMLInputElement>("a4-ink").value : colors === "tape" ? (INK_CSS[st.ink] ?? INK_CSS.black) : "#000";
  const percent = (id: string) => Math.min(150, Math.max(50, Number($<HTMLInputElement>(id).value) || 100)) / 100;
  const doc = state.filePath?.split(/[\\/]/).pop() ?? t("toolbar.untitled");
  const date = new Date().toLocaleDateString(langInfo(currentLang()).locale);
  return {
    title: doc,
    header: $<HTMLInputElement>("a4-header").checked,
    gray: $<HTMLInputElement>("a4-gray").checked,
    background,
    ink,
    outline: $<HTMLInputElement>("a4-outline").checked,
    gapMm: Math.max(0, Number($<HTMLInputElement>("a4-gap").value) || 0),
    scaleX: percent("a4-sx"),
    scaleY: percent("a4-sy"),
    logo: appIcon,
    headerText: (page, pages) => `${doc} · ${date} · ${t("a4.page", { page, pages })}`,
  };
}

/** Shortcut overview: groups of [keys, i18n key of the action]. */
const SHORTCUTS: [string, [string, string][]][] = [
  ["keys.file", [
    ["Strg+O", "toolbar.open"],
    ["Strg+S", "toolbar.saveNow"],
    ["Strg+Umschalt+S", "toolbar.saveAs"],
    ["Strg+P", "print.print"],
  ]],
  ["keys.edit", [
    ["Strg+Z", "toolbar.undo"],
    ["Strg+Y / Strg+Umschalt+Z", "keys.redo"],
    ["Strg+C / Strg+X / Strg+V", "keys.clipboard"],
    ["Strg+D", "keys.duplicate"],
    ["Entf", "keys.delete"],
    ["Esc", "keys.deselect"],
  ]],
  ["keys.layout", [
    ["← → ↑ ↓", "keys.nudge"],
    ["Umschalt+← → ↑ ↓", "keys.nudgeCoarse"],
    ["Bild↑ / Bild↓", "keys.order"],
    ["Umschalt+Bild↑ / Bild↓", "keys.orderAll"],
    ["Umschalt/Strg+Klick", "keys.multi"],
    ["Alt (beim Ziehen)", "keys.noSnap"],
    ["Umschalt (Ecke ziehen)", "keys.ratio"],
    ["Mausrad", "keys.zoom"],
  ]],
  ["keys.text", [
    ["Strg+B / Strg+I", "keys.bold"],
  ]],
];

/** Key names in the UI language (the table is written with German names). */
function keyName(k: string): string {
  if (currentLang() === "de") return k;
  return k
    .replace(/Strg/g, "Ctrl")
    .replace(/Umschalt/g, "Shift")
    .replace(/Entf/g, "Del")
    .replace(/Bild↑/g, "PgUp")
    .replace(/Bild↓/g, "PgDn")
    .replace(/Klick/g, t("keys.click"))
    .replace(/Mausrad/g, t("keys.wheel"))
    .replace(/\(beim Ziehen\)/g, `(${t("keys.whileDragging")})`)
    .replace(/\(Ecke ziehen\)/g, `(${t("keys.cornerDrag")})`);
}

function showShortcuts(): void {
  const table = $("keys-list");
  table.replaceChildren();
  for (const [group, rows] of SHORTCUTS) {
    const head = document.createElement("tr");
    head.className = "head";
    const cell = document.createElement("td");
    cell.colSpan = 2;
    cell.textContent = t(group);
    head.append(cell);
    table.append(head);
    for (const [keys, action] of rows) {
      const tr = document.createElement("tr");
      const k = document.createElement("td");
      for (const [i, part] of keyName(keys).split(" / ").entries()) {
        if (i) k.append(" / ");
        const kbd = document.createElement("kbd");
        kbd.textContent = part;
        k.append(kbd);
      }
      const what = document.createElement("td");
      what.textContent = t(action);
      tr.append(k, what);
      table.append(tr);
    }
  }
  const dialog = $<HTMLDialogElement>("keys-dialog");
  if (!dialog.open) dialog.showModal();
}

/** Saves the current sheet as a PNG in tape colours (720 dpi, first label of a series). */
async function exportPng(): Promise<void> {
  syncSheet();
  const sheet = state.sheets[state.sheet];
  const base = (sheet?.name || state.filePath?.split(/[\\/]/).pop()?.replace(/\.llabel$/i, "") || "label").replace(/[\\/:*?"<>|]/g, "_");
  const path = await save({ defaultPath: `${base}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
  if (!path) return;
  try {
    const width = selectedWidth();
    const tape = state.models.find((m) => m.name === selectedModel())?.tapes.find((tp) => tp.width_mm === width);
    const preview = await api.renderPreview(state.label, selectedModel(), width, 1, numbering(), RENDER_SCALE);
    const st = parseStyleKey($<HTMLSelectElement>("tape-style").value) ?? TAPE_STYLES[0];
    const url = await labelPng(
      { name: base, png: preview.png, tapeMm: width, printableMm: tape?.printable_mm ?? width },
      // Clear tape: transparent background.
      TAPE_CSS[st.tape] ?? null,
      INK_CSS[st.ink] ?? INK_CSS.black,
    );
    await api.saveBinaryFile(path, url.replace(/^data:[^,]*,/, ""));
    setMessage(t("export.saved", { path }));
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
}

/** Renders the chosen sheets (each `copies` times) for A4. */
async function a4Labels(): Promise<A4Label[]> {
  const model = state.models.find((m) => m.name === selectedModel());
  const out: A4Label[] = [];
  for (const [i, sheet] of state.sheets.entries()) {
    const copies = a4Copies[i] ?? 0;
    if (copies <= 0) continue;
    const width = sheet.width_mm ?? selectedWidth();
    const tape = model?.tapes.find((tp) => tp.width_mm === width);
    const preview = await api.renderPreview(sheet.label, selectedModel(), width, null, null, RENDER_SCALE);
    for (let c = 0; c < copies; c++) {
      out.push({ name: sheet.name, png: preview.png, tapeMm: width, printableMm: tape?.printable_mm ?? width });
    }
  }
  return out;
}

function scheduleA4Preview(): void {
  window.clearTimeout(a4Timer);
  a4Timer = window.setTimeout(() => void renderA4Preview(), 250);
}

async function renderA4Preview(): Promise<void> {
  const seq = ++a4Seq;
  const info = $("a4-info");
  try {
    const labels = await a4Labels();
    const pages = await buildPages(labels, a4Options());
    if (seq !== a4Seq) return;
    $("a4-preview").replaceChildren(
      ...pages.map((p) => {
        const holder = document.createElement("div");
        holder.className = "a4-thumb";
        holder.append(p);
        return holder;
      }),
    );
    info.textContent = t("a4.summary", { labels: labels.length, pages: pages.length });
    $<HTMLButtonElement>("a4-print-btn").disabled = labels.length === 0;
  } catch (e) {
    if (seq === a4Seq) info.textContent = t("error.prefix", { error: errorText(e) });
  }
}

function renderA4Sheets(): void {
  const list = $("a4-sheets");
  list.replaceChildren();
  state.sheets.forEach((sheet, i) => {
    const row = document.createElement("label");
    row.className = "a4-sheet";
    const check = document.createElement("input");
    check.type = "checkbox";
    check.checked = (a4Copies[i] ?? 0) > 0;
    const name = document.createElement("span");
    name.textContent = `${sheet.name} (${sheet.width_mm ?? selectedWidth()} mm)`;
    const copies = document.createElement("input");
    copies.type = "number";
    copies.min = "1";
    copies.value = String(Math.max(1, a4Copies[i] ?? 1));
    copies.title = t("print.copies");
    check.addEventListener("change", () => {
      a4Copies[i] = check.checked ? Math.max(1, Number(copies.value) || 1) : 0;
      scheduleA4Preview();
    });
    copies.addEventListener("input", () => {
      if (check.checked) a4Copies[i] = Math.max(1, Number(copies.value) || 1);
      scheduleA4Preview();
    });
    row.append(check, name, copies);
    list.append(row);
  });
}

function bindA4(): void {
  const dialog = $<HTMLDialogElement>("a4-dialog");
  for (const [id, key] of A4_FIELDS) {
    const el = $<HTMLInputElement | HTMLSelectElement>(id);
    const isCheck = el instanceof HTMLInputElement && el.type === "checkbox";
    const saved = getSetting(key);
    if (saved !== null) {
      if (isCheck) (el as HTMLInputElement).checked = saved === "1";
      else el.value = saved;
    }
    el.addEventListener("input", () => {
      setSetting(key, isCheck ? ((el as HTMLInputElement).checked ? "1" : "0") : el.value);
      $("a4-custom").hidden = $<HTMLSelectElement>("a4-colors").value !== "custom";
      scheduleA4Preview();
    });
    el.addEventListener("change", () => el.dispatchEvent(new Event("input")));
  }
  $("a4-custom").hidden = $<HTMLSelectElement>("a4-colors").value !== "custom";
  $("btn-a4").addEventListener("click", () => {
    syncSheet();
    // Default: the current sheet once (keep earlier choices of this session).
    if (a4Copies.length !== state.sheets.length) a4Copies = state.sheets.map((_, i) => (i === state.sheet ? 1 : 0));
    renderA4Sheets();
    $("a4-preview").replaceChildren();
    dialog.showModal();
    scheduleA4Preview();
  });
  $("a4-print-btn").addEventListener("click", async (e) => {
    e.preventDefault();
    try {
      const pages = await buildPages(await a4Labels(), a4Options());
      dialog.close();
      await printPages(pages);
    } catch (err) {
      $("a4-info").textContent = t("error.prefix", { error: errorText(err) });
    }
  });
  $("a4-test").addEventListener("click", async () => {
    const page = testPage(a4Options(), {
      hint: t("a4.testHint"),
      horizontal: t("a4.testHorizontal"),
      vertical: t("a4.testVertical"),
    });
    dialog.close();
    await printPages([page]);
  });
  // Measured length of the 100 mm rulers → new correction factor.
  $("a4-apply-measure").addEventListener("click", () => {
    for (const [measured, factor] of [["a4-mx", "a4-sx"], ["a4-my", "a4-sy"]] as const) {
      const m = Number($<HTMLInputElement>(measured).value);
      if (!(m > 50 && m < 150)) continue;
      const current = Number($<HTMLInputElement>(factor).value) || 100;
      const next = Math.round(((current * 100) / m) * 10) / 10;
      $<HTMLInputElement>(factor).value = String(next);
      $<HTMLInputElement>(factor).dispatchEvent(new Event("input"));
      $<HTMLInputElement>(measured).value = "";
    }
  });
}

// ---------------------------------------------------------------- clipboard

/** Clipboard type for copied elements (JSON list of items). */
const CLIPBOARD_TYPE = "application/x-labellab+json";
/** Marker in the plain-text fallback, for webviews without custom types. */
const CLIPBOARD_MARKER = "labellab-elements";
const IMAGE_TYPES: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/bmp": "bmp",
  "image/svg+xml": "svg",
};

/** Clipboard shortcuts act on elements unless a field or dialog has focus. */
function clipboardFree(): boolean {
  const el = document.activeElement;
  const editable = el instanceof HTMLElement && el.isContentEditable && el.id !== "paste-catcher";
  return !isTyping() && !editable && !document.querySelector("dialog[open]");
}

function copySelected(e: ClipboardEvent): boolean {
  const item = state.label.elements[state.selected];
  if (!item || !e.clipboardData || !clipboardFree()) return false;
  const json = JSON.stringify({ [CLIPBOARD_MARKER]: [item] });
  e.clipboardData.setData(CLIPBOARD_TYPE, json);
  e.clipboardData.setData("text/plain", item.type === "text" ? item.text : json);
  e.preventDefault();
  return true;
}

function copiedItems(data: DataTransfer): Item[] | null {
  for (const text of [data.getData(CLIPBOARD_TYPE), data.getData("text/plain")]) {
    try {
      const items = JSON.parse(text)?.[CLIPBOARD_MARKER];
      if (Array.isArray(items) && items.length) return items as Item[];
    } catch {
      // not ours
    }
  }
  return null;
}

/** Adds items behind the existing ones and selects the last. */
function insertItems(items: Item[]): void {
  for (const item of items) {
    const rect = newRect(item.type);
    // Keep the size, start where a new element would (along the length).
    item.rect = item.rect
      ? roundRect(portrait() ? { ...item.rect, y_mm: rect.y_mm } : { ...item.rect, x_mm: rect.x_mm })
      : rect;
    state.label.elements.push(item);
  }
  state.selected = state.label.elements.length - 1;
  changed(true);
}

function readBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** Width / height of an image file (1 if it can't be measured). */
function aspectRatio(file: Blob): Promise<number> {
  return new Promise((resolve) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve(img.naturalWidth && img.naturalHeight ? img.naturalWidth / img.naturalHeight : 1);
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      resolve(1);
    };
    img.src = url;
  });
}

function insertImage(path: string, ratio: number): void {
  const rect = newRect("image");
  rect.w_mm = rect.h_mm * ratio;
  insertItems([{ type: "image", path, invert: false, rect: roundRect(rect) }]);
}

async function pasteImage(file: File, extension: string): Promise<void> {
  try {
    const [data, ratio] = await Promise.all([readBase64(file), aspectRatio(file)]);
    insertImage(await api.savePastedImage(data, extension), ratio);
  } catch (err) {
    setStatus(t("error.prefix", { error: errorText(err) }), "error");
  }
}

/** Image straight from the system clipboard (WebKitGTK passes none to the page). */
async function pasteSystemImage(): Promise<void> {
  try {
    const image = await api.pasteClipboardImage();
    if (image) insertImage(image.path, image.width / Math.max(1, image.height));
  } catch (err) {
    setStatus(t("error.prefix", { error: errorText(err) }), "error");
  }
}

function paste(e: ClipboardEvent): void {
  const data = e.clipboardData;
  if (!data || !clipboardFree()) return;
  for (const file of Array.from(data.files)) {
    const extension = IMAGE_TYPES[file.type];
    if (extension) {
      e.preventDefault();
      void pasteImage(file, extension);
      return;
    }
  }
  const items = copiedItems(data);
  if (items) {
    e.preventDefault();
    insertItems(items);
    return;
  }
  e.preventDefault();
  const text = data.getData("text/plain").replace(/\r\n?/g, "\n").trim();
  if (text) insertItems([{ ...(defaultElement("text") as Item), text } as Item]);
  else void pasteSystemImage();
}

/**
 * Webviews only fire clipboard events where something is editable or
 * selected (WebKitGTK). Before Ctrl+C/X/V reaches the webview, move the
 * focus to a hidden editable node with a selection so the event fires.
 */
function clipboardTarget(e: KeyboardEvent): void {
  const key = e.key.toLowerCase();
  if (!(e.ctrlKey || e.metaKey) || !["c", "x", "v"].includes(key) || !clipboardFree()) return;
  if (key !== "v" && state.selected < 0) return;
  const catcher = $("paste-catcher");
  catcher.textContent = "\u00a0";
  catcher.focus();
  const range = document.createRange();
  range.selectNodeContents(catcher);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
  window.setTimeout(() => {
    catcher.blur();
    catcher.textContent = "";
    selection?.removeAllRanges();
  });
}

function bindClipboard(): void {
  document.addEventListener("keydown", clipboardTarget, true);
  document.addEventListener("copy", (e) => void copySelected(e));
  document.addEventListener("cut", (e) => {
    if (copySelected(e)) removeItem(state.selected);
  });
  document.addEventListener("paste", paste);
}

// ---------------------------------------------------------------- startup

function isTyping(): boolean {
  const el = document.activeElement;
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement;
}

function nudge(dx: number, dy: number): void {
  const items = selection()
    .map((i) => state.label.elements[i])
    .filter((it) => it?.rect && !it.locked);
  if (!items.length) return;
  for (const item of items) item.rect = roundRect({ ...item.rect!, x_mm: item.rect!.x_mm + dx, y_mm: item.rect!.y_mm + dy });
  repositionBoxes();
  updateRectInputs(state.selected);
  commitSoon();
  schedulePreview();
}

function bindUi(): void {
  $("btn-new").addEventListener("click", async () => {
    if (!(await confirmDiscard())) return;
    await setDocument({ version: 3, sheets: [] }, null);
  });
  void getCurrentWindow().onCloseRequested(async (event) => {
    if (!(await confirmDiscard())) event.preventDefault();
  });
  $("btn-open").addEventListener("click", openFile);
  $<HTMLSelectElement>("recent").addEventListener("change", (e) => {
    const select = e.target as HTMLSelectElement;
    const path = select.value;
    select.value = "";
    if (path) void confirmDiscard().then((ok) => (ok ? openPath(path) : undefined));
  });
  renderRecent();
  $("btn-series").addEventListener("click", () => void showSeries());
  $("btn-history").addEventListener("click", () => void showHistory());
  bindSymbolPicker();
  $("btn-save").addEventListener("click", () => void saveFile());
  $("btn-save-as").addEventListener("click", () => void saveFile(true));
  bindAutosave();
  $("btn-undo").addEventListener("click", () => stepHistory(-1));
  $("btn-redo").addEventListener("click", () => stepHistory(1));
  $("btn-refresh").addEventListener("click", () => void refreshDevices());
  $("device").addEventListener("change", () => applyDeviceModel(state.devices[Number($<HTMLSelectElement>("device").value)]));
  $("btn-csv").addEventListener("click", loadCsvFile);
  $("btn-csv-sample").addEventListener("click", () => void createCsvSample());
  for (const id of ["num-count", "num-start", "num-step"]) $(id).addEventListener("input", schedulePreview);
  document.querySelectorAll<HTMLButtonElement>("#num-chips [data-token]").forEach((b) =>
    b.addEventListener("click", () => insertPlaceholder(b.dataset.token!)),
  );
  bindWizard();
  bindCodeWizard();
  bindImageEditor();
  bindCutOptions();
  bindDecor();
  bindPrinterInfo();
  bindClipboard();
  bindPairing();
  $("btn-csv-clear").addEventListener("click", clearCsvFile);
  $("preview-row").addEventListener("input", schedulePreview);
  for (const id of ["row-from", "row-to"]) $(id).addEventListener("input", updateCsvSummary);
  document.querySelectorAll<HTMLInputElement>('input[name="rows"]').forEach((r) => r.addEventListener("change", updateCsvSummary));
  $("btn-status").addEventListener("click", readStatus);
  $("btn-keepalive").addEventListener("click", () => setKeepAlive(keepAliveTimer === undefined));
  $("btn-print").addEventListener("click", print);
  $("btn-feed-cut").addEventListener("click", () => void feedCut());
  $("model").addEventListener("change", () => {
    fillWidths();
    tapeChanged();
  });
  $("width").addEventListener("change", () => {
    fitZoomPending = true;
    tapeChanged();
  });
  $("zoom").addEventListener("input", layoutStage);
  bindWheelZoom();
  $("tape-style").addEventListener("change", applyTapeStyle);
  $("quality").addEventListener("change", schedulePreview);
  $("strips").addEventListener("change", () => {
    state.label.strips = Number($<HTMLSelectElement>("strips").value) || 1;
    fitZoom();
    updateStripsHint();
    changed(true);
    layoutStage();
  });
  $("tape-wrap").addEventListener("pointerdown", (e) => {
    if (!(e.target as HTMLElement).closest(".box")) select(-1);
  });
  document.querySelectorAll<HTMLButtonElement>("[data-add]").forEach((b) =>
    b.addEventListener("click", () => {
      const type = b.dataset.add as Element["type"];
      state.label.elements.push({ ...defaultElement(type), rect: newRect(type) });
      state.selected = state.label.elements.length - 1;
      changed(true);
    }),
  );
  $("btn-export-png").addEventListener("click", () => void exportPng());
  $("btn-keys").addEventListener("click", showShortcuts);
  document.addEventListener("keydown", (e) => {
    if (e.key === "F1") {
      e.preventDefault();
      showShortcuts();
    }
  });
  langPicker($("lang"), currentLang, (code: Lang) => {
    setLang(code);
    fillTapeStyles();
    renderAll();
    renderCsv();
    renderRecent();
    setPrinting(state.printing);
  });
  document.addEventListener("keydown", (e) => {
    // Shortcuts act on the label: not behind an open dialog, not in the language list.
    if (document.querySelector("dialog[open]") || (e.target as HTMLElement | null)?.closest?.(".lang-picker")) return;
    if (e.ctrlKey || e.metaKey) {
      const key = e.key.toLowerCase();
      const actions: Record<string, () => void> = {
        z: () => stepHistory(e.shiftKey ? 1 : -1),
        y: () => stepHistory(1),
        s: () => void saveFile(e.shiftKey),
        o: () => void openFile(),
        p: () => void print(),
        d: () => state.selected >= 0 && duplicateItem(state.selected),
      };
      if (actions[key]) {
        e.preventDefault();
        actions[key]();
      }
      return;
    }
    if (isTyping() || state.selected < 0) return;
    const step = e.shiftKey ? NUDGE_COARSE_MM : NUDGE_MM;
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    };
    if (moves[e.key]) {
      e.preventDefault();
      nudge(...moves[e.key]);
    } else if (e.key === "PageUp" || e.key === "PageDown") {
      // Drawing order: a step forward/back, with Shift all the way.
      e.preventDefault();
      const up = e.key === "PageUp";
      moveItem(state.selected, e.shiftKey ? (up ? Infinity : 0) : state.selected + (up ? 1 : -1));
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      removeItems(selection());
    } else if (e.key === "Escape") {
      select(-1);
    }
  });
}

async function init(): Promise<void> {
  const started = Date.now();
  await initSettings();
  loadLang();
  makeSectionsCollapsible();
  applyLang(currentLang());
  bindSplash();
  bindUi();
  bindLayout();

  state.models = await api.models();
  const modelSelect = $<HTMLSelectElement>("model");
  for (const m of state.models) modelSelect.add(new Option(m.name, m.name));
  fillWidths(12);
  fitZoomPending = false;
  fitZoom();
  $<HTMLInputElement>("margin").value = String(await api.defaultMarginDots());
  fillTapeStyles();

  // Restore remembered fields first: they change the label (e.g. strips),
  // which must not count as an unsaved change.
  bindPersistedFields();
  await setDocument({ version: 3, sheets: [{ name: t("sheet.default", { n: 1 }), label: state.label }] }, null);
  applyStatic();
  renderCsv();
  setPrinting(false);
  // Font scan can take a moment; fill the font pickers when it's done.
  bindA4();
  void loadIconsets();
  void loadFrameSets();
  api.fontFamilies().then((fonts) => {
    state.fonts = fonts;
    renderElements();
  });
  window.setTimeout(hideSplash, Math.max(0, SPLASH_MIN_MS - (Date.now() - started)));
  await refreshDevices();
}

init().catch((e) => setMessage(t("error.prefix", { error: errorText(e) }), true));
