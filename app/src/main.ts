// LabelLab editor: elements are boxes freely placed on the label (drag to
// move, handles to resize, magnetic snapping to edges of the tape and of
// other boxes). Underneath the boxes sits the live preview: the exact
// 1-bit raster that gets printed, rendered by the Rust backend through the
// same path as the print job.
import { open, save } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { Connection, Device, Element, Item, Label, Rect } from "./api";
import { applyLang, applyStatic, currentLang, errorText, setLang, t, type Lang } from "./i18n";
import { roundRect, snapMove, snapResize, targets, type Guides } from "./snap";
import { INK_CSS, TAPE_CSS, TAPE_STYLES, parseStyleKey, styleKey, type TapeStyle } from "./tapes";

const DOTS_PER_MM = 180 / 25.4;
const PREVIEW_DEBOUNCE_MS = 40;
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
  filePath: null as string | null,
  models: [] as api.Model[],
  devices: [] as Device[],
  selected: -1,
  fonts: [] as string[],
  symbols: [] as string[],
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
  previewTimer = window.setTimeout(updatePreview, PREVIEW_DEBOUNCE_MS);
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
    msg.textContent = preview.overflowing.length
      ? t("preview.overflow", { items: preview.overflowing.map((i) => i + 1).join(", ") })
      : "";
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
  const lines = $("strip-lines");
  lines.replaceChildren();
  for (let k = 1; k < strips(); k++) {
    const line = document.createElement("div");
    line.className = "strip-line";
    line.style.top = `${k * tapeMm() * ppm}px`;
    lines.append(line);
  }
  repositionBoxes();
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
}

// ---------------------------------------------------------------- boxes (canvas)

function elementTitle(el: Element): string {
  return t(`elements.${el.type}`);
}

function boxCaption(item: Item): string {
  switch (item.type) {
    case "text":
      return item.text.split("\n")[0] || elementTitle(item);
    case "qr":
    case "barcode":
      return `${elementTitle(item)}: ${item.data}`;
    case "image":
      return item.path.split(/[\\/]/).pop() || elementTitle(item);
    case "symbol":
      return `${elementTitle(item)}: ${item.name}`;
    case "fill":
      return elementTitle(item);
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
    box.className = `box${index === state.selected ? " selected" : ""}${overflowing.has(index) ? " overflow" : ""}`;
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
  if (!item.rect) return;
  const box = (e.currentTarget as HTMLElement);
  const handle = (e.target as HTMLElement).dataset.handle ?? null;
  const start = { ...item.rect };
  const startX = e.clientX;
  const startY = e.clientY;
  const others = state.label.elements.filter((_, i) => i !== index && state.label.elements[i].rect).map((i) => i.rect!);
  const snapTargets = targets(others, labelHeightMm());
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
  const x = state.label.elements.length ? end + NEW_ITEM_GAP_MM : state.label.padding_mm;
  const w = { text: 25, qr: h, barcode: 30, image: h * 1.5, symbol: h, fill: 0.5 }[type];
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
      const row = document.createElement("div");
      row.className = "row";
      row.append(field("elements.size", size), field("elements.align", align));

      const font = document.createElement("select");
      font.add(new Option(t("elements.defaultFont"), ""));
      const families = item.font && !state.fonts.includes(item.font) ? [item.font, ...state.fonts] : state.fonts;
      for (const f of families) font.add(new Option(f, f, false, f === item.font));
      font.value = item.font ?? "";
      font.addEventListener("change", () => {
        item.font = font.value || null;
        changed(true);
      });
      const toggle = (label: string, title: string, key: "bold" | "italic") => {
        const b = makeButton(label, title, () => {
          item[key] = !item[key];
          b.classList.toggle("on", !!item[key]);
          changed(true);
        });
        b.className = `toggle${item[key] ? " on" : ""}`;
        b.style.fontWeight = key === "bold" ? "700" : "";
        b.style.fontStyle = key === "italic" ? "italic" : "";
        return b;
      };
      const style = document.createElement("div");
      style.className = "row font-row";
      style.append(
        field("elements.font", font),
        toggle("F", t("elements.bold"), "bold"),
        toggle("K", t("elements.italic"), "italic"),
      );
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
      return [row, invert];
    }
    case "symbol": {
      const select = document.createElement("select");
      const names = state.symbols.includes(item.name) ? state.symbols : [item.name, ...state.symbols];
      for (const n of names) select.add(new Option(n, n, false, n === item.name));
      select.addEventListener("change", () => {
        item.name = select.value;
        changed(true);
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
  }
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
    makeButton("⧉", t("elements.duplicate"), () => duplicateItem(index)),
    makeButton("✕", t("elements.remove"), () => removeItem(index)),
  );
  li.append(header, ...contentFields(item));
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
      return { type, name: state.symbols[0] ?? "warning", invert: false };
    case "fill":
      return { type };
  }
}

// ---------------------------------------------------------------- label settings

function renderLayout(): void {
  const l = state.label;
  $<HTMLInputElement>("padding").value = String(l.padding_mm);
  $<HTMLInputElement>("min-length").value = l.min_length_mm ? String(l.min_length_mm) : "";
  $<HTMLInputElement>("frame").checked = l.frame;
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
  num("padding", (v) => (state.label.padding_mm = v ?? 0));
  num("min-length", (v) => (state.label.min_length_mm = v && v > 0 ? v : null));
  $<HTMLInputElement>("frame").addEventListener("change", (e) => {
    state.label.frame = (e.target as HTMLInputElement).checked;
    changed(true);
  });
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
  } catch (e) {
    setMessage(t("error.prefix", { error: errorText(e) }), true);
  } finally {
    unlisten();
    setPrinting(false);
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
  const name = state.filePath?.split(/[\\/]/).pop() ?? t("toolbar.untitled");
  $("file-name").textContent = name;
  document.title = `${name} – LabelLab`;
}

const LLABEL_FILTER = () => [{ name: t("file.filter"), extensions: ["llabel"] }];

async function openFile(): Promise<void> {
  const path = await open({ multiple: false, filters: LLABEL_FILTER() });
  if (typeof path !== "string") return;
  await openPath(path);
}

async function openPath(path: string): Promise<void> {
  try {
    state.label = await api.loadLabel(path);
    state.filePath = path;
    state.selected = -1;
    await ensureRects();
    resetHistory();
    renderAll();
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
    await api.saveLabel(path, state.label);
    state.filePath = path;
    updateFileName();
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

function wizardLayout(): api.Layout {
  const kind = $<HTMLSelectElement>("wz-kind").value;
  const text = $<HTMLInputElement>("wz-text").value;
  const diameter_mm = Math.max(0.5, num("wz-diameter"));
  if (kind === "cable_flag") return { kind, text, diameter_mm, flag_mm: Math.max(5, num("wz-flag")) };
  if (kind === "cable_wrap") {
    const repeats = Math.trunc(num("wz-repeats"));
    return { kind, text, diameter_mm, repeats: repeats > 0 ? repeats : null, vertical: $<HTMLInputElement>("wz-vertical").checked };
  }
  return {
    kind: "patch_panel",
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

function updateWizard(): void {
  const kind = $<HTMLSelectElement>("wz-kind").value;
  document.querySelectorAll<HTMLElement>("#wizard .wz-group").forEach((g) => {
    g.hidden = !(g.dataset.kind ?? "").split(" ").includes(kind);
  });
  const layout = wizardLayout();
  const info = $("wz-info");
  if (layout.kind === "patch_panel") {
    info.textContent = t("wizard.panelInfo", { length: (2 * layout.margin_mm + layout.count * layout.pitch_mm).toFixed(1) });
  } else {
    info.textContent = t("wizard.wrapInfo", { wrap: (Math.PI * layout.diameter_mm).toFixed(1) });
  }
}

function bindWizard(): void {
  const dialog = $<HTMLDialogElement>("wizard");
  $("btn-wizard").addEventListener("click", () => {
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

// ---------------------------------------------------------------- startup

function isTyping(): boolean {
  const el = document.activeElement;
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement;
}

function nudge(dx: number, dy: number): void {
  const item = state.label.elements[state.selected];
  if (!item?.rect) return;
  item.rect = roundRect({ ...item.rect, x_mm: item.rect.x_mm + dx, y_mm: item.rect.y_mm + dy });
  repositionBoxes();
  updateRectInputs(state.selected);
  commitSoon();
  schedulePreview();
}

function bindUi(): void {
  $("btn-new").addEventListener("click", async () => {
    state.label = newLabel();
    state.filePath = null;
    state.selected = -1;
    await ensureRects();
    resetHistory();
    renderAll();
  });
  $("btn-open").addEventListener("click", openFile);
  $<HTMLSelectElement>("recent").addEventListener("change", (e) => {
    const path = (e.target as HTMLSelectElement).value;
    if (path) void openPath(path);
  });
  renderRecent();
  $("btn-series").addEventListener("click", () => void showSeries());
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
  bindPairing();
  $("btn-csv-clear").addEventListener("click", clearCsvFile);
  $("preview-row").addEventListener("input", schedulePreview);
  for (const id of ["row-from", "row-to"]) $(id).addEventListener("input", updateCsvSummary);
  document.querySelectorAll<HTMLInputElement>('input[name="rows"]').forEach((r) => r.addEventListener("change", updateCsvSummary));
  $("btn-status").addEventListener("click", readStatus);
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

  await ensureRects();
  resetHistory();
  renderAll();
  applyStatic();
  renderCsv();
  setPrinting(false);
  // Font scan can take a moment; fill the font pickers when it's done.
  api.symbols().then((names) => {
    state.symbols = names;
    renderElements();
  });
  api.fontFamilies().then((fonts) => {
    state.fonts = fonts;
    renderElements();
  });
  await refreshDevices();
}

init().catch((e) => setMessage(t("error.prefix", { error: errorText(e) }), true));
