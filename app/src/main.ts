// LabelLab editor: elements are boxes freely placed on the label (drag to
// move, handles to resize, magnetic snapping to edges of the tape and of
// other boxes). Underneath the boxes sits the live preview: the exact
// 1-bit raster that gets printed, rendered by the Rust backend through the
// same path as the print job.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { Connection, Device, Element, Item, Label, Rect } from "./api";
import { bindImageEditor, openImageEditor } from "./imageEditor";
import { BOLD_MARK, ITALIC_MARK, stripMarkup, toggleMark } from "./richtext";
import { applyLang, applyStatic, currentLang, errorText, setLang, t, type Lang } from "./i18n";
import { roundRect, snapMove, snapResize, targets, type Guides } from "./snap";
import { INK_CSS, TAPE_CSS, TAPE_STYLES, parseStyleKey, styleKey, type TapeStyle } from "./tapes";

const DOTS_PER_MM = 180 / 25.4;
const PREVIEW_DEBOUNCE_MS = 40;
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
function marginViolations(): number[] {
  const l = state.label;
  const fixed = l.fixed_length && l.min_length_mm ? l.min_length_mm : null;
  const eps = 0.05;
  return l.elements.flatMap((item, i) => {
    const r = item.rect;
    if (!r || item.type === "fill") return [];
    const left = r.x_mm < startPad() - eps;
    const right = fixed !== null && r.x_mm + r.w_mm > fixed - endPad() + eps;
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
      const lengthMm = (img.naturalWidth / scale / DOTS_PER_MM).toFixed(1);
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
  const inkWidth = (img.naturalWidth / scale) * zoom();
  const boxesEnd = Math.max(0, ...state.label.elements.map((i) => (i.rect ? i.rect.x_mm + i.rect.w_mm : 0)));
  const width = Math.max(inkWidth, boxesEnd * ppm);
  const height = labelHeightMm() * ppm;
  const ink = $("ink");
  ink.style.width = `${inkWidth}px`;
  ink.style.height = `${height}px`;
  const stage = $("stage");
  stage.style.width = `${width}px`;
  stage.style.height = `${height}px`;
  // Left/right margins as marked zones at both label ends.
  const zone = (id: string, left: number, w: number) => {
    const z = $(id).style;
    z.left = `${left}px`;
    z.width = `${Math.max(0, w)}px`;
    z.display = w > 0 ? "" : "none";
  };
  const lengthPx = inkWidth; // rendered label length (fixed length cuts boxes beyond it)
  zone("margin-start", 0, startPad() * ppm);
  zone("margin-end", lengthPx - endPad() * ppm, endPad() * ppm);
  // Show the whole tape: grey bands for what the print head can't reach.
  $("tape-frame").style.paddingBlock = `${tapeMarginMm() * ppm}px`;
  const lines = $("strip-lines");
  lines.replaceChildren();
  for (let k = 1; k < strips(); k++) {
    const line = document.createElement("div");
    line.className = "strip-line";
    line.style.top = `${k * tapeMm() * ppm}px`;
    lines.append(line);
  }
  drawRuler(width, ppm);
  repositionBoxes();
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

function fitZoom(): void {
  const z = Math.min(10, Math.max(1, Math.round(FIT_TAPE_PX / (labelHeightMm() * DOTS_PER_MM))));
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
    return localStorage.getItem(TAPE_STYLE_KEY) ?? "";
  } catch {
    return "";
  }
}

function applyTapeStyle(): void {
  const key = $<HTMLSelectElement>("tape-style").value;
  try {
    localStorage.setItem(TAPE_STYLE_KEY, key);
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
    box.className = `box${index === state.selected ? " selected" : ""}${overflowing.has(index) ? " overflow" : ""}${item.locked ? " locked" : ""}`;
    box.dataset.index = String(index);
    box.title = boxCaption(item);
    placeBox(box, item.rect);
    const tag = document.createElement("span");
    tag.className = "tag";
    tag.textContent = `${index + 1}`;
    box.append(tag);
    for (const h of ["e", "s", "se"]) {
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
  select(index);
  const item = state.label.elements[index];
  if (!item.rect || item.locked) return;
  const box = (e.currentTarget as HTMLElement);
  const handle = (e.target as HTMLElement).dataset.handle ?? null;
  const start = { ...item.rect };
  const startX = e.clientX;
  const startY = e.clientY;
  const others = state.label.elements.filter((_, i) => i !== index && state.label.elements[i].rect).map((i) => i.rect!);
  const snapTargets = targets(others, labelHeightMm());
  snapTargets.x.push(startPad());
  const fixedLength = state.label.fixed_length ? state.label.min_length_mm : null;
  if (fixedLength) snapTargets.x.push(fixedLength - endPad());
  for (let k = 1; k < strips(); k++) snapTargets.y.push(k * tapeMm());
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
      const rx = handle.includes("e");
      const ry = handle.includes("s");
      const resized = {
        ...start,
        w_mm: rx ? Math.max(MIN_BOX_MM, start.w_mm + dx) : start.w_mm,
        h_mm: ry ? Math.max(MIN_BOX_MM, start.h_mm + dy) : start.h_mm,
      };
      if (rx && ry && ev.shiftKey && start.w_mm > 0 && start.h_mm > 0) {
        // Keep the aspect ratio: follow the larger relative change.
        const f = Math.max(resized.w_mm / start.w_mm, resized.h_mm / start.h_mm);
        resized.w_mm = Math.max(MIN_BOX_MM, start.w_mm * f);
        resized.h_mm = Math.max(MIN_BOX_MM, start.h_mm * f);
      }
      ({ rect: next, guides } = snapResize(resized, snapTargets, threshold, rx, ry));
    }
    item.rect = roundRect(next);
    placeBox(box, item.rect);
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

function select(index: number): void {
  if (state.selected === index) return;
  state.selected = index;
  document.querySelectorAll<HTMLElement>(".box").forEach((b) => b.classList.toggle("selected", Number(b.dataset.index) === index));
  document.querySelectorAll<HTMLElement>(".element").forEach((c) => {
    const on = Number(c.dataset.index) === index;
    c.classList.toggle("selected", on);
    if (on) c.scrollIntoView({ block: "nearest" });
  });
}

/** Box for a new element: after the rightmost box, full tape height. */
function newRect(type: Element["type"]): Rect {
  const h = labelHeightMm();
  const end = Math.max(0, ...state.label.elements.map((i) => (i.rect ? i.rect.x_mm + i.rect.w_mm : 0)));
  const x = state.label.elements.length ? end + NEW_ITEM_GAP_MM : startPad();
  const w = { text: 25, qr: h, barcode: 30, image: h * 1.5, symbol: h, fill: 0.5, shape: h * 1.5 }[type];
  return roundRect({ x_mm: x, y_mm: 0, w_mm: w, h_mm: h });
}

/** Gives every element without a box the box the flow layout uses. */
async function ensureRects(): Promise<void> {
  if (state.label.elements.every((i) => i.rect)) return;
  try {
    const rects = await api.resolveRects(state.label, selectedModel(), selectedWidth());
    state.label.elements.forEach((item, i) => {
      if (!item.rect && rects[i]) item.rect = roundRect(rects[i]);
    });
  } catch {
    // e.g. no font or invalid content: fall back to simple placement
    for (const item of state.label.elements) {
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

function contentFields(item: Item): HTMLElement[] {
  switch (item.type) {
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
      const align = document.createElement("select");
      for (const a of ["left", "center", "right"] as const) {
        align.add(new Option(t(`align.${a}`), a, false, a === item.align));
      }
      align.addEventListener("change", () => {
        item.align = align.value as api.TextAlign;
        changed(true);
      });
      const spacing = numberInput(item.line_spacing, 0.1, "1.0", (v) => {
        item.line_spacing = v && v > 0 ? Math.min(3, Math.max(0.5, v)) : null;
      });
      spacing.min = "0.5";
      spacing.max = "3";
      spacing.title = t("elements.lineSpacingHint");
      const row = document.createElement("div");
      row.className = "row";
      row.append(field("elements.size", size), field("elements.lineSpacing", spacing), field("elements.align", align));

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
      return [textInput(item.data, (v) => (item.data = v))];
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
      return [select, textInput(item.data, (v) => (item.data = v))];
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
      edit.disabled = !item.path;
      edit.classList.toggle("on", !!item.edit);
      return [row, edit, invert, slider("brightness"), slider("contrast")];
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
    ...l.elements.filter((it, i) => i !== index && it.rect).map((it) => it.rect!.x_mm + it.rect!.w_mm),
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

function removeItem(index: number): void {
  state.label.elements.splice(index, 1);
  state.selected = Math.min(state.selected, state.label.elements.length - 1);
  changed(true);
}

function duplicateItem(index: number): void {
  const copy: Item = JSON.parse(JSON.stringify(state.label.elements[index]));
  if (copy.rect) copy.rect.x_mm = roundRect({ ...copy.rect, x_mm: copy.rect.x_mm + copy.rect.w_mm }).x_mm;
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
  li.addEventListener("pointerdown", () => select(index));

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
    lockButton(item),
    makeButton("⧉", t("elements.duplicate"), () => duplicateItem(index)),
    makeButton("✕", t("elements.remove"), () => removeItem(index)),
  );
  li.append(header, ...contentFields(item));
  if (item.rect && !item.locked) li.append(alignRow(index));
  if (item.rect) li.append(rectFields(item, index));
  return li;
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
  state.label.elements.forEach((item, i) => list.append(elementCard(item, i)));
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
  }
}

// ---------------------------------------------------------------- label settings

function renderLayout(): void {
  const l = state.label;
  $<HTMLInputElement>("padding").value = String(l.padding_mm);
  $<HTMLInputElement>("padding-start").value = String(l.padding_start_mm ?? l.padding_mm);
  $<HTMLInputElement>("min-length").value = l.min_length_mm ? String(l.min_length_mm) : "";
  $<HTMLInputElement>("fixed-length").checked = !!l.fixed_length;
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
  num("padding-start", (v) => {
    const before = startPad();
    state.label.padding_start_mm = v;
    const delta = startPad() - before;
    for (const item of state.label.elements) {
      if (item.rect) item.rect = roundRect({ ...item.rect, x_mm: Math.max(0, item.rect.x_mm + delta) });
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
  $<HTMLInputElement>("border-inset").value = String(d.inset_mm);
  document.querySelectorAll<HTMLInputElement>("[data-side]").forEach((c) => {
    c.checked = d.sides[c.dataset.side as keyof api.Border["sides"]];
  });
  document.querySelectorAll<HTMLElement>(".border-opt").forEach((el) => (el.hidden = !b));
  const patterned = b?.style === "dashed" || b?.style === "striped";
  document.querySelectorAll<HTMLElement>(".border-pattern").forEach((el) => (el.hidden = !patterned));
}

/** Space the border takes from each edge, in mm (matches `border_reserve` in `ll-core`). */
function borderReserveMm(b: api.Border): number {
  const width = Math.max(0, b.width_mm);
  return Math.max(0, b.inset_mm) + width + Math.max(width, BORDER_CLEARANCE_MM);
}

/**
 * Moves/shrinks boxes so they don't overlap the border on the top, bottom
 * and left edges (the label end grows by itself). Returns whether a box
 * changed.
 */
function fitBoxesInsideBorder(b: api.Border): boolean {
  const r = borderReserveMm(b);
  const top = b.sides.top ? r : 0;
  const bottom = labelHeightMm() - (b.sides.bottom ? r : 0);
  const left = b.sides.left ? r : 0;
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
  num("border-inset", 0, (b, v) => (b.inset_mm = v));
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

async function refreshDevices(autoStatus = true): Promise<void> {
  const select = $<HTMLSelectElement>("device");
  const { devices, warnings } = await api.listDevices();
  state.devices = devices;
  select.replaceChildren();
  if (devices.length === 0) select.add(new Option(t("device.none"), ""));
  devices.forEach((d, i) => select.add(new Option(d.model ? `${d.name} – ${d.model}` : d.name, String(i))));
  if (warnings.length) console.warn("device enumeration:", warnings);

  // Pick the first recognized printer, switch to its model and read the
  // tape status right away.
  const index = devices.findIndex((d) => d.model);
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
      setStatus(t("device.statusError", { e1: hex(s.error1), e2: hex(s.error2) }), "error");
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
      fillTapeStyles({ tape: s.tape_color_id, ink: s.text_color_id });
    }
    setStatus(t("device.statusOk", { width: s.width_mm }), "ok");
    return true;
  } catch (e) {
    setStatus(t("error.prefix", { error: errorText(e) }), "error");
    return false;
  }
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
    const time = new Date().toLocaleTimeString(currentLang(), { hour: "2-digit", minute: "2-digit" });
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
  const unlisten = await api.onPrintProgress(({ done, total }) => {
    if (state.printing) setPrinting(true, t("print.printingProgress", { done: Math.min(done + 1, total), total }));
    if (done === total) setMessage(t("print.doneCount", { total }));
  });
  try {
    await api.printLabel({
      label: state.label,
      connection,
      model: selectedModel(),
      job: {
        copies: Math.max(1, Number($<HTMLInputElement>("copies").value) || 1),
        cut: $<HTMLInputElement>("cut").checked,
        chain: $<HTMLInputElement>("chain").checked,
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
  return text[currentLang()] ?? text.de ?? text.en ?? Object.values(text)[0] ?? "";
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

async function showSeries(): Promise<void> {
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
    if (!dialog.open) return; // closed while drawing
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
      tape.style.width = `${img.naturalWidth * pxPerDot}px`;
      tape.style.height = `${heightPx}px`;
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
      msg.textContent = t("preview.error", { error: errorText(e) });
      return;
    }
  }
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

function isDirty(): boolean {
  return state.sheets.length > 0 && JSON.stringify(currentDocument()) !== state.savedSnapshot;
}

function markSaved(): void {
  state.savedSnapshot = JSON.stringify(currentDocument());
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
async function setDocument(doc: api.LabelDocument, path: string | null): Promise<void> {
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

async function openFile(): Promise<void> {
  if (!(await confirmDiscard())) return;
  const path = await open({ multiple: false, filters: LLABEL_FILTER() });
  if (typeof path !== "string") return;
  await openPath(path);
}

async function openPath(path: string): Promise<void> {
  try {
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
    const list: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((p): p is string => typeof p === "string") : [];
  } catch {
    return [];
  }
}

function rememberRecent(path: string): void {
  const list = [path, ...recentFiles().filter((p) => p !== path)].slice(0, RECENT_MAX);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
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

async function saveFile(): Promise<void> {
  const path = await save({ defaultPath: state.filePath ?? "label.llabel", filters: LLABEL_FILTER() });
  if (!path) return;
  try {
    await api.saveDocument(path, currentDocument());
    state.filePath = path;
    markSaved();
    rememberRecent(path);
    setMessage(t("file.saved", { path }));
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  }
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
const TEMPLATES: { kind: api.Layout["kind"]; name: string; cat: string; svg: string }[] = [
  { kind: "cable_flag", name: "wizard.cableFlag", cat: "wizard.catCable",
    svg: '<rect x="2" y="8" width="26" height="16" rx="2"/><rect x="31" y="13" width="10" height="6"/><rect x="44" y="8" width="26" height="16" rx="2"/>' },
  { kind: "single_flag", name: "wizard.singleFlag", cat: "wizard.catCable",
    svg: '<rect x="4" y="13" width="14" height="6"/><rect x="20" y="8" width="48" height="16" rx="2"/>' },
  { kind: "cable_wrap", name: "wizard.cableWrap", cat: "wizard.catCable",
    svg: '<rect x="6" y="6" width="60" height="20" rx="10"/><path d="M20 6v20M36 6v20M52 6v20"/>' },
  { kind: "patch_panel", name: "wizard.patchPanel", cat: "wizard.catPanel",
    svg: '<rect x="2" y="8" width="68" height="16"/><path d="M19 8v16M36 8v16M53 8v16"/>' },
  { kind: "terminal_block", name: "wizard.terminalBlock", cat: "wizard.catPanel",
    svg: '<rect x="2" y="4" width="68" height="24"/><path d="M2 16h68M19 4v24M36 4v24M53 4v24"/>' },
  { kind: "fuse_box", name: "wizard.fuseBox", cat: "wizard.catPanel",
    svg: '<rect x="2" y="6" width="68" height="20"/><path d="M22 6v20M34 6v20M46 6v20M58 6v20"/><path d="M28 10v12M40 10v12M52 10v12M64 10v12" stroke-width="2"/>' },
];

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
      tile.className = `wz-tile${tp.kind === current ? " active" : ""}`;
      tile.innerHTML = `<svg viewBox="0 0 72 32" fill="none" stroke="currentColor" stroke-width="1.5">${tp.svg}</svg>`;
      const label = document.createElement("span");
      label.textContent = t(tp.name);
      tile.append(label);
      tile.addEventListener("click", () => {
        if ($<HTMLInputElement>("wz-kind").value !== tp.kind) applyFieldDefaults(tp.kind);
        $<HTMLInputElement>("wz-kind").value = tp.kind;
        renderGallery();
        updateWizard();
      });
      tiles.append(tile);
    }
    gallery.append(head, tiles);
  }
}

/** Typical count/pitch/prefix per template (19" patch panel, LSA strip, DIN module). */
const FIELD_DEFAULTS: Partial<Record<api.Layout["kind"], [number, number, string]>> = {
  patch_panel: [24, 12.7, ""],
  terminal_block: [6, 15, "1A-A"],
  fuse_box: [12, 17.5, "F"],
};

function applyFieldDefaults(kind: api.Layout["kind"]): void {
  const d = FIELD_DEFAULTS[kind];
  if (!d) return;
  $<HTMLInputElement>("wz-count").value = String(d[0]);
  $<HTMLInputElement>("wz-pitch").value = String(d[1]);
  $<HTMLInputElement>("wz-prefix").value = d[2];
  $<HTMLInputElement>("wz-digits").value = kind === "terminal_block" ? "2" : "0";
}

function fieldSpec(): api.FieldSpec {
  return {
    count: Math.max(1, Math.trunc(num("wz-count"))),
    pitch_mm: Math.max(1, num("wz-pitch")),
    start: Math.trunc(num("wz-start")),
    step: Math.trunc(num("wz-step")),
    prefix: $<HTMLInputElement>("wz-prefix").value,
    digits: Math.max(0, Math.trunc(num("wz-digits"))),
    separators: $<HTMLInputElement>("wz-separators").checked,
    margin_mm: Math.max(0, num("wz-margin")),
  };
}

function wizardLayout(): api.Layout {
  const kind = $<HTMLInputElement>("wz-kind").value as api.Layout["kind"];
  const text = $<HTMLInputElement>("wz-text").value;
  const diameter_mm = Math.max(0.5, num("wz-diameter"));
  switch (kind) {
    case "cable_flag":
    case "single_flag":
      return { kind, text, diameter_mm, flag_mm: Math.max(5, num("wz-flag")) };
    case "cable_wrap": {
      const repeats = Math.trunc(num("wz-repeats"));
      return { kind, text, diameter_mm, repeats: repeats > 0 ? repeats : null, vertical: $<HTMLInputElement>("wz-vertical").checked };
    }
    case "terminal_block":
      return { kind, rows: Number($<HTMLSelectElement>("wz-rows").value) || 2, ...fieldSpec() };
    case "fuse_box":
      return {
        kind,
        vertical: $<HTMLInputElement>("wz-fb-vertical").checked,
        main_switch: $<HTMLInputElement>("wz-main").value,
        main_switch_mm: Math.max(1, num("wz-main-width")),
        main_switch_right: $<HTMLInputElement>("wz-main-right").checked,
        ...fieldSpec(),
      };
    default:
      return { kind: "patch_panel", ...fieldSpec() };
  }
}

let wizardSeq = 0;

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
      const preview = await api.renderPreview(label, selectedModel(), selectedWidth(), null, null, 2);
      if (seq !== wizardSeq) return;
      $<HTMLImageElement>("wz-preview-img").src = `data:image/png;base64,${preview.png}`;
      const length = label.min_length_mm ?? 0;
      info.textContent = [info.textContent, t("wizard.length", { length: length.toFixed(1) })].filter(Boolean).join(" · ");
    } catch (err) {
      if (seq === wizardSeq) info.textContent = t("error.prefix", { error: errorText(err) });
    }
  })();
}

function bindWizard(): void {
  const dialog = $<HTMLDialogElement>("wizard");
  $("btn-wizard").addEventListener("click", () => {
    renderGallery();
    updateWizard();
    dialog.showModal();
  });
  dialog.addEventListener("input", updateWizard);
  dialog.addEventListener("change", updateWizard);
  $("wz-create").addEventListener("click", async (e) => {
    e.preventDefault();
    try {
      const label = await api.generateLayout(wizardLayout(), selectedModel(), selectedWidth());
      for (const item of label.elements) {
        if (item.rect) item.rect = roundRect(item.rect);
      }
      state.label = label;
      state.selected = -1;
      dialog.close();
      changed(true);
      renderAll();
    } catch (err) {
      $("wz-info").textContent = t("error.prefix", { error: errorText(err) });
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
    item.rect = item.rect ? roundRect({ ...item.rect, x_mm: rect.x_mm }) : rect;
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
  const item = state.label.elements[state.selected];
  if (!item?.rect || item.locked) return;
  item.rect = roundRect({ ...item.rect, x_mm: item.rect.x_mm + dx, y_mm: item.rect.y_mm + dy });
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
  $("btn-save").addEventListener("click", saveFile);
  $("btn-undo").addEventListener("click", () => stepHistory(-1));
  $("btn-redo").addEventListener("click", () => stepHistory(1));
  $("btn-refresh").addEventListener("click", () => void refreshDevices());
  $("device").addEventListener("change", () => applyDeviceModel(state.devices[Number($<HTMLSelectElement>("device").value)]));
  $("btn-csv").addEventListener("click", loadCsvFile);
  for (const id of ["num-count", "num-start", "num-step"]) $(id).addEventListener("input", schedulePreview);
  document.querySelectorAll<HTMLButtonElement>("#num-chips [data-token]").forEach((b) =>
    b.addEventListener("click", () => insertPlaceholder(b.dataset.token!)),
  );
  bindWizard();
  bindImageEditor();
  bindClipboard();
  bindPairing();
  $("btn-csv-clear").addEventListener("click", clearCsvFile);
  $("preview-row").addEventListener("input", schedulePreview);
  for (const id of ["row-from", "row-to"]) $(id).addEventListener("input", updateCsvSummary);
  document.querySelectorAll<HTMLInputElement>('input[name="rows"]').forEach((r) => r.addEventListener("change", updateCsvSummary));
  $("btn-status").addEventListener("click", readStatus);
  $("btn-keepalive").addEventListener("click", () => setKeepAlive(keepAliveTimer === undefined));
  $("btn-print").addEventListener("click", print);
  $("model").addEventListener("change", () => {
    fillWidths();
    tapeChanged();
  });
  $("width").addEventListener("change", () => {
    fitZoomPending = true;
    tapeChanged();
  });
  $("zoom").addEventListener("input", layoutStage);
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
  const lang = $<HTMLSelectElement>("lang");
  lang.value = currentLang();
  lang.addEventListener("change", () => {
    setLang(lang.value as Lang);
    fillTapeStyles();
    renderAll();
    renderCsv();
    renderRecent();
    setPrinting(state.printing);
  });
  document.addEventListener("keydown", (e) => {
    if (e.ctrlKey || e.metaKey) {
      const key = e.key.toLowerCase();
      const actions: Record<string, () => void> = {
        z: () => stepHistory(e.shiftKey ? 1 : -1),
        y: () => stepHistory(1),
        s: () => void saveFile(),
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
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      removeItem(state.selected);
    } else if (e.key === "Escape") {
      select(-1);
    }
  });
}

async function init(): Promise<void> {
  applyLang(currentLang());
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

  await setDocument({ version: 3, sheets: [{ name: t("sheet.default", { n: 1 }), label: state.label }] }, null);
  applyStatic();
  renderCsv();
  setPrinting(false);
  // Font scan can take a moment; fill the font pickers when it's done.
  void loadIconsets();
  api.fontFamilies().then((fonts) => {
    state.fonts = fonts;
    renderElements();
  });
  await refreshDevices();
}

init().catch((e) => setMessage(t("error.prefix", { error: errorText(e) }), true));
