// Image editor dialog: crop, rotate, flip, remove background, choose the
// print halftone. Edits are non-destructive (`ImageEdit` on the element);
// the result preview uses the normal render path.

import * as api from "./api";
import type { ImageEdit } from "./api";
import { errorText, t } from "./i18n";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

type Crop = { x: number; y: number; w: number; h: number };

const FULL: Crop = { x: 0, y: 0, w: 1, h: 1 };
/** Smallest crop side, as a fraction of the image. */
const MIN_CROP = 0.02;
const DEFAULT_TOLERANCE = 20;
const DEFAULT_THRESHOLD = 128;
const RELOAD_DEBOUNCE_MS = 200;

/** Renders the print result of the image with `edit` (base64 PNG). */
export type ResultPreview = (edit: ImageEdit) => Promise<string>;

let edit: ImageEdit = {};
let path = "";
let resultPreview: ResultPreview = async () => "";
let picking = false;
let seq = 0;
let timer: number | undefined;
let resolveOpen: ((edit: ImageEdit | null) => void) | undefined;
/** The unmasked image for picking colors. */
const plainCanvas = document.createElement("canvas");

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** `undefined` when the edit changes nothing (keeps saved files small). */
export function normalized(e: ImageEdit): ImageEdit | undefined {
  const out: ImageEdit = {};
  if (e.rotation) out.rotation = e.rotation;
  if (e.flip_h) out.flip_h = true;
  if (e.flip_v) out.flip_v = true;
  if (e.crop && (e.crop.w < 0.999 || e.crop.h < 0.999)) out.crop = e.crop;
  if (e.background) out.background = e.background;
  if (e.halftone === "threshold") {
    out.halftone = "threshold";
    out.threshold = e.threshold ?? DEFAULT_THRESHOLD;
  }
  return Object.keys(out).length ? out : undefined;
}

/** Opens the editor; resolves with the new edit, or `null` when cancelled. */
export function openImageEditor(imagePath: string, initial: ImageEdit | undefined, preview: ResultPreview): Promise<ImageEdit | null> {
  path = imagePath;
  edit = structuredClone(initial ?? {});
  resultPreview = preview;
  setPicking(false);
  syncControls();
  layoutCrop();
  reload();
  $<HTMLDialogElement>("image-dialog").showModal();
  return new Promise((resolve) => (resolveOpen = resolve));
}

function crop(): Crop {
  return edit.crop ?? FULL;
}

function setCrop(c: Crop): void {
  edit.crop = c.w >= 0.999 && c.h >= 0.999 ? null : c;
  layoutCrop();
}

function layoutCrop(): void {
  const c = crop();
  const box = $("ie-crop").style;
  box.left = `${c.x * 100}%`;
  box.top = `${c.y * 100}%`;
  box.width = `${c.w * 100}%`;
  box.height = `${c.h * 100}%`;
}

function setPicking(on: boolean): void {
  picking = on;
  $("ie-stage").classList.toggle("picking", on);
  $("ie-crop").hidden = on;
  $("ie-hint").textContent = t(on ? "imgedit.pickHint" : "imgedit.cropHint");
  $("ie-pick").classList.toggle("on", on);
}

function syncControls(): void {
  const bg = edit.background;
  $<HTMLInputElement>("ie-bg").checked = !!bg;
  $("ie-bg-opts").hidden = !bg;
  const tol = bg?.tolerance ?? DEFAULT_TOLERANCE;
  $<HTMLInputElement>("ie-tol").value = String(tol);
  $("ie-tol-out").textContent = String(tol);
  $<HTMLInputElement>("ie-contig").checked = bg?.contiguous ?? true;
  const swatch = $("ie-swatch");
  const color = bg?.color;
  swatch.style.background = color ? `rgb(${color.join(",")})` : "";
  swatch.title = color ? `#${color.map((v) => v.toString(16).padStart(2, "0")).join("")}` : t("imgedit.auto");
  swatch.classList.toggle("auto", !color);
  const threshold = edit.halftone === "threshold";
  $<HTMLSelectElement>("ie-halftone").value = threshold ? "threshold" : "dither";
  $("ie-thr-row").hidden = !threshold;
  const level = edit.threshold ?? DEFAULT_THRESHOLD;
  $<HTMLInputElement>("ie-thr").value = String(level);
  $("ie-thr-out").textContent = String(level);
}

/** Reloads the editor image and the print result after a pause. */
function reload(): void {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => void load(), RELOAD_DEBOUNCE_MS);
}

async function load(): Promise<void> {
  const mine = ++seq;
  const hint = $("ie-hint");
  try {
    const source = await api.imageEditorSource(path, edit);
    if (mine !== seq) return;
    $<HTMLImageElement>("ie-img").src = `data:image/png;base64,${source.masked}`;
    const plain = new Image();
    plain.onload = () => {
      plainCanvas.width = source.width;
      plainCanvas.height = source.height;
      plainCanvas.getContext("2d")?.drawImage(plain, 0, 0);
    };
    plain.src = `data:image/png;base64,${source.plain}`;
    const result = await resultPreview(normalized(edit) ?? {});
    if (mine !== seq) return;
    $<HTMLImageElement>("ie-result").src = `data:image/png;base64,${result}`;
  } catch (err) {
    if (mine === seq) hint.textContent = t("error.prefix", { error: errorText(err) });
  }
}

/** Pointer position as a fraction of the shown image. */
function fraction(e: PointerEvent): { x: number; y: number } {
  const r = $("ie-img").getBoundingClientRect();
  return { x: clamp((e.clientX - r.left) / r.width, 0, 1), y: clamp((e.clientY - r.top) / r.height, 0, 1) };
}

function pickColor(e: PointerEvent): void {
  const p = fraction(e);
  const x = Math.min(plainCanvas.width - 1, Math.floor(p.x * plainCanvas.width));
  const y = Math.min(plainCanvas.height - 1, Math.floor(p.y * plainCanvas.height));
  const data = plainCanvas.getContext("2d")?.getImageData(x, y, 1, 1).data;
  if (!data) return;
  edit.background = {
    tolerance: edit.background?.tolerance ?? DEFAULT_TOLERANCE,
    contiguous: edit.background?.contiguous ?? true,
    color: [data[0], data[1], data[2]],
  };
  setPicking(false);
  syncControls();
  reload();
}

/** Crop dragging: move the frame, drag a corner, or draw a new frame. */
function bindCrop(): void {
  const stage = $("ie-stage");
  stage.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    if (picking) {
      pickColor(e);
      return;
    }
    const target = e.target as HTMLElement;
    const handle = target.dataset.h;
    const start = fraction(e);
    const from = { ...crop() };
    let mode: "move" | "corner";
    // Fixed corner while resizing (opposite of the dragged one).
    let anchor = { x: from.x, y: from.y };
    if (handle) {
      mode = "corner";
      anchor = { x: handle.includes("w") ? from.x + from.w : from.x, y: handle.includes("n") ? from.y + from.h : from.y };
    } else if (target.id === "ie-crop") {
      mode = "move";
    } else {
      mode = "corner";
      anchor = start;
    }
    stage.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const p = fraction(ev);
      if (mode === "move") {
        setCrop({
          ...from,
          x: clamp(from.x + p.x - start.x, 0, 1 - from.w),
          y: clamp(from.y + p.y - start.y, 0, 1 - from.h),
        });
      } else {
        const x = Math.min(anchor.x, p.x);
        const y = Math.min(anchor.y, p.y);
        setCrop({
          x,
          y,
          w: Math.max(MIN_CROP, Math.abs(p.x - anchor.x)),
          h: Math.max(MIN_CROP, Math.abs(p.y - anchor.y)),
        });
      }
    };
    const up = () => {
      stage.removeEventListener("pointermove", move);
      stage.removeEventListener("pointerup", up);
      stage.removeEventListener("pointercancel", up);
      // Keep the frame inside the image after clamping the minimum size.
      const c = crop();
      setCrop({ ...c, x: clamp(c.x, 0, 1 - c.w), y: clamp(c.y, 0, 1 - c.h) });
      reload();
    };
    stage.addEventListener("pointermove", move);
    stage.addEventListener("pointerup", up);
    stage.addEventListener("pointercancel", up);
  });
}

export function bindImageEditor(): void {
  bindCrop();
  const dialog = $<HTMLDialogElement>("image-dialog");
  const update = () => {
    syncControls();
    reload();
  };
  $("ie-rotate").addEventListener("click", () => {
    edit.rotation = ((edit.rotation ?? 0) + 90) % 360;
    edit.crop = null; // fractions refer to the rotated image
    layoutCrop();
    update();
  });
  $("ie-flip-h").addEventListener("click", () => {
    edit.flip_h = !edit.flip_h;
    if (edit.crop) setCrop({ ...edit.crop, x: 1 - edit.crop.x - edit.crop.w });
    update();
  });
  $("ie-flip-v").addEventListener("click", () => {
    edit.flip_v = !edit.flip_v;
    if (edit.crop) setCrop({ ...edit.crop, y: 1 - edit.crop.y - edit.crop.h });
    update();
  });
  $("ie-crop-reset").addEventListener("click", () => {
    setCrop(FULL);
    reload();
  });
  $<HTMLInputElement>("ie-bg").addEventListener("change", (e) => {
    edit.background = (e.target as HTMLInputElement).checked ? { tolerance: DEFAULT_TOLERANCE, contiguous: true, color: null } : null;
    if (!edit.background) setPicking(false);
    update();
  });
  $("ie-pick").addEventListener("click", () => setPicking(!picking));
  $("ie-auto").addEventListener("click", () => {
    if (edit.background) edit.background.color = null;
    setPicking(false);
    update();
  });
  $<HTMLInputElement>("ie-tol").addEventListener("input", (e) => {
    if (edit.background) edit.background.tolerance = Number((e.target as HTMLInputElement).value);
    update();
  });
  $<HTMLInputElement>("ie-contig").addEventListener("change", (e) => {
    if (edit.background) edit.background.contiguous = (e.target as HTMLInputElement).checked;
    update();
  });
  $<HTMLSelectElement>("ie-halftone").addEventListener("change", (e) => {
    const threshold = (e.target as HTMLSelectElement).value === "threshold";
    edit.halftone = threshold ? "threshold" : null;
    edit.threshold = threshold ? (edit.threshold ?? DEFAULT_THRESHOLD) : null;
    update();
  });
  $<HTMLInputElement>("ie-thr").addEventListener("input", (e) => {
    edit.threshold = Number((e.target as HTMLInputElement).value);
    update();
  });
  dialog.addEventListener("close", () => {
    window.clearTimeout(timer);
    seq++;
    resolveOpen?.(dialog.returnValue === "apply" ? (normalized(edit) ?? {}) : null);
    resolveOpen = undefined;
    dialog.returnValue = "";
  });
}
