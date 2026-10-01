// LabelLab editor: a row of elements along the tape, live preview of the
// exact 1-bit raster that gets printed (rendered by the Rust backend via
// the same path as the print job), device picker with tape status.
import { open, save } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { Connection, Device, Element, Label } from "./api";
import { applyLang, applyStatic, currentLang, setLang, t, type Lang } from "./i18n";

const DOTS_PER_MM = 180 / 25.4;
const PREVIEW_DEBOUNCE_MS = 40;
const HISTORY_DEBOUNCE_MS = 400;
const HISTORY_LIMIT = 200;
/** On-screen tape height the zoom is fitted to when the tape changes. */
const FIT_TAPE_PX = 210;

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function newLabel(): Label {
  return { version: 1, elements: [{ type: "text", text: "LabelLab" }], gap_mm: 2, padding_mm: 1, min_length_mm: null, frame: false };
}

const state = {
  label: newLabel(),
  filePath: null as string | null,
  models: [] as api.Model[],
  devices: [] as Device[],
  history: [] as string[],
  historyIndex: -1,
};

// ---------------------------------------------------------------- history

let historyTimer: number | undefined;

function snapshot(): string {
  return JSON.stringify(state.label);
}

/** Records the current label as an undo step (if it changed). */
function commit(): void {
  window.clearTimeout(historyTimer);
  const snap = snapshot();
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
    commit();
  } else {
    commitSoon();
  }
  schedulePreview();
}

// ---------------------------------------------------------------- preview

let previewTimer: number | undefined;
let previewSeq = 0;
let previewUrl: string | null = null;
/** Re-fit the zoom to the tape on the next preview (tape width changed). */
let fitZoom = true;

function schedulePreview(): void {
  window.clearTimeout(previewTimer);
  previewTimer = window.setTimeout(updatePreview, PREVIEW_DEBOUNCE_MS);
}

async function updatePreview(): Promise<void> {
  const seq = ++previewSeq;
  const img = $<HTMLImageElement>("preview");
  const msg = $("preview-msg");
  const model = selectedModel();
  const width = selectedWidth();
  if (!model || !width) return;

  if (state.label.elements.length === 0) {
    img.hidden = true;
    msg.textContent = t("preview.empty");
    $("dims").textContent = "";
    return;
  }
  try {
    const png = await api.renderPreview(state.label, model, width);
    if (seq !== previewSeq) return; // a newer render is on its way
    if (previewUrl) URL.revokeObjectURL(previewUrl);
    previewUrl = URL.createObjectURL(new Blob([png], { type: "image/png" }));
    img.onload = () => {
      if (fitZoom) {
        fitZoom = false;
        const zoom = Math.min(8, Math.max(1, Math.round(FIT_TAPE_PX / img.naturalHeight)));
        $<HTMLInputElement>("zoom").value = String(zoom);
      }
      applyZoom();
      const lengthMm = (img.naturalWidth / DOTS_PER_MM).toFixed(1);
      $("dims").textContent = t("preview.dims", { length: lengthMm, width });
    };
    img.src = previewUrl;
    img.hidden = false;
    msg.textContent = "";
  } catch (e) {
    if (seq !== previewSeq) return;
    img.hidden = true;
    $("dims").textContent = "";
    msg.textContent = t("preview.error", { error: String(e) });
  }
}

function applyZoom(): void {
  const img = $<HTMLImageElement>("preview");
  const zoom = Number($<HTMLInputElement>("zoom").value);
  img.style.width = `${img.naturalWidth * zoom}px`;
  img.style.height = `${img.naturalHeight * zoom}px`;
}

// ---------------------------------------------------------------- elements

function elementTitle(el: Element): string {
  return t(`elements.${el.type}`);
}

function makeButton(text: string, title: string, onClick: () => void, disabled = false): HTMLButtonElement {
  const b = document.createElement("button");
  b.textContent = text;
  b.title = title;
  b.disabled = disabled;
  b.addEventListener("click", onClick);
  return b;
}

function textInput(value: string, onInput: (v: string) => void): HTMLInputElement {
  const input = document.createElement("input");
  input.type = "text";
  input.value = value;
  input.placeholder = t("elements.data");
  input.addEventListener("input", () => {
    onInput(input.value);
    changed();
  });
  return input;
}

function moveElement(index: number, delta: number): void {
  const els = state.label.elements;
  const target = index + delta;
  if (target < 0 || target >= els.length) return;
  [els[index], els[target]] = [els[target], els[index]];
  changed(true);
}

function elementCard(el: Element, index: number): HTMLLIElement {
  const li = document.createElement("li");
  li.className = "element";
  const count = state.label.elements.length;

  const header = document.createElement("header");
  const title = document.createElement("strong");
  title.textContent = `${index + 1}. ${elementTitle(el)}`;
  header.append(
    title,
    makeButton("◀", t("elements.up"), () => moveElement(index, -1), index === 0),
    makeButton("▶", t("elements.down"), () => moveElement(index, 1), index === count - 1),
    makeButton("✕", t("elements.remove"), () => {
      state.label.elements.splice(index, 1);
      changed(true);
    }),
  );
  li.append(header);

  switch (el.type) {
    case "text":
      li.append(textInput(el.text, (v) => (el.text = v)));
      break;
    case "qr":
      li.append(textInput(el.data, (v) => (el.data = v)));
      break;
    case "barcode": {
      const select = document.createElement("select");
      for (const s of api.SYMBOLOGIES) {
        const opt = new Option(api.SYMBOLOGY_NAMES[s], s, false, s === el.symbology);
        select.add(opt);
      }
      select.title = t("elements.symbology");
      select.addEventListener("change", () => {
        el.symbology = select.value as api.Symbology;
        changed(true);
      });
      li.append(select, textInput(el.data, (v) => (el.data = v)));
      break;
    }
    case "image": {
      const row = document.createElement("div");
      row.className = "row";
      const name = document.createElement("span");
      name.className = "muted";
      name.textContent = el.path ? el.path.split(/[\\/]/).pop()! : t("elements.noImage");
      name.title = el.path;
      row.append(
        makeButton(t("elements.chooseImage"), "", async () => {
          const path = await open({
            multiple: false,
            filters: [{ name: t("file.imageFilter"), extensions: ["png", "jpg", "jpeg", "bmp", "svg"] }],
          });
          if (typeof path === "string") {
            el.path = path;
            changed(true);
          }
        }),
        name,
      );
      const invert = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = el.invert;
      box.addEventListener("change", () => {
        el.invert = box.checked;
        changed(true);
      });
      invert.append(box, ` ${t("elements.invert")}`);
      li.append(row, invert);
      break;
    }
  }
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
  state.label.elements.forEach((el, i) => list.append(elementCard(el, i)));
}

function defaultElement(type: Element["type"]): Element {
  switch (type) {
    case "text":
      return { type, text: "Text" };
    case "qr":
      return { type, data: "https://" };
    case "barcode":
      return { type, symbology: "code128", data: "12345" };
    case "image":
      return { type, path: "", invert: false };
  }
}

// ---------------------------------------------------------------- layout

function renderLayout(): void {
  const l = state.label;
  $<HTMLInputElement>("gap").value = String(l.gap_mm);
  $<HTMLInputElement>("padding").value = String(l.padding_mm);
  $<HTMLInputElement>("min-length").value = l.min_length_mm ? String(l.min_length_mm) : "";
  $<HTMLInputElement>("frame").checked = l.frame;
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
  num("gap", (v) => (state.label.gap_mm = v ?? 0));
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
  updateFileName();
  schedulePreview();
}

// ---------------------------------------------------------------- model, tape, devices

function selectedModel(): string {
  return $<HTMLSelectElement>("model").value;
}

function selectedWidth(): number {
  return Number($<HTMLSelectElement>("width").value);
}

function fillWidths(preferred?: number): void {
  const select = $<HTMLSelectElement>("width");
  const current = preferred ?? (selectedWidth() || 12);
  const model = state.models.find((m) => m.name === selectedModel());
  select.replaceChildren();
  fitZoom = true;
  for (const w of model?.tape_widths ?? []) {
    select.add(new Option(`${w} mm`, String(w), false, w === current));
  }
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

async function refreshDevices(): Promise<void> {
  const select = $<HTMLSelectElement>("device");
  const { devices, warnings } = await api.listDevices();
  state.devices = devices;
  select.replaceChildren();
  if (devices.length === 0) select.add(new Option(t("device.none"), ""));
  devices.forEach((d, i) => select.add(new Option(d.name, String(i))));
  if (warnings.length) console.warn("device enumeration:", warnings);
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
    if (!model?.tape_widths.includes(s.width_mm)) {
      setStatus(t("device.tapeUnknown", { width: s.width_mm }), "error");
      return false;
    }
    fillWidths(s.width_mm);
    schedulePreview();
    setStatus(t("device.statusOk", { width: s.width_mm }), "ok");
    return true;
  } catch (e) {
    setStatus(t("error.prefix", { error: String(e) }), "error");
    return false;
  }
}

function hex(v: number): string {
  return `0x${v.toString(16).padStart(2, "0")}`;
}

async function print(): Promise<void> {
  const connection = selectedConnection();
  if (!connection) {
    setMessage(t("print.noDevice"), true);
    return;
  }
  const button = $<HTMLButtonElement>("btn-print");
  button.disabled = true;
  setMessage(t("print.printing"));
  try {
    await api.printLabel({
      label: state.label,
      connection,
      model: selectedModel(),
      copies: Math.max(1, Number($<HTMLInputElement>("copies").value) || 1),
      autoCut: $<HTMLInputElement>("cut").checked,
      marginDots: Math.max(0, Number($<HTMLInputElement>("margin").value) || 0),
    });
    setMessage(t("print.done"));
  } catch (e) {
    setMessage(t("error.prefix", { error: String(e) }), true);
  } finally {
    button.disabled = false;
  }
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
  try {
    state.label = await api.loadLabel(path);
    state.filePath = path;
    resetHistory();
    renderAll();
    setMessage("");
  } catch (e) {
    setMessage(t("error.prefix", { error: String(e) }), true);
  }
}

async function saveFile(): Promise<void> {
  const path = await save({ defaultPath: state.filePath ?? "label.llabel", filters: LLABEL_FILTER() });
  if (!path) return;
  try {
    await api.saveLabel(path, state.label);
    state.filePath = path;
    updateFileName();
    setMessage(t("file.saved", { path }));
  } catch (e) {
    setMessage(t("error.prefix", { error: String(e) }), true);
  }
}

// ---------------------------------------------------------------- startup

function bindUi(): void {
  $("btn-new").addEventListener("click", () => {
    state.label = newLabel();
    state.filePath = null;
    resetHistory();
    renderAll();
  });
  $("btn-open").addEventListener("click", openFile);
  $("btn-save").addEventListener("click", saveFile);
  $("btn-undo").addEventListener("click", () => stepHistory(-1));
  $("btn-redo").addEventListener("click", () => stepHistory(1));
  $("btn-refresh").addEventListener("click", refreshDevices);
  $("btn-status").addEventListener("click", readStatus);
  $("btn-print").addEventListener("click", print);
  $("model").addEventListener("change", () => {
    fillWidths();
    schedulePreview();
  });
  $("width").addEventListener("change", () => {
    fitZoom = true;
    schedulePreview();
  });
  $("zoom").addEventListener("input", applyZoom);
  document.querySelectorAll<HTMLButtonElement>("[data-add]").forEach((b) =>
    b.addEventListener("click", () => {
      state.label.elements.push(defaultElement(b.dataset.add as Element["type"]));
      changed(true);
    }),
  );
  const lang = $<HTMLSelectElement>("lang");
  lang.value = currentLang();
  lang.addEventListener("change", () => {
    setLang(lang.value as Lang);
    renderAll();
  });
  document.addEventListener("keydown", (e) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    const key = e.key.toLowerCase();
    if (key === "z" && !e.shiftKey) {
      e.preventDefault();
      stepHistory(-1);
    } else if (key === "y" || (key === "z" && e.shiftKey)) {
      e.preventDefault();
      stepHistory(1);
    } else if (key === "s") {
      e.preventDefault();
      saveFile();
    } else if (key === "o") {
      e.preventDefault();
      openFile();
    } else if (key === "p") {
      e.preventDefault();
      print();
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
  $<HTMLInputElement>("margin").value = String(await api.defaultMarginDots());

  resetHistory();
  renderAll();
  applyStatic();
  await refreshDevices();
}

init().catch((e) => setMessage(t("error.prefix", { error: String(e) }), true));
